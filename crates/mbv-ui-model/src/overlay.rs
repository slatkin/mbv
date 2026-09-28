use crate::confirm::ConfirmModal;
use crate::context_menu::{ContextMenu, MultiSelectKind};
use crate::daemon_lost::DaemonLostModal;
use crate::feed::SavePlaylistDialog;

/// Which sidebar overlay a shell handoff targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarId {
    Settings,
    Sessions,
    Playlists,
    Search,
}

/// Shell handoffs used while App action code is still called below Model.
/// These are requests, not a second copy of component interaction state.
#[derive(Debug)]
pub enum OverlayRequest {
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
