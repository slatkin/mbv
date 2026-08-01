enum PlayerProxyInner {
    Local(Player),
    Remote(crate::remote_player::RemotePlayer),
}

pub struct PlayerProxy {
    pub always_play_next: bool,
    pub status: Arc<Mutex<PlayerStatus>>,
    pub subtitle_prefs: Arc<Mutex<SubtitlePrefs>>,
    inner: PlayerProxyInner,
}

impl PlayerProxy {
    /// Test helper for root-crate integration tests that need a local player
    /// proxy without starting a real mpv session.
    #[cfg(any(test, feature = "test-support"))]
    pub fn stub(status: Arc<Mutex<PlayerStatus>>) -> Self {
        let (tx, _rx) = std::sync::mpsc::channel();
        let player = Player::new(
            String::new(),
            String::new(),
            false,
            false,
            false,
            false,
            false,
            SubtitlePrefs::default(),
            tx,
            None,
        );
        let subtitle_prefs = player.subtitle_prefs.clone();
        PlayerProxy {
            always_play_next: false,
            status,
            subtitle_prefs,
            inner: PlayerProxyInner::Local(player),
        }
    }

    /// Like [`stub`](Self::stub), but with a real background thread wired up
    /// as the local `Player`'s `thread_handle`, which sleeps for `sleep`
    /// before finishing. Lets root-crate tests prove teardown code that
    /// calls `join_or_timeout` actually returns within its bound instead of
    /// blocking for the full `sleep` duration — the previously-unbounded
    /// normal in-app quit path (#202) is exactly this scenario with a
    /// hanging player thread.
    #[cfg(any(test, feature = "test-support"))]
    pub fn stub_with_hung_thread(status: Arc<Mutex<PlayerStatus>>, sleep: Duration) -> Self {
        let (tx, _rx) = std::sync::mpsc::channel();
        let player = Player::new(
            String::new(),
            String::new(),
            false,
            false,
            false,
            false,
            false,
            SubtitlePrefs::default(),
            tx,
            None,
        );
        let handle = std::thread::spawn(move || {
            std::thread::sleep(sleep);
        });
        *player.thread_handle.lock().unwrap() = Some(handle);
        let subtitle_prefs = player.subtitle_prefs.clone();
        PlayerProxy {
            always_play_next: false,
            status,
            subtitle_prefs,
            inner: PlayerProxyInner::Local(player),
        }
    }

    /// Wires a fresh command channel into the local player and returns the
    /// receiving end, so a test can assert on what `send_command` actually sent
    /// without a real mpv thread running.
    /// Test helper that exposes the next command sent through a local proxy.
    #[cfg(any(test, feature = "test-support"))]
    pub fn spy_on_commands(&self) -> mpsc::Receiver<PlayerCommand> {
        let (tx, rx) = mpsc::channel();
        if let PlayerProxyInner::Local(p) = &self.inner {
            *p.cmd_tx.lock().unwrap() = Some(tx);
        }
        rx
    }

    pub fn local(player: Player, always_play_next: bool) -> Self {
        let status = player.status.clone();
        let subtitle_prefs = player.subtitle_prefs.clone();
        PlayerProxy {
            always_play_next,
            status,
            subtitle_prefs,
            inner: PlayerProxyInner::Local(player),
        }
    }

    pub fn remote(remote: crate::remote_player::RemotePlayer, always_play_next: bool) -> Self {
        let status = remote.status.clone();
        let subtitle_prefs = remote.subtitle_prefs.clone();
        PlayerProxy {
            always_play_next,
            status,
            subtitle_prefs,
            inner: PlayerProxyInner::Remote(remote),
        }
    }

    pub fn play(
        &self,
        item: &MediaItem,
        source: crate::config::QueueSource,
        client: Arc<EmbyClient>,
        initial_volume: u8,
    ) {
        match &self.inner {
            PlayerProxyInner::Local(p) => p.play(item, client, initial_volume),
            PlayerProxyInner::Remote(r) => r.play(item, source, client, initial_volume),
        }
    }

    pub fn play_queue(
        &self,
        items: Vec<MediaItem>,
        start_idx: usize,
        source: crate::config::QueueSource,
        client: Arc<EmbyClient>,
        initial_volume: u8,
    ) {
        match &self.inner {
            PlayerProxyInner::Local(p) => p.play_queue(items, start_idx, client, initial_volume),
            PlayerProxyInner::Remote(r) => {
                r.play_queue(items, start_idx, source, client, initial_volume)
            }
        }
    }

    pub fn stop(&self) {
        match &self.inner {
            PlayerProxyInner::Local(p) => p.stop(),
            PlayerProxyInner::Remote(r) => r.stop(),
        }
    }

    pub fn stop_for_shutdown(&self, timeout: Duration) {
        match &self.inner {
            PlayerProxyInner::Local(p) => p.stop_for_shutdown(timeout),
            PlayerProxyInner::Remote(_) => {}
        }
    }

    pub fn join(&self) {
        match &self.inner {
            PlayerProxyInner::Local(p) => p.join(),
            PlayerProxyInner::Remote(r) => r.join(),
        }
    }

    /// Tears down the underlying connection if this proxy is currently
    /// remote (#233): a no-op for `Local` (there's no socket to close).
    /// Call this on the *old* `PlayerProxy` before overwriting it with a
    /// freshly connected one -- a remote-to-remote swap that skips this
    /// leaks the old connection's reader thread (see
    /// `RemotePlayer::disconnect`'s doc comment for why `Drop` alone isn't
    /// enough).
    pub fn disconnect_remote(&self) {
        if let PlayerProxyInner::Remote(r) = &self.inner {
            r.disconnect();
        }
    }

    pub fn join_or_timeout(&self, timeout: std::time::Duration) {
        match &self.inner {
            PlayerProxyInner::Local(p) => p.join_or_timeout(timeout),
            PlayerProxyInner::Remote(_) => {}
        }
    }

    pub fn send_ctrl_cmd(&self, cmd: crate::ctrl::CtrlCmd) -> bool {
        match &self.inner {
            PlayerProxyInner::Local(_) => false,
            PlayerProxyInner::Remote(r) => r.send_ctrl_cmd(cmd),
        }
    }

    pub fn send_playback_intent(&self, intent: crate::ctrl::PlaybackIntent) -> bool {
        match &self.inner {
            PlayerProxyInner::Local(_) => false,
            PlayerProxyInner::Remote(remote) => remote.send_playback_intent(intent),
        }
    }

    pub fn send_command(&self, cmd: PlayerCommand) -> bool {
        match &self.inner {
            PlayerProxyInner::Local(p) => p.send_command(cmd),
            PlayerProxyInner::Remote(r) => r.send_command(cmd),
        }
    }

    /// Send QueueRemoveActive to the daemon (task 7.1). Only valid for
    /// v8+ remote connections; returns false for local or v7 peers.
    pub fn send_queue_remove_active(&self, revision: crate::playback_queue::QueueRevision) -> bool {
        match &self.inner {
            PlayerProxyInner::Local(_) => false,
            PlayerProxyInner::Remote(r) => r.send_queue_remove_active(revision),
        }
    }

    /// Send QueueInsertAt to the daemon for undo restoration (task 9.3).
    /// Only valid for v8+ remote connections; returns false for local or v7 peers.
    pub fn send_queue_insert_at(
        &self,
        item: crate::api::MediaItem,
        position: usize,
        revision: crate::playback_queue::QueueRevision,
    ) -> bool {
        match &self.inner {
            PlayerProxyInner::Local(_) => false,
            PlayerProxyInner::Remote(r) => r.send_queue_insert_at(item, position, revision),
        }
    }

    /// Populate slot state for v8 command translation (test helper).
    /// No-op for local players.
    pub fn set_slot_state(
        &self,
        slot_ids: Vec<crate::playback_queue::QueueSlotId>,
        revision: crate::playback_queue::QueueRevision,
    ) {
        match &self.inner {
            PlayerProxyInner::Local(_) => {}
            PlayerProxyInner::Remote(r) => r.set_slot_state(slot_ids, revision),
        }
    }

    pub fn supports_queue_append(&self) -> bool {
        match &self.inner {
            PlayerProxyInner::Local(_) => true,
            PlayerProxyInner::Remote(r) => r.supports_queue_append(),
        }
    }

    pub fn next(&self) -> bool {
        match self.status.lock().unwrap().next_idx() {
            Some(idx) => match &self.inner {
                PlayerProxyInner::Local(_) => self.send_command(PlayerCommand::JumpTo(idx)),
                PlayerProxyInner::Remote(remote) => remote.send_playback_intent(
                    remote.new_playback_intent(crate::ctrl::PlaybackIntentAction::Next),
                ),
            },
            None => false,
        }
    }

    pub fn previous(&self) -> bool {
        match self.status.lock().unwrap().previous_idx() {
            Some(idx) => match &self.inner {
                PlayerProxyInner::Local(_) => self.send_command(PlayerCommand::JumpTo(idx)),
                PlayerProxyInner::Remote(remote) => remote.send_playback_intent(
                    remote.new_playback_intent(crate::ctrl::PlaybackIntentAction::Previous),
                ),
            },
            None => false,
        }
    }

    pub fn set_paused(&self, paused: bool) -> bool {
        match &self.inner {
            PlayerProxyInner::Local(_) => match self.status.lock().unwrap().toggle_to_reach(paused)
            {
                Some(cmd) => self.send_command(cmd),
                None => false,
            },
            PlayerProxyInner::Remote(remote) => remote.send_playback_intent(
                remote.new_playback_intent(crate::ctrl::PlaybackIntentAction::SetPaused { paused }),
            ),
        }
    }

    pub fn is_remote(&self) -> bool {
        matches!(self.inner, PlayerProxyInner::Remote(_))
    }

    /// Returns a clone of the raw local `Player`'s command channel, or
    /// `None` when this proxy currently wraps a `RemotePlayer`.
    ///
    /// Intended for callers (e.g. the stay-alive tray, `src/tray.rs`) that
    /// must drive playback through the in-process `Player` mpsc *only* --
    /// never through `send_command`/`next`/`previous`/`set_paused` above,
    /// which forward to `RemotePlayer::send_command` (a ctrl-socket call)
    /// when this proxy is `Remote`. Capture this once, while the proxy is
    /// known to be `Local`, and keep the returned `Arc` rather than
    /// re-deriving it later: `App::switch_to_direct_remote` /
    /// `restore_local_mode` can swap `PlayerProxy`'s inner variant at
    /// runtime, but they never replace the underlying local `Player`
    /// object itself (it is suspended/resumed, not dropped), so the `Arc`
    /// returned here stays valid and keeps targeting the same in-process
    /// player for the caller's whole lifetime -- immune to later
    /// local/remote swaps on `self`.
    pub fn local_cmd_tx(&self) -> Option<Arc<Mutex<Option<mpsc::Sender<PlayerCommand>>>>> {
        match &self.inner {
            PlayerProxyInner::Local(p) => Some(p.cmd_tx.clone()),
            PlayerProxyInner::Remote(_) => None,
        }
    }

    pub fn is_remote_disconnected(&self) -> bool {
        match &self.inner {
            PlayerProxyInner::Local(_) => false,
            PlayerProxyInner::Remote(r) => r.is_disconnected(),
        }
    }

    /// Whether the remote disconnect (if any) followed an announced daemon
    /// shutdown, as opposed to an unannounced loss (crash). `Local` has no
    /// daemon connection to lose, so it is always `false`. Mirrors
    /// `is_remote_disconnected()`.
    pub fn is_shutdown_announced(&self) -> bool {
        match &self.inner {
            PlayerProxyInner::Local(_) => false,
            PlayerProxyInner::Remote(r) => r.is_shutdown_announced(),
        }
    }

    /// Shared disconnect flag for the current inner target, or `None` for a
    /// local `Player` (which has no daemon connection to lose). Mirrors
    /// `RemotePlayer::disconnected_flag()` -- used by callers (MPRIS
    /// wiring, see #175) that need to rebind to whichever target currently
    /// owns playback without caring whether that's local or remote.
    pub fn disconnected_flag(&self) -> Option<Arc<AtomicBool>> {
        match &self.inner {
            PlayerProxyInner::Local(_) => None,
            PlayerProxyInner::Remote(r) => Some(r.disconnected_flag()),
        }
    }

    /// A `'static`, cheaply-cloneable command sender for whichever target
    /// this proxy currently wraps, independent of `self`'s lifetime.
    ///
    /// Exists so callers that need to hand a command path to something
    /// that outlives `self` (e.g. MPRIS's polling thread, see #175) don't
    /// have to hand-roll the local/remote match themselves -- and so that
    /// rebinding after `App::switch_to_direct_remote` /
    /// `restore_local_mode` always routes through whatever `self.inner`
    /// currently is at the moment this is called, not whatever it was when
    /// some earlier closure was built.
    pub fn command_sender(&self) -> Arc<dyn Fn(PlayerCommand) + Send + Sync> {
        match &self.inner {
            PlayerProxyInner::Local(p) => {
                let cmd_tx = p.cmd_tx.clone();
                Arc::new(move |cmd: PlayerCommand| {
                    if let Some(tx) = cmd_tx.lock().unwrap().as_ref() {
                        let _ = tx.send(cmd);
                    }
                })
            }
            PlayerProxyInner::Remote(r) => {
                let remote = r.clone();
                Arc::new(move |cmd: PlayerCommand| {
                    remote.send_command(cmd);
                })
            }
        }
    }

    /// Returns a clonable stop handle for use from other threads (e.g. the
    /// quit watchdog). None in remote mode — the daemon owns the player.
    pub fn quit_handle(&self) -> Option<QuitHandle> {
        match &self.inner {
            PlayerProxyInner::Local(p) => Some(QuitHandle {
                stop_tx: p.stop_tx.clone(),
                shutdown_report_timeout: p.shutdown_report_timeout.clone(),
            }),
            PlayerProxyInner::Remote(_) => None,
        }
    }
}

/// Retry mark_played in a detached thread with exponential backoff.
/// Max 3 attempts (initial + 2 retries), delays: 500ms, 2s.
fn retry_mark_played(client: Arc<EmbyClient>, item_id: String) {
    std::thread::spawn(move || {
        let delays = [500, 2000]; // ms
        for (i, delay_ms) in delays.iter().enumerate() {
            std::thread::sleep(Duration::from_millis(*delay_ms));
            match client.mark_played(&item_id) {
                Ok(()) => {
                    log::info!(target: "player", "mark_played retry {} ok id={item_id}", i + 1);
                    return;
                }
                Err(e) => {
                    log::warn!(target: "player", "mark_played retry {} failed id={item_id}: {e}", i + 1);
                }
            }
        }
    });
}

// True when the track ended close enough to its natural end to count as played.
// Threshold: position ≥ 95% of runtime (19/20 integer check avoids floating point).
pub(crate) fn is_near_end(
    is_audio: bool,
    natural: bool,
    last_valid_pos: i64,
    runtime_ticks: i64,
) -> bool {
    !natural && !is_audio && runtime_ticks > 0 && last_valid_pos * 20 / runtime_ticks >= 19
}

// Position to report to Emby when a playlist track ends.
// Zero means "treat as fully played / reset resume point".
// was_next_up alone does NOT zero the position — the user may have dismissed
// or ignored the overlay, and we must preserve where they actually were.
pub(crate) fn queue_completed_pos(
    is_audio: bool,
    natural: bool,
    near_end: bool,
    last_valid_pos: i64,
) -> i64 {
    if is_audio || natural || near_end {
        0
    } else {
        last_valid_pos
    }
}

fn quit_timeout_stop_flags(
    origin: PlaybackOrigin,
    is_audio: bool,
    last_valid_pos: i64,
    runtime_ticks: i64,
    stopped_near_end: bool,
) -> (bool, bool) {
    match origin {
        PlaybackOrigin::Standalone => (
            is_near_end(is_audio, false, last_valid_pos, runtime_ticks),
            false,
        ),
        PlaybackOrigin::Queue => (stopped_near_end, stopped_near_end),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StopReportContext {
    Ordinary,
    ShutdownAware,
}

fn end_file_stop_report_context(reason: EndFileReason) -> StopReportContext {
    if reason == mpv_end_file_reason::Quit {
        StopReportContext::ShutdownAware
    } else {
        StopReportContext::Ordinary
    }
}
