//! TuiRealm interactive-component contracts: `ComponentId`, `Msg`, `UserEvent`
//! (design `migrate-tui-to-tuirealm` D3–D5).
//!
//! Pre-wiring scaffolding: the enums and their payload types are declared here
//! so the TuiRealm `Application<ComponentId, Msg, UserEvent>` can be assembled
//! in task 1.4, but nothing references them yet. Per-item dead code is expected
//! and allowed here until the Model wires the types; revisit once 1.4 lands.

#![allow(dead_code)]

pub mod audiobookshelf_book;
pub mod audiobookshelf_podcast;
pub mod browser;
pub mod component_id;
pub mod confirm;
pub mod context_menu;
pub mod daemon_lost;
pub mod feeds;
pub mod feeds_manage;
pub mod help;
pub mod home;
pub mod inline_search;
pub mod legacy_input;
pub mod library_routes;
pub mod msg;
pub mod multiselect;
pub mod playback;
pub mod playback_gates;
pub mod playback_prompt;
pub mod playlists;
pub mod queue;
pub mod remote_reanchor;
pub mod save_playlist;
pub mod search_sidebar;
pub mod selection_modal;
pub mod sessions;
pub mod settings;
pub mod tv_workspace;
pub mod user_event;

pub use self::audiobookshelf_book::AudiobookshelfBookComponent;
pub use self::audiobookshelf_podcast::AudiobookshelfPodcastComponent;
pub use self::browser::BrowserComponent;
pub use self::component_id::{BrowserKey, BrowserKind, ComponentId, ModalId, OverlayId, PopupId};
pub use self::confirm::ConfirmComponent;
pub use self::context_menu::ContextMenuComponent;
pub use self::daemon_lost::DaemonLostComponent;
pub use self::feeds::FeedsComponent;
pub use self::feeds_manage::FeedsManageComponent;
pub use self::help::HelpComponent;
pub use self::home::HomeComponent;
pub(in crate::app) use self::inline_search::{InlineSearchComponent, SearchPool};
pub use self::legacy_input::LegacyInput;
pub use self::library_routes::LibraryRoutesComponent;
pub use self::msg::{
    LegacyTerminalEvent, Msg, PersistRequest, PlaybackRequest, QueueMove, QueueRequest,
    ServiceRequest, ShellRequest,
};
pub use self::multiselect::MultiselectComponent;
pub(in crate::app) use self::playback::{PlaybackComponent, PlaybackProjection};
pub use self::playback_gates::{ATTR_NEXT_UP_PROMPT_VISIBLE, ATTR_SKIP_INTRO_PROMPT_VISIBLE};
pub use self::playback_prompt::PlaybackPromptComponent;
pub use self::playlists::PlaylistsComponent;
pub use self::queue::QueueComponent;
pub use self::remote_reanchor::RemoteReanchorComponent;
pub use self::save_playlist::SavePlaylistComponent;
pub use self::search_sidebar::SearchSidebarComponent;
pub use self::selection_modal::SelectionModalComponent;
pub use self::sessions::SessionsComponent;
pub(in crate::app) use self::settings::{
    ServiceRow, SettingsComponent, SettingsRow, SettingsSnapshot, SetupDraft,
};
pub use self::tv_workspace::TvWorkspaceComponent;
pub use self::user_event::UserEvent;

#[cfg(test)]
#[path = "audiobookshelf_book_component_tests.rs"]
mod audiobookshelf_book_component_tests;
#[cfg(test)]
#[path = "audiobookshelf_podcast_component_tests.rs"]
mod audiobookshelf_podcast_component_tests;
#[cfg(test)]
#[path = "browser_component_tests.rs"]
mod browser_component_tests;
#[cfg(test)]
#[path = "feeds_component_tests.rs"]
mod feeds_component_tests;
#[cfg(test)]
#[path = "home_component_tests.rs"]
mod home_component_tests;
#[cfg(test)]
#[path = "playback_prompt_component_tests.rs"]
mod playback_prompt_component_tests;
#[cfg(test)]
#[path = "playlists_component_tests.rs"]
mod playlists_component_tests;
#[cfg(test)]
#[path = "queue_component_tests.rs"]
mod queue_component_tests;
#[cfg(test)]
#[path = "save_playlist_component_tests.rs"]
mod save_playlist_component_tests;
#[cfg(test)]
#[path = "selection_modal_component_tests.rs"]
mod selection_modal_component_tests;
