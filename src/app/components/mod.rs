//! TuiRealm interactive-component contracts: `ComponentId`, `Msg`, `UserEvent`
//! (design `migrate-tui-to-tuirealm` D3–D5).
//!
//! Pre-wiring scaffolding: the enums and their payload types are declared here
//! so the TuiRealm `Application<ComponentId, Msg, UserEvent>` can be assembled
//! in task 1.4, but nothing references them yet. Per-item dead code is expected
//! and allowed here until the Model wires the types; revisit once 1.4 lands.

#![allow(dead_code)]

pub mod component_id;
pub mod confirm;
pub mod context_menu;
pub mod daemon_lost;
pub mod feeds;
pub mod help;
pub mod home;
pub mod legacy_input;
pub mod msg;
pub mod playback_gates;
pub mod remote_reanchor;
pub mod search_sidebar;
pub mod sessions;
pub mod user_event;

pub use self::component_id::{ComponentId, ModalId, OverlayId};
pub use self::confirm::ConfirmComponent;
pub use self::context_menu::ContextMenuComponent;
pub use self::daemon_lost::DaemonLostComponent;
pub use self::feeds::FeedsComponent;
pub use self::help::HelpComponent;
pub use self::home::HomeComponent;
pub use self::legacy_input::LegacyInput;
pub use self::msg::{LegacyTerminalEvent, Msg, ServiceRequest, ShellRequest};
pub use self::playback_gates::{
    PlaybackGatesComponent, ATTR_NEXT_UP_PROMPT_VISIBLE, ATTR_SKIP_INTRO_PROMPT_VISIBLE,
};
pub use self::remote_reanchor::RemoteReanchorComponent;
pub use self::search_sidebar::SearchSidebarComponent;
pub use self::sessions::SessionsComponent;
pub use self::user_event::UserEvent;

#[cfg(test)]
#[path = "feeds_component_tests.rs"]
mod feeds_component_tests;
#[cfg(test)]
#[path = "home_component_tests.rs"]
mod home_component_tests;
