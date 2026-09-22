//! TuiRealm interactive-component contracts: `ComponentId`, `Msg`, `UserEvent`
//! (design `migrate-tui-to-tuirealm` D3–D5).

pub mod book_content;
pub mod component_id;
pub mod confirm;
pub mod context_menu;
pub mod daemon_lost;
pub mod emby_library_content;
pub mod feeds_content;
pub mod feeds_manage;
pub mod help;
pub mod home_content;
pub mod inline_search;
pub mod library_panel;
pub mod library_playback_panel;
pub mod library_routes;
pub mod list;
pub mod media_list;
pub mod mouse;
pub mod msg;
pub mod multiselect;
pub mod music_content;
pub mod music_tree_target;
pub mod playlists;
pub mod podcast_content;
pub mod queue;
pub mod queue_boundary;
pub mod queue_playback_panel;
pub mod root;
pub mod save_playlist;
pub mod search_sidebar;
pub mod sessions;
pub mod settings;
pub mod status_bar_panel;
pub mod tab_panel;
pub mod tv_content;
pub mod user_event;

pub use self::component_id::{ComponentId, ModalId, OverlayId, PopupId};
pub use self::confirm::ConfirmComponent;
pub use self::context_menu::ContextMenuComponent;
pub use self::daemon_lost::DaemonLostComponent;
pub use self::feeds_manage::FeedsManageComponent;
pub use self::help::HelpComponent;
pub(in crate::app) use self::inline_search::SearchPool;
pub use self::library_panel::{LibraryKey, LibraryKind};
pub(in crate::app) use self::library_playback_panel::{LibraryPlaybackPanel, PlaybackProjection};
pub use self::library_routes::LibraryRoutesComponent;
pub use self::mouse::{mouse_event_clause, mouse_sub};
pub use self::msg::{
    Msg, PlaybackRequest, QueueColumnResize, QueueIntent, QueueMove, QueueRequest, ServiceRequest,
    SettingsIntent, ShellRequest, TerminalObserverEvent,
};
pub use self::multiselect::MultiselectComponent;
#[cfg(test)]
pub(in crate::app) use self::music_content::MusicContent;
pub use self::playlists::PlaylistsComponent;
pub(in crate::app) use self::playlists::PlaylistsContent;
pub use self::queue::QueueComponent;
pub(in crate::app) use self::queue::QueueCursorUpdate;
pub use self::queue_boundary::QueueBoundaryComponent;
pub use self::queue_playback_panel::QueuePlaybackPanel;
pub(in crate::app) use self::root::UiRootComponent;
pub use self::save_playlist::SavePlaylistComponent;
pub use self::search_sidebar::SearchSidebarComponent;
pub use self::sessions::SessionsComponent;
pub(in crate::app) use self::settings::{
    ServiceRow, SettingsComponent, SettingsRow, SettingsSnapshot, SetupDraft,
};
pub use self::status_bar_panel::StatusBarPanel;
pub use self::tab_panel::TabPanel;
pub use self::user_event::UserEvent;

#[cfg(test)]
#[path = "emby_library_inline_search_tests.rs"]
mod emby_library_inline_search_tests;
#[cfg(test)]
#[path = "feeds_component_tests.rs"]
mod feeds_component_tests;
#[cfg(test)]
#[path = "feeds_manage_component_tests.rs"]
mod feeds_manage_component_tests;
#[cfg(test)]
#[path = "library_routes_component_tests.rs"]
mod library_routes_component_tests;
#[cfg(test)]
#[path = "multiselect_component_tests.rs"]
mod multiselect_component_tests;
#[cfg(test)]
#[path = "playlists_component_tests.rs"]
mod playlists_component_tests;
#[cfg(test)]
#[path = "queue_boundary_component_tests.rs"]
mod queue_boundary_component_tests;
#[cfg(test)]
#[path = "queue_component_tests.rs"]
mod queue_component_tests;
#[cfg(test)]
#[path = "queue_drag_component_tests.rs"]
mod queue_drag_component_tests;
#[cfg(test)]
#[path = "save_playlist_component_tests.rs"]
mod save_playlist_component_tests;
#[cfg(test)]
#[path = "search_sidebar_component_tests.rs"]
mod search_sidebar_component_tests;
#[cfg(test)]
#[path = "tv_content_component_tests.rs"]
mod tv_content_component_tests;
