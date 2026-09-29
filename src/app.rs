mod dispatch;
mod infra;
mod input;
pub(crate) mod state;

pub(in crate::app) use self::infra::paging::{PAGE_SIZE, PREFETCH_AHEAD};
pub(in crate::app) use self::infra::signals::{
    QUIT_REQUESTED, install_signal_handlers, start_quit_watchdog,
};
pub(crate) use self::infra::terminal::set_mouse_capture;
pub(in crate::app) use self::infra::terminal::{init_terminal, open_url, restore_terminal};
pub use self::state::app_struct::App;
#[cfg(test)]
mod test_seams;
#[cfg(test)]
pub(in crate::app) use self::test_seams::{
    CAST_CONNECT_OVERRIDE, CAST_CONNECT_TEST_LOCK, CastConnectFn, DAEMON_ROUTE_CONNECT_OVERRIDE,
    DAEMON_ROUTE_CONNECT_TEST_LOCK, DIRECT_CONNECT_OVERRIDE, SESSIONS_LOAD_OVERRIDE,
};
mod shell;
use self::dispatch::notify::ToastSeverity;
pub use self::shell::Model;
use self::state::app_init::AppInit;
use self::state::bootstrap::bootstrap_unified_queue;
use self::state::playback_target::{
    CastPlaybackTarget, LocalPlaybackTarget, PlaybackTarget, RemotePlaybackTarget,
};
use crate::app::state::events::SessionEvent;
pub(in crate::app) use crate::app::state::events::{
    AudiobookshelfEvent, BrowseEvent, LibEvent, ModelContentEvent, MusicEvent, PlaylistEvent,
    SeriesEvent,
};
use crate::app::state::playback::{
    DestinationLatestSource, PendingQueueAction, QueueScopeResolution, RemoteSlotState,
    ReplacementExecutor, RoutedReplacementPrep, SuspendedLocalSession, UndoEntry,
};
use crate::app::state::player_tab::PlayerTab;
use crate::app::state::queue_deferrals::QueueDeferrals;
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
    AlbumIndex, AlbumIndexState, AlbumPathPart, AlbumSearchEntry, BrowseLevel, SeriesDetail,
    restore_library_position_with_fetched_rows_for_kind,
};
use mbv_ui_model::confirm::{ConfirmAction, ConfirmModal};
#[cfg(test)]
use mbv_ui_model::context_menu::MultiSelectKind;
use mbv_ui_model::context_menu::{ContextAction, ContextMenuAnchor, ContextMenuEntry};
use mbv_ui_model::daemon_lost::DaemonLostModal;
use mbv_ui_model::feed::{
    FeedHomeVideoGroup, FeedHomeVideoState, IdleFeed, SavePlaylistDialog, SavePlaylistStage,
};
use mbv_ui_model::library_tab::LibraryTab;
pub(in crate::app) use mbv_ui_model::overlay::SidebarId;
use mbv_ui_model::playback::{PlaybackState, QueueScope};
#[cfg(test)]
use mbv_ui_model::settings::SettingKey;
use mbv_ui_model::settings::{PanelFocus, PanelMode};
use mbv_ui_model::tab_selection::TabSelection;
#[cfg(test)]
use std::sync::Arc;
use std::time::{Duration, Instant};

#[cfg(test)]
pub(crate) mod tests;
