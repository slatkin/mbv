//! The queue-end continuation's adoption (emby-session-output-followups
//! design D4): the item the device starts of its own at the queue's end is
//! looked up on the output's worker and appended as the queue's next slot
//! when the Session still presents it. The observation alone cannot build the
//! slot, because a queue item holds the full `EmbyItem` the `SessionInfo`
//! does not carry.

use std::sync::mpsc;
use std::time::Instant;

use mbv_emby_model::EmbyItem;
use mbv_queue::QueueItem;

use super::end::ContinuationStart;
use super::worker::{EmbySessionJob, SessionApi};
use super::{
    ContinuationWatch, EmbySessionEvent, EmbySessionLoadedItem, EmbySessionOutput, presents_item,
};
use crate::DaemonEvent;
use crate::cast::PlaybackOutput;
use crate::event_loop::DaemonLoop;

/// Starts the continuation one observation detected (design D4): the ended
/// slot's completion is recorded when this detection is the one that handles
/// the end (the direct path; a natural end's own record is reused on the
/// idle-gap path), then the one lookup is submitted on the output's worker.
pub(crate) fn continuation_started(daemon: &mut DaemonLoop, start: &ContinuationStart) {
    if start.records_completion {
        let ended = {
            let PlaybackOutput::EmbySession(output) = &daemon.owner.output else {
                return;
            };
            output
                .loaded
                .as_ref()
                .map(|loaded| loaded.slot)
                .filter(|ended| daemon.owner.core.queue.slot(*ended).is_some())
        };
        if let Some(ended) = ended {
            crate::output::advance::record_natural_end(daemon, ended, true);
        }
    }
    let PlaybackOutput::EmbySession(output) = &mut daemon.owner.output else {
        return;
    };
    submit_lookup(output, &daemon.merged_tx, &start.item_id);
}

/// Submits the one lookup for the item the device started at the queue's end
/// (design D4): the job runs on the output's worker and returns the fetched
/// item as an `EmbySession` event. A worker that is gone logs the failure and
/// drops the pending continuation.
fn submit_lookup(
    output: &mut EmbySessionOutput,
    merged_tx: &mpsc::Sender<DaemonEvent>,
    item_id: &str,
) {
    let job_item_id = item_id.to_owned();
    let merged_tx = merged_tx.clone();
    let job: EmbySessionJob = Box::new(move |api: &dyn SessionApi| {
        let result = api.get_item(&job_item_id);
        let _ = merged_tx.send(DaemonEvent::EmbySession(EmbySessionEvent::ItemLookup {
            item_id: job_item_id,
            result: result.map(Box::new),
        }));
    });
    if output.worker.send(job).is_err() {
        output.continuation = None;
        tracing::warn!(
            name: "daemon.emby_session_adopt.worker_gone",
            target: "daemon",
            item = %item_id,
            "the Emby Session worker is not running; the queue-end item was not adopted"
        );
    }
}

/// Handles the lookup's result (design D4): the item is appended at the
/// queue's tail and made the active slot and the tracked item only while the
/// Session still presents it. A failed lookup, or a device that moved on,
/// leaves the item presented and not adopted, and the failure is logged.
/// Returns whether the canonical owner queue changed.
pub(crate) fn handle_lookup(
    daemon: &mut DaemonLoop,
    item_id: &str,
    result: Result<Box<EmbyItem>, String>,
) -> bool {
    let (pending, still_presented) = {
        let PlaybackOutput::EmbySession(output) = &mut daemon.owner.output else {
            return false;
        };
        let pending = matches!(
            &output.continuation,
            Some(ContinuationWatch::Lookup { item_id: pending }) if pending == item_id
        );
        let still_presented = pending && presents_item(output.last_observation.as_ref(), item_id);
        (pending, still_presented)
    };
    let item = match result {
        Ok(item) => item,
        Err(error) => {
            if pending && let PlaybackOutput::EmbySession(output) = &mut daemon.owner.output {
                output.continuation = None;
            }
            tracing::warn!(
                name: "daemon.emby_session_adopt.lookup_failed",
                target: "daemon",
                item = %item_id,
                error = %error,
                "the queue-end item's lookup failed; it stays presented and not adopted"
            );
            return false;
        }
    };
    if !pending {
        tracing::debug!(
            name: "daemon.emby_session_adopt.stale_result",
            target: "daemon",
            item = %item_id,
            "the lookup's result names no pending queue-end continuation; nothing was adopted"
        );
        return false;
    }
    if !still_presented {
        if let PlaybackOutput::EmbySession(output) = &mut daemon.owner.output {
            output.continuation = None;
        }
        tracing::info!(
            name: "daemon.emby_session_adopt.moved_on",
            target: "daemon",
            item = %item_id,
            "the device moved on before the queue-end item's lookup returned; it stays presented and not adopted"
        );
        return false;
    }
    // The adopted item is the queue's next slot: the append and the active
    // marker go through the owner's queue mutation path.
    let slot = daemon.owner.core.queue.append(QueueItem::Emby(item));
    // The device plays the adopted item now, so it is the observed active
    // slot: a following Next steps from it and Clients highlight its row.
    daemon.owner.core.note_observed_active_slot(Some(slot));
    let runtime_ticks = daemon
        .owner
        .core
        .queue
        .slot(slot)
        .and_then(|slot| slot.item.as_emby())
        .map_or(0, |item| item.runtime_ticks);
    let PlaybackOutput::EmbySession(output) = &mut daemon.owner.output else {
        return false;
    };
    output.loaded = Some(EmbySessionLoadedItem {
        slot,
        item_id: item_id.to_owned(),
        runtime_ticks,
        loaded_at: Instant::now(),
        // The Session presents the item now: the 30 s failure timeout has
        // nothing to expire.
        seen_playing: true,
        end_handled: false,
        failure_reported: false,
    });
    // The device holds only the adopted item (design D4): the window and
    // the ended item's advance and owed pause go with the adoption.
    output.continuation = None;
    output.queued_next = None;
    output.advance = None;
    output.pending_pause = false;
    daemon.shared_queue.publish_observed(&daemon.owner.core);
    daemon.broadcast_owner_queue_state();
    crate::cast::broadcast_output_change(daemon);
    tracing::info!(
        name: "daemon.emby_session_adopt.adopted",
        target: "daemon",
        item = %item_id,
        slot = ?slot,
        "the device's queue-end autoplay was adopted as the queue's next slot"
    );
    true
}
