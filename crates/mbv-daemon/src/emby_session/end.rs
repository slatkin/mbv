//! The loaded item's fate on the Emby Session output (emby-session-owner-
//! output design D6, D7; task 4.3). Because Emby reports no end reason and the
//! device fetches its own stream, the Owner infers both the start and the
//! failure of a load:
//!
//! - an observation that presents the loaded item settles its transition, so a
//!   step queued behind the load dispatches;
//! - a `play` job the device refuses, or 30 seconds of observations that never
//!   present the item, report the item once and expire the transition without
//!   advancing anything.
//!
//! The 30 second rule runs on each observation and on the daemon tick (design
//! D6), so a Session that is idle and unchanged still expires a load it never
//! showed.

use std::time::{Duration, Instant};

use mbv_ctrl::CtrlEvent;
use mbv_queue::QueueSlotId;

use crate::DaemonLoop;
use crate::cast::PlaybackOutput;

/// How long a load may go without an observation presenting it before the
/// Owner reports that it never started (design D6, task 4.3).
pub(crate) const EMBY_SESSION_LOAD_TIMEOUT: Duration = Duration::from_secs(30);

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
