use mbv_queue::QueueLineage;

/// Client-local counter advanced on every Client-side Local-queue
/// replacement or clear. It fences async completions (playlist saves, owner
/// loads) against queue changes this Client itself made. It is never sent
/// over ctrl and is unrelated to the owner-minted [`QueueLineage`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct QueueEpoch(pub u64);

impl QueueEpoch {
    pub fn advance(&mut self) {
        self.0 = self.0.saturating_add(1);
    }
}

/// Which queue a playlist mutation (or another fenced async request) was
/// made against, shaped by the owner at request time. The owner lineage can
/// only be present under Stay-alive, and must be present there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueueOrigin {
    ThisProcess {
        epoch: QueueEpoch,
    },
    StayAlive {
        epoch: QueueEpoch,
        lineage: QueueLineage,
    },
}

impl QueueOrigin {
    #[must_use]
    pub fn epoch(&self) -> QueueEpoch {
        match self {
            QueueOrigin::ThisProcess { epoch } | QueueOrigin::StayAlive { epoch, .. } => *epoch,
        }
    }
}
