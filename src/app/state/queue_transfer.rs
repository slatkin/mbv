//! The at-most-one pending queue transfer and its tick-phase settlement
//! (queue-handoff-and-take design D2). The Client copies one Player owner's
//! queue to the other, waits for the target to report playback of the start
//! slot, and only then stops the source — a failed or lost target never
//! stops the source.

use std::sync::mpsc;

use crate::app::dispatch::notify::ToastSeverity;
use crate::app::dispatch::queue::QueueOpEdit;
use crate::app::state::panel_targets::{PanelTarget, resolve_session_target};
use crate::app::{App, QueueLink};
use mbv_queue::QueueSlotId;
use mbv_ui_model::confirm::{ConfirmAction, ConfirmModal};
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

/// The link a transfer opened ahead of its confirmation (design D1): read
/// with it, attached on confirm, dropped on cancel. Owned like every other
/// connection the App holds, so the prompt's answer decides its fate.
pub(in crate::app) struct StagedQueueTransferLink {
    pub remote: mbv_remote_player::RemotePlayer,
    pub remote_rx: mpsc::Receiver<mbv_ctrl::player::PlayerEvent>,
}

/// The transfer staged for its confirmation prompt (design D1): the resolved
/// target plus, when the target link was not the current attachment, the
/// link opened to read the target's queue. The connected case stages without
/// a link, so one confirm arm runs both shapes.
pub(in crate::app) struct StagedQueueTransfer {
    pub direction: QueueTransferDirection,
    /// The F3 target the transfer was started for (stable identity, D6).
    pub target: SessionTargetKey,
    /// The target's Emby Session row as the F3 snapshot named it: the attach
    /// path reads the device name and session id from it.
    pub sess: mbv_emby::SessionInfo,
    pub endpoint: mbv_remote_player::DaemonEndpoint,
    /// The link opened to read the target's queue before the prompt (design
    /// D1 unconnected case). `None` when the target link was already the
    /// current attachment.
    pub link: Option<StagedQueueTransferLink>,
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
        match direction {
            QueueTransferDirection::HandOff => self.begin_queue_hand_off(target),
            // Row 8.1 fills the Take flow (design D1 right column); the
            // cancel above already replaced any pending transfer.
            QueueTransferDirection::Take => {}
        }
    }

    /// Resolves an F3 Hand off target (design D6): a live Emby row that
    /// advertises a direct Player-owner endpoint other than the local Owner.
    /// Returns the session row, its endpoint, and its display name.
    fn resolve_queue_hand_off_target(
        &self,
        target: &SessionTargetKey,
    ) -> Option<(
        mbv_emby::SessionInfo,
        mbv_remote_player::DaemonEndpoint,
        String,
    )> {
        let Some(PanelTarget::Emby(sess)) = resolve_session_target(&self.panel_targets, target)
        else {
            tracing::debug!(
                name: "queue_transfer.target_unresolved",
                target: "queue_transfer",
                session = ?target,
                "a Hand off target must be a live F3 session row"
            );
            return None;
        };
        let sess = *sess;
        if !self.session_has_direct_endpoint(&sess) {
            tracing::debug!(
                name: "queue_transfer.target_without_direct_endpoint",
                target: "queue_transfer",
                device = %sess.device_name,
                "a Hand off target must advertise a direct Player-owner endpoint"
            );
            return None;
        }
        let endpoint = self.session_direct_endpoint(&sess)?;
        let name = queue_transfer_target_name(&sess);
        Some((sess, endpoint, name))
    }

    /// Opens the peer link the unconnected Hand off reads its queue through
    /// (design D1): the link is attached only after confirmation and dropped
    /// when the user cancels. `None` after a failed connect, which is
    /// reported here.
    fn open_queue_hand_off_probe_link(
        &mut self,
        endpoint: &mbv_remote_player::DaemonEndpoint,
        name: &str,
        device: &str,
    ) -> Option<StagedQueueTransferLink> {
        let (remote, remote_rx) = match Self::connect_direct_endpoint(endpoint) {
            Ok(connect) => connect,
            Err(error) => {
                tracing::warn!(
                    name: "queue_transfer.hand_off.connect_failed",
                    target: "queue_transfer",
                    device = %device,
                    endpoint = %endpoint,
                    error = %error,
                    "the Hand off probe connect failed"
                );
                self.flash(
                    format!("Hand off: cannot reach {name}"),
                    ToastSeverity::Warning,
                );
                return None;
            }
        };
        Some(StagedQueueTransferLink { remote, remote_rx })
    }

    /// Hand off (design D1 left column): ask before replacing a populated
    /// peer queue, attach the peer when its link is not the current
    /// attachment, copy the local queue with its position, and arm the
    /// pending transfer. The tick-phase settlement then waits for the peer
    /// to observe the start slot before local playback stops.
    fn begin_queue_hand_off(&mut self, target: &SessionTargetKey) {
        let Some((sess, endpoint, name)) = self.resolve_queue_hand_off_target(target) else {
            return;
        };
        let connected = self.queue_transfer_peer_link_is_current(&sess.id);
        let staged_link = if connected {
            None
        } else {
            let Some(link) =
                self.open_queue_hand_off_probe_link(&endpoint, &name, &sess.device_name)
            else {
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

    /// The transfer prompt's confirm answer (design D1): run the staged
    /// transfer. Every other key cancels it: an opened link is dropped and
    /// nothing is sent (the confirm requirement).
    pub(in crate::app) fn run_staged_queue_transfer(&mut self) {
        let Some(staged) = self.staged_queue_transfer.take() else {
            return;
        };
        match staged.direction {
            QueueTransferDirection::HandOff => self.run_staged_queue_hand_off(staged),
            // Row 8.1 fills the Take confirm (design D1 right column); no
            // Take transfer is staged before it is implemented.
            QueueTransferDirection::Take => {}
        }
    }

    /// The transfer prompt's cancel answer (design D1): drop the staged
    /// transfer and any link opened to read the target queue. Nothing is
    /// sent to either Player owner.
    pub(in crate::app) fn cancel_staged_queue_transfer(&mut self) {
        self.drop_staged_queue_transfer();
    }

    /// Drop the staged transfer, closing a link opened for it (#233: an
    /// explicit disconnect lets the connection's reader thread exit instead
    /// of leaking).
    fn drop_staged_queue_transfer(&mut self) {
        if let Some(link) = self
            .staged_queue_transfer
            .take()
            .and_then(|staged| staged.link)
        {
            link.remote.disconnect();
        }
    }

    /// Drop a probe link the pre-checks refused, before any staging happened.
    /// Free of `self` on purpose: the link is dropped where it was refused.
    fn drop_staged_probe_link(link: Option<StagedQueueTransferLink>) {
        if let Some(link) = link {
            link.remote.disconnect();
        }
    }

    /// Run the staged Hand off past its confirm gate (design D1 steps 2-4):
    /// attach the peer when a link was opened for the transfer, then copy the
    /// local queue onto it.
    fn run_staged_queue_hand_off(&mut self, staged: StagedQueueTransfer) {
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
                    "Hand off failed: the peer connection was lost".to_string(),
                    ToastSeverity::Error,
                );
                return;
            }
            // Design D1 step 2: sever the current connection and attach the
            // peer, as by Connect — the Hand off ends connected (step 7).
            self.sever_active_connection();
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
        self.pending_queue_transfer = Some(PendingQueueTransfer {
            direction: QueueTransferDirection::HandOff,
            target: target.clone(),
            start_slot,
            op: QueueTransferFollowUp::for_direction(QueueTransferDirection::HandOff),
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

    /// Cancel the pending transfer without a report (design D2 cancel): a
    /// disconnect leaves both owners as they are, and the source keeps
    /// playing. A newer F3 action or a disconnect also replaces a staged
    /// transfer prompt, dropping any link opened for it.
    pub(in crate::app) fn cancel_pending_queue_transfer(&mut self) {
        self.pending_queue_transfer = None;
        self.drop_staged_queue_transfer();
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
                    follow_up = ?transfer.op
                },
                "target reports the start slot playing"
            );
            self.stop_transfer_source(transfer.direction);
            Self::run_queue_transfer_follow_up(transfer);
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

    /// The settled transfer's step-7 tail (design D1), run by the
    /// settlement's success block after the source stops. Free of `self` for
    /// now: the Hand off tail only logs — row 8.1 moves it back onto `App`
    /// when the Take tail needs the shell state to close the peer link.
    fn run_queue_transfer_follow_up(transfer: &PendingQueueTransfer) {
        match transfer.op {
            QueueTransferFollowUp::StayConnected => tracing::debug!(
                name: "queue_transfer.hand_off.stayed_connected",
                target: "queue_transfer",
                session = ?transfer.target,
                "Hand off settled: the Client stays attached to the peer"
            ),
            // Row 8.1 (Take) closes the peer link here.
            QueueTransferFollowUp::ClosePeerLink => {}
        }
    }
}

/// The target's display name for transfer prompts and messages: the Emby
/// device name when the row carries one, else its device id — the same
/// fallback `activate_session_target` uses.
fn queue_transfer_target_name(sess: &mbv_emby::SessionInfo) -> String {
    let name = sess.device_name.trim();
    if name.is_empty() {
        sess.device_id.clone()
    } else {
        name.to_string()
    }
}
