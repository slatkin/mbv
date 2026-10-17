//! The hand-over of the rest of the queue at Owner exit
//! (emby-session-owner-output design D15, task 9.5): while the device plays the
//! loaded item, the Owner gives it every admissible slot after the item it
//! holds next, so the device plays out the queue without the Owner and never
//! reaches its own next-episode autoplay. The active item plays on unless a
//! queue edit made the queued next item stale (design D14): then one `play`
//! reloads the active item at its estimated position with the queue's correct
//! window, so the device never starts the stale item.
//!
//! The one job acks on a `sync_channel(0)` and the Owner waits for it with the
//! shutdown deadline's remaining time, so the wait shares the shutdown bound and
//! also covers the jobs queued ahead of it. A failed or late call is logged, and
//! the exit goes on. Shutdown still sends no `Stop` (design D11).

use std::sync::mpsc;
use std::time::Instant;

use mbv_queue::QueueSlotId;

use super::load::admissible_after;
use super::worker::{EmbySessionJob, SessionApi, acknowledged};
use super::{EmbySessionOutput, presents_item};
use crate::DaemonLoop;
use crate::cast::PlaybackOutput;
use crate::reconciliation::remaining;

/// Hands the rest of the queue to the device and waits for the one job within
/// the shutdown deadline (design D15). Nothing is sent when the output is not
/// an Emby Session, the device is idle or plays content the Owner did not load,
/// or the device already holds the last playable slot.
pub(crate) fn hand_over(daemon: &DaemonLoop, deadline: Instant) {
    let Some(exit) = plan(daemon) else {
        return;
    };
    if exit.worker.send(exit.job).is_err() {
        tracing::warn!(
            name: "daemon.emby_session_handover.worker_gone",
            target: "daemon",
            "the Emby Session worker is not running; the rest of the queue was not handed over"
        );
        return;
    }
    match exit.ack.recv_timeout(remaining(deadline)) {
        Ok(()) => tracing::info!(
            name: "daemon.emby_session_handover.done",
            target: "daemon",
            "the rest of the queue was handed to the Emby Session before the Owner exits"
        ),
        Err(error) => tracing::warn!(
            name: "daemon.emby_session_handover.unfinished",
            target: "daemon",
            error = %error,
            "the queue hand-over did not finish within the shutdown bound; the exit goes on"
        ),
    }
}

/// The one job the exit submits (design D15): the worker that runs it, the job
/// itself, and the ack the Owner waits on.
struct ExitJob {
    worker: mpsc::Sender<EmbySessionJob>,
    job: EmbySessionJob,
    ack: mpsc::Receiver<()>,
}

/// The one device call the exit makes (design D15): the rest of the queue
/// appended after the item the device holds next, or — when a queue edit made
/// that item stale — the active item reloaded at its estimate with the queue's
/// correct window.
enum ExitHandover {
    /// `play_last` of every admissible slot after the queued next item.
    PlayLast { item_ids: Vec<String> },
    /// `play` of the active item, the correct next slot, and the rest.
    Play {
        item_ids: Vec<String>,
        start_ticks: i64,
    },
}

impl ExitHandover {
    /// Runs the call on the worker thread (design D15): the active item is
    /// never interrupted by the append, and a refusal is logged because the
    /// Owner is already exiting.
    fn run(&self, api: &dyn SessionApi, session_id: &str) {
        let result = match self {
            Self::PlayLast { item_ids } => api.play_last(session_id, &item_id_refs(item_ids)),
            Self::Play {
                item_ids,
                start_ticks,
            } => api.play(session_id, &item_id_refs(item_ids), *start_ticks),
        };
        if let Err(error) = result {
            tracing::warn!(
                name: "daemon.emby_session_handover.failed",
                target: "daemon",
                error = %error,
                "the device refused the queue hand-over at exit; the exit goes on"
            );
        }
    }
}

/// Builds the exit job (design D15), or nothing when the device is not the
/// Owner's to hand over: an idle Session, content the Owner did not load, and a
/// queue with nothing left after the window are all no call.
fn plan(daemon: &DaemonLoop) -> Option<ExitJob> {
    let PlaybackOutput::EmbySession(output) = &daemon.owner.output else {
        return None;
    };
    let loaded = output.loaded.as_ref()?;
    if !presents_item(output.last_observation.as_ref(), &loaded.item_id) {
        tracing::info!(
            name: "daemon.emby_session_handover.not_playing",
            target: "daemon",
            "the device plays nothing the Owner loaded; no queue hand-over is sent"
        );
        return None;
    }
    let session_id = output.session_id.clone()?;
    let handover = exit_handover(daemon, output)?;
    let (ack_tx, ack) = mpsc::sync_channel(0);
    let job: EmbySessionJob = Box::new(move |api: &dyn SessionApi| {
        handover.run(api, &session_id);
    });
    Some(ExitJob {
        worker: output.worker.clone(),
        job: acknowledged(ack_tx, job),
        ack,
    })
}

/// What the exit sends, if anything (design D15 steps 1-3): the admissible
/// slots after the queued next item — after the active slot when nothing is
/// queued — are appended with one `play_last`. A queued next item that a queue
/// edit made stale is not appended after: the active item and the queue's
/// correct window are reloaded with one `play` instead. Slots before the active
/// slot have played, and they are not sent.
fn exit_handover(daemon: &DaemonLoop, output: &EmbySessionOutput) -> Option<ExitHandover> {
    let loaded = output.loaded.as_ref()?;
    let queued = output.queued_next.as_ref();
    if queued.is_some_and(|queued| queued.stale) {
        let mut item_ids = vec![loaded.item_id.clone()];
        item_ids.extend(item_ids_after(daemon, output, loaded.slot));
        return Some(ExitHandover::Play {
            item_ids,
            start_ticks: output.estimate_ticks(Instant::now()),
        });
    }
    let after = queued.map_or(loaded.slot, |queued| queued.slot);
    let item_ids = item_ids_after(daemon, output, after);
    (!item_ids.is_empty()).then_some(ExitHandover::PlayLast { item_ids })
}

/// The Emby item ids of the admissible slots after `slot` (design D15): a slot
/// the Session cannot play is dropped with no report, because the Owner is
/// exiting.
fn item_ids_after(
    daemon: &DaemonLoop,
    output: &EmbySessionOutput,
    slot: QueueSlotId,
) -> Vec<String> {
    admissible_after(&daemon.owner.core, slot, &output.playable_media_types)
        .into_iter()
        .map(|(_, item_id)| item_id)
        .collect()
}

/// The borrowed ids one `SessionApi` call takes, from the owned ids the job
/// carries across the worker boundary.
fn item_id_refs(item_ids: &[String]) -> Vec<&str> {
    item_ids.iter().map(String::as_str).collect()
}
