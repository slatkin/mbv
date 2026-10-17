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

use super::end::DeviceAdvance;
use super::worker::{EmbySessionJob, SessionApi};
use super::{EmbySessionEvent, EmbySessionOutput};
use crate::DaemonEvent;
use crate::cast::{PlaybackOutput, broadcast_output_change, output_state};
use crate::event_loop::DaemonLoop;
use mbv_queue::QueueSlotId;

/// The poll cadence (design D5): the same 1 s `/Sessions` cadence the TUI
/// polls at today.
pub(crate) const EMBY_SESSION_POLL_INTERVAL: Duration = Duration::from_secs(1);

/// Observations in a row that do not list the device before the output
/// reports `Disconnected` (design D5).
pub(crate) const EMBY_SESSION_DISCONNECT_MISSES: u8 = 3;

/// Submits this tick's poll (design D5), reconciles the device's window with the
/// queue (design D14, task 9.4), and runs the load timeout (design D6, task
/// 4.3). The timeout runs on every tick, not only when a poll is due, so a
/// Session that is idle and unchanged still expires a load it never showed, and
/// the window check runs on every tick too, so a queue edit reaches the device
/// without a new observation. The tick also re-derives the presented status
/// from the last observation (design D10), so the position estimate advances
/// between polls.
pub(crate) fn tick(daemon: &mut DaemonLoop, now: Instant) {
    submit_due_poll(daemon, now);
    super::load::check_queued_next(daemon);
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
    let now = Instant::now();
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
            Ok(sessions) => (Some(observe(output, sessions, now)), None),
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
    // The observation that shows the queued next item after one that showed
    // the loaded item is the device's own move (design D6, D14, task 9.3): the
    // Owner follows it to that slot without a load, or corrects the device when
    // a queue edit made that slot stale (task 9.4). The window is reconciled
    // with the queue first, so an edit that landed between two polls makes the
    // move stale before it is judged. An observation of the loaded item that
    // does not move settles its transition (design D6, D7, task 4.3), and a
    // step queued behind the load then dispatches.
    if let Some(observation) = observation {
        super::load::check_queued_next(daemon);
        if let Some(advance) = observation.device_advance {
            crate::output::advance::apply_device_advance(daemon, advance, now);
        } else if let Some(loaded_slot) = observation.presented {
            super::end::settle_loaded_item(daemon, loaded_slot);
            // The first sighting is also when a hand-off's pause applies (design
            // D9, task 6.1): a `play` job cannot start paused.
            apply_pending_pause(daemon);
        }
        // The observation that went idle after a sighting of the loaded item ends
        // it (design D6, task 5.2): near the runtime the owner records its
        // completion and steps the queue, and earlier it presents nothing and
        // advances nothing.
        if observation.ended_naturally {
            super::end::advance_from_natural_end(daemon);
        }
    }
    // An observation that does not show the loaded item is also when the 30 s
    // load timeout is measured (design D6); this poll's own reading is the
    // observation time.
    super::end::expire_unseen_load(daemon, now);
    // The fresh observation re-presents the Owner's status from it (design
    // D10), so Clients see the device's position, pause, and track state.
    super::status::refresh_presentation(daemon, now);
    // The state the peers hold is what the Owner last broadcast, not what this
    // poll changed: a load replaces the loaded item without any observation,
    // so an observation that reads the same as the previous one can still
    // project to a different state — the item peers saw as `now_playing` is
    // now the loaded item, or an earlier item that is still playing became
    // foreign content (design D3, D10; review r5).
    let state_changed =
        output_state(&daemon.owner.output) != *daemon.shared_queue.playback_output.lock().unwrap();
    if state_changed {
        broadcast_output_change(daemon);
    }
}

/// Sends the pause a hand-off could not start with (design D9, task 6.1):
/// the first observation that shows the loaded item pauses it. The flag is
/// spent either way, because an observation that already reports paused needs
/// no command (`set_paused` sends nothing when the states match).
fn apply_pending_pause(daemon: &mut DaemonLoop) {
    let PlaybackOutput::EmbySession(output) = &mut daemon.owner.output else {
        return;
    };
    if !output.pending_pause {
        return;
    }
    output.pending_pause = false;
    super::command::set_paused(output, true);
}

/// What one `/Sessions` observation changed (design D5, D6): the loaded slot
/// the observation presents, whether it ended the loaded item naturally, and
/// the device advance it shows (design D14). Whether the ctrl peers' state
/// changed is decided by comparing the projected state with the one the Owner
/// last broadcast (`handle_sessions`).
#[derive(Clone, Copy)]
struct Observation {
    presented: Option<QueueSlotId>,
    ended_naturally: bool,
    device_advance: Option<DeviceAdvance>,
}

impl Observation {
    /// The observation nothing changed: a missing device, or a failed poll.
    const UNCHANGED: Self = Self {
        presented: None,
        ended_naturally: false,
        device_advance: None,
    };
}

/// Applies one `/Sessions` observation of this device (design D5): store the
/// entry, adopt its session id (a reattach after the device app restarts), and
/// reset the miss count; a missing entry counts one miss and, at the
/// threshold, reports the output `Disconnected`. The end table's idle rows read
/// the position estimate from the observation this one replaces (design D6),
/// so they run before the store, and so does the device advance's end margin
/// (design D14). Returns the loaded slot when this observation presents it, so
/// the caller settles its transition (design D6, D7), whether it ended that
/// item naturally, and the device advance it shows.
fn observe(
    output: &mut EmbySessionOutput,
    sessions: Vec<SessionInfo>,
    now: Instant,
) -> Observation {
    let Some(session) = sessions
        .into_iter()
        .find(|session| session.device_id == output.device_id)
    else {
        output.misses = output.misses.saturating_add(1);
        if output.misses >= EMBY_SESSION_DISCONNECT_MISSES {
            output.connection = OutputConnection::Disconnected;
        }
        return Observation::UNCHANGED;
    };
    output.misses = 0;
    output.session_id = Some(session.id.clone());
    let device_advance = super::end::device_advance(output, &session, now);
    let ended_naturally = super::end::ended_naturally(output, &session, now);
    let presented = presented_loaded_slot(output, &session);
    output.last_observation = Some(session);
    output.observed_at = Some(now);
    output.connection = OutputConnection::Connected;
    Observation {
        presented,
        ended_naturally,
        device_advance,
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
