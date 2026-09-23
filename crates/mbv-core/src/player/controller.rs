use super::*;

// Write end of a self-pipe used to wake the player event loop immediately
// (see player_session_run.rs) instead of it polling on a fixed timeout.
// Closes the fd on drop so replacing it (each play()/play_queue() call makes
// a fresh pipe) never leaks fds.
pub(super) struct WakeupWriter(pub(super) RawFd);

impl WakeupWriter {
    fn notify(&self) {
        let byte = [0u8; 1];
        unsafe {
            libc::write(self.0, byte.as_ptr() as *const libc::c_void, 1);
        }
    }
}

impl Drop for WakeupWriter {
    fn drop(&mut self) {
        unsafe {
            libc::close(self.0);
        }
    }
}

// Returns (read_fd, write end) on success. Both fds are non-blocking. `None`
// on pipe(2) failure; callers fall back to bounded polling in that case.
pub(super) fn make_wakeup_pipe() -> Option<(RawFd, WakeupWriter)> {
    let mut fds = [-1i32; 2];
    if unsafe { libc::pipe(fds.as_mut_ptr()) } != 0 {
        log::warn!(target: "player", "wakeup pipe: pipe(2) failed: {}", std::io::Error::last_os_error());
        return None;
    }
    for fd in fds {
        unsafe {
            libc::fcntl(fd, libc::F_SETFL, libc::O_NONBLOCK);
        }
    }
    Some((fds[0], WakeupWriter(fds[1])))
}

pub struct QuitHandle {
    pub(super) stop_tx: Arc<Mutex<Option<mpsc::Sender<()>>>>,
    pub(super) shutdown_report_timeout: Arc<Mutex<Option<Duration>>>,
}

impl QuitHandle {
    pub fn stop(&self) {
        if let Some(tx) = self.stop_tx.lock().unwrap().take() {
            let _ = tx.send(());
        }
    }

    pub fn stop_for_shutdown(&self, timeout: Duration) {
        *self.shutdown_report_timeout.lock().unwrap() = Some(timeout);
        self.stop();
    }
}

// ── Player ────────────────────────────────────────────────────────────────────

pub struct Player {
    pub(super) credentials: Arc<Mutex<Option<(String, String)>>>,
    pub(super) audiobookshelf_context: Arc<Mutex<Option<AudiobookshelfPlayerContext>>>,
    pub(super) show_audio_window: bool,
    pub(super) use_mpv_config: bool,
    pub(super) video_cache_forward_mb: u32,
    pub(super) video_cache_back_mb: u32,
    pub(super) no_scripts: bool,
    /// Fixed for this Player's lifetime; `Some` only for packaged-daemon
    /// clocked output (see `with_audio_device`). `None` for bare mode and
    /// the Local daemon, and for any run where pipe output is selected.
    pub(super) audio_device: Option<String>,
    pub always_skip_intro: bool,
    pub subtitle_prefs: Arc<Mutex<SubtitlePrefs>>,
    pub(super) origin: Mutex<PlaybackOrigin>,
    pub(super) current_is_headless: Arc<AtomicBool>,
    pub event_tx: mpsc::Sender<PlayerEvent>,
    pub(super) stop_tx: Arc<Mutex<Option<mpsc::Sender<()>>>>,
    pub(super) shutdown_report_timeout: Arc<Mutex<Option<Duration>>>,
    pub cmd_tx: Arc<Mutex<Option<mpsc::Sender<PlayerCommand>>>>,
    pub(super) wakeup_fd: Arc<Mutex<Option<WakeupWriter>>>,
    pub(super) pre_warmed_mpv: Arc<Mutex<Option<(Mpv, bool)>>>,
    /// Test-only inhibit (`PlayerProxy::stub`): `submit_queue_slots` keeps its
    /// queue/status seeding but never spawns the player thread, whose first
    /// act is `init_mpv` — a real libmpv/libav/GnuTLS handle that unit tests
    /// must not construct (it raced process teardown at test exit; issue #757).
    pub(super) mpv_inhibited: AtomicBool,
    pub status: Arc<Mutex<PlayerStatus>>,
    pub(super) thread_handle: Mutex<Option<thread::JoinHandle<()>>>,
    pub(super) ws_tx: Arc<Mutex<Option<crate::ws::WsSender>>>,
}

impl Player {
    pub fn new(
        server_url: String,
        token: String,
        show_audio_window: bool,
        use_mpv_config: bool,
        no_scripts: bool,
        always_skip_intro: bool,
        subtitle_prefs: SubtitlePrefs,
        event_tx: mpsc::Sender<PlayerEvent>,
        ws_tx: Option<crate::ws::WsSender>,
    ) -> Self {
        Player {
            credentials: Arc::new(Mutex::new(
                (!server_url.is_empty() || !token.is_empty()).then_some((server_url, token)),
            )),
            audiobookshelf_context: Arc::new(Mutex::new(None)),
            show_audio_window,
            use_mpv_config,
            video_cache_forward_mb: crate::config::DEFAULT_VIDEO_CACHE_FORWARD_MB,
            video_cache_back_mb: crate::config::DEFAULT_VIDEO_CACHE_BACK_MB,
            no_scripts,
            audio_device: None,
            always_skip_intro,
            subtitle_prefs: Arc::new(Mutex::new(subtitle_prefs)),
            origin: Mutex::new(PlaybackOrigin::Standalone),
            current_is_headless: Arc::new(AtomicBool::new(false)),
            event_tx,
            stop_tx: Arc::new(Mutex::new(None)),
            shutdown_report_timeout: Arc::new(Mutex::new(None)),
            cmd_tx: Arc::new(Mutex::new(None)),
            wakeup_fd: Arc::new(Mutex::new(None)),
            pre_warmed_mpv: Arc::new(Mutex::new(None)),
            mpv_inhibited: AtomicBool::new(false),
            status: Arc::new(Mutex::new(PlayerStatus::default())),
            thread_handle: Mutex::new(None),
            ws_tx: Arc::new(Mutex::new(ws_tx)),
        }
    }

    /// Sets the video cache budgets projected on every run for this Player's lifetime.
    pub fn with_video_cache(mut self, forward_mb: u32, back_mb: u32) -> Self {
        self.video_cache_forward_mb = forward_mb;
        self.video_cache_back_mb = back_mb;
        self
    }

    /// Sets the fixed ALSA device identifier packaged-daemon clocked output
    /// projects on every run for the remainder of this Player's lifetime
    /// (restart-required, matching `audio_device`'s owner-local semantics).
    /// Bare mode and the Local daemon must never call this.
    pub fn with_audio_device(mut self, audio_device: Option<String>) -> Self {
        self.audio_device = audio_device;
        self
    }

    pub fn pre_warm(&self, pipe_path: Option<String>, samplerate: u32, bitdepth: u8) {
        if pipe_path.is_none() {
            return;
        }
        let config = MpvRunConfig {
            headless: true,
            use_mpv_config: self.use_mpv_config,
            video_cache_forward_mb: self.video_cache_forward_mb,
            video_cache_back_mb: self.video_cache_back_mb,
            no_scripts: self.no_scripts,
            always_skip_intro: self.always_skip_intro,
            audio_pipe_path: pipe_path,
            audio_pipe_samplerate: samplerate,
            audio_pipe_bitdepth: bitdepth,
            audio_device: None,
        };
        match init_mpv(&config) {
            Ok(warmed) => {
                log::info!(target: "player", "pre-warmed mpv for pipe output");
                *self.pre_warmed_mpv.lock().unwrap() = Some(warmed);
            }
            Err(e) => {
                log::warn!(target: "player", "pre-warm failed: {e}");
            }
        }
    }

    /// Update the local Player owner's Emby access after late Service setup.
    /// This is intentionally an in-process seam; it is not part of ctrl.
    pub fn update_emby_credentials(&self, server_url: String, token: String) {
        // The playback thread snapshots these fields when a run starts. The
        // mutexes keep late setup and a concurrent submission from racing.
        let mut credentials = self.credentials.lock().unwrap();
        if server_url.is_empty() && token.is_empty() {
            *credentials = None;
        } else {
            *credentials = Some((server_url, token));
        }
    }

    pub fn update_emby_runtime(
        &self,
        server_url: String,
        token: String,
        ws_tx: crate::ws::WsSender,
    ) {
        self.update_emby_credentials(server_url, token);
        *self.ws_tx.lock().unwrap() = Some(ws_tx);
    }

    /// Replace or clear runtime-only Audiobookshelf access. This seam is
    /// deliberately in-process and is not represented by PlayerCommand/ctrl.
    pub fn update_audiobookshelf_context(&self, context: Option<AudiobookshelfPlayerContext>) {
        *self.audiobookshelf_context.lock().unwrap() = context;
    }

    /// Audiobookshelf admission belongs only to this in-process Player. The
    /// prepared-source boundary and the closed reporting/finalization
    /// lifecycle are owner-local code paths in `PlaybackRun`; the current
    /// context is the runtime gate that makes that complete capability
    /// available. Clearing it immediately makes new Bound admission fail.
    pub fn can_admit_audiobookshelf(&self) -> bool {
        self.audiobookshelf_context.lock().unwrap().is_some()
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn audiobookshelf_generation(&self) -> Option<crate::service_runtime::SetupGeneration> {
        self.audiobookshelf_context
            .lock()
            .unwrap()
            .as_ref()
            .map(AudiobookshelfPlayerContext::generation)
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn emby_credentials(&self) -> Option<(String, String)> {
        self.credentials.lock().unwrap().clone()
    }

    pub fn join(&self) {
        let handle = self.thread_handle.lock().unwrap().take();
        if let Some(h) = handle {
            let _ = h.join();
        }
    }

    // Join the player thread but give up after `timeout`. Used on SIGHUP/SIGTERM
    // so the process always exits even if an HTTP call is hanging.
    pub fn join_or_timeout(&self, timeout: std::time::Duration) {
        let handle = self.thread_handle.lock().unwrap().take();
        if let Some(h) = handle {
            let (tx, rx) = std::sync::mpsc::channel::<()>();
            std::thread::spawn(move || {
                let _ = h.join();
                let _ = tx.send(());
            });
            let _ = rx.recv_timeout(timeout);
        }
    }

    /// Returns `true` if the command was sent, `false` if the player thread is gone.
    pub fn send_command(&self, cmd: PlayerCommand) -> bool {
        let sent = if let Some(tx) = self.cmd_tx.lock().unwrap().as_ref() {
            tx.send(cmd).is_ok()
        } else {
            false
        };
        if sent {
            if let Some(w) = self.wakeup_fd.lock().unwrap().as_ref() {
                w.notify();
            }
        }
        sent
    }

    #[cfg(test)]
    pub(crate) fn spy_on_commands(&self) -> mpsc::Receiver<PlayerCommand> {
        let (tx, rx) = mpsc::channel();
        *self.cmd_tx.lock().unwrap() = Some(tx);
        rx
    }

    pub fn next(&self) -> bool {
        self.send_command(PlayerCommand::Next)
    }

    pub fn previous(&self) -> bool {
        self.send_command(PlayerCommand::Previous)
    }

    pub fn set_paused(&self, paused: bool) -> bool {
        match self.status.lock().unwrap().toggle_to_reach(paused) {
            Some(cmd) => self.send_command(cmd),
            None => false,
        }
    }

    /// Seed queue/status state without starting playback. Used when a freshly
    /// spawned local daemon should inherit a queue snapshot before any thin
    /// client connects, while an already-running daemon keeps its live state.
    pub fn set_initial_queue(&self, items: &[QueueItem], cursor: usize) {
        let mut st = self.status.lock().unwrap();
        if items.is_empty() {
            st.position_ticks = 0;
            st.runtime_ticks = 0;
            st.paused = false;
            st.current_idx = 0;
            st.queue_len = 0;
            st.active = false;
            st.clear_current_item_metadata();
            return;
        }

        let cursor = cursor.min(items.len().saturating_sub(1));
        let start_item = &items[cursor];
        st.seed_from_item(start_item, cursor, items.len());
        st.active = false;
    }

    // Pipe mode always forces headless (no video window), regardless of item
    // type. Reads `audio_pipe_enabled` from `client.config` (rather than a
    // field cached on `Player`) so a setting toggled mid-session takes effect
    // on the very next play() call instead of requiring an app restart.
    pub(crate) fn headless_for(&self, client: &EmbyClient, is_audio: bool) -> bool {
        client.config.audio_pipe_enabled || (!self.show_audio_window && is_audio)
    }

    pub fn stop(&self) {
        if let Some(tx) = self.stop_tx.lock().unwrap().take() {
            let _ = tx.send(());
        }
        if let Some(w) = self.wakeup_fd.lock().unwrap().as_ref() {
            w.notify();
        }
        // Don't clear cmd_tx here: a LoadNew command sent after stop() must still
        // reach the thread so it can cancel the quit and load the new file instead.
    }

    pub fn stop_for_shutdown(&self, timeout: Duration) {
        *self.shutdown_report_timeout.lock().unwrap() = Some(timeout);
        self.stop();
    }
}
