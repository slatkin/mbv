// Write end of a self-pipe used to wake the player event loop immediately
// (see player_session_run.rs) instead of it polling on a fixed timeout.
// Closes the fd on drop so replacing it (each play()/play_queue() call makes
// a fresh pipe) never leaks fds.
struct WakeupWriter(RawFd);

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
fn make_wakeup_pipe() -> Option<(RawFd, WakeupWriter)> {
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
    stop_tx: Arc<Mutex<Option<mpsc::Sender<()>>>>,
    shutdown_report_timeout: Arc<Mutex<Option<Duration>>>,
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
    server_url: String,
    token: String,
    show_audio_window: bool,
    use_mpv_config: bool,
    no_scripts: bool,
    #[allow(dead_code)]
    pub always_play_next: bool,
    pub always_skip_intro: bool,
    pub subtitle_prefs: Arc<Mutex<SubtitlePrefs>>,
    is_queue_mode: Arc<AtomicBool>,
    current_is_headless: Arc<AtomicBool>,
    pub event_tx: mpsc::Sender<PlayerEvent>,
    stop_tx: Arc<Mutex<Option<mpsc::Sender<()>>>>,
    shutdown_report_timeout: Arc<Mutex<Option<Duration>>>,
    pub cmd_tx: Arc<Mutex<Option<mpsc::Sender<PlayerCommand>>>>,
    wakeup_fd: Arc<Mutex<Option<WakeupWriter>>>,
    pre_warmed_mpv: Arc<Mutex<Option<(Mpv, bool)>>>,
    pub status: Arc<Mutex<PlayerStatus>>,
    thread_handle: Mutex<Option<thread::JoinHandle<()>>>,
    ws_tx: Option<crate::ws::WsSender>,
}

impl Player {
    pub fn new(
        server_url: String,
        token: String,
        show_audio_window: bool,
        use_mpv_config: bool,
        no_scripts: bool,
        always_play_next: bool,
        always_skip_intro: bool,
        subtitle_prefs: SubtitlePrefs,
        event_tx: mpsc::Sender<PlayerEvent>,
        ws_tx: Option<crate::ws::WsSender>,
    ) -> Self {
        Player {
            server_url,
            token,
            show_audio_window,
            use_mpv_config,
            no_scripts,
            always_play_next,
            always_skip_intro,
            subtitle_prefs: Arc::new(Mutex::new(subtitle_prefs)),
            is_queue_mode: Arc::new(AtomicBool::new(false)),
            current_is_headless: Arc::new(AtomicBool::new(false)),
            event_tx,
            stop_tx: Arc::new(Mutex::new(None)),
            shutdown_report_timeout: Arc::new(Mutex::new(None)),
            cmd_tx: Arc::new(Mutex::new(None)),
            wakeup_fd: Arc::new(Mutex::new(None)),
            pre_warmed_mpv: Arc::new(Mutex::new(None)),
            status: Arc::new(Mutex::new(PlayerStatus::default())),
            thread_handle: Mutex::new(None),
            ws_tx,
        }
    }

    pub fn pre_warm(&self, pipe_path: Option<String>, samplerate: u32, bitdepth: u8) {
        if pipe_path.is_none() {
            return;
        }
        let config = MpvRunConfig {
            headless: true,
            use_mpv_config: self.use_mpv_config,
            no_scripts: self.no_scripts,
            always_skip_intro: self.always_skip_intro,
            audio_pipe_path: pipe_path,
            audio_pipe_samplerate: samplerate,
            audio_pipe_bitdepth: bitdepth,
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
        match self.status.lock().unwrap().next_idx() {
            Some(idx) => self.send_command(PlayerCommand::JumpTo(idx)),
            None => false,
        }
    }

    pub fn previous(&self) -> bool {
        match self.status.lock().unwrap().previous_idx() {
            Some(idx) => self.send_command(PlayerCommand::JumpTo(idx)),
            None => false,
        }
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
    pub fn set_initial_queue(&self, items: &[MediaItem], cursor: usize) {
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
        st.position_ticks = items[cursor].playback_position_ticks;
        st.runtime_ticks = items[cursor].runtime_ticks;
        st.paused = false;
        st.current_idx = cursor;
        st.queue_len = items.len();
        st.active = false;
        st.set_current_item_metadata(&items[cursor]);
    }

    // Pipe mode always forces headless (no video window), regardless of item
    // type. Reads `audio_pipe_enabled` from `client.config` (rather than a
    // field cached on `Player`) so a setting toggled mid-session takes effect
    // on the very next play() call instead of requiring an app restart.
    fn headless_for(&self, client: &EmbyClient, is_audio: bool) -> bool {
        client.config.audio_pipe_enabled || (!self.show_audio_window && is_audio)
    }

    pub fn play(&self, item: &MediaItem, client: Arc<EmbyClient>, initial_volume: u8) {
        // Reuse the existing mpv window only when the headless state matches:
        // video→video and audio→audio reuse; video→audio and audio→video always
        // spawn a new process so the window visibility is correct.
        let new_is_headless = self.headless_for(&client, item.is_audio());
        if self.status.lock().unwrap().active
            && (self.current_is_headless.load(Ordering::Relaxed) == new_is_headless)
        {
            let ep = if item.is_audio() { "Audio" } else { "Videos" };
            let url = format!(
                "{}/{}/{}/stream?static=true&api_key={}",
                self.server_url, ep, item.id, self.token
            );
            let start_pos = if item.should_resume() {
                item.resume_seconds()
            } else {
                0.0
            };
            {
                let mut st = self.status.lock().unwrap();
                st.position_ticks = item.playback_position_ticks;
                st.runtime_ticks = item.runtime_ticks;
                st.paused = false;
                st.current_idx = 0;
                st.queue_len = 1;
                st.set_current_item_metadata(item);
            }
            self.send_command(PlayerCommand::LoadNew {
                url,
                start_pos,
                item: Box::new(item.clone()),
            });
            return;
        }

        self.stop();
        self.join();

        let item = item.clone();
        let is_audio = item.is_audio();
        let headless = new_is_headless;
        let item_pos = if is_audio {
            0
        } else {
            item.playback_position_ticks
        };
        let start_pos = if is_audio || !item.should_resume() {
            0.0
        } else {
            item.resume_seconds()
        };
        let ep = if is_audio { "Audio" } else { "Videos" };
        let url = format!(
            "{}/{}/{}/stream?static=true&api_key={}",
            self.server_url, ep, item.id, self.token
        );
        let title = item.display_name();

        let config = MpvRunConfig {
            headless,
            use_mpv_config: self.use_mpv_config,
            no_scripts: self.no_scripts,
            always_skip_intro: self.always_skip_intro,
            audio_pipe_path: client.config.audio_pipe_target(),
            audio_pipe_samplerate: client.config.audio_pipe_samplerate,
            audio_pipe_bitdepth: client.config.audio_pipe_bitdepth,
        };
        let status = self.status.clone();
        let event_tx = self.event_tx.clone();
        let ws_tx = self.ws_tx.clone();
        let subtitle_prefs = self.subtitle_prefs.clone();
        let is_queue_mode = self.is_queue_mode.clone();
        let shutdown_report_timeout = self.shutdown_report_timeout.clone();
        let server_url = self.server_url.clone();
        let token = self.token.clone();
        self.current_is_headless.store(headless, Ordering::Relaxed);

        {
            let mut st = status.lock().unwrap();
            st.position_ticks = item_pos;
            st.runtime_ticks = item.runtime_ticks;
            st.paused = false;
            st.current_idx = 0;
            st.queue_len = 1;
            st.active = true;
            st.set_current_item_metadata(&item);
        }

        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        *self.stop_tx.lock().unwrap() = Some(stop_tx);
        *self.shutdown_report_timeout.lock().unwrap() = None;
        let (cmd_tx, cmd_rx) = mpsc::channel::<PlayerCommand>();
        *self.cmd_tx.lock().unwrap() = Some(cmd_tx);
        let wakeup_pipe = make_wakeup_pipe();
        let wakeup_read_fd = wakeup_pipe.as_ref().map(|(r, _)| *r).unwrap_or(-1);
        let wakeup_write_fd = wakeup_pipe.as_ref().map(|(_, w)| w.0).unwrap_or(-1);
        *self.wakeup_fd.lock().unwrap() = wakeup_pipe.map(|(_, w)| w);
        let pre_warmed = self.pre_warmed_mpv.lock().unwrap().take();

        let handle = thread::spawn(move || {
            is_queue_mode.store(false, Ordering::Relaxed);

            let (mpv, startup_pause_for_pipe) = match pre_warmed {
                Some(w) => w,
                None => match init_mpv(&config) {
                    Ok(v) => v,
                    Err(e) => {
                        log::error!(target: "player", "{}", e);
                        return;
                    }
                },
            };
            init_volume(&mpv, &status, initial_volume);

            if start_pos > 0.0 {
                let _ = mpv.set_property("start", format!("{:.0}", start_pos));
            }
            let title_opt = mpv_title_opt(&title);
            log::info!(target: "player", "loadfile url={url} opts={title_opt:?}");
            if let Err(e) = mpv.command(
                "loadfile",
                &[url.as_str(), "replace", "-1", title_opt.as_str()],
            ) {
                log::warn!(target: "player", "loadfile error: {} | url={url} opts={title_opt:?}", mpv_err_str(&e));
                return;
            }
            send_ep_info(&mpv, &item);
            observe_properties(&mpv, config.use_mpv_config);

            let info = client.get_playback_info(&item.id);
            {
                let client = client.clone();
                let item = item.clone();
                let media_source_id = info.media_source_id.clone();
                let session_id = info.session_id.clone();
                thread::spawn(move || {
                    let ok = client.report_start(&item, &media_source_id, &session_id);
                    if !ok {
                        log::warn!(target: "player", "report_start failed for item={}", item.id);
                    }
                });
            }
            let reporter = SessionReporter::new(
                client,
                ws_tx,
                item.id.clone(),
                info.media_source_id,
                info.session_id,
                is_audio,
                status.clone(),
            );
            let progress = spawn_progress_reporter(reporter.clone());
            let session = PlaybackRun::new(
                vec![item.clone()],
                0,
                PlaybackOrigin::Standalone,
                reporter,
                config,
                startup_pause_for_pipe,
                status,
                event_tx,
                subtitle_prefs,
                is_queue_mode.clone(),
                shutdown_report_timeout,
                server_url,
                token,
                info.external_subtitle_urls,
            );
            session.run(mpv, stop_rx, cmd_rx, progress, wakeup_read_fd, wakeup_write_fd);
        });
        *self.thread_handle.lock().unwrap() = Some(handle);
    }

    pub fn play_queue(
        &self,
        items: Vec<MediaItem>,
        start_idx: usize,
        client: Arc<EmbyClient>,
        initial_volume: u8,
    ) {
        if items.is_empty() {
            return;
        }

        let all_audio = items
            .iter()
            .all(|i| i.media_type == "Audio" || i.item_type == "Audio");
        let new_is_headless = self.headless_for(&client, all_audio);

        // If playlist loop already running and headless state matches, replace in
        // place (no window close). Mismatched state (e.g. video→audio-only or
        // vice-versa) always spawns a new process so visibility is correct.
        if self.status.lock().unwrap().active
            && self.is_queue_mode.load(Ordering::Relaxed)
            && (self.current_is_headless.load(Ordering::Relaxed) == new_is_headless)
        {
            let start_idx = start_idx.min(items.len() - 1);
            {
                let mut st = self.status.lock().unwrap();
                st.position_ticks = items[start_idx].playback_position_ticks;
                st.runtime_ticks = items[start_idx].runtime_ticks;
                st.paused = false;
                st.current_idx = start_idx;
                st.queue_len = items.len();
                st.set_current_item_metadata(&items[start_idx]);
            }
            self.send_command(PlayerCommand::ReplaceQueue { items, start_idx });
            return;
        }

        self.stop();
        self.join();

        let start_idx = start_idx.min(items.len() - 1);
        let headless = new_is_headless;

        let config = MpvRunConfig {
            headless,
            use_mpv_config: self.use_mpv_config,
            no_scripts: self.no_scripts,
            always_skip_intro: self.always_skip_intro,
            audio_pipe_path: client.config.audio_pipe_target(),
            audio_pipe_samplerate: client.config.audio_pipe_samplerate,
            audio_pipe_bitdepth: client.config.audio_pipe_bitdepth,
        };
        let status = self.status.clone();
        let event_tx = self.event_tx.clone();
        let ws_tx = self.ws_tx.clone();
        let subtitle_prefs = self.subtitle_prefs.clone();
        let is_queue_mode = self.is_queue_mode.clone();
        let shutdown_report_timeout = self.shutdown_report_timeout.clone();
        let server_url = self.server_url.clone();
        let token = self.token.clone();
        self.current_is_headless.store(headless, Ordering::Relaxed);

        {
            let mut st = status.lock().unwrap();
            st.position_ticks = 0;
            st.runtime_ticks = items[start_idx].runtime_ticks;
            st.paused = false;
            st.current_idx = start_idx;
            st.queue_len = items.len();
            st.active = true;
            st.set_current_item_metadata(&items[start_idx]);
        }

        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        *self.stop_tx.lock().unwrap() = Some(stop_tx);
        *self.shutdown_report_timeout.lock().unwrap() = None;
        let (cmd_tx, cmd_rx) = mpsc::channel::<PlayerCommand>();
        *self.cmd_tx.lock().unwrap() = Some(cmd_tx);
        let wakeup_pipe = make_wakeup_pipe();
        let wakeup_read_fd = wakeup_pipe.as_ref().map(|(r, _)| *r).unwrap_or(-1);
        let wakeup_write_fd = wakeup_pipe.as_ref().map(|(_, w)| w.0).unwrap_or(-1);
        *self.wakeup_fd.lock().unwrap() = wakeup_pipe.map(|(_, w)| w);
        let pre_warmed = self.pre_warmed_mpv.lock().unwrap().take();

        let handle = thread::spawn(move || {
            is_queue_mode.store(true, Ordering::Relaxed);

            let (mpv, startup_pause_for_pipe) = match pre_warmed {
                Some(w) => w,
                None => match init_mpv(&config) {
                    Ok(v) => v,
                    Err(e) => {
                        log::error!(target: "player", "{}", e);
                        return;
                    }
                },
            };
            init_volume(&mpv, &status, initial_volume);

            // Load the full playlist into mpv so every index matches items[i] directly.
            for (i, item) in items.iter().enumerate() {
                let ep = if item.is_audio() { "Audio" } else { "Videos" };
                let url = format!(
                    "{}/{}/{}/stream?static=true&api_key={}",
                    server_url, ep, item.id, token
                );
                let mode = if i == 0 { "replace" } else { "append-play" };
                let title_opt = mpv_title_opt(&item.display_name());
                if let Err(e) =
                    mpv.command("loadfile", &[url.as_str(), mode, "-1", title_opt.as_str()])
                {
                    log::warn!(target: "player", "loadfile error: {} | opts={title_opt:?}", mpv_err_str(&e));
                    if i == 0 {
                        // First file failed: nothing queued, exit cleanly.
                        status.lock().unwrap().active = false;
                        return;
                    }
                    // Subsequent file failed: skip it, keep playing what loaded.
                }
            }
            send_ep_info(&mpv, &items[start_idx]);
            observe_properties(&mpv, config.use_mpv_config);

            let info = client.get_playback_info(&items[start_idx].id);
            {
                let client = client.clone();
                let item = items[start_idx].clone();
                let media_source_id = info.media_source_id.clone();
                let session_id = info.session_id.clone();
                thread::spawn(move || {
                    let ok = client.report_start(&item, &media_source_id, &session_id);
                    if !ok {
                        log::warn!(target: "player", "report_start failed for item={}", item.id);
                    }
                });
            }
            let reporter = SessionReporter::new(
                client,
                ws_tx,
                items[start_idx].id.clone(),
                info.media_source_id,
                info.session_id,
                items[start_idx].is_audio(),
                status.clone(),
            );
            let progress = spawn_progress_reporter(reporter.clone());
            let session = PlaybackRun::new(
                items,
                start_idx,
                PlaybackOrigin::Queue,
                reporter,
                config,
                startup_pause_for_pipe,
                status,
                event_tx,
                subtitle_prefs,
                is_queue_mode.clone(),
                shutdown_report_timeout,
                server_url,
                token,
                info.external_subtitle_urls,
            );
            session.run(mpv, stop_rx, cmd_rx, progress, wakeup_read_fd, wakeup_write_fd);
        });
        *self.thread_handle.lock().unwrap() = Some(handle);
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

// ── PlayerProxy ─────────────────────────────────────────────────────────────
// Wraps either a local Player or a RemotePlayer so App can use a single type.
