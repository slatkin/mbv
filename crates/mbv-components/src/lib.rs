//! `TuiRealm` interactive-component contracts: `ComponentId`, `Msg`, `UserEvent`
//! (design `migrate-tui-to-tuirealm` D3–D5).

pub mod book_content;
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

#[must_use]
pub fn selector_markers(len: usize, latest_marker: bool) -> Vec<bool> {
    let mut markers = vec![false; len];
    if let Some(first) = markers.first_mut() {
        *first = latest_marker;
    }
    markers
}

pub use self::confirm::ConfirmComponent;
pub use self::context_menu::ContextMenuComponent;
pub use self::daemon_lost::DaemonLostComponent;
pub use self::feeds_manage::FeedsManageComponent;
pub use self::help::HelpComponent;
pub use self::inline_search::SearchPool;
pub use self::library_playback_panel::{LibraryPlaybackPanel, PlaybackProjection};
pub use self::library_routes::LibraryRoutesComponent;
pub use self::mouse::{mouse_event_clause, mouse_sub};
pub use self::multiselect::MultiselectComponent;
#[cfg(test)]
pub use self::music_content::MusicContent;
pub use self::playlists::PlaylistsComponent;
pub use self::playlists::PlaylistsContent;
pub use self::queue::QueueComponent;
pub use self::queue::QueueCursorUpdate;
pub use self::queue_boundary::QueueBoundaryComponent;
pub use self::queue_playback_panel::QueuePlaybackPanel;
pub use self::root::UiRootComponent;
pub use self::save_playlist::SavePlaylistComponent;
pub use self::search_sidebar::SearchSidebarComponent;
pub use self::sessions::SessionsComponent;
pub use self::settings::{SettingsComponent, SettingsSnapshot};
pub use self::status_bar_panel::StatusBarPanel;
pub use self::tab_panel::TabPanel;

#[cfg(test)]
mod tests;
