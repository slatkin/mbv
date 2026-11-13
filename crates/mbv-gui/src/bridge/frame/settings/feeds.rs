//! The Feeds section's content (`add-gui-settings-screen` design D9):
//! the subscriptions, addressed by URL, and whether an add is in flight.

use mbv::GuiSettingsFeedProjection;
use serde::Serialize;

/// One feed subscription (design D9).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiSettingsFeed {
    pub name: String,
    /// The subscription's only stable identity: the `feedEdit` and
    /// `feedRemove` commands address it.
    pub url: String,
    pub kind: mbv_queue::FeedKind,
}

impl From<&GuiSettingsFeedProjection> for GuiSettingsFeed {
    fn from(subscription: &GuiSettingsFeedProjection) -> Self {
        Self {
            name: subscription.name.clone(),
            url: subscription.url.clone(),
            kind: subscription.kind,
        }
    }
}
