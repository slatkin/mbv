use crate::app::state::types::confirm::ConfirmModal;
use crate::app::state::types::context_menu::{ContextMenu, MultiSelectKind};
use crate::app::state::types::daemon_lost::DaemonLostModal;
use crate::app::state::types::feed::SavePlaylistDialog;
use crate::app::state::types::sidebar::SidebarId;

/// Shell handoffs used while App action code is still called below Model.
/// These are requests, not a second copy of component interaction state.
pub(in crate::app) enum OverlayRequest {
    OpenSidebar(SidebarId),
    DismissSidebar(SidebarId),
    ToggleSidebar(SidebarId),
    Confirm(ConfirmModal),
    DaemonLost(DaemonLostModal),
    SavePlaylist(SavePlaylistDialog),
    ContextMenu(ContextMenu),
    DismissContextMenu,
    /// Open a nested Settings Multiselect popup of the given kind (task 5.3c).
    OpenMultiselect(MultiSelectKind),
    /// Open a nested Settings Library-routes popup (task 5.3c).
    OpenLibraryRoutes,
    /// Open a nested Settings Feed-management popup (task 5.3c).
    OpenFeedsManage,
    DismissConfirm,
    DismissDaemonLost,
    DismissSavePlaylist,
}
