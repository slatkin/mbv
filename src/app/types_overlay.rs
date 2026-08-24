use super::types_confirm::ConfirmModal;
use super::types_daemon_lost::DaemonLostModal;
use super::types_feed::SavePlaylistDialog;
use super::types_playback::RemoteReanchorPopup;
use super::types_selection_modal::{
    SelectionModal, SelectionModalFilter, SelectionModalListState, SelectionModalSource,
};
use super::SidebarId;

/// Shell handoffs used while App action code is still called below Model.
/// These are requests, not a second copy of component interaction state.
pub(super) enum OverlayRequest {
    OpenSidebar(SidebarId),
    DismissSidebar(SidebarId),
    ToggleSidebar(SidebarId),
    Confirm(ConfirmModal),
    DaemonLost(DaemonLostModal),
    RemoteReanchor(RemoteReanchorPopup),
    SavePlaylist(SavePlaylistDialog),
    SelectionModal(SelectionModal),
    RefreshSelectionModal {
        source: SelectionModalSource,
        state: SelectionModalListState,
        filter: Option<SelectionModalFilter>,
    },
    RefreshSelectionModalAtSelectedFilter {
        source: SelectionModalSource,
    },
    DismissConfirm,
    DismissDaemonLost,
    DismissRemoteReanchor,
    DismissSavePlaylist,
    DismissSelectionModal,
}
