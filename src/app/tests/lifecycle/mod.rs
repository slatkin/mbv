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

mod lifecycle_launch_migration;
mod lifecycle_launch_restore;
mod lifecycle_render_transport;
mod lifecycle_teardown;
