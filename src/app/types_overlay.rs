use super::types_confirm::ConfirmModal;
use super::types_daemon_lost::DaemonLostModal;
use super::types_feed::SavePlaylistDialog;
use super::types_playback::RemoteReanchorPopup;

/// Shell handoffs used while App action code is still called below Model.
/// These are requests, not a second copy of component interaction state.
pub(super) enum OverlayRequest {
    Confirm(ConfirmModal),
    DaemonLost(DaemonLostModal),
    RemoteReanchor(RemoteReanchorPopup),
    SavePlaylist(SavePlaylistDialog),
    DismissConfirm,
    DismissDaemonLost,
    DismissRemoteReanchor,
    DismissSavePlaylist,
}
