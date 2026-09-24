pub mod components;
pub(crate) use self::state::home_latest::{capture_launch_window, current_launch_secs};
mod dispatch;
mod infra;
mod input;
pub mod render;
mod shell_draw;
pub(in crate::app) mod state;

pub(in crate::app) use self::infra::layout::{
    LEFT_WIDTH_DEFAULT, LEFT_WIDTH_STEP, MINI_VIEW_THRESHOLD, SEARCH_PANEL_W, TABBAR_LEFT_RESERVE,
    TWO_COLUMN_THRESHOLD,
};
use self::infra::layout::{PAGE_SIZE, PREFETCH_AHEAD};
pub(in crate::app) use self::infra::signals::{
    install_signal_handlers, start_quit_watchdog, QUIT_REQUESTED,
};
pub(crate) use self::infra::terminal::set_mouse_capture;
pub(in crate::app) use self::infra::terminal::{init_terminal, open_url, restore_terminal};
pub(crate) use self::infra::{images, layout, palette, ui_util};
pub use self::state::app_struct::App;
#[cfg(test)]
mod test_seams;
#[cfg(test)]
pub(in crate::app) use self::test_seams::{
    CastConnectFn, CAST_CONNECT_OVERRIDE, CAST_CONNECT_TEST_LOCK, DAEMON_ROUTE_CONNECT_OVERRIDE,
    DAEMON_ROUTE_CONNECT_TEST_LOCK, DIRECT_CONNECT_OVERRIDE, LOCAL_PLAYER_PREPARE_OVERRIDE,
    SESSIONS_LOAD_OVERRIDE, SESSIONS_LOAD_TEST_LOCK,
};
mod shell;
mod shell_audiobookshelf_book;
mod shell_audiobookshelf_podcast;
mod shell_chrome_panels;
mod shell_emby_library;
mod shell_emby_library_content;
mod shell_feeds;
mod shell_feeds_manage;
mod shell_home;
mod shell_home_content;
mod shell_inline_search;
mod shell_library;
#[path = "shell_library_panel.rs"]
mod shell_library_panel;
mod shell_modal_actions;
mod shell_music_workspace;
mod shell_overlays;
mod shell_playback;
mod shell_playlists;
mod shell_queue;
mod shell_root;
mod shell_settings;
mod shell_tv_workspace;
use self::dispatch::notify::ToastSeverity;
use self::infra::resize::spawn_resize_worker;
pub use self::shell::Model;
use self::state::app_init::AppInit;
use self::state::bootstrap::bootstrap_unified_queue;
pub(in crate::app) use self::state::playback_target::NowPlayingStatus;
#[cfg(test)]
use self::state::types::browse::restore_library_position;
use self::state::types::browse::{
    restore_library_position_with_fetched_rows_for_kind, AlbumIndex, AlbumIndexState,
    AlbumPathPart, AlbumSearchEntry, BrowseLevel, SeriesDetail,
};
use self::state::types::confirm::{ConfirmAction, ConfirmModal};
#[cfg(test)]
use self::state::types::context_menu::LibraryRoutePopup;
use self::state::types::context_menu::{
    ContextAction, ContextMenuAnchor, ContextMenuEntry, LibraryRouteStage, MultiSelectKind,
};
use self::state::types::daemon_lost::DaemonLostModal;
use self::state::types::events::{LibEvent, SessionEvent};
use self::state::types::feed::{
    FeedHomeVideoGroup, FeedHomeVideoState, IdleFeed, SavePlaylistDialog, SavePlaylistStage,
};
use self::state::types::library_tab::LibraryTab;
use self::state::types::playback::{
    CastPlaybackTarget, DestinationLatestSource, LocalPlaybackTarget, PendingQueueAction,
    PlaybackState, PlaybackTarget, QueueScope, QueueScopeResolution, RemotePlaybackTarget,
    RemoteSlotState, ReplacementExecutor, RoutedReplacementPrep, SuspendedLocalSession, UndoEntry,
};
use self::state::types::player_tab::PlayerTab;
use self::state::types::settings::{PanelFocus, PanelMode, SettingKey};
pub(crate) use self::state::types::sidebar::SidebarId;
use self::state::types::tab_selection::TabSelection;
#[cfg(test)]
use mbv_core::api::EmbyItem;
#[cfg(test)]
use mbv_core::playback_queue::RemoveSlotResult;
#[cfg(test)]
use mbv_core::player::PlayerEvent;
#[cfg(test)]
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

include!("app_test_modules.rs");
