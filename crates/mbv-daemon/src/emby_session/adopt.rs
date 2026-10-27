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

/// Drops the output's pending queue-end continuation (emby-session-output-
/// followups design D4): the three exits of a lookup that adopts nothing end
/// the same way, so they share this one teardown.
fn clear_continuation(daemon: &mut DaemonLoop) {
    if let PlaybackOutput::EmbySession(output) = &mut daemon.owner.output {
        output.continuation = None;
    }
}

/// Handles the lookup's result (design D4): the item is appended at the
/// queue's tail and made the active slot and the tracked item only while the
/// Session still presents it and the owner and Session admit it. A failed
/// lookup, a device that moved on, or an item admission refuses leaves the
/// item presented and not adopted, and the failure is logged.
/// Returns whether the canonical owner queue changed.
pub(crate) fn handle_lookup(
    daemon: &mut DaemonLoop,
    item_id: &str,
    result: Result<Box<EmbyItem>, String>,
) -> bool {
    let (pending, still_tracking, still_presented) = {
        let PlaybackOutput::EmbySession(output) = &mut daemon.owner.output else {
            return false;
        };
        let pending = matches!(
            &output.continuation,
            Some(ContinuationWatch::Lookup { item_id: pending }) if pending == item_id
        );
        // The adoption re-establishes the tracked item, so the output must
        // still track one: a user stop cleared it, and a stopped output
        // advances nothing (emby-session-owner-output design D9). The
        // pending continuation names the looked-up item, not the ended one,
        // so `loaded.is_some()` is the available check; `still_presented`
        // covers a device that moved on to other content.
        let still_tracking = output.loaded.is_some();
        let still_presented = presents_item(output.last_observation.as_ref(), item_id);
        (pending, still_tracking, still_presented)
    };
    let item = match result {
        Ok(item) => item,
        Err(error) => {
            if pending {
                clear_continuation(daemon);
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
    if !still_tracking {
        clear_continuation(daemon);
        tracing::info!(
            name: "daemon.emby_session_adopt.not_tracking",
            target: "daemon",
            item = %item_id,
            "the output no longer tracks the ended item the lookup continued; nothing was adopted"
        );
        return false;
    }
    if !still_presented {
        clear_continuation(daemon);
        tracing::info!(
            name: "daemon.emby_session_adopt.moved_on",
            target: "daemon",
            item = %item_id,
            "the device moved on before the queue-end item's lookup returned; it stays presented and not adopted"
        );
        return false;
    }
    // Owner admission is the capability boundary every queue-entry path runs,
    // the adoption included: the owner must admit the item (media kind and
    // Service availability, `daemon_admits`), and the Session's own
    // playable-media list judges whether the device can play it — the same
    // rule the queue's load path applies (emby-session-owner-output design
    // D7). An item that fails either check stays presented.
    let candidate = QueueItem::Emby(item);
    let rejection = {
        let PlaybackOutput::EmbySession(output) = &daemon.owner.output else {
            return false;
        };
        if crate::control_queue::daemon_admits(
            &candidate,
            daemon.audio_only,
            crate::control::has_emby(&daemon.client),
            daemon.audiobookshelf_runtime.is_some(),
        ) {
            super::load::admit(&candidate, &output.playable_media_types)
                .err()
                .map(|admission| admission.reason)
        } else {
            Some("the owner does not admit the item".to_string())
        }
    };
    if let Some(reason) = rejection {
        clear_continuation(daemon);
        tracing::info!(
            name: "daemon.emby_session_adopt.not_admitted",
            target: "daemon",
            item = %item_id,
            reason = %reason,
            "the queue-end item is not admitted; it stays presented and not adopted"
        );
        return false;
    }
    commit_adoption(daemon, item_id, candidate)
}

/// Commits the queue-end adoption (design D4): the item is appended at the
/// queue's tail through the owner's queue mutation path and becomes the
/// observed active slot — the device plays it now, so a following Next steps
/// from it and Clients highlight its row — and the tracked item. The ended
/// item's advance and owed pause go with the adoption.
fn commit_adoption(daemon: &mut DaemonLoop, item_id: &str, item: QueueItem) -> bool {
    // The output is matched before anything mutates: a non-Session output
    // appends no slot and records no observation.
    let PlaybackOutput::EmbySession(output) = &mut daemon.owner.output else {
        return false;
    };
    let runtime_ticks = item.as_emby().map_or(0, |item| item.runtime_ticks);
    let slot = daemon.owner.core.queue.append(item);
    daemon.owner.core.note_observed_active_slot(Some(slot));
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
    // The window and the ended item's advance and owed pause go with the
    // adoption. The queued next item is kept whatever the flight left: the
    // trigger cleared it, so a record here is one a queue edit re-established
    // with a `play_last` while the lookup ran — the device holds it, and
    // dropping the record would adopt it a second time at its end.
    output.continuation = None;
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
