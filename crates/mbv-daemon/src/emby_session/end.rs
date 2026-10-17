//! The loaded item's fate on the Emby Session output (emby-session-owner-
//! output design D6, D7; tasks 4.3 and 9.3). Because Emby reports no end reason
//! and the device fetches its own stream, the Owner infers both the start and
//! the failure of a load:
//!
//! - an observation that presents the loaded item settles its transition, so a
//!   step queued behind the load dispatches;
//! - an observation that presents the queued next item after one that
//!   presented the loaded item is the device's own move, which the Owner
//!   follows (design D14);
//! - a `play` job the device refuses, or 30 seconds of observations that never
//!   present the item, report the item once and expire the transition without
//!   advancing anything.
//!
//! The 30 second rule runs on each observation and on the daemon tick (design
//! D6), so a Session that is idle and unchanged still expires a load it never
//! showed.

use std::time::{Duration, Instant};

use mbv_ctrl::CtrlEvent;
use mbv_emby::SessionInfo;
use mbv_queue::QueueSlotId;

use super::{EmbySessionOutput, presents_item};
use crate::DaemonLoop;
use crate::cast::PlaybackOutput;

/// How long a load may go without an observation presenting it before the
/// Owner reports that it never started (design D6, task 4.3).
pub(crate) const EMBY_SESSION_LOAD_TIMEOUT: Duration = Duration::from_secs(30);

/// How close to the runtime the estimated position must be for an idle
/// Session to count as the loaded item's natural end (design D6, spec "The
/// owner infers a natural end on an Emby Session"): Emby reports no end
/// reason, so a stop this near the end is the item's end.
pub(crate) const EMBY_SESSION_END_MARGIN_TICKS: i64 = 10 * mbv_emby_model::TICKS_PER_SECOND;

/// Applies the idle rows of design D6's end table to one observation (task
/// 5.2), before that observation replaces the one the position estimate reads:
/// an idle Session after a sighting of the loaded item ends it whatever the
/// position. Near the runtime it is the natural end the Owner advances from;
/// earlier it is a stop on the device, which presents no active playback and
/// advances nothing. `end_handled` records either outcome, so the second poll
/// of the same idle advances at most once.
///
/// Returns whether this observation ended the loaded item naturally. An idle
/// Session before the loaded item was ever presented is not an end (the 30 s
/// failure rule owns that case). The replaced observation must present the
/// loaded item: a Session that presented an item the Owner did not load ends
/// nothing (design D6's last row), and the idle that follows such an item is
/// the other app's stop, not this item's end — its runtime and position belong
/// to that item.
pub(crate) fn ended_naturally(
    output: &mut EmbySessionOutput,
    session: &SessionInfo,
    now: Instant,
) -> bool {
    if session.now_playing_item_id.is_some() {
        return false;
    }
    let unhandled_sighting = output.loaded.as_ref().is_some_and(|loaded| {
        loaded.seen_playing
            && !loaded.end_handled
            && presents_item(output.last_observation.as_ref(), &loaded.item_id)
    });
    if !unhandled_sighting {
        return false;
    }
    let runtime = output.presented_runtime_ticks();
    let estimate = output.estimate_ticks(now);
    if let Some(loaded) = output.loaded.as_mut() {
        loaded.end_handled = true;
    }
    // A loaded slot whose queue item and Session both report no runtime has no
    // end to infer (design D6, risks): the idle counts as a stop on the device.
    runtime > 0 && estimate.saturating_add(EMBY_SESSION_END_MARGIN_TICKS) >= runtime
}

/// The device advance one observation shows (design D6's end table, D14; task
/// 9.3): the Session moved from the loaded item to the queued next one by
/// itself, so the Owner follows it to that slot. The decision is taken before
/// the observation replaces the one the position estimate reads, because the
/// end margin is measured on the replaced observation.
#[derive(Debug, Clone, Copy)]
pub(crate) struct DeviceAdvance {
    /// The queued slot the Session started.
    pub(crate) queued: QueueSlotId,
    /// Whether the old slot's estimate was within
    /// [`EMBY_SESSION_END_MARGIN_TICKS`] of its runtime: only then does the
    /// advance record the old slot's completion (design D14 step 2).
    pub(crate) completes: bool,
}

/// Whether this observation shows the device move of design D14: the Session
/// presents the queued next item, and the replaced observation presented the
/// loaded item. The same item in both slots does not change the id the Session
/// shows, so that move is a position drop to less than 10 seconds after an
/// estimate within the end margin.
///
/// An item the Owner did not load is not an advance, and neither is a Session
/// that presents the queued item when the replaced observation presented
/// something else (design D6's last row).
pub(crate) fn device_advance(
    output: &EmbySessionOutput,
    session: &SessionInfo,
    now: Instant,
) -> Option<DeviceAdvance> {
    let loaded = output.loaded.as_ref()?;
    let queued = output.queued_next.as_ref()?;
    if !presents_item(output.last_observation.as_ref(), &loaded.item_id)
        || !presents_item(Some(session), &queued.item_id)
    {
        return None;
    }
    let runtime = output.presented_runtime_ticks();
    let estimate = output.estimate_ticks(now);
    let completes =
        runtime > 0 && estimate.saturating_add(EMBY_SESSION_END_MARGIN_TICKS) >= runtime;
    if loaded.item_id == queued.item_id
        && !(completes && session.position_ticks < EMBY_SESSION_END_MARGIN_TICKS)
    {
        return None;
    }
    Some(DeviceAdvance {
        queued: queued.slot,
        completes,
    })
}

/// Handles the natural-end row of design D6's end table (task 5.2): the shared
/// advance records the ended slot's completion through `record_completed_slot`
/// and steps the queue through `OutputAdvance`, which reports and steps past a
/// slot this output cannot play. It is the same advance a Cast natural end
/// starts, so the two outputs never drift apart.
pub(crate) fn advance_from_natural_end(daemon: &mut DaemonLoop) {
    crate::output::advance::begin_advance(daemon);
}

/// Settles the in-flight transition that the observation presenting the
/// loaded item closes (design D6, D7). The settle promotes a step queued
/// behind the load, which is why quick repeated Next commands end on the
/// newest target instead of staying stuck behind the first load.
pub(crate) fn settle_loaded_item(daemon: &mut DaemonLoop, loaded_slot: QueueSlotId) {
    crate::core::settle_remote_transition(daemon, loaded_slot);
}

/// Expires a load that no observation has presented for
/// [`EMBY_SESSION_LOAD_TIMEOUT`] (design D6, task 4.3): the item is reported
/// once and the in-flight transition expires. Nothing else changes — the
/// queue, the active slot, and the loaded item stay as they are, so no advance
/// follows. Returns whether this call reported the failure.
pub(crate) fn expire_unseen_load(daemon: &mut DaemonLoop, now: Instant) -> bool {
    let expired = {
        let PlaybackOutput::EmbySession(output) = &daemon.owner.output else {
            return false;
        };
        output.loaded.as_ref().is_some_and(|loaded| {
            !loaded.seen_playing
                && !loaded.failure_reported
                && now
                    .checked_duration_since(loaded.loaded_at)
                    .is_some_and(|elapsed| elapsed >= EMBY_SESSION_LOAD_TIMEOUT)
        })
    };
    if !expired {
        return false;
    }
    fail_loaded_item(daemon, None)
}

/// Handles a `play` job the device refused (design D6, D7): the item is
/// reported once with the device's error and the load's transition expires, so
/// the one-in-flight gate cannot stay wedged behind a load that never
/// happened. Returns whether this call reported the failure.
pub(crate) fn handle_play_failed(daemon: &mut DaemonLoop, slot: QueueSlotId, error: &str) -> bool {
    let matches_loaded = {
        let PlaybackOutput::EmbySession(output) = &daemon.owner.output else {
            return false;
        };
        output
            .loaded
            .as_ref()
            .is_some_and(|loaded| loaded.slot == slot)
    };
    if !matches_loaded {
        return false;
    }
    fail_loaded_item(
        daemon,
        Some(&format!("the device refused the play: {error}")),
    )
}

/// Reports the loaded item once and expires its transition (design D6): the
/// report names the item and, when the caller has one, the reason it did not
/// start. `failure_reported` keeps the report and the expiry to one per load,
/// however many observations, ticks, or worker outcomes see it afterwards.
fn fail_loaded_item(daemon: &mut DaemonLoop, detail: Option<&str>) -> bool {
    let (slot, item) = {
        let PlaybackOutput::EmbySession(output) = &mut daemon.owner.output else {
            return false;
        };
        let Some(loaded) = output.loaded.as_mut() else {
            return false;
        };
        if loaded.failure_reported {
            return false;
        }
        loaded.failure_reported = true;
        (loaded.slot, loaded.item_id.clone())
    };
    let name = daemon
        .owner
        .core
        .queue
        .slot(slot)
        .map_or(item, |slot| slot.item.display_name());
    let report = match detail {
        Some(detail) => format!("{name} did not start on the Emby Session ({detail})"),
        None => format!("{name} did not start on the Emby Session"),
    };
    tracing::warn!(
        name: "daemon.emby_session_load.not_started",
        target: "daemon",
        item = %name,
        detail = detail.unwrap_or("not presented within 30 seconds"),
        "the Emby Session never presented the loaded item"
    );
    crate::core::broadcast(&daemon.ctrl_clients, &CtrlEvent::CommandRejected(report));
    let mut ctx = crate::cast::owner_context(daemon);
    crate::cast::expire_remote_transition(&mut ctx);
    true
}
