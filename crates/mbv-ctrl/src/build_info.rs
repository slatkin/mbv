//! The three identity values one binary reports in its hello (design D2).

use crate::{BUILD_ID, wire_id};

/// Application version, build identity, and wire identity of one binary.
///
/// The build identity is for display only. The local attach gate compares the
/// application version and the wire identity, through [`Self::differs_for_gate`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuildInfo {
    pub app_version: String,
    pub build_id: String,
    pub wire_id: String,
}

impl BuildInfo {
    /// The identity of this binary.
    #[must_use]
    pub fn current() -> Self {
        Self {
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            build_id: BUILD_ID.to_string(),
            wire_id: wire_id().to_string(),
        }
    }

    /// Whether a local Client must refuse an Owner reporting `other`. Compares
    /// the application version and the wire identity only. An empty wire
    /// identity (an Owner that predates the field) always differs.
    #[must_use]
    pub fn differs_for_gate(&self, other: &Self) -> bool {
        self.app_version != other.app_version || self.wire_id != other.wire_id
    }
}
