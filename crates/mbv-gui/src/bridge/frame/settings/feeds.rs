//! The Feeds section's frame content (`add-gui-settings-screen` design D9):
//! the subscriptions, addressed by URL, and whether an add is in flight.

use mbv::GuiSettingsFeedProjection;
use serde::{Deserialize, Serialize};

/// One feed subscription (design D9).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiSettingsFeed {
    pub name: String,
    /// The subscription's only stable identity: the `feedEdit` and
    /// `feedRemove` commands address it.
    pub url: String,
    pub kind: GuiFeedKind,
}

impl From<&GuiSettingsFeedProjection> for GuiSettingsFeed {
    fn from(subscription: &GuiSettingsFeedProjection) -> Self {
        Self {
            name: subscription.name.clone(),
            url: subscription.url.clone(),
            kind: GuiFeedKind::from(subscription.kind),
        }
    }
}

/// A subscription's default media kind (design D9), mirroring
/// `mbv_queue::FeedKind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GuiFeedKind {
    Audio,
    Video,
}

impl From<mbv_queue::FeedKind> for GuiFeedKind {
    fn from(kind: mbv_queue::FeedKind) -> Self {
        match kind {
            mbv_queue::FeedKind::Audio => Self::Audio,
            mbv_queue::FeedKind::Video => Self::Video,
        }
    }
}

impl From<GuiFeedKind> for mbv_queue::FeedKind {
    fn from(kind: GuiFeedKind) -> Self {
        match kind {
            GuiFeedKind::Audio => Self::Audio,
            GuiFeedKind::Video => Self::Video,
        }
    }
}
