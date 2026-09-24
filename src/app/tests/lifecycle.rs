use super::*;
use crate::app::components::feeds_content::{FeedsContent, FeedsOwnerPush};
use crate::app::components::home_content::HomeContent;
use crate::app::components::msg::{HomeRowTarget, Msg, ShellRequest};
use crate::app::components::{ComponentId, LibraryKey};
use crate::app::state::types::settings;
use crate::app::tests::tick_integration::harness::TickHarness;
use mbv_core::config::{FeedKind, FeedSubscription, ServiceKind};
use mbv_core::playback_queue::FeedEntry;
use rstest::rstest;
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};

#[path = "lifecycle_launch_migration.rs"]
mod lifecycle_launch_migration;
#[path = "lifecycle_launch_restore.rs"]
mod lifecycle_launch_restore;
#[path = "lifecycle_render_transport.rs"]
mod lifecycle_render_transport;
#[path = "lifecycle_teardown.rs"]
mod lifecycle_teardown;
