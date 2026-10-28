//! The Hand off flow (queue-handoff-and-take design D1 left column): the
//! Client copies the local Owner queue and its position to the peer, starts
//! the copy there, stops local playback only once the peer reports the
//! handed-off slot playing, and stays connected.

use crate::app::dispatch::notify::ToastSeverity;
use crate::app::dispatch::queue::QueueOpEdit;
use crate::app::state::queue_transfer::{
    QueueTransferDirection, QueueTransferFollowUp, StagedQueueTransfer, TransferFailure,
    transfer_failure_message,
};
use crate::app::{App, QueueLink};
use mbv_queue::QueueSlotId;
use mbv_ui_model::confirm::{ConfirmAction, ConfirmModal};
use mbv_ui_model::panel_targets::SessionTargetKey;

impl App {
    /// Hand off (design D1 left column): ask before replacing a populated
    /// peer queue, attach the peer when its link is not the current
    /// attachment, copy the local queue with its position, and arm the
    /// pending transfer. The tick-phase settlement then waits for the peer
    /// to observe the start slot before local playback stops.
    pub(super) fn begin_queue_hand_off(&mut self, target: &SessionTargetKey) {
        let Some((sess, endpoint, name)) = self.resolve_queue_transfer_target(target) else {
            return;
        };
        let connected = self.queue_transfer_peer_link_is_current(&sess.id);
        let staged_link = if connected {
            None
        } else {
            let Some(link) = self.open_queue_transfer_probe_link(
                "Hand off",
                &endpoint,
                &name,
                &sess.device_name,
            ) else {
                return;
            };
            Some(link)
        };
        let (supports_start_position, target_audio_only, peer_queue_len) = if connected {
            let remote = self.player.remote();
            (
                remote.supports_queue_replace_start_position(),
                remote.supports_audio_only(),
                self.queue_for_link(QueueLink::Peer).total_queue_len(),
            )
        } else {
            let link = staged_link
                .as_ref()
                .map(|link| &link.remote)
                .expect("probe link staged");
            let snapshot = link.unified_queue_state();
            (
                link.supports_queue_replace_start_position(),
                link.supports_audio_only(),
                snapshot.map_or(0, |state| state.slots.len()),
            )
        };
        // The capability gate (queue-handoff spec, "Hand off and Take need a
        // peer that accepts a start position"): refuse before anything is
        // asked or sent.
        if !supports_start_position {
            Self::drop_staged_probe_link(staged_link);
            self.flash(
                format!("Hand off: {name} does not support Hand off"),
                ToastSeverity::Warning,
            );
            return;
        }
        // Hand off needs playback on the source (queue-handoff spec): a
        // stale F3 row reports and sends nothing.
        if !self.queue_hand_off_source_is_playing() {
            Self::drop_staged_probe_link(staged_link);
            self.flash(
                "Hand off: nothing is playing on the local Owner".to_string(),
                ToastSeverity::Warning,
            );
            return;
        }
        // Design D3 admission pre-check: an audio-only target cannot hold a
        // non-audio active item, so the copy is refused before it is sent.
        if target_audio_only && !self.queue_hand_off_active_item_is_audio() {
            Self::drop_staged_probe_link(staged_link);
            self.flash(
                format!("Hand off: {name} cannot play the active item"),
                ToastSeverity::Warning,
            );
            return;
        }
        let staged = StagedQueueTransfer {
            direction: QueueTransferDirection::HandOff,
            target: target.clone(),
            sess,
            endpoint,
            link: staged_link,
        };
        // The confirm requirement: ask before replacing a populated target
        // queue, naming the target. An empty target queue runs without a
        // prompt.
        self.staged_queue_transfer = Some(staged);
        if peer_queue_len > 0 {
            self.ask_confirm(ConfirmModal::two_button(
                format!("Replace the queue on {name}?"),
                "Hand off",
                "Cancel",
                ConfirmAction::ConfirmQueueTransfer(target.clone()),
            ));
            return;
        }
        self.run_staged_queue_transfer();
    }

    /// Run the staged Hand off past its confirm gate (design D1 steps 2-4):
    /// attach the peer when a link was opened for the transfer, then copy the
    /// local queue onto it.
    pub(super) fn run_staged_queue_hand_off(&mut self, staged: StagedQueueTransfer) {
        let StagedQueueTransfer {
            sess,
            endpoint,
            link,
            ..
        } = staged;
        if let Some(link) = link {
            if link.remote.is_disconnected() {
                // The link opened for the transfer died while the prompt was
                // up: report and drop it, so nothing is sent.
                self.flash(
                    transfer_failure_message(
                        QueueTransferDirection::HandOff,
                        TransferFailure::LinkLost,
                    ),
                    ToastSeverity::Error,
                );
                Self::drop_staged_probe_link(Some(link));
                return;
            }
            // Design D1 step 2: sever the current connection and attach the
            // peer, as by Connect — the Hand off ends connected (step 7).
            // The sever keeps the Owner output alone (design D2): the local
            // Owner may be the transfer source playing through a Cast or
            // Emby Session output, and its Stop must wait for the
            // settlement's observation, not ride the attach.
            self.sever_connections_without_output_change();
            self.switch_to_direct_remote(&sess, link.remote, link.remote_rx, &endpoint);
        }
        self.send_queue_hand_off_copy(&staged.target);
    }

    /// Hand off steps 3-4 (design D1): read the local snapshot and the
    /// status position, then replace the peer queue with the copy — the
    /// start slot at the local playback position (design D4). The submitted
    /// slot ids are the local queue's own (design D8), so the settlement's
    /// `start_slot` survives into the peer's snapshots.
    fn send_queue_hand_off_copy(&mut self, target: &SessionTargetKey) {
        // The target gate, re-checked at the copy (PR #973 review): the
        // connected shape's peer link can die while the confirm prompt is
        // up, and `queue_link(Peer)` would then fall through to
        // `self.player` — the local Owner the teardown reinstalled. The
        // copy must not replace the source queue it has to leave playing,
        // the same gate the Take's copy applies to its target link.
        if !self.queue_hand_off_target_link_present() {
            self.flash(
                transfer_failure_message(
                    QueueTransferDirection::HandOff,
                    TransferFailure::LinkLost,
                ),
                ToastSeverity::Error,
            );
            return;
        }
        let Some((start_slot, start_idx, position_secs)) = self.queue_hand_off_source_start()
        else {
            // A stale F3 row (design D2): the Client sends nothing, changes
            // nothing, and reports that nothing plays on the source.
            self.flash(
                "Hand off: nothing is playing on the local Owner".to_string(),
                ToastSeverity::Warning,
            );
            return;
        };
        let slots: Vec<mbv_ctrl::UnifiedQueueSlot> = self
            .local_view
            .slots()
            .iter()
            .map(|slot| mbv_ctrl::UnifiedQueueSlot {
                slot_id: mbv_ctrl::slot_id_to_u64(slot.slot_id),
                item: slot.item.clone(),
            })
            .collect();
        let source = self.local_view.source().clone();
        tracing::info!(
            name: "queue_transfer.hand_off.sending",
            target: "queue_transfer",
            session = ?target,
            start_slot = ?start_slot,
            start_idx,
            position_secs,
            slots = slots.len(),
            "sending the handed-off queue copy to the peer"
        );
        let sent = self.queue_op(
            QueueLink::Peer,
            mbv_remote_player::QueueOp::Replace {
                items: slots,
                start_idx: Some(start_idx),
                source,
                start_position_secs: Some(position_secs),
            },
        );
        if sent == QueueOpEdit::NotApplied {
            // The answered-edit bound reported the rejection or the timeout
            // (design D2 failure): the pending transfer fails and the source
            // keeps playing.
            self.fail_pending_queue_transfer(
                "Hand off failed: the peer did not take the queue copy".to_string(),
            );
            return;
        }
        // Design D2: the pending transfer arms after the copy is sent; the
        // tick-phase settlement waits for the peer to observe the start slot
        // before local playback stops.
        self.pending_queue_transfer =
            Some(crate::app::state::queue_transfer::PendingQueueTransfer {
                direction: QueueTransferDirection::HandOff,
                target: target.clone(),
                start_slot,
                op: QueueTransferFollowUp::for_direction(QueueTransferDirection::HandOff),
                source_link: None,
            });
    }

    /// The Hand off source's copy inputs (design D1 step 3): the local
    /// active slot, its queue index, and the local playback position in
    /// seconds.
    fn queue_hand_off_source_start(&self) -> Option<(QueueSlotId, usize, f64)> {
        let status = self.queue_hand_off_source_status()?;
        if !status.active || status.paused {
            return None;
        }
        let start_slot = self.local_view.active_slot_id()?;
        let start_idx = self.local_view.slot_index(start_slot)?;
        Some((
            start_slot,
            start_idx,
            mbv_emby_model::ticks_to_seconds(status.position_ticks),
        ))
    }

    /// Whether the Hand off source currently plays (design D6's source gate,
    /// re-checked at the copy): the local Owner has a link and audible
    /// playback of an active slot.
    fn queue_hand_off_source_is_playing(&self) -> bool {
        self.queue_hand_off_source_start().is_some()
    }

    /// The peer link the Hand off's copy must reach (design D1): the
    /// current attachment holding a peer queue. Absent once the
    /// connection's teardown has reinstalled the local link — the copy
    /// would then reach the local Owner instead of the peer (PR #973
    /// review).
    fn queue_hand_off_target_link_present(&self) -> bool {
        self.has_remote_queue() && !self.player.is_remote_disconnected()
    }

    /// Whether the local active item is audio (design D3's pre-check input).
    fn queue_hand_off_active_item_is_audio(&self) -> bool {
        self.local_view.active_slot_id().is_some_and(|slot_id| {
            self.local_view
                .slot(slot_id)
                .is_some_and(|slot| slot.item.is_audio())
        })
    }

    /// The local Owner link's status (the Hand off source): the suspended
    /// home link while a peer connection holds the player, the current
    /// player while it is the local daemon. `None` when no local Owner link
    /// exists — a peer connection without a suspended home link means the
    /// home link is already gone.
    fn queue_hand_off_source_status(&self) -> Option<mbv_ctrl::player::PlayerStatus> {
        match &self.suspended_local {
            Some(home) => Some(home.player.status_snapshot()),
            None if self.is_local_daemon() => Some(self.player.status_snapshot()),
            None => None,
        }
    }
}
