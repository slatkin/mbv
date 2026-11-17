//! Owner-local queue revisions: the stamp that tells a Client whether its copy
//! of the queue is current, and the source that mints them.

use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct QueueRevision(u64);

/// An owner-local source of unique queue revisions, retained across queue replacements.
#[derive(Debug, Default)]
pub struct QueueRevisionMint(AtomicU64);

impl QueueRevisionMint {
    /// Mint a revision unique within this owner.
    #[must_use]
    pub fn mint(&self) -> QueueRevision {
        QueueRevision(self.0.fetch_add(1, Ordering::Relaxed))
    }
}

impl QueueRevision {
    #[must_use]
    pub fn raw(self) -> u64 {
        self.0
    }

    /// Mint a new owner-local revision.
    pub(crate) fn bump(&mut self, mint: &QueueRevisionMint) {
        *self = mint.mint();
    }
}
