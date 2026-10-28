//! The at-most-one pending queue transfer and its tick-phase settlement
//! (queue-handoff-and-take design D2). The Client copies one Player owner's
//! queue to the other, waits for the target to report playback of the start
//! slot, and only then stops the source — a failed or lost target never
//! stops the source.

use crate::app::dispatch::notify::ToastSeverity;
use crate::app::{App, QueueLink};
use mbv_queue::QueueSlotId;
use mbv_ui_model::panel_targets::SessionTargetKey;

/// Which Player owner a queue transfer copies from and which it copies to
/// (design D1): Hand off moves the local Owner's queue to the peer; Take
/// moves the peer's queue to the local Owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app) enum QueueTransferDirection {
    HandOff,
    Take,
}

impl QueueTransferDirection {
    /// The link whose owner receives the copy and must report playback.
    pub(in crate::app) fn target_link(self) -> QueueLink {
        match self {
            Self::HandOff => QueueLink::Peer,
            Self::Take => QueueLink::Local,
        }
    }

    /// The link whose owner keeps playing until the target reports playback.
    pub(in crate::app) fn source_link(self) -> QueueLink {
        match self {
            Self::HandOff => QueueLink::Local,
            Self::Take => QueueLink::Peer,
        }
    }
}

/// The step-7 follow-up a settled transfer runs after it stops the source
/// (design D1/D2). The settlement stays direction-agnostic by dispatching on
/// this value; rows 7.2 (Hand off) and 8.1 (Take) fill their own tail.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app) enum QueueTransferFollowUp {
    /// Hand off step 7: the Client stays connected to the peer.
    StayConnected,
    /// Take step 7: the Client closes the peer link; the queue panel shows
    /// the local Owner queue.
    ClosePeerLink,
}

impl QueueTransferFollowUp {
    /// The tail each direction's flow arms (design D1 step 7): a Hand off
    /// stays connected to the peer, a Take closes the peer link.
    pub(in crate::app) fn for_direction(direction: QueueTransferDirection) -> Self {
        match direction {
            QueueTransferDirection::HandOff => Self::StayConnected,
            QueueTransferDirection::Take => Self::ClosePeerLink,
        }
    }
}

/// The at-most-one in-flight queue transfer (design D2). Armed after the
/// replacement is sent to the target; settled in the tick phase that adopts
/// owner snapshots. `start_slot` is the submitted slot id the target must
/// observe as its active, playing slot — the target keeps submitted slot ids
/// (design D8), so the id survives into its snapshots.
#[derive(Clone, Debug)]
pub(in crate::app) struct PendingQueueTransfer {
    pub direction: QueueTransferDirection,
    /// The F3 target the transfer was started for (stable identity, D6).
    pub target: SessionTargetKey,
    pub start_slot: QueueSlotId,
    pub op: QueueTransferFollowUp,
}

/// Why a pending transfer failed (design D2 failure causes). A replacement
/// the target rejected is reported by the answered send that carried it (the
/// existing answered-edit bound) through `fail_pending_queue_transfer`; the
/// settlement owns the failures only a later observation can show.
enum TransferFailure {
    LinkLost,
    StartSlotDropped,
}

impl PendingQueueTransfer {
    fn failure_message(&self, cause: TransferFailure) -> String {
        match (self.direction, cause) {
            (QueueTransferDirection::HandOff, TransferFailure::LinkLost) => {
                "Hand off failed: the peer connection was lost".to_string()
            }
            (QueueTransferDirection::Take, TransferFailure::LinkLost) => {
                "Take failed: the local Owner connection was lost".to_string()
            }
            (QueueTransferDirection::HandOff, TransferFailure::StartSlotDropped) => {
                "Hand off failed: the peer could not start the handed-off item".to_string()
            }
            (QueueTransferDirection::Take, TransferFailure::StartSlotDropped) => {
                "Take failed: the local Owner could not start the taken item".to_string()
            }
        }
    }
}

impl App {
    /// Entry point of an F3 Hand off or Take action (design D6): the shell
    /// dispatch resolved the key against the current snapshot, so the target
    /// is live. A newer F3 action replaces the pending value (design D2
    /// cancel): the older transfer is dropped without a report and the source
    /// keeps playing.
    ///
    /// Row 7.2 fills the Hand off flow here and row 8.1 the Take flow
    /// (design D1): the confirm gate, the copy send over the target link, and
    /// the arming of `PendingQueueTransfer { direction, target, start_slot,
    /// op }` once the replacement is sent.
    pub(in crate::app) fn begin_queue_transfer(
        &mut self,
        direction: QueueTransferDirection,
        target: &SessionTargetKey,
    ) {
        self.cancel_pending_queue_transfer();
        tracing::debug!(
            name: "queue_transfer.action_replaces_pending",
            target: "queue_transfer",
            {
                direction = ?direction,
                session = ?target,
                follow_up = ?QueueTransferFollowUp::for_direction(direction)
            },
            "F3 transfer action replaces any pending transfer"
        );
    }

    /// Cancel the pending transfer without a report (design D2 cancel): a
    /// disconnect leaves both owners as they are, and the source keeps
    /// playing.
    pub(in crate::app) fn cancel_pending_queue_transfer(&mut self) {
        self.pending_queue_transfer = None;
    }

    /// Report a failed transfer and drop it (design D2 failure): the source
    /// keeps playing and nothing is sent to either owner here. The
    /// replacement's rejection travels through this same report path.
    pub(in crate::app) fn fail_pending_queue_transfer(&mut self, message: String) {
        self.pending_queue_transfer = None;
        self.flash(message, ToastSeverity::Error);
    }

    /// Tick-phase settlement (design D2): the run loop calls this right after
    /// the drain step that adopts owner snapshots, so the target's
    /// post-replacement state is what gets judged. The value survives until
    /// the target reports playback of `start_slot` — playback observation has
    /// no own timeout, because the user can cancel and the source still
    /// plays. Returns whether the settlement acted (a report or a stop), so
    /// the caller can count the tick as having events.
    pub(in crate::app) fn settle_pending_queue_transfer(&mut self) -> bool {
        let Some(transfer) = self.pending_queue_transfer.clone() else {
            return false;
        };
        if self.transfer_target_link_lost(transfer.direction) {
            let message = transfer.failure_message(TransferFailure::LinkLost);
            self.fail_pending_queue_transfer(message);
            return true;
        }
        self.settle_target_observation(&transfer)
    }

    /// Whether the transfer's target link is gone before the target reported
    /// playback (design D2 failure). A peer target is lost when its queue
    /// view is gone or its connection reports a drop the drain has not torn
    /// down yet. The local target is lost when its own link dropped — the
    /// suspended home link, or the active link while no peer connection holds
    /// it; a peer connection without a suspended home session means the home
    /// link behind it is already gone.
    fn transfer_target_link_lost(&self, direction: QueueTransferDirection) -> bool {
        match direction {
            QueueTransferDirection::HandOff => {
                !self.has_remote_queue() || self.player.is_remote_disconnected()
            }
            QueueTransferDirection::Take => match &self.suspended_local {
                Some(suspended) => suspended.player.is_remote_disconnected(),
                None => self.has_remote_queue() || self.player.is_remote_disconnected(),
            },
        }
    }

    /// Whether the target owner currently plays (design D2 success gate):
    /// `active` alone only marks a loaded item; the paused flag decides
    /// audible playback, as `local_owner_plays` does.
    fn transfer_target_playing(&self, direction: QueueTransferDirection) -> bool {
        let status = match direction {
            QueueTransferDirection::HandOff => self.player.status_snapshot(),
            QueueTransferDirection::Take => match &self.suspended_local {
                Some(suspended) => suspended.player.status_snapshot(),
                None => self.player.status_snapshot(),
            },
        };
        status.active && !status.paused
    }

    /// Judge the target's adopted queue state (design D2). Success names the
    /// start slot as the observed active slot with audible playback; a start
    /// slot still queued or still transitioning keeps the value armed; a
    /// start slot the target no longer holds was dropped by admission, so
    /// the target started a different item and is stopped too.
    fn settle_target_observation(&mut self, transfer: &PendingQueueTransfer) -> bool {
        let target_playing = self.transfer_target_playing(transfer.direction);
        let (target_active, target_pending, start_slot_present) = {
            let view = self.queue_for_link(transfer.direction.target_link());
            (
                view.active_slot_id() == Some(transfer.start_slot),
                view.pending_playback_slot() == Some(transfer.start_slot),
                view.slot(transfer.start_slot).is_some(),
            )
        };
        if target_active && target_playing {
            tracing::info!(
                name: "queue_transfer.settled",
                target: "queue_transfer",
                {
                    direction = ?transfer.direction,
                    session = ?transfer.target,
                    start_slot = ?transfer.start_slot,
                    // The flow's step-7 tail (design D1): row 7.2 dispatches
                    // the Hand off tail here and row 8.1 the Take tail.
                    follow_up = ?transfer.op
                },
                "target reports the start slot playing"
            );
            self.stop_transfer_source(transfer.direction);
            self.pending_queue_transfer = None;
            return true;
        }
        if target_pending || start_slot_present {
            // The target is still starting the start slot, or has not yet
            // reported: keep waiting (design D2 — no observation timeout).
            return false;
        }
        tracing::info!(
            name: "queue_transfer.start_slot_dropped",
            target: "queue_transfer",
            { direction = ?transfer.direction, session = ?transfer.target, start_slot = ?transfer.start_slot },
            "target no longer holds the start slot"
        );
        self.stop_transfer_target(transfer.direction);
        let message = transfer.failure_message(TransferFailure::StartSlotDropped);
        self.fail_pending_queue_transfer(message);
        true
    }

    /// Send `Stop` to the transfer's source owner (design D2 success, step 6).
    /// Best-effort like every other fire-and-forget player send.
    fn stop_transfer_source(&mut self, direction: QueueTransferDirection) {
        let (proxy, _rx) = self.queue_link(direction.source_link());
        proxy.stop();
    }

    /// Send `Stop` to the transfer's target owner after admission removed the
    /// start slot (design D2 failure): the target started a different item.
    /// Only reached past the link-lost guard, so the link owns this queue.
    fn stop_transfer_target(&mut self, direction: QueueTransferDirection) {
        let (proxy, _rx) = self.queue_link(direction.target_link());
        proxy.stop();
    }
}
