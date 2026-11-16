//! The Named-queue view one ctrl connection holds (gui-named-queues design D3):
//! the `NamedQueueShow` subscription, the resting snapshot it is sent on show
//! and after each change of that queue, and the two events that end it — the
//! queue closing, and the queue becoming current (it is then read through the
//! current-queue snapshots).
//!
//! The subscription lives on the connection (`CtrlClients`), so it ends with the
//! connection and never outlives it.

use super::{
    CtrlClientId, CtrlContext, CtrlEvent, NamedQueueId, NamedQueueTarget, QueueTransport,
    RestingNamedQueue, UnifiedQueueStateData, current_snapshot, send_to, serialize_ctrl_event,
    target, unified_queue_state_for_peer,
};

/// The reason a show names a queue the Owner no longer holds.
const UNKNOWN_SHOW: &str = "named queue not found; show skipped";

/// `CtrlCmd::NamedQueueShow`: subscribe one connection to a Named queue's
/// snapshots (design D3). The queue's whole state is sent at once and again
/// after each change of it, and a connection shows one Named queue at a time,
/// so a second show replaces the first. Naming the current queue ends the
/// resting view: the connection drops its subscription and receives the
/// current queue's own snapshot, which is how that queue is read.
pub(crate) fn handle_show(ctx: &mut CtrlContext<'_>, queue: NamedQueueId) {
    let Some(target) = target(ctx, Some(queue), UNKNOWN_SHOW) else {
        return;
    };
    match target {
        NamedQueueTarget::Current => {
            ctx.ctrl_clients
                .lock()
                .unwrap()
                .show_named_queue(ctx.client_id, None);
            if let Some((_, data)) = current_snapshot(ctx) {
                send_to(ctx.reply_tx, &CtrlEvent::UnifiedQueueState(data));
            }
        }
        NamedQueueTarget::Resting(index) => {
            ctx.ctrl_clients
                .lock()
                .unwrap()
                .show_named_queue(ctx.client_id, Some(queue));
            if let Some((id, data)) = resting_snapshot(ctx, index) {
                send_to(
                    ctx.reply_tx,
                    &CtrlEvent::NamedQueueSnapshot { queue: id, data },
                );
            }
        }
    }
}

/// Sends one resting queue's snapshot to the connections that show it (design
/// D3: after each change of that queue), each projected for the queue
/// transports that connection negotiated. The connection whose `op` the change
/// already answered receives nothing extra.
pub(super) fn publish_snapshot(ctx: &CtrlContext<'_>, index: usize, except: Option<CtrlClientId>) {
    let Some(resting) = ctx.owner.named_queues.resting.get(index) else {
        return;
    };
    ctx.ctrl_clients
        .lock()
        .unwrap()
        .broadcast_named_queue_snapshot(resting.id, except, |queue_transport| {
            snapshot_json(resting, queue_transport)
        });
}

/// One resting queue's `NamedQueueSnapshot` event as one connection's queue
/// transports project it, serialized for the wire.
fn snapshot_json(resting: &RestingNamedQueue, queue_transport: QueueTransport) -> Option<String> {
    let event = unified_queue_state_for_peer(
        Some(resting.id),
        &mbv_ctrl::player::PlayerStatus::default(),
        &resting.queue,
        &resting.source,
        resting.lineage,
        None,
        resting.start_slot,
        None,
        None,
        queue_transport,
    );
    let CtrlEvent::UnifiedQueueState(data) = event else {
        return None;
    };
    serialize_ctrl_event(&CtrlEvent::NamedQueueSnapshot {
        queue: resting.id,
        data,
    })
}

/// The snapshot one resting queue's Clients hold (design D3): its slots, its
/// source, its lineage, and its start slot, with no playhead — a resting queue
/// has no run, so nothing about the current queue's playback describes it.
pub(super) fn resting_snapshot(
    ctx: &CtrlContext<'_>,
    index: usize,
) -> Option<(NamedQueueId, UnifiedQueueStateData)> {
    let resting = ctx.owner.named_queues.resting.get(index)?;
    let queue_transport = {
        let clients = ctx.ctrl_clients.lock().unwrap();
        clients.queue_transport(ctx.client_id)
    };
    let CtrlEvent::UnifiedQueueState(data) = unified_queue_state_for_peer(
        Some(resting.id),
        &mbv_ctrl::player::PlayerStatus::default(),
        &resting.queue,
        &resting.source,
        resting.lineage,
        None,
        resting.start_slot,
        None,
        None,
        queue_transport,
    ) else {
        return None;
    };
    Some((resting.id, data))
}

/// Ends every connection's subscription to `queue` (design D3).
pub(super) fn drop_shows(ctx: &CtrlContext<'_>, queue: NamedQueueId) {
    ctx.ctrl_clients
        .lock()
        .unwrap()
        .drop_named_queue_shows(queue);
}

/// Ends every connection's subscription to the current queue: it is read
/// through the current-queue snapshots, not through a resting view (design D3).
pub(super) fn drop_current_shows(ctx: &CtrlContext<'_>) {
    drop_shows(ctx, ctx.owner.named_queues.current);
}
