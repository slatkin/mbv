/// State for the blocking modal raised when a local daemon's connection is
/// lost with no announced shutdown (a crash) -- see `player_event.rs`'s
/// `PlayerEvent::Stopped` handling and `render/overlays/daemon_lost_modal.rs`.
/// Only one blocking modal can be active at a time:
/// Option<DaemonLostModal>`), mirroring `ConfirmModal`'s convention.
pub(super) struct DaemonLostModal {
    /// The label of whatever was playing right before the daemon vanished,
    /// if known.
    pub(super) last_playing_title: Option<String>,
    /// Where the local daemon's own log lives, so the user can inspect why
    /// it died without having to know the path by heart.
    pub(super) daemon_log_path: String,
    /// Set when a `[R]`/`[S]` restart attempt fails, so the modal can show
    /// an updated diagnostic line instead of silently doing nothing.
    pub(super) restart_error: Option<String>,
}
