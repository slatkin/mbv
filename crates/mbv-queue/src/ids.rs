//! Identities the queue hands out: the stable slot identity a Bound queue
//! addresses, and the Named queue identity the Owner mints and persists.

// serde derives so the owner-assigned slot identity can travel on
// `PlayerEvent` / `PlayerCommand` across the ctrl seam; a newtype over `u64`
// serializes as its inner value.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
pub struct QueueSlotId(u64);

impl QueueSlotId {
    #[must_use]
    pub fn raw(self) -> u64 {
        self.0
    }

    #[must_use]
    pub fn from_raw(raw: u64) -> Self {
        Self(raw)
    }
}

/// The identity of one Named queue, minted by the Owner and persisted with it.
/// It is stable across renames and restarts and is never reused within one
/// state file (design D3 of `gui-named-queues`).
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
pub struct NamedQueueId(u64);

impl NamedQueueId {
    #[must_use]
    pub fn raw(self) -> u64 {
        self.0
    }

    #[must_use]
    pub fn from_raw(raw: u64) -> Self {
        Self(raw)
    }
}
