//! Where a queue item's start position comes from: one per-kind decision
//! (`ResumeSource`), the value each source yields, and the Emby refresh that
//! makes the server's position current before a start is decided.
//!
//! Every resume decision reads `resume_source` and matches on its result, never
//! on a `QueueItem` variant (resume-seam design D2).

use std::thread;
use std::time::Duration;

use mbv_emby::EmbyError;
use mbv_emby_model::{EmbyItem, seconds_to_ticks, ticks_to_seconds};
use mbv_queue::{AudiobookshelfItem, PlaybackQueue, QueueItem, QueueSlotId};

/// Which authority an item kind reads its start position from (resume-seam
/// design D2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResumeSource {
    /// Emby video: the position Emby reports when the start is decided.
    Server,
    /// Audiobookshelf: the position of the opened playback session.
    Session,
    /// Feed: the locally recorded position.
    Local,
    /// Emby audio: always the beginning.
    Beginning,
}

/// The resume source for `item`'s kind.
///
/// Every `QueueItem` kind has its own arm and there is no wildcard, so a new
/// kind fails to compile until it decides where its position comes from.
pub(crate) fn resume_source(item: &QueueItem) -> ResumeSource {
    match item {
        QueueItem::Emby(emby) => {
            if emby.is_audio() {
                ResumeSource::Beginning
            } else {
                ResumeSource::Server
            }
        }
        QueueItem::Feed(_) => ResumeSource::Local,
        QueueItem::Audiobookshelf(AudiobookshelfItem::Episode(_) | AudiobookshelfItem::Book(_)) => {
            ResumeSource::Session
        }
    }
}

/// The start position under the one-percent rule, in seconds: `position_ticks`
/// when it qualifies as resumable, otherwise the beginning.
fn resume_seconds(position_ticks: i64, runtime_ticks: i64) -> f64 {
    if mbv_emby_model::should_resume(position_ticks, runtime_ticks) {
        ticks_to_seconds(position_ticks)
    } else {
        0.0
    }
}

/// The resume position mpv should start `item` from, per the same per-kind
/// gate `mpv_load_opts` bakes into a fresh `loadfile`'s `start=` option.
/// Reused by the daemon's slot-jump dispatch to re-seek a re-visited entry —
/// `loadfile`'s baked `start=` only applies the first time an entry loads, so
/// a later `playlist-pos` jump back to it needs this recomputed explicitly.
pub(crate) fn resume_start_pos(item: &QueueItem) -> f64 {
    match resume_source(item) {
        // Emby video reads the position Emby reports; Audiobookshelf reads the
        // position of the opened playback session. Both pass the same gate.
        ResumeSource::Server | ResumeSource::Session => {
            resume_seconds(item.playback_position_ticks(), item.runtime_ticks())
        }
        // Feed keeps the position recorded locally, gated the same way.
        ResumeSource::Local => {
            let runtime = i64::try_from(item.duration().unwrap_or(0)).unwrap_or(i64::MAX);
            resume_seconds(item.playback_position_ticks(), runtime)
        }
        // Emby audio always starts from the beginning.
        ResumeSource::Beginning => 0.0,
    }
}

/// `resume_start_pos`, in ticks rather than seconds, gated on being positive.
/// `None` when the item should not resume.
#[must_use]
pub fn resume_ticks_for_item(item: &QueueItem) -> Option<i64> {
    let seconds = resume_start_pos(item);
    (seconds > 0.0).then_some(seconds_to_ticks(seconds))
}

/// The resume position, in ticks, for a slot-jump target — evaluated against
/// the canonical queue's current item for `slot_id`, so progress recorded
/// since the queue was submitted is honored. `None` when the slot is gone or
/// the item should not resume.
#[must_use]
pub fn resume_ticks_for_slot(queue: &PlaybackQueue, slot_id: QueueSlotId) -> Option<i64> {
    resume_ticks_for_item(&queue.slot(slot_id)?.item)
}

/// Retry delays for the Emby resume-position fetch (design D2): two delays
/// after the initial attempt, i.e. three attempts in total.
pub(crate) const EMBY_RESUME_RETRY_DELAYS: [Duration; 2] =
    [Duration::from_millis(500), Duration::from_secs(2)];

/// Overwrite each Emby video item's `playback_position_ticks` and `played`
/// with what Emby reports now (design D1), so every start-position decision
/// reads the server's value rather than a locally remembered one.
///
/// One batched fetch covers every distinct Emby video id; an `Ok` response
/// that omits a requested id counts as a failed attempt for that id only, and
/// only still-missing ids are retried. There are at most `delays.len() + 1`
/// attempts. Emby audio ids are never fetched (they always start from the
/// beginning), and feed and Audiobookshelf items pass through untouched. An id
/// that never arrives after the final attempt starts from the beginning
/// (`0`, unplayed).
pub(crate) fn refresh_emby_resume(
    items: Vec<QueueItem>,
    mut fetch: impl FnMut(&[String]) -> Result<Vec<EmbyItem>, EmbyError>,
    delays: &[Duration],
) -> Vec<QueueItem> {
    let mut seen = std::collections::HashSet::new();
    let mut missing: Vec<String> = Vec::new();
    for item in &items {
        if let QueueItem::Emby(emby) = item
            && !emby.is_audio()
            && seen.insert(emby.id.clone())
        {
            missing.push(emby.id.clone());
        }
    }

    let mut refreshed: std::collections::HashMap<String, EmbyItem> =
        std::collections::HashMap::new();
    for delay in std::iter::once(None).chain(delays.iter().map(Some)) {
        if missing.is_empty() {
            break;
        }
        if let Some(delay) = delay {
            thread::sleep(*delay);
        }
        let Ok(fetched) = fetch(&missing) else {
            continue;
        };
        for item in fetched {
            refreshed.insert(item.id.clone(), item);
        }
        missing.retain(|id| !refreshed.contains_key(id));
    }
    if !missing.is_empty() {
        tracing::warn!(name: "player.emby_resume.refresh_failed", target: "player", item_count = missing.len(), "Emby resume position unavailable after retries; starting those items from the beginning");
    }

    items
        .into_iter()
        .map(|item| match item {
            QueueItem::Emby(mut emby) if !emby.is_audio() => {
                if let Some(server) = refreshed.get(&emby.id) {
                    emby.playback_position_ticks = server.playback_position_ticks;
                    emby.played = server.played;
                } else {
                    emby.playback_position_ticks = 0;
                    emby.played = false;
                }
                QueueItem::Emby(emby)
            }
            other => other,
        })
        .collect()
}
