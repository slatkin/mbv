mod action;
mod actions;
mod actions_navigation;
mod app_audiobookshelf_service_completion;
mod app_emby_service_completion;
mod app_struct;
mod audio_subtitle_actions;
mod audiobookshelf_browse_actions;
mod audiobookshelf_service_actions;
mod bootstrap;
mod browse_level_actions;
mod cast_actions;
mod cast_status_actions;
pub mod components;
mod construct;
mod consume_quit_actions;
mod context_menu_actions;
mod context_menu_capabilities;
mod cw_library_tab_actions;
mod daemon_restart;
mod emby_service_actions;
mod feed_actions;
mod feed_tab_actions;
mod feeds_manage_actions;
mod home_actions;
mod home_latest;
pub(crate) use self::home_latest::{capture_launch_window, current_launch_secs};
mod dispatch;
mod infra;
mod input;
mod input_browse_dispatch;
mod input_confirm_keys;
mod input_lib_keys;
mod input_playlist_keys;
mod input_queue_keys;
mod input_resolver;
mod input_search_sidebar_keys;
mod key_policy;
mod lib_cursor_actions;
mod lib_event_actions;
mod lib_event_actions_reconcile;
mod library_browse_actions;
mod library_load_actions;
mod library_position_state;
mod library_route;
mod library_search_actions;
mod list_pane_width;
mod mouse_gestures;
mod music_actions;
mod music_artist_detail;
mod music_grouping;
mod notify_actions;
mod panel_focus_state;
mod panel_targets;
mod playback_target;
mod playback_target_cast;
mod playback_target_local;
mod playback_target_remote;
mod player_event;
mod queue_actions;
mod queue_column_width;
mod queue_scope;
mod remote_slot_state;
pub mod render;
mod router;
mod run_loop_drains;
mod run_loop_events;
mod search_sidebar;
mod service_startup;
mod services_settings;
mod session_command_actions;
mod session_connect;
mod session_switch;
mod shell_draw;
mod shuffle_folder_actions;
mod state;
mod ws_event_actions;

pub use self::app_struct::App;
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
pub use self::shell::Model;
mod app_init;
use self::app_init::AppInit;
use self::bootstrap::bootstrap_unified_queue;
use self::infra::resize::spawn_resize_worker;
use self::notify_actions::ToastSeverity;
pub(in crate::app) use self::playback_target::NowPlayingStatus;
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
