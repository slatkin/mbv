//! The at-most-one pending queue transfer and its tick-phase settlement
//! (queue-handoff-and-take design D2). The Client copies one Player owner's
//! queue to the other, waits for the target to report playback of the start
//! slot, and only then stops the source — a failed or lost target never
//! stops the source. Each direction's flow arms its own file:
//! `hand_off` (design D1 left column) and `take` (right column).

mod hand_off;
mod take;

use std::sync::mpsc;

use crate::app::dispatch::notify::ToastSeverity;
use crate::app::dispatch::queue::QUEUE_OP_ANSWER_BOUND;
use crate::app::state::panel_targets::{PanelTarget, resolve_session_target};
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
pub(in crate::app) struct PendingQueueTransfer {
    pub direction: QueueTransferDirection,
    /// The F3 target the transfer was started for (stable identity, D6).
    pub target: SessionTargetKey,
    pub start_slot: QueueSlotId,
    pub op: QueueTransferFollowUp,
    /// The peer link opened to read an unconnected Take's source queue
    /// (design D1 unconnected Take). It never became `self.player`, so the
    /// settlement's source stop and its close go through it. `None` when the
    /// source is (or was) the current attachment.
    pub source_link: Option<StagedQueueTransferLink>,
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

fn transfer_failure_message(direction: QueueTransferDirection, cause: TransferFailure) -> String {
    match (direction, cause) {
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
            QueueTransferDirection::Take => self.begin_queue_take(target),
        }
    }

    /// Resolves an F3 Hand off or Take target (design D6): a live Emby row
    /// that advertises a direct Player-owner endpoint other than the local
    /// Owner. Returns the session row, its endpoint, and its display name.
    fn resolve_queue_transfer_target(
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

    /// Opens the peer link an unconnected transfer reads its source queue
    /// through (design D1): Hand off attaches it after confirmation; Take
    /// keeps it as the settlement's source stop. `None` after a failed
    /// connect, which is reported here.
    fn open_queue_transfer_probe_link(
        &mut self,
        action: &str,
        endpoint: &mbv_remote_player::DaemonEndpoint,
        name: &str,
        device: &str,
    ) -> Option<StagedQueueTransferLink> {
        let (remote, remote_rx) = match Self::connect_direct_endpoint(endpoint) {
            Ok(connect) => connect,
            Err(error) => {
                tracing::warn!(
                    name: "queue_transfer.probe_connect_failed",
                    target: "queue_transfer",
                    action = %action,
                    device = %device,
                    endpoint = %endpoint,
                    error = %error,
                    "the transfer probe connect failed"
                );
                self.flash(
                    format!("{action}: cannot reach {name}"),
                    ToastSeverity::Warning,
                );
                return None;
            }
        };
        Some(StagedQueueTransferLink { remote, remote_rx })
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
            QueueTransferDirection::Take => self.run_staged_queue_take(staged),
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

    /// Cancel the pending transfer without a report (design D2 cancel): a
    /// disconnect leaves both owners as they are, and the source keeps
    /// playing. A newer F3 action or a disconnect also replaces a staged
    /// transfer prompt, dropping any link opened for it.
    pub(in crate::app) fn cancel_pending_queue_transfer(&mut self) {
        if let Some(transfer) = self.pending_queue_transfer.take() {
            Self::close_transfer_source_link(transfer.source_link);
        }
        self.drop_staged_queue_transfer();
    }

    /// Report a failed transfer and drop it (design D2 failure): the source
    /// keeps playing and nothing is sent to either owner here. The
    /// replacement's rejection travels through this same report path.
    pub(in crate::app) fn fail_pending_queue_transfer(&mut self, message: String) {
        if let Some(transfer) = self.pending_queue_transfer.take() {
            Self::close_transfer_source_link(transfer.source_link);
        }
        self.flash(message, ToastSeverity::Error);
    }

    /// Close the source link an unconnected Take carried (design D1 step 7):
    /// an explicit disconnect lets the connection's reader thread exit
    /// instead of leaking (#233). Free of `self`: the link is closed where
    /// its owner settles it.
    fn close_transfer_source_link(link: Option<StagedQueueTransferLink>) {
        if let Some(link) = link {
            link.remote.disconnect();
        }
    }

    /// Tick-phase settlement (design D2): the run loop calls this right after
    /// the drain step that adopts owner snapshots, so the target's
    /// post-replacement state is what gets judged. The value survives until
    /// the target reports playback of `start_slot` — playback observation has
    /// no own timeout, because the user can cancel and the source still
    /// plays. Returns whether the settlement acted (a report or a stop), so
    /// the caller can count the tick as having events.
    pub(in crate::app) fn settle_pending_queue_transfer(&mut self) -> bool {
        let Some(transfer) = self.pending_queue_transfer.take() else {
            return false;
        };
        let PendingQueueTransfer {
            direction,
            target,
            start_slot,
            op,
            source_link,
        } = transfer;
        if self.transfer_target_link_lost(direction) {
            Self::close_transfer_source_link(source_link);
            let message = transfer_failure_message(direction, TransferFailure::LinkLost);
            self.fail_pending_queue_transfer(message);
            return true;
        }
        self.settle_target_observation(direction, target, start_slot, op, source_link)
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
    fn settle_target_observation(
        &mut self,
        direction: QueueTransferDirection,
        target: SessionTargetKey,
        start_slot: QueueSlotId,
        op: QueueTransferFollowUp,
        source_link: Option<StagedQueueTransferLink>,
    ) -> bool {
        let target_playing = self.transfer_target_playing(direction);
        let (target_active, target_pending, start_slot_present) = {
            let view = self.queue_for_link(direction.target_link());
            (
                view.active_slot_id() == Some(start_slot),
                view.pending_playback_slot() == Some(start_slot),
                view.slot(start_slot).is_some(),
            )
        };
        if target_active && target_playing {
            tracing::info!(
                name: "queue_transfer.settled",
                target: "queue_transfer",
                {
                    direction = ?direction,
                    session = ?target,
                    start_slot = ?start_slot,
                    follow_up = ?op
                },
                "target reports the start slot playing"
            );
            self.stop_transfer_source(direction, source_link.as_ref());
            self.run_queue_transfer_follow_up(op, &target);
            Self::close_transfer_source_link(source_link);
            self.pending_queue_transfer = None;
            return true;
        }
        if target_pending || start_slot_present {
            // The target is still starting the start slot, or has not yet
            // reported: keep waiting (design D2 — no observation timeout).
            self.pending_queue_transfer = Some(PendingQueueTransfer {
                direction,
                target,
                start_slot,
                op,
                source_link,
            });
            return false;
        }
        tracing::info!(
            name: "queue_transfer.start_slot_dropped",
            target: "queue_transfer",
            { direction = ?direction, session = ?target, start_slot = ?start_slot },
            "target no longer holds the start slot"
        );
        self.stop_transfer_target(direction);
        Self::close_transfer_source_link(source_link);
        let message = transfer_failure_message(direction, TransferFailure::StartSlotDropped);
        self.fail_pending_queue_transfer(message);
        true
    }

    /// Send `Stop` to the transfer's source owner (design D2 success, step 6).
    /// Best-effort like every other fire-and-forget player send. A Hand off
    /// source is the local Owner link: the suspended home link while a peer
    /// connection holds the player, the current player while it is the local
    /// daemon. When no local Owner link exists — the home link died
    /// mid-transfer while the peer is attached — there is nothing to stop,
    /// and falling through to `self.player` would stop the handed-off
    /// playback on the peer instead.
    fn stop_transfer_source(
        &mut self,
        direction: QueueTransferDirection,
        source_link: Option<&StagedQueueTransferLink>,
    ) {
        match direction {
            QueueTransferDirection::HandOff => {
                let source = match (&self.suspended_local, self.is_local_daemon()) {
                    (Some(home), _) => &home.player,
                    (None, true) => &self.player,
                    (None, false) => {
                        tracing::warn!(
                            name: "queue_transfer.source_link_missing",
                            target: "queue_transfer",
                            "the local Owner link is gone; skipping the source stop"
                        );
                        return;
                    }
                };
                source.stop();
            }
            QueueTransferDirection::Take => {
                if let Some(link) = source_link {
                    // The unconnected Take's probe link is the source (design
                    // D1): it never became `self.player`, so the stop goes
                    // through it. A dead probe link is nothing to stop — the
                    // peer's playback is already gone.
                    if link.remote.is_disconnected() {
                        tracing::warn!(
                            name: "queue_transfer.source_link_missing",
                            target: "queue_transfer",
                            "the peer probe link is gone; skipping the source stop"
                        );
                        return;
                    }
                    // Review P1 (row 8.1): the settlement closes this probe
                    // link right after, and a `Stop` still queued on the
                    // writer thread loses that race — wait for the owner's
                    // correlated reply, the delivery proof, before any
                    // socket teardown.
                    link.remote.stop_and_await_ack(QUEUE_OP_ANSWER_BOUND);
                } else {
                    // The connected Take's source is the current peer
                    // attachment, whose socket the follow-up's disconnect
                    // tears down: the stop must be delivered first (review
                    // P1). The Hand off arm above never closes its link, so
                    // it needs no proof. When the attachment no longer holds
                    // the source — the peer link dropped mid-transfer and
                    // the teardown reinstalled the home link, or the link is
                    // dead and its teardown is still pending — `queue_link`
                    // falls through to `self.player`, which is then the
                    // local Owner the Take just started; a gone source is
                    // nothing to stop (PR #973 review).
                    if !self.can_disconnect_remote() || self.player.is_remote_disconnected() {
                        tracing::warn!(
                            name: "queue_transfer.source_link_missing",
                            target: "queue_transfer",
                            "the peer attachment is gone; skipping the source stop"
                        );
                        return;
                    }
                    let (proxy, _rx) = self.queue_link(direction.source_link());
                    proxy.stop_and_await_ack(QUEUE_OP_ANSWER_BOUND);
                }
            }
        }
    }

    /// Send `Stop` to the transfer's target owner after admission removed the
    /// start slot (design D2 failure): the target started a different item.
    /// Only reached past the link-lost guard, so the link owns this queue.
    fn stop_transfer_target(&mut self, direction: QueueTransferDirection) {
        let (proxy, _rx) = self.queue_link(direction.target_link());
        proxy.stop();
    }

    /// The settled transfer's step-7 tail (design D1), run by the
    /// settlement's success block after the source stops. A Hand off stays
    /// connected to the peer. A Take closes the F3 connection when the
    /// transfer ran over one — never touching a library route (ADR 0012);
    /// an unconnected Take has no F3 connection to close, and its probe
    /// link is closed by the settlement itself.
    fn run_queue_transfer_follow_up(
        &mut self,
        op: QueueTransferFollowUp,
        target: &SessionTargetKey,
    ) {
        match op {
            QueueTransferFollowUp::StayConnected => tracing::debug!(
                name: "queue_transfer.hand_off.stayed_connected",
                target: "queue_transfer",
                session = ?target,
                "Hand off settled: the Client stays attached to the peer"
            ),
            QueueTransferFollowUp::ClosePeerLink => {
                if self.can_disconnect_remote() {
                    self.disconnect_remote();
                } else {
                    tracing::debug!(
                        name: "queue_transfer.take.no_connection_to_close",
                        target: "queue_transfer",
                        session = ?target,
                        "Take settled: the Client has no F3 connection to close"
                    );
                }
            }
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
