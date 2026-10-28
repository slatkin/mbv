//! The Take flow (queue-handoff-and-take design D1 right column): the
//! Client copies the peer queue and its position to the local Owner, starts
//! the copy here, and stops the peer only once the local Owner reports the
//! taken slot playing. It works with or without a connection, and the
//! success settlement closes the F3 connection — never a library route
//! (ADR 0012).

use crate::app::dispatch::notify::ToastSeverity;
use crate::app::dispatch::queue::QueueOpEdit;
use crate::app::state::queue_transfer::{
    PendingQueueTransfer, QueueTransferDirection, QueueTransferFollowUp, StagedQueueTransfer,
    StagedQueueTransferLink,
};
use crate::app::{App, QueueLink};
use mbv_queue::QueueSlotId;
use mbv_ui_model::confirm::{ConfirmAction, ConfirmModal};
use mbv_ui_model::panel_targets::SessionTargetKey;

/// The Take source's copy inputs read from one link (design D1 step 3): the
/// peer queue's slots, source, active slot, and status.
struct QueueTakeSource {
    slots: Vec<mbv_ctrl::UnifiedQueueSlot>,
    source: mbv_queue::QueueSource,
    active_slot: Option<QueueSlotId>,
    status: mbv_ctrl::player::PlayerStatus,
}

impl QueueTakeSource {
    /// The start slot, its queue index, and the peer playback position in
    /// seconds (design D1 step 3) — `None` when the peer has no audible
    /// active slot (the stale-F3-row case). `active` alone only marks a
    /// loaded item; the paused flag decides audible playback.
    fn start(&self) -> Option<(QueueSlotId, usize, f64)> {
        if !self.status.active || self.status.paused {
            return None;
        }
        let start_slot = self.active_slot?;
        let start_idx = self
            .slots
            .iter()
            .position(|slot| slot.slot_id == mbv_ctrl::slot_id_to_u64(start_slot))?;
        Some((
            start_slot,
            start_idx,
            mbv_emby_model::ticks_to_seconds(self.status.position_ticks),
        ))
    }
}

impl App {
    /// The stale-F3-row report (queue-handoff spec, "Hand off and Take need
    /// playback on the source"): the peer snapshot has no audible active
    /// slot, so nothing is sent and nothing changes.
    fn flash_nothing_plays_on_the_peer(&mut self) {
        self.flash(
            "Take: nothing is playing on the peer".to_string(),
            ToastSeverity::Warning,
        );
    }

    /// Take (design D1 right column): ask before replacing a populated local
    /// Owner queue, read the peer's queue and position through its link —
    /// the current attachment when connected, a probe link otherwise — and
    /// copy them onto the local Owner. The tick-phase settlement then waits
    /// for the local Owner to observe the start slot before the peer stops.
    /// Take never touches a library route (ADR 0012): it severs nothing and
    /// the flow's only connection change is the follow-up's F3 disconnect.
    /// The local Owner link the Take's copy must reach: the suspended home
    /// link behind the attachment, or the current player when it is the
    /// local daemon. None means the home link is already gone — the copy
    /// would reach the peer attachment instead of the local Owner.
    fn queue_take_target_link_present(&self) -> bool {
        self.suspended_local.is_some() || self.is_local_daemon()
    }

    pub(super) fn begin_queue_take(&mut self, target: &SessionTargetKey) {
        let Some((sess, endpoint, peer_name)) = self.resolve_queue_transfer_target(target) else {
            return;
        };
        // The target gate: the copy must reach a live local Owner link
        // (review P2, row 8.1). Behind an attachment whose home link died
        // there is none — refuse before anything is connected or sent.
        if !self.queue_take_target_link_present() {
            self.flash(
                "Take failed: the local Owner connection was lost".to_string(),
                ToastSeverity::Error,
            );
            return;
        }
        let connected = self.queue_transfer_peer_link_is_current(&sess.id);
        let staged_link = if connected {
            None
        } else {
            let Some(link) = self.open_queue_transfer_probe_link(
                "Take",
                &endpoint,
                &peer_name,
                &sess.device_name,
            ) else {
                return;
            };
            Some(link)
        };
        // Take needs playback on the source (queue-handoff spec): a stale F3
        // row reports and sends nothing.
        if !self.queue_take_source_is_playing(staged_link.as_ref()) {
            Self::drop_staged_probe_link(staged_link);
            self.flash_nothing_plays_on_the_peer();
            return;
        }
        let staged = StagedQueueTransfer {
            direction: QueueTransferDirection::Take,
            target: target.clone(),
            sess,
            endpoint,
            link: staged_link,
        };
        // The confirm requirement: ask before replacing a populated local
        // Owner queue, naming the target — here the local Owner itself. An
        // empty local queue runs without a prompt.
        self.staged_queue_transfer = Some(staged);
        if self.local_view.total_queue_len() > 0 {
            self.ask_confirm(ConfirmModal::two_button(
                format!("Replace the queue on {}?", mbv_emby::device_name()),
                "Take",
                "Cancel",
                ConfirmAction::ConfirmQueueTransfer(target.clone()),
            ));
            return;
        }
        self.run_staged_queue_transfer();
    }

    /// Run the staged Take past its confirm gate (design D1 steps 2-3): the
    /// unconnected shape's probe link is never attached — the Client stays
    /// unconnected — and is carried into the pending transfer, where the
    /// settlement's source stop reaches the probe.
    pub(super) fn run_staged_queue_take(&mut self, staged: StagedQueueTransfer) {
        let StagedQueueTransfer { target, link, .. } = staged;
        let source_link = match link {
            Some(link) if link.remote.is_disconnected() => {
                // The link opened for the transfer died while the prompt was
                // up: report and drop it, so nothing is sent. The loss is
                // the peer's, so the message names the peer (review P2,
                // row 8.1) — the settlement's link-lost failure names the
                // local Owner, its own lost link.
                self.flash(
                    "Take failed: the peer connection was lost".to_string(),
                    ToastSeverity::Error,
                );
                Self::drop_staged_probe_link(Some(link));
                return;
            }
            other => other,
        };
        self.send_queue_take_copy(&target, source_link);
    }

    /// Take steps 3-4 (design D1): read the peer snapshot and the peer status
    /// position, then replace the local Owner queue with the copy — the
    /// start slot at the peer playback position (design D4). The submitted
    /// slot ids are the peer queue's own (design D8), so the settlement's
    /// `start_slot` survives into the local Owner's snapshots.
    fn send_queue_take_copy(
        &mut self,
        target: &SessionTargetKey,
        source_link: Option<StagedQueueTransferLink>,
    ) {
        // The target gate, re-checked at the copy (review P2, row 8.1): the
        // home link can die while the confirm prompt is up, and the copy
        // would then reach the peer attachment instead of the local Owner.
        if !self.queue_take_target_link_present() {
            Self::drop_staged_probe_link(source_link);
            self.flash(
                "Take failed: the local Owner connection was lost".to_string(),
                ToastSeverity::Error,
            );
            return;
        }
        let Some(read) = self.queue_take_source_read(source_link.as_ref()) else {
            // A stale F3 row (design D2): the Client sends nothing, changes
            // nothing, and reports that nothing plays on the source.
            Self::drop_staged_probe_link(source_link);
            self.flash_nothing_plays_on_the_peer();
            return;
        };
        let Some((start_slot, start_idx, position_secs)) = read.start() else {
            Self::drop_staged_probe_link(source_link);
            self.flash_nothing_plays_on_the_peer();
            return;
        };
        tracing::info!(
            name: "queue_transfer.take.sending",
            target: "queue_transfer",
            session = ?target,
            start_slot = ?start_slot,
            start_idx,
            position_secs,
            "sending the taken queue copy to the local Owner"
        );
        let sent = self.queue_op(
            QueueLink::Local,
            mbv_remote_player::QueueOp::Replace {
                items: read.slots,
                start_idx: Some(start_idx),
                source: read.source,
                start_position_secs: Some(position_secs),
            },
        );
        if sent == QueueOpEdit::NotApplied {
            // The answered-edit bound reported the rejection or the timeout
            // (design D2 failure): the pending transfer fails and the source
            // keeps playing.
            Self::drop_staged_probe_link(source_link);
            self.fail_pending_queue_transfer(
                "Take failed: the local Owner did not take the queue copy".to_string(),
            );
            return;
        }
        // Design D2: the pending transfer arms after the copy is sent; the
        // tick-phase settlement waits for the local Owner to observe the
        // start slot before the peer stops.
        self.pending_queue_transfer = Some(PendingQueueTransfer {
            direction: QueueTransferDirection::Take,
            target: target.clone(),
            start_slot,
            op: QueueTransferFollowUp::for_direction(QueueTransferDirection::Take),
            source_link,
        });
    }

    /// The Take source's read (design D1 step 3): the peer queue's slots,
    /// source, active slot, and status — from the current peer attachment
    /// when connected, from the probe link otherwise. `None` when the peer
    /// holds no readable queue.
    fn queue_take_source_read(
        &self,
        source_link: Option<&StagedQueueTransferLink>,
    ) -> Option<QueueTakeSource> {
        if let Some(link) = source_link {
            let snapshot = link.remote.unified_queue_state()?;
            Some(QueueTakeSource {
                slots: snapshot.slots,
                source: snapshot.source,
                active_slot: snapshot.active_slot.map(QueueSlotId::from_raw),
                status: link.remote.status_snapshot(),
            })
        } else {
            if !self.has_remote_queue() {
                return None;
            }
            let view = self.queue_for_link(QueueLink::Peer);
            Some(QueueTakeSource {
                slots: view
                    .slots()
                    .iter()
                    .map(|slot| mbv_ctrl::UnifiedQueueSlot {
                        slot_id: mbv_ctrl::slot_id_to_u64(slot.slot_id),
                        item: slot.item.clone(),
                    })
                    .collect(),
                source: view.source().clone(),
                active_slot: view.active_slot_id(),
                status: self.player.status_snapshot(),
            })
        }
    }

    /// Whether the Take source currently plays (design D6's source gate,
    /// re-checked at staging and at the copy): the peer has a readable queue
    /// and audible playback of an active slot.
    fn queue_take_source_is_playing(&self, source_link: Option<&StagedQueueTransferLink>) -> bool {
        self.queue_take_source_read(source_link)
            .and_then(|read| read.start())
            .is_some()
    }
}
