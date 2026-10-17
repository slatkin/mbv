//! Cast status handling (cast-owner-output design D5/D7/D8): the status-poll
//! result handler, the transition gate's Cast settle, and the Owner's
//! presented `PlayerStatus`. While a receiver is the output the Owner is the
//! presenter: it translates the receiver's status into its `PlayerStatus`
//! (active slot, position, duration, paused; media the Owner did not load is
//! presented by the receiver's state alone), extrapolates the position on
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
use crate::output::not_presenting;

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
    let (error, settle_slot, feed_events) = {
        let PlaybackOutput::Cast(cast) = &mut daemon.owner.output else {
            return;
        };
        if cast.receiver.id != receiver_id
            || !matches!(cast.connection, mbv_ctrl::OutputConnection::Connected)
            || cast
                .poll_sent
                .is_none_or(|(in_flight, _)| in_flight != poll_id)
        {
            return;
        }
        match result {
            Ok(status) => {
                // The fresh status's Feed events are read ahead of the
                // installation, while the previous status is still stored
                // (design D9).
                let feed_events = super::feed_events::status_feed_events(
                    cast.loaded.as_ref(),
                    cast.last_status.as_ref(),
                    &status,
                    cast.pending_seek,
                    poll_id,
                );
                cast.last_status = Some(status);
                cast.status_at = Some(Instant::now());
                cast.poll_sent = None;
                cast.last_poll = Some(Instant::now());
                (None, settle_check(cast), feed_events)
            }
            Err(error) => {
                cast.poll_sent = None;
                (
                    Some(error),
                    None,
                    super::feed_events::StatusFeedEvents::none(),
                )
            }
        }
    };
    if let Some(error) = error {
        tracing::warn!(name: "cast.status_poll.failed", target: "cast", receiver = %receiver_id, error = %error, "cast status poll failed; reconnecting");
        super::connect::disconnect_and_reconnect(daemon);
        return;
    }
    if let Some(loaded_slot) = settle_slot {
        crate::core::settle_remote_transition(daemon, loaded_slot);
    }
    super::advance::apply_end_report(daemon);
    refresh_presentation(daemon, Instant::now());
    // The applied status feeds the loaded item's provider reporting (design
    // D9): the presented status the refresh installed is what the reporter
    // snapshots, and the content-id gate is the presentation's own.
    super::progress::report_presenting_status(daemon);
    // The applied status's Feed events go out after the presentation refresh
    // installed the state they report (design D9, row 6.4's Feed piece).
    super::feed_events::emit_status_events(daemon, feed_events);
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
/// other report — idle, foreign content, no status yet — does not present the
/// loaded item, and only a confirming status settles the load's transition.
pub(super) fn presents_loaded_item(loaded: &CastLoadedItem, status: &CastStatus) -> bool {
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
/// item, the loaded slot is the active slot. Any other playing or paused
/// receiver — media the Owner did not load, or a loaded slot edited out of
/// the queue — is presented by its reported state alone, with no item
/// identity. Only an idle receiver, or no report, presents no active
/// playback.
pub(super) fn presented_player_status(
    base: &PlayerStatus,
    queue: &PlaybackQueue,
    loaded: Option<&CastLoadedItem>,
    last_status: Option<&CastStatus>,
    elapsed: Duration,
) -> PlayerStatus {
    let Some(status) = last_status.filter(|status| status.state != CastPlaybackState::Idle) else {
        return not_presenting(base, queue);
    };
    let loaded_slot = loaded
        .filter(|loaded| presents_loaded_item(loaded, status))
        .and_then(|loaded| queue.slot(loaded.slot).map(|slot| (loaded.slot, slot)));
    let mut presented = base.clone();
    if let Some((slot_id, slot)) = loaded_slot {
        let index = queue.slot_index(slot_id).unwrap_or(0);
        presented.seed_from_item(&slot.item, index, queue.len());
    } else {
        // No item identity and no timing from the previous presentation:
        // the receiver's own report fills what it states.
        presented.clear_current_item_metadata();
        presented.position_ticks = 0;
        presented.runtime_ticks = 0;
        presented.queue_len = queue.len();
        presented.current_idx = queue.active_index().unwrap_or(0);
    }
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

/// Re-derives the Owner's presented `PlayerStatus` from the Cast output's
/// state and installs it as the player's shared status (design D8, row 6.1).
/// The status-broadcast thread publishes it to Clients at the local cadence;
/// `UnifiedQueueState` broadcasts embed the same status. While the connection
/// is down the presented state holds at the last report — the receiver may
/// still be playing it, and the first report after the reconnect corrects
/// the presentation — so extrapolation never runs on a connection that can
/// no longer report.
pub(crate) fn refresh_presentation(daemon: &mut DaemonLoop, now: Instant) {
    let PlaybackOutput::Cast(cast) = &daemon.owner.output else {
        return;
    };
    let elapsed = if matches!(cast.connection, mbv_ctrl::OutputConnection::Connected) {
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
