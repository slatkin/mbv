use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc, Mutex,
};
use std::thread;
use std::time::{Duration, Instant};
use std::{
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
};

use crate::api::{EmbyClient, EmbyItem, TICKS_PER_SECOND};
use crate::id_types::{EmbySessionId, ItemId, MediaSourceId};
use crate::playback_execution_sequence::{ExecSlot, ExecutionSequence};
#[cfg(test)]
use crate::playback_queue::QueueMutationResult;
use crate::playback_queue::{PlaybackQueue, QueueItem, QueueSlotId};
use libmpv2::{
    events::{Event, PropertyData},
    mpv_end_file_reason, EndFileReason, Format, Mpv,
};

fn mpv_err_str(e: &libmpv2::Error) -> String {
    if let libmpv2::Error::Raw(code) = e {
        format!("Raw({}) [{}]", code, libmpv2_sys::mpv_error_str(*code))
    } else {
        format!("{e:?}")
    }
}

fn mpv_title_opt(title: &str) -> String {
    // Use mpv's %N% length-prefix format so the value is passed verbatim —
    // no escaping needed, handles commas, backslashes, and any other character.
    format!("force-media-title=%{}%{}", title.len(), title)
}

/// The resume position mpv should start `item` from, per the same per-kind
/// gate `mpv_load_opts` bakes into a fresh `loadfile`'s `start=` option.
/// Reused by the daemon's slot-jump dispatch to re-seek a re-visited entry —
/// `loadfile`'s baked `start=` only applies the first time an entry loads, so
/// a later `playlist-pos` jump back to it needs this recomputed explicitly.
pub(crate) fn resume_start_pos(item: &QueueItem) -> f64 {
    match item {
        QueueItem::Emby(emby) if !emby.is_audio() && emby.should_resume() => emby.resume_seconds(),
        QueueItem::Emby(_) => 0.0,
        QueueItem::Feed(entry) => {
            let runtime = entry.duration_ticks.unwrap_or(0) as i64;
            if crate::api::should_resume(entry.position_ticks, runtime) {
                entry.position_ticks as f64 / crate::api::TICKS_PER_SECOND as f64
            } else {
                0.0
            }
        }
        QueueItem::Audiobookshelf(ep) => {
            let runtime = ep.duration_ticks.unwrap_or(0) as i64;
            if crate::api::should_resume(ep.position_ticks, runtime) {
                ep.position_ticks as f64 / crate::api::TICKS_PER_SECOND as f64
            } else {
                0.0
            }
        }
        QueueItem::AudiobookshelfBook(book) => {
            let runtime = book.duration_ticks.unwrap_or(0) as i64;
            if crate::api::should_resume(book.position_ticks, runtime) {
                book.position_ticks as f64 / crate::api::TICKS_PER_SECOND as f64
            } else {
                0.0
            }
        }
    }
}

/// `resume_start_pos`, in ticks rather than seconds, gated on being positive.
/// `None` when the item should not resume.
pub fn resume_ticks_for_item(item: &QueueItem) -> Option<i64> {
    let seconds = resume_start_pos(item);
    (seconds > 0.0).then_some((seconds * TICKS_PER_SECOND as f64) as i64)
}

/// The resume position, in ticks, for a slot-jump target — evaluated against
/// the canonical queue's current item for `slot_id`, so progress recorded
/// since the queue was submitted is honored. `None` when the slot is gone or
/// the item should not resume.
pub fn resume_ticks_for_slot(queue: &PlaybackQueue, slot_id: QueueSlotId) -> Option<i64> {
    resume_ticks_for_item(&queue.slot(slot_id)?.item)
}

fn mpv_load_opts(item: &QueueItem) -> String {
    let title = mpv_title_opt(&item.display_name());
    let start = resume_start_pos(item);
    if start > 0.0 {
        format!("{title},start={start}")
    } else {
        title
    }
}

fn queue_load_indices(len: usize, start_idx: usize) -> impl Iterator<Item = usize> {
    std::iter::once(start_idx)
        .chain(0..start_idx)
        .chain(start_idx + 1..len)
}

/// mpv loadfile mode for each queue load, design D3 load-then-play: the start
/// slot loads first as a no-play `append` (mpv never starts playback for
/// `append`), earlier slots are `insert-at` before it, later slots `append`
/// after it. No load in the plan starts playback — `start_queue_playback`
/// does that once the whole playlist is built.
fn queue_load_location(index: usize, start_idx: usize) -> (&'static str, String) {
    if index < start_idx {
        ("insert-at", index.to_string())
    } else {
        ("append", "-1".to_string())
    }
}

/// mpv loadfile mode for the cold active-file (Audiobookshelf) single load in
/// submit: that branch loads exactly one slot and skips
/// `start_queue_playback`, so the load itself must start playback — `replace`
/// on an empty idle playlist does. The D3 no-play plan modes never would.
fn active_file_load_location() -> (&'static str, String) {
    ("replace", "-1".to_string())
}

/// Start playback at `start_idx` after the no-play queue loads (design D3).
///
/// Set the deferred-start ordinal and select the target after the no-play
/// loads. `playlist-play-index` is required as well as the position writes:
/// `playlist-pos` can be a no-op when mpv already points at the requested
/// ordinal (commonly entry zero), leaving a freshly loaded playlist idle.
/// An armed audio-pipe startup pause stays in force until PlaybackRestart.
fn start_queue_playback(mpv: &Mpv, start_idx: usize) {
    // A position write can be a no-op on an already-selected idle entry, so
    // issue mpv's explicit play command rather than inferring playback from it.
    warn_on_set_property(
        mpv,
        "start_queue_playback",
        "playlist-start",
        start_idx as i64,
    );
    warn_on_set_property(
        mpv,
        "start_queue_playback",
        "playlist-pos",
        start_idx as i64,
    );
    if let Err(error) = mpv.command("playlist-play-index", &[&start_idx.to_string()]) {
        log::warn!(target: "player", "start_queue_playback playlist-play-index={start_idx} failed: {}", mpv_err_str(&error));
    }
}

fn warn_on_set_property(mpv: &Mpv, caller: &str, name: &str, value: i64) {
    if let Err(e) = mpv.set_property(name, value) {
        log::warn!(
            target: "player",
            "{caller} {name}={value} failed: {}",
            mpv_err_str(&e),
        );
    }
}

/// What the initial queue layout verification found in mpv's playlist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QueueLayoutVerdict {
    /// Every item present, active one playing.
    Ok,
    /// The playlist holds every item but mpv is on another entry: reassert
    /// the ordinal the run's `current_idx` (and every reported item) is
    /// derived from.
    Reassert,
    /// The playlist is missing entries, so an ordinal no longer names the
    /// item it should: report instead of repairing by position.
    ShortLayout,
}

fn queue_layout_verdict(
    start_idx: usize,
    item_count: usize,
    mpv_pos: i64,
    mpv_count: i64,
    mpv_idle: bool,
) -> QueueLayoutVerdict {
    if mpv_count != item_count as i64 {
        QueueLayoutVerdict::ShortLayout
    } else if mpv_pos == start_idx as i64 && !mpv_idle {
        QueueLayoutVerdict::Ok
    } else {
        QueueLayoutVerdict::Reassert
    }
}

/// mpv's playlist ordinal, when it names an entry that is not the run's
/// active one. `None` when mpv has no entry (`-1`), the ordinal is out of
/// range, or it is where the run already believes playback is. The caller
/// gates the `active_file` projection, whose one-entry playlist never carries
/// the run's ordinal.
fn divergent_entry(pos: i64, current_idx: usize, queue_len: usize) -> Option<usize> {
    if pos < 0 {
        return None;
    }
    let index = pos as usize;
    (index < queue_len && index != current_idx).then_some(index)
}

/// mpv's position inside the entry it is playing, in ticks.
fn mpv_position_ticks(mpv: &Mpv) -> i64 {
    (mpv.get_property::<f64>("time-pos").unwrap_or(0.0) * TICKS_PER_SECOND as f64) as i64
}

/// Verify — and if needed reassert — the playlist projection the load
/// sequence above is supposed to have built: every item present, in `items`
/// order, with the active one playing at `start_idx`.
///
/// The run's `current_idx` and mpv's `playlist-pos` are one coordinate:
/// `on_playlist_pos_changed` maps mpv's index straight back to a queue slot.
/// If mpv finishes the layout on a different entry, that coordinate is
/// silently wrong, and the `playlist-pos` events that would report it are
/// suppressed as transient (`pending_initial_playlist_layout`) until the
/// layout settles, so nothing else ever re-derives it: reports keep naming the
/// requested item while mpv streams another one.
///
/// Called after the loads and `start_queue_playback` as a safety net. A
/// complete, already-playing target observes `Ok`; if mpv still reports idle,
/// the explicit play command is retried. A layout that is short an entry is
/// reported rather than repaired, because a missing
/// entry means the ordinal no longer names the item we think it does.
fn reassert_queue_layout(mpv: &Mpv, start_idx: usize, item_count: usize) {
    let mpv_count = mpv.get_property::<i64>("playlist-count").unwrap_or(-1);
    let mpv_pos = mpv.get_property::<i64>("playlist-pos").unwrap_or(-1);
    let mpv_idle = mpv.get_property::<bool>("idle-active").unwrap_or(false);
    match queue_layout_verdict(start_idx, item_count, mpv_pos, mpv_count, mpv_idle) {
        QueueLayoutVerdict::Ok => {}
        QueueLayoutVerdict::Reassert => {
            log::warn!(
                target: "player",
                "queue layout mismatch: start_idx={start_idx} items={item_count} \
                 mpv_pos={mpv_pos} mpv_count={mpv_count}; reasserting the active ordinal",
            );
            if mpv_idle {
                if let Err(error) = mpv.command("playlist-play-index", &[&start_idx.to_string()]) {
                    log::warn!(target: "player", "queue layout repair playlist-play-index={start_idx} failed: {}", mpv_err_str(&error));
                }
            } else {
                let _ = mpv.set_property("playlist-pos", start_idx as i64);
                let _ = mpv.set_property("pause", false);
            }
        }
        QueueLayoutVerdict::ShortLayout => {
            log::error!(
                target: "player",
                "queue layout incomplete: start_idx={start_idx} items={item_count} \
                 mpv_pos={mpv_pos} mpv_count={mpv_count}",
            );
        }
    }
}

fn send_ep_info(mpv: &Mpv, item: &crate::api::EmbyItem) {
    let val =
        if item.item_type == "Episode" && item.parent_index_number > 0 && item.index_number > 0 {
            format!(
                "Season {}  Episode {}",
                item.parent_index_number, item.index_number
            )
        } else {
            String::new()
        };
    let _ = mpv.set_property("user-data/mbv/ep-tag", val.as_str());
}

pub mod owner_state;
pub use owner_state::*;
mod types;
pub use types::*;
mod sources;
pub use sources::*;
mod runtime;
pub use runtime::*;
mod report_worker;
pub use report_worker::*;
mod reporting;
pub use reporting::*;
mod run;
pub use run::*;
// `run/` keeps the hot-loop files physically grouped while the controller and
// submission concerns remain sibling modules.
mod controller;
mod submit;
pub use controller::*;
mod proxy;
pub use proxy::*;

#[cfg(test)]
mod tests;
