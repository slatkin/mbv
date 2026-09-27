use super::*;
use crate::app::components::feeds_content::{FeedsContent, FeedsOwnerPush};
use crate::app::components::home_content::HomeContent;
use mbv_config::FeedSubscription;
use mbv_queue::FeedEntry;
use mbv_queue::{FeedKind, ServiceKind};
use mbv_ui_model::settings;
use rstest::rstest;

mod lifecycle_launch_migration;
mod lifecycle_launch_restore;
mod lifecycle_render_transport;
mod lifecycle_teardown;
