//! Queue-owner identity for the Local queue: the owner process holds the
//! authoritative Local queue, and the Client fences async completions
//! against the owner's lineage.

use crate::app::App;
use crate::app::dispatch::notify::ToastSeverity;

impl App {
    /// The origin to fence a request against, captured at request time.
    /// `None` when the owner has not yet delivered a queue snapshot, so no
    /// owner lineage exists to carry. The lineage is read from the Local
    /// queue's owner link (the suspended home link when present), matching
    /// `queue_link`'s Local resolution.
    pub(in crate::app) fn queue_origin(&self) -> Option<QueueOrigin> {
        let lineage = self
            .local_queue_player()
            .remote()
            .unified_queue_state()
            .map(|state| state.lineage)?;
        Some(QueueOrigin {
            epoch: self.queue_epoch,
            lineage,
        })
    }

    /// Like `queue_origin`, but flashes the "not available yet" error when
    /// there is none, for call sites that can't proceed without one.
    pub(in crate::app) fn queue_origin_or_flash(&mut self) -> Option<QueueOrigin> {
        let origin = self.queue_origin();
        if origin.is_none() {
            self.flash(
                "Stay-alive queue not available yet".to_string(),
                ToastSeverity::Error,
            );
        }
        origin
    }

    /// Whether `origin` was captured against the current epoch. The owner
    /// lineage is deliberately not compared: the owner is the final judge of
    /// its own lineage, and the Client's last snapshot can lag a replacement
    /// the Client itself just sent (design D3).
    pub(in crate::app) fn origin_is_current(&self, origin: QueueOrigin) -> bool {
        origin.epoch() == self.queue_epoch
    }
}

// Queue identity types shared by shell-owned async operations.
use mbv_queue::QueueLineage;

/// Client-local counter advanced on every Client-side Local-queue
/// replacement or clear. It fences async completions (playlist saves, owner
/// loads) against queue changes this Client itself made. It is never sent
/// over ctrl and is unrelated to the owner-minted [`QueueLineage`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub(in crate::app) struct QueueEpoch(pub u64);

impl QueueEpoch {
    pub fn advance(&mut self) {
        self.0 = self.0.saturating_add(1);
    }
}

/// Which queue a playlist mutation (or another fenced async request) was
/// made against: the Client-local epoch plus the owner lineage it was
/// shaped by. The owner lineage must be present: the owner is authoritative
/// for the queue, and this Client only projects its snapshots.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app) struct QueueOrigin {
    pub(in crate::app) epoch: QueueEpoch,
    pub(in crate::app) lineage: QueueLineage,
}

impl QueueOrigin {
    #[must_use]
    pub fn epoch(&self) -> QueueEpoch {
        self.epoch
    }
}
