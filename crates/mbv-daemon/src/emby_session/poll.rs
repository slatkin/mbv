//! Polling the Emby Session output (emby-session-owner-output design D5, task
//! 3.3): the daemon tick submits at most one `/Sessions` poll per second and
//! stamps it with a monotonic id; the result finds this device's entry by
//! `DeviceId`, adopts a changed session id, and counts the misses that
//! disconnect the output. Polling continues while the output is disconnected,
//! so the same device reappearing reconnects it.

use std::sync::mpsc;
use std::time::{Duration, Instant};

use mbv_ctrl::OutputConnection;
use mbv_emby::SessionInfo;

use super::worker::{EmbySessionJob, SessionApi};
use super::{EmbySessionEvent, EmbySessionOutput};
use crate::DaemonEvent;
use crate::cast::{PlaybackOutput, broadcast_output_change};
use crate::event_loop::DaemonLoop;
use mbv_queue::QueueSlotId;

/// The poll cadence (design D5): the same 1 s `/Sessions` cadence the TUI
/// polls at today.
pub(crate) const EMBY_SESSION_POLL_INTERVAL: Duration = Duration::from_secs(1);

/// Observations in a row that do not list the device before the output
/// reports `Disconnected` (design D5).
pub(crate) const EMBY_SESSION_DISCONNECT_MISSES: u8 = 3;

/// Submits this tick's poll (design D5) and runs the load timeout (design D6,
/// task 4.3). The timeout runs on every tick, not only when a poll is due, so
/// a Session that is idle and unchanged still expires a load it never showed.
/// The tick also re-derives the presented status from the last observation
/// (design D10), so the position estimate advances between polls.
pub(crate) fn tick(daemon: &mut DaemonLoop, now: Instant) {
    submit_due_poll(daemon, now);
    super::end::expire_unseen_load(daemon, now);
    super::status::refresh_presentation(daemon, now);
}

/// Submits this tick's poll when one is due: one in flight at a time, at most
/// one per [`EMBY_SESSION_POLL_INTERVAL`], stamped with the next monotonic id
/// so a late result from a replaced worker can never match the current poll.
fn submit_due_poll(daemon: &mut DaemonLoop, now: Instant) {
    let PlaybackOutput::EmbySession(output) = &mut daemon.owner.output else {
        return;
    };
    let due = output.poll_sent.is_none()
        && output
            .last_poll
            .is_none_or(|at| now.duration_since(at) >= EMBY_SESSION_POLL_INTERVAL);
    if !due {
        return;
    }
    let poll_id = output.next_poll_id;
    output.next_poll_id += 1;
    submit_poll(&output.worker, daemon.merged_tx.clone(), poll_id);
    output.poll_sent = Some((poll_id, now));
    output.last_poll = Some(now);
}

/// Submits one unfiltered `/Sessions` poll on the output's worker (design D4,
/// D5): the worker returns the result as `DaemonEvent::EmbySession`, stamped
/// with `poll_id`.
fn submit_poll(
    worker: &mpsc::Sender<EmbySessionJob>,
    events: mpsc::Sender<DaemonEvent>,
    poll_id: u64,
) {
    let _ = worker.send(Box::new(move |api: &dyn SessionApi| {
        let result = api.get_sessions();
        let _ = events.send(DaemonEvent::EmbySession(EmbySessionEvent::Sessions {
            poll_id,
            result,
        }));
    }));
}

/// Applies one poll result (design D5). Only the result for the in-flight poll
/// is applied: a late result from an earlier poll or a replaced worker carries
/// a stale id and changes nothing. A failed call is not a missing device, so
/// it leaves the connection and the miss count alone. The queue, the active
/// slot, and the loaded item never change here.
pub(super) fn handle_sessions(
    daemon: &mut DaemonLoop,
    poll_id: u64,
    result: Result<Vec<SessionInfo>, String>,
) {
    let (observation, failed) = {
        let PlaybackOutput::EmbySession(output) = &mut daemon.owner.output else {
            return;
        };
        if output
            .poll_sent
            .is_none_or(|(in_flight, _)| in_flight != poll_id)
        {
            return;
        }
        output.poll_sent = None;
        match result {
            Ok(sessions) => {
                let now = Instant::now();
                (Some(observe(output, &sessions, now)), None)
            }
            Err(error) => (None, Some(error)),
        }
    };
    if let Some(error) = failed {
        tracing::warn!(
            name: "daemon.emby_session_output.poll_failed",
            target: "daemon",
            poll = poll_id,
            error = %error,
            "the Emby Session poll failed; the output is unchanged"
        );
    }
    let changed = observation
        .as_ref()
        .is_some_and(|changed| changed.broadcast);
    // The observation that shows the loaded item settles its transition
    // (design D6, D7, task 4.3); a step queued behind the load then dispatches.
    if let Some(loaded_slot) = observation
        .as_ref()
        .and_then(|observation| observation.presented)
    {
        super::end::settle_loaded_item(daemon, loaded_slot);
    }
    // The observation that went idle after a sighting of the loaded item ends
    // it (design D6, task 5.2): near the runtime the owner records its
    // completion and steps the queue, and earlier it presents nothing and
    // advances nothing.
    if observation.is_some_and(|observation| observation.ended_naturally) {
        super::end::advance_from_natural_end(daemon);
    }
    // An observation that does not show the loaded item is also when the 30 s
    // load timeout is measured (design D6); this poll's own reading is the
    // observation time.
    super::end::expire_unseen_load(daemon, Instant::now());
    // The fresh observation re-presents the Owner's status from it (design
    // D10), so Clients see the device's position, pause, and track state.
    super::status::refresh_presentation(daemon, Instant::now());
    if changed {
        broadcast_output_change(daemon);
    }
}

/// What one `/Sessions` observation changed (design D5, D6): whether the state
/// the ctrl peers hold changed, the loaded slot the observation presents, and
/// whether it ended the loaded item naturally.
struct Observation {
    broadcast: bool,
    presented: Option<QueueSlotId>,
    ended_naturally: bool,
}

impl Observation {
    /// The observation nothing changed: a missing device below the disconnect
    /// threshold, or a failed poll.
    const UNCHANGED: Self = Self {
        broadcast: false,
        presented: None,
        ended_naturally: false,
    };
}

/// Applies one `/Sessions` observation of this device (design D5): store the
/// entry, adopt its session id (a reattach after the device app restarts), and
/// reset the miss count; a missing entry counts one miss and, at the
/// threshold, reports the output `Disconnected`. The end table's idle rows read
/// the position estimate from the observation this one replaces (design D6),
/// so they run before the store. Reports whether the broadcast state changed —
/// the connection, or the item the device presents that the Owner did not load
/// (design D3, D10) — the loaded slot when this observation presents it, so the
/// caller settles its transition (design D6, D7), and whether it ended that
/// item naturally.
fn observe(output: &mut EmbySessionOutput, sessions: &[SessionInfo], now: Instant) -> Observation {
    let Some(session) = sessions
        .iter()
        .find(|session| session.device_id == output.device_id)
    else {
        output.misses = output.misses.saturating_add(1);
        if output.misses < EMBY_SESSION_DISCONNECT_MISSES
            || output.connection == OutputConnection::Disconnected
        {
            return Observation::UNCHANGED;
        }
        output.connection = OutputConnection::Disconnected;
        return Observation {
            broadcast: true,
            ..Observation::UNCHANGED
        };
    };
    output.misses = 0;
    output.session_id = Some(session.id.clone());
    // The state the ctrl peers hold changes only when the device's own item
    // changes, not on every poll (design D3, D10).
    let now_playing_before = output.now_playing();
    let ended_naturally = super::end::ended_naturally(output, session, now);
    output.last_observation = Some(session.clone());
    output.observed_at = Some(now);
    let presented = presented_loaded_slot(output, session);
    let now_playing_changed = output.now_playing() != now_playing_before;
    let was_connected = output.connection == OutputConnection::Connected;
    output.connection = OutputConnection::Connected;
    Observation {
        broadcast: now_playing_changed || !was_connected,
        presented,
        ended_naturally,
    }
}

/// The loaded slot this observation presents (design D6, task 4.3): the
/// Session's now-playing item is the one the Owner loaded. Recording the
/// sighting here keeps every other reader — the 30 s timeout, the end rule,
/// and the presentation — on the one observation, and an idle Session or one
/// playing other content presents nothing.
fn presented_loaded_slot(
    output: &mut EmbySessionOutput,
    session: &SessionInfo,
) -> Option<QueueSlotId> {
    let item_id = session.now_playing_item_id.as_deref()?;
    let loaded = output
        .loaded
        .as_mut()
        .filter(|loaded| loaded.item_id == item_id)?;
    loaded.seen_playing = true;
    Some(loaded.slot)
}
