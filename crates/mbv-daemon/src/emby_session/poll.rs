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

/// The poll cadence (design D5): the same 1 s `/Sessions` cadence the TUI
/// polls at today.
pub(crate) const EMBY_SESSION_POLL_INTERVAL: Duration = Duration::from_secs(1);

/// Observations in a row that do not list the device before the output
/// reports `Disconnected` (design D5).
pub(crate) const EMBY_SESSION_DISCONNECT_MISSES: u8 = 3;

/// Submits this tick's poll (design D5): one in flight at a time, at most one
/// per [`EMBY_SESSION_POLL_INTERVAL`], stamped with the next monotonic id so a
/// late result from a replaced worker can never match the current poll.
pub(crate) fn tick(daemon: &mut DaemonLoop, now: Instant) {
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
    let (changed, failed) = {
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
            Ok(sessions) => (observe(output, &sessions), None),
            Err(error) => (false, Some(error)),
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
    if changed {
        broadcast_output_change(daemon);
    }
}

/// Applies one `/Sessions` observation of this device (design D5): store the
/// entry, adopt its session id (a reattach after the device app restarts), and
/// reset the miss count; a missing entry counts one miss and, at the
/// threshold, reports the output `Disconnected`. Returns whether the
/// connection changed, so the caller broadcasts the transition.
fn observe(output: &mut EmbySessionOutput, sessions: &[SessionInfo]) -> bool {
    let Some(session) = sessions
        .iter()
        .find(|session| session.device_id == output.device_id)
    else {
        output.misses = output.misses.saturating_add(1);
        if output.misses < EMBY_SESSION_DISCONNECT_MISSES
            || output.connection == OutputConnection::Disconnected
        {
            return false;
        }
        output.connection = OutputConnection::Disconnected;
        return true;
    };
    output.misses = 0;
    output.session_id = Some(session.id.clone());
    output.last_observation = Some(session.clone());
    output.observed_at = Some(Instant::now());
    if output.connection == OutputConnection::Connected {
        return false;
    }
    output.connection = OutputConnection::Connected;
    true
}
