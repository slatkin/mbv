mod error;
pub use error::PlayerError;

use std::sync::{Arc, atomic::Ordering, mpsc};
use std::thread;
use std::time::{Duration, Instant};
use std::{
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
};

use libmpv2::{
    EndFileReason, Format, Mpv,
    events::{Event, PropertyData},
    mpv_end_file_reason,
};
use mbv_ctrl::player::{PlayerCommand, PlayerEvent, PlayerStatus, SubtitlePrefs};
use mbv_emby::{EmbyClient, EmbyError};
use mbv_emby_model::{EmbyItem, TICKS_PER_SECOND, seconds_to_ticks, ticks_to_seconds};
#[cfg(test)]
use mbv_queue::QueueMutationResult;
use mbv_queue::{AudiobookshelfItem, PlaybackQueue, QueueItem, QueueSlotId};
use mbv_queue::{ExecSlot, ExecutionSequence};

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
            let runtime = i64::try_from(entry.duration_ticks.unwrap_or(0)).unwrap_or(i64::MAX);
            if mbv_emby_model::should_resume(entry.position_ticks, runtime) {
                ticks_to_seconds(entry.position_ticks)
            } else {
                0.0
            }
        }
        QueueItem::Audiobookshelf(AudiobookshelfItem::Episode(item)) => item.resume_seconds(),
        QueueItem::Audiobookshelf(AudiobookshelfItem::Book(item)) => item.resume_seconds(),
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
/// An armed audio-pipe startup pause stays in force until `PlaybackRestart`.
fn start_queue_playback(mpv: &Mpv, start_idx: usize) {
    // A position write can be a no-op on an already-selected idle entry, so
    // issue mpv's explicit play command rather than inferring playback from it.
    warn_on_set_property(
        mpv,
        "start_queue_playback",
        "playlist-start",
        i64::try_from(start_idx).unwrap_or(i64::MAX),
    );
    warn_on_set_property(
        mpv,
        "start_queue_playback",
        "playlist-pos",
        i64::try_from(start_idx).unwrap_or(i64::MAX),
    );
    if let Err(error) = mpv.command("playlist-play-index", &[&start_idx.to_string()]) {
        tracing::warn!(name: "player.queue_playback.start_failed", target: "player", index = start_idx, error = %mpv_err_str(&error), "failed to start playlist index");
    }
}

fn warn_on_set_property(mpv: &Mpv, caller: &str, name: &str, value: i64) {
    if let Err(e) = mpv.set_property(name, value) {
        tracing::warn!(name: "player.property.set_failed", target: "player", caller, property = name, value, error = %mpv_err_str(&e), "failed to set mpv property");
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
    if usize::try_from(mpv_count).ok() != Some(item_count) {
        QueueLayoutVerdict::ShortLayout
    } else if usize::try_from(mpv_pos).ok() == Some(start_idx) && !mpv_idle {
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
    let index = usize::try_from(pos).ok()?;
    (index < queue_len && index != current_idx).then_some(index)
}

/// mpv's position inside the entry it is playing, in ticks.
fn mpv_position_ticks(mpv: &Mpv) -> i64 {
    seconds_to_ticks(mpv.get_property::<f64>("time-pos").unwrap_or(0.0))
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
            tracing::warn!(name: "player.queue_layout.mismatch", target: "player", start_index = start_idx, item_count, mpv_position = mpv_pos, mpv_count, "reasserting the active ordinal");
            if mpv_idle {
                if let Err(error) = mpv.command("playlist-play-index", &[&start_idx.to_string()]) {
                    tracing::warn!(name: "player.queue_layout.repair_failed", target: "player", index = start_idx, error = %mpv_err_str(&error), "failed to repair playlist position");
                }
            } else {
                let _ =
                    mpv.set_property("playlist-pos", i64::try_from(start_idx).unwrap_or(i64::MAX));
                let _ = mpv.set_property("pause", false);
            }
        }
        QueueLayoutVerdict::ShortLayout => {
            tracing::error!(name: "player.queue_layout.incomplete", target: "player", start_index = start_idx, item_count, mpv_position = mpv_pos, mpv_count, "queue layout is incomplete");
        }
    }
}

fn send_ep_info(mpv: &Mpv, item: &mbv_emby_model::EmbyItem) {
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
pub mod transition;
pub use owner_state::{PlayerOwnerState, StepTarget};
mod tracks;
#[cfg(test)]
use tracks::lang_code_to_name;
use tracks::{auto_select_tracks, refresh_tracks};
mod sources;
pub use sources::{
    AudiobookshelfBookProgressUpdate, AudiobookshelfPlayerContext, AudiobookshelfProgressUpdate,
};
pub(crate) use sources::{PreparedSource, prepare_source};
mod runtime;
pub(crate) use runtime::{
    MpvRunConfig, ProgressGuard, handle_intro, init_mpv, init_volume, observe_properties,
    shift_index_for_move, spawn_progress_reporter,
};
mod report_worker;
use report_worker::{ReportJob, SessionReporter};
mod reporting;
pub(crate) use reporting::{
    ActiveItemLifecycle, AudiobookshelfBookPlaybackLifecycle, AudiobookshelfPlaybackLifecycle,
    PreparedLifecycle,
};
mod run;
#[cfg(test)]
pub(crate) use run::{
    AdvanceDecisionInput, CompletedMedia, Drained, FinishReason, NextUp, NextUpDecision,
    NextUpFire, StopReport, active_item_state, advance_decision, is_clocked_audio_error,
    is_superseded_jump_end_file, provider_lifecycle_close_pos, queue_next_up_decision,
    reject_stale_jump, resolve_jump_target, seek_decision, standalone_next_up_decision,
    volume_decision,
};
pub(crate) use run::{IntroState, PlaybackOrigin, PlaybackRun, RunInit, mpv_url_for_queue_item};
// `run/` keeps the hot-loop files physically grouped while the controller and
// submission concerns remain sibling modules.
mod controller;
mod submit;
pub(crate) use controller::make_wakeup_pipe;
pub use controller::{Player, QuitHandle};
mod proxy;
pub use proxy::PlayerProxy;
pub(crate) use proxy::{
    StopReportContext, end_file_stop_report_context, is_near_end, queue_completed_pos,
    quit_timeout_stop_flags, retry_mark_played,
};

#[cfg(test)]
mod tests;
