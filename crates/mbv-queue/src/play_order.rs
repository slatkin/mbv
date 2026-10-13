//! Playback order of a Bound queue: the owner-side order with its pick, and
//! the order mode that crosses the ctrl wire.

/// The playback order mode a Client can request. The client never sends a
/// slot pick; the owner picks (design D5).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlayOrderMode {
    #[default]
    Sequential,
    Shuffle,
}

/// The playback order the Player owner holds for its Bound queue. In
/// `Shuffle` the order carries the owner's picked next slot, so a pick can
/// only exist while Shuffle is on (design D1). `upcoming` is `None` only when
/// the queue has no slot other than the active slot. A new Player owner
/// starts in Sequential.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PlayOrder {
    #[default]
    Sequential,
    Shuffle {
        upcoming: Option<crate::QueueSlotId>,
    },
}

impl PlayOrder {
    /// The mode this order is in, without the owner-side pick.
    #[must_use]
    pub fn mode(self) -> PlayOrderMode {
        match self {
            PlayOrder::Sequential => PlayOrderMode::Sequential,
            PlayOrder::Shuffle { .. } => PlayOrderMode::Shuffle,
        }
    }
}
