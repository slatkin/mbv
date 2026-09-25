//! Queue-owner identity for the Local queue (openspec change
//! `encode-local-queue-owner`): which process holds the authoritative Local
//! queue, and one fence value shaped by that owner.

use crate::app::dispatch::notify::ToastSeverity;
use crate::app::App;
use mbv_core::ctrl::QueueLineage;
use mbv_core::remote_player::DaemonEndpoint;

/// Which process holds the authoritative Local queue. Derived from
/// `player_endpoint`, never stored: an owner-kind change always goes through
/// an endpoint write, and every such write advances the epoch (design D1/D5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app) enum LocalQueueOwner {
    /// This process holds the authoritative Local queue (Bare mode and
    /// direct-remote playback both behave identically at every owner site).
    ThisProcess,
    /// A Stay-alive daemon process holds it; this Client projects its
    /// snapshots.
    StayAlive,
}

impl LocalQueueOwner {
    /// Whether this Client owns the Local queue's on-disk persistence and
    /// sequence-generation stamping — false under Stay-alive, where the
    /// owner daemon is authoritative for both.
    pub(in crate::app) fn owns_local_persistence(self) -> bool {
        match self {
            LocalQueueOwner::ThisProcess => true,
            LocalQueueOwner::StayAlive => false,
        }
    }
}

/// Client-local counter advanced on every Client-side Local-queue
/// replacement or clear. It fences async completions (playlist saves, owner
/// loads) against queue changes this Client itself made. It is never sent
/// over ctrl and is unrelated to the owner-minted [`QueueLineage`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub(in crate::app) struct QueueEpoch(pub(in crate::app) u64);

impl QueueEpoch {
    pub(in crate::app) fn advance(&mut self) {
        self.0 = self.0.saturating_add(1);
    }
}

/// Which queue a playlist mutation (or another fenced async request) was
/// made against, shaped by the owner at request time. The owner lineage can
/// only be present under Stay-alive, and must be present there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app) enum QueueOrigin {
    ThisProcess {
        epoch: QueueEpoch,
    },
    StayAlive {
        epoch: QueueEpoch,
        lineage: QueueLineage,
    },
}

impl QueueOrigin {
    pub(in crate::app) fn epoch(&self) -> QueueEpoch {
        match self {
            QueueOrigin::ThisProcess { epoch } | QueueOrigin::StayAlive { epoch, .. } => *epoch,
        }
    }
}

impl App {
    /// Which process currently holds the authoritative Local queue.
    pub(in crate::app) fn local_queue_owner(&self) -> LocalQueueOwner {
        match self.player_endpoint {
            Some(DaemonEndpoint::Local) => LocalQueueOwner::StayAlive,
            _ => LocalQueueOwner::ThisProcess,
        }
    }

    /// The origin to fence a request against, captured at request time.
    /// `None` only under Stay-alive, when the owner has not yet delivered a
    /// queue snapshot, so no owner lineage exists to carry.
    pub(in crate::app) fn queue_origin(&self) -> Option<QueueOrigin> {
        let epoch = self.queue_epoch;
        match self.local_queue_owner() {
            LocalQueueOwner::ThisProcess => Some(QueueOrigin::ThisProcess { epoch }),
            LocalQueueOwner::StayAlive => {
                let lineage = self
                    .player
                    .as_remote()
                    .and_then(|remote| remote.unified_queue_state())
                    .map(|state| state.lineage)?;
                Some(QueueOrigin::StayAlive { epoch, lineage })
            }
        }
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
