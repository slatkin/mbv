pub mod components;
mod dispatch;
mod infra;
mod input;
pub mod render;
pub(in crate::app) mod state;
pub(crate) mod ui_model;
pub(crate) mod ui_msg;

pub(in crate::app) use self::infra::layout::{
    LEFT_WIDTH_DEFAULT, LEFT_WIDTH_STEP, MINI_VIEW_THRESHOLD, SEARCH_PANEL_W, TABBAR_LEFT_RESERVE,
    TWO_COLUMN_THRESHOLD,
};
pub(in crate::app) use self::infra::paging::{PAGE_SIZE, PREFETCH_AHEAD};
pub(in crate::app) use self::infra::signals::{
    install_signal_handlers, start_quit_watchdog, QUIT_REQUESTED,
};
pub(crate) use self::infra::terminal::set_mouse_capture;
pub(in crate::app) use self::infra::terminal::{init_terminal, open_url, restore_terminal};
pub(crate) use self::infra::{images, layout};
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
use self::infra::images::resize::spawn_resize_worker;
pub use self::shell::Model;
use self::state::app_init::AppInit;
use self::state::bootstrap::bootstrap_unified_queue;
#[cfg(test)]
use self::ui_model::browse::restore_library_position;
use self::ui_model::browse::{
    restore_library_position_with_fetched_rows_for_kind, AlbumIndex, AlbumIndexState,
    AlbumPathPart, AlbumSearchEntry, BrowseLevel, SeriesDetail,
};
use self::ui_model::confirm::{ConfirmAction, ConfirmModal};
#[cfg(test)]
use self::ui_model::context_menu::MultiSelectKind;
use self::ui_model::context_menu::{
    ContextAction, ContextMenuAnchor, ContextMenuEntry, LibraryRouteStage,
};
use self::ui_model::daemon_lost::DaemonLostModal;
use self::ui_model::events::{LibEvent, SessionEvent};
use self::ui_model::feed::{
    FeedHomeVideoGroup, FeedHomeVideoState, IdleFeed, SavePlaylistDialog, SavePlaylistStage,
};
use self::ui_model::library_tab::LibraryTab;
use self::ui_model::playback::{
    CastPlaybackTarget, DestinationLatestSource, LocalPlaybackTarget, PendingQueueAction,
    PlaybackState, PlaybackTarget, QueueScope, QueueScopeResolution, RemotePlaybackTarget,
    RemoteSlotState, ReplacementExecutor, RoutedReplacementPrep, SuspendedLocalSession, UndoEntry,
};
use self::ui_model::player_tab::PlayerTab;
#[cfg(test)]
use self::ui_model::settings::SettingKey;
use self::ui_model::settings::{PanelFocus, PanelMode};
use self::ui_model::tab_selection::TabSelection;
#[cfg(test)]
use mbv_ctrl::player::PlayerEvent;
#[cfg(test)]
use mbv_emby_model::EmbyItem;
#[cfg(test)]
use mbv_queue::RemoveSlotResult;
#[cfg(test)]
use std::sync::Arc;
use std::time::{Duration, Instant};

#[cfg(test)]
pub(crate) mod tests;
