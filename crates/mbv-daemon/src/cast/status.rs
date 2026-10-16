//! Cast status handling (cast-owner-output design D5/D7/D8): the status-poll
//! result handler, the transition gate's Cast settle, and the Owner's
//! presented `PlayerStatus`. While a receiver is the output the Owner is the
//! presenter: it translates the receiver's status into its `PlayerStatus`
//! (active slot, position, duration, paused), extrapolates the position on
//! the daemon tick while the receiver plays, and the daemon's status
//! broadcast thread (`run.rs` `spawn_status_broadcast`) publishes that status
//! to Clients at the local broadcast cadence — so Clients present the
//! receiver's state in the same form as local playback, without
//! extrapolating anything themselves (design D8).

use std::time::{Duration, Instant};

use mbv_cast::client::{CastPlaybackState, CastStatus};
use mbv_ctrl::player::PlayerStatus;
use mbv_emby_model::seconds_to_ticks;
use mbv_queue::PlaybackQueue;

use super::super::DaemonLoop;
use super::{CastLoadedItem, PlaybackOutput};
use mbv_ctrl::CtrlEvent;

/// One status-poll result (design D8, task 3.3). Only the result whose id
/// matches the in-flight poll — the same id stamped into that poll's job —
/// disarms the watchdog clock: a late result from a previous worker (the
/// watchdog reconnect race) carries a stale id and is dropped, so it can
/// neither spuriously disconnect the restored worker nor disturb the new
/// poll's bookkeeping. A returned status is stored and re-presents the
/// Owner's `PlayerStatus` from it (row 6.1); a poll error is a broken
/// connection, so the output disconnects and reconnects.
pub(super) fn handle_cast_status(
    daemon: &mut DaemonLoop,
    receiver_id: &str,
    poll_id: u64,
    result: Result<CastStatus, String>,
) {
    let (error, settle_slot) = {
        let PlaybackOutput::Cast(cast) = &mut daemon.owner.output else {
            return;
        };
        if cast.receiver.id != receiver_id
            || !matches!(cast.connection, mbv_ctrl::CastConnection::Connected)
            || cast
                .poll_sent
                .is_none_or(|(in_flight, _)| in_flight != poll_id)
        {
            return;
        }
        match result {
            Ok(status) => {
                cast.last_status = Some(status);
                cast.status_at = Some(Instant::now());
                cast.poll_sent = None;
                cast.last_poll = Some(Instant::now());
                (None, settle_check(cast))
            }
            Err(error) => {
                cast.poll_sent = None;
                (Some(error), None)
            }
        }
    };
    if let Some(error) = error {
        tracing::warn!(name: "cast.status_poll.failed", target: "cast", receiver = %receiver_id, error = %error, "cast status poll failed; reconnecting");
        super::connect::disconnect_and_reconnect(daemon);
        return;
    }
    if let Some(loaded_slot) = settle_slot {
        settle_cast_transition(daemon, loaded_slot);
    }
    refresh_presentation(daemon, Instant::now());
}

/// Whether a stored status settles the loaded item's transition (design D5,
/// row 5.2): the status names the loaded content id in a playing, buffering,
/// or paused state — the Cast counterpart of the run's `TrackChanged`
/// confirmation. Returns the loaded slot when it does.
fn settle_check(cast: &super::CastOutput) -> Option<mbv_queue::QueueSlotId> {
    let loaded = cast.loaded.as_ref()?;
    let status = cast.last_status.as_ref()?;
    (presents_loaded_item(loaded, status)).then_some(loaded.slot)
}

/// Whether the receiver's status presents the item the Owner loaded (spec
/// "Displayed playback state comes from the receiver"): the status names the
/// loaded content id while the receiver plays, buffers, or pauses it. Every
/// other report — idle, foreign content, no status yet — presents no active
/// playback, and only a confirming status settles the load's transition.
fn presents_loaded_item(loaded: &CastLoadedItem, status: &CastStatus) -> bool {
    matches!(
        status.state,
        CastPlaybackState::Playing | CastPlaybackState::Buffering | CastPlaybackState::Paused
    ) && status.playing_content_id.as_deref() == Some(loaded.content_id.as_str())
}

/// The position one receiver status presents `elapsed` after it was reported
/// (design D8, row 6.1): advancing by the reported playback rate while the
/// receiver plays — clamped to the reported duration — and held at the
/// reported position while it is paused, buffering, or idle. Drift
/// correction is structural: the position always derives from the last
/// report plus the elapsed time since it, so a fresh report replaces the
/// extrapolated value with what the receiver says.
fn extrapolated_position_seconds(status: &CastStatus, elapsed: Duration) -> Option<f32> {
    let position = status.position_seconds?;
    match status.state {
        CastPlaybackState::Playing => {
            let advanced = position + elapsed.as_secs_f32() * status.playback_rate;
            Some(match status.duration_seconds {
                Some(duration) => advanced.min(duration),
                None => advanced,
            })
        }
        CastPlaybackState::Paused | CastPlaybackState::Buffering | CastPlaybackState::Idle => {
            Some(position)
        }
    }
}

/// The Owner's presented `PlayerStatus` for the Cast output's current state
/// (design D8, row 6.1). When the receiver's status presents the loaded
/// item, the loaded slot is the active slot with the receiver's reported
/// duration and paused state and the extrapolated position; everything else
/// presents no active playback, so an idle, stopped, or foreign-playing
/// receiver never masquerades as the queue's active slot.
pub(super) fn presented_player_status(
    base: &PlayerStatus,
    queue: &PlaybackQueue,
    loaded: Option<&CastLoadedItem>,
    last_status: Option<&CastStatus>,
    elapsed: Duration,
) -> PlayerStatus {
    let presenting = loaded
        .zip(last_status)
        .filter(|(loaded, status)| presents_loaded_item(loaded, status));
    let Some((loaded, status)) = presenting else {
        return not_presenting(base, queue);
    };
    let Some(slot) = queue.slot(loaded.slot) else {
        // The active slot was edited out of the queue while the receiver
        // still reports it: without a slot there is no active-slot identity
        // to present, so the output presents no active playback.
        return not_presenting(base, queue);
    };
    let index = queue.slot_index(loaded.slot).unwrap_or(0);
    let mut presented = base.clone();
    presented.seed_from_item(&slot.item, index, queue.len());
    if let Some(position) = extrapolated_position_seconds(status, elapsed) {
        presented.position_ticks = seconds_to_ticks(f64::from(position));
    }
    if let Some(duration) = status.duration_seconds {
        presented.runtime_ticks = seconds_to_ticks(f64::from(duration));
    }
    presented.paused = status.state == CastPlaybackState::Paused;
    presented.active = true;
    presented
}

/// The `PlayerStatus` for a Cast output with nothing to present (design D8,
/// row 6.1): no active playback, no position, no now-playing metadata. The
/// queue length and the queue's own active marker stay coherent so
/// status-only consumers never index a slot the queue no longer holds.
fn not_presenting(base: &PlayerStatus, queue: &PlaybackQueue) -> PlayerStatus {
    let mut inactive = base.clone();
    inactive.active = false;
    inactive.paused = false;
    inactive.position_ticks = 0;
    inactive.clear_current_item_metadata();
    inactive.queue_len = queue.len();
    inactive.current_idx = queue.active_index().unwrap_or(0);
    inactive
}

/// Re-derives the Owner's presented `PlayerStatus` from the Cast output's
/// state and installs it as the player's shared status (design D8, row 6.1).
/// The status-broadcast thread publishes it to Clients at the local cadence;
/// `UnifiedQueueState` broadcasts embed the same status. While the connection
/// is down the presented state holds at the last report — the receiver may
/// still be playing it, and the first report after the reconnect corrects
/// the presentation — so extrapolation never runs on a connection that can
/// no longer report.
pub(super) fn refresh_presentation(daemon: &mut DaemonLoop, now: Instant) {
    let PlaybackOutput::Cast(cast) = &daemon.owner.output else {
        return;
    };
    let elapsed = if matches!(cast.connection, mbv_ctrl::CastConnection::Connected) {
        cast.status_at
            .and_then(|at| now.checked_duration_since(at))
            .unwrap_or(Duration::ZERO)
    } else {
        Duration::ZERO
    };
    let base = daemon.player.status.lock().unwrap().clone();
    let presented = presented_player_status(
        &base,
        &daemon.owner.core.queue,
        cast.loaded.as_ref(),
        cast.last_status.as_ref(),
        elapsed,
    );
    *daemon.player.status.lock().unwrap() = presented;
}

/// Settles the in-flight transition a confirming receiver status closes (row
/// 5.2): the loaded slot becomes the observed active slot, the transition
/// settles, a queued transition the settle promotes is dispatched, and the
/// guarded play intent the load carried is applied. Mirrors the run's
/// `TrackChanged` handling (observe → settle → publish).
fn settle_cast_transition(daemon: &mut DaemonLoop, loaded_slot: mbv_queue::QueueSlotId) {
    // Only the transition the loaded item belongs to settles here: a newer
    // in-flight transition targets a slot the receiver has not confirmed.
    let Some(in_flight) = daemon
        .owner
        .core
        .transitions
        .in_flight()
        .filter(|t| t.target == loaded_slot)
    else {
        return;
    };
    let mut ctx = super::owner_context(daemon);
    let order_before = ctx.owner.core.play_order();
    ctx.owner.core.note_observed_active_slot(Some(loaded_slot));
    if ctx.owner.core.play_order() != order_before {
        crate::forward_play_order_advance(&ctx.owner.core, ctx.player);
    }
    crate::core::settle_and_redispatch(&mut ctx, in_flight.request_id, loaded_slot);
    // The guarded play intent the load registered under settles with its
    // transition; without this a Cast play would sit unresolved and coalesce
    // every later play from its connection.
    if let Some((connection_id, request_id, generation)) = ctx
        .owner
        .intents
        .current
        .as_ref()
        .filter(|current| {
            current.request_id == in_flight.request_id
                && matches!(current.action, mbv_ctrl::PlaybackIntentAction::Play { .. })
        })
        .map(|current| {
            (
                current.connection_id,
                current.request_id,
                current.generation,
            )
        })
        && let Some(event) =
            ctx.owner
                .intents
                .applied_if_current(connection_id, request_id, generation)
    {
        ctx.ctrl_clients
            .lock()
            .unwrap()
            .send_to_client(connection_id, &CtrlEvent::PlaybackIntent(event));
    }
    ctx.shared_queue.publish_observed(&ctx.owner.core);
    crate::broadcast_queue_state(
        ctx.ctrl_clients,
        ctx.player,
        ctx.shared_queue,
        &ctx.owner.core.queue,
        &ctx.owner.core.source,
        &ctx.owner.core.transitions,
        None,
    );
}
