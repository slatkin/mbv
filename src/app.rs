mod dispatch;
mod infra;
mod input;
pub(in crate::app) mod state;
pub(crate) mod ui_msg;

pub(in crate::app) use self::infra::paging::{PAGE_SIZE, PREFETCH_AHEAD};
pub(in crate::app) use self::infra::signals::{
    install_signal_handlers, start_quit_watchdog, QUIT_REQUESTED,
};
pub(crate) use self::infra::terminal::set_mouse_capture;
pub(in crate::app) use self::infra::terminal::{init_terminal, open_url, restore_terminal};
pub use self::state::app_struct::App;
#[cfg(test)]
mod test_seams;
#[cfg(test)]
pub(in crate::app) use self::test_seams::{
    CastConnectFn, CAST_CONNECT_OVERRIDE, CAST_CONNECT_TEST_LOCK, DAEMON_ROUTE_CONNECT_OVERRIDE,
    DAEMON_ROUTE_CONNECT_TEST_LOCK, DIRECT_CONNECT_OVERRIDE, LOCAL_PLAYER_PREPARE_OVERRIDE,
    SESSIONS_LOAD_OVERRIDE,
};
mod shell;
use self::dispatch::notify::ToastSeverity;
pub use self::shell::Model;
use self::state::app_init::AppInit;
use self::state::bootstrap::bootstrap_unified_queue;
use self::state::playback_target::{
    CastPlaybackTarget, LocalPlaybackTarget, PlaybackTarget, RemotePlaybackTarget,
};
#[cfg(test)]
use mbv_ctrl::player::PlayerEvent;
#[cfg(test)]
use mbv_emby_model::EmbyItem;
pub(crate) use mbv_images::resize::spawn_resize_worker;
#[cfg(test)]
use mbv_queue::RemoveSlotResult;
#[cfg(test)]
use mbv_ui_model::browse::restore_library_position;
use mbv_ui_model::browse::{
    restore_library_position_with_fetched_rows_for_kind, AlbumIndex, AlbumIndexState,
    AlbumPathPart, AlbumSearchEntry, BrowseLevel, SeriesDetail,
};
use mbv_ui_model::confirm::{ConfirmAction, ConfirmModal};
#[cfg(test)]
use mbv_ui_model::context_menu::MultiSelectKind;
use mbv_ui_model::context_menu::{ContextAction, ContextMenuAnchor, ContextMenuEntry};
use mbv_ui_model::daemon_lost::DaemonLostModal;
use mbv_ui_model::events::{LibEvent, SessionEvent};
use mbv_ui_model::feed::{
    FeedHomeVideoGroup, FeedHomeVideoState, IdleFeed, SavePlaylistDialog, SavePlaylistStage,
};
use mbv_ui_model::library_tab::LibraryTab;
use mbv_ui_model::playback::{
    DestinationLatestSource, PendingQueueAction, PlaybackState, QueueScope, QueueScopeResolution,
    RemoteSlotState, ReplacementExecutor, RoutedReplacementPrep, SuspendedLocalSession, UndoEntry,
};
use mbv_ui_model::player_tab::PlayerTab;
#[cfg(test)]
use mbv_ui_model::settings::SettingKey;
use mbv_ui_model::settings::{PanelFocus, PanelMode};
pub(in crate::app) use mbv_ui_model::sidebar::SidebarId;
use mbv_ui_model::tab_selection::TabSelection;
#[cfg(test)]
use std::sync::Arc;
use std::time::{Duration, Instant};

#[cfg(test)]
pub(crate) mod tests;
