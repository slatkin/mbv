//! The Emby Session status projection (emby-session-owner-output design D10,
//! task 5.1). While a Session is the output the Owner is the presenter: it
//! translates the last observation into its `PlayerStatus` — the D6 position
//! estimate, the pause, volume, and mute state, the track lists and their
//! selected ids, the video height, and the item's title fields — so Clients
//! present the Session in the same form as local playback. The daemon's status
//! broadcast thread (`run.rs` `spawn_status_broadcast`) publishes that status
//! as `StatusOnly` at the local broadcast cadence, and the daemon tick
//! re-derives it, so the position extrapolates while the Session plays.
//!
//! Title fields come from the queue item when the Session presents the loaded
//! item, and from the observation for content the Owner did not load: the
//! Owner never invents a queue identity for another app's playback.

use std::time::Instant;

use mbv_ctrl::player::PlayerStatus;
use mbv_emby::{SessionInfo, SessionMediaInfo};
use mbv_queue::PlaybackQueue;

use super::{EmbySessionLoadedItem, EmbySessionOutput, presents_item};
use crate::DaemonLoop;
use crate::cast::PlaybackOutput;
use crate::output::not_presenting;

/// A Session's own volume scale (design D10): the device reports and takes a
/// 0–100 volume, unlike the local player's overshooting mpv volume.
pub(crate) const EMBY_SESSION_VOLUME_MAX: i64 = 100;

/// Re-derives the Owner's presented `PlayerStatus` from the Session output's
/// observation and installs it as the player's shared status (design D10). The
/// status-broadcast thread publishes it to Clients at the local cadence, and
/// the tick calls this with its injected clock, so the presented position
/// advances while the Session plays.
pub(crate) fn refresh_presentation(daemon: &mut DaemonLoop, now: Instant) {
    let PlaybackOutput::EmbySession(output) = &daemon.owner.output else {
        return;
    };
    let mut status = daemon.player.status.lock().unwrap();
    *status = presented_player_status(&status, &daemon.owner.core.queue, output, now);
}

/// The Owner's presented `PlayerStatus` for the Session output's current
/// observation (design D10). An observation that presents the loaded item is
/// presented as that slot — the queue item's title fields, the slot's runtime,
/// and the Session's position, pause, volume, and track state. Content the
/// Owner did not load is presented by the observation alone, with no queue
/// identity and with the queue's own active marker unchanged. An idle Session,
/// or no observation at all, presents no active playback.
pub(crate) fn presented_player_status(
    base: &PlayerStatus,
    queue: &PlaybackQueue,
    output: &EmbySessionOutput,
    now: Instant,
) -> PlayerStatus {
    let Some(observed) = presented_observation(output) else {
        return not_presenting(base, queue);
    };
    let mut presented = base.clone();
    if let Some((slot_id, slot)) = presented_loaded_slot(queue, output, observed) {
        present_loaded_slot(&mut presented, queue, output, slot_id, slot);
    } else {
        present_foreign_content(&mut presented, queue, observed);
    }
    presented.position_ticks = output.estimate_ticks(now);
    presented.paused = observed.is_paused;
    presented.volume = observed.volume;
    presented.volume_max = EMBY_SESSION_VOLUME_MAX;
    presented.muted = observed.muted;
    apply_tracks(&mut presented, observed);
    presented.active = true;
    presented
}

/// Presents the loaded item as the queue's own slot (design D10): the queue
/// item's title fields and occurrence index, held to the runtime the end rule
/// and the position estimate use.
fn present_loaded_slot(
    presented: &mut PlayerStatus,
    queue: &PlaybackQueue,
    output: &EmbySessionOutput,
    slot_id: mbv_queue::QueueSlotId,
    slot: &mbv_queue::QueueSlot,
) {
    let index = queue.slot_index(slot_id).unwrap_or(0);
    presented.seed_from_item(&slot.item, index, queue.len());
    presented.runtime_ticks = output.presented_runtime_ticks();
}

/// Presents content the Owner did not load by the observation alone (design
/// D10): its own name and runtime, no queue item identity, and the queue's own
/// active marker left where the Owner set it.
fn present_foreign_content(
    presented: &mut PlayerStatus,
    queue: &PlaybackQueue,
    observed: &SessionInfo,
) {
    presented.clear_current_item_metadata();
    presented.title = observed.now_playing.clone().unwrap_or_default();
    presented.current_idx = queue.active_index().unwrap_or(0);
    presented.queue_len = queue.len();
    presented.runtime_ticks = observed.runtime_ticks;
}

/// The observation the Owner presents (design D10): the last `/Sessions` entry
/// of this device, when it presents an item. An idle Session presents nothing,
/// and the end rule reads the same absence (design D6).
fn presented_observation(output: &EmbySessionOutput) -> Option<&SessionInfo> {
    output
        .last_observation
        .as_ref()
        .filter(|observed| observed.now_playing_item_id.is_some())
}

/// The loaded slot this observation presents and its queue slot (design D6,
/// D10): the queue row the presented title fields come from. A slot edited out
/// of the queue presents from the observation instead, like foreign content —
/// the queue cannot name an occurrence it no longer holds.
///
/// Whether the Session currently presents the item the Owner loaded and that
/// item is still the queue's active slot (queue-content-shuffle): the Shuffle
/// pins the active slot first only then. Foreign content — media the Owner did
/// not load — names no queue slot and never pins (PR #957 review).
pub(crate) fn presents_active_queue_slot(
    session: &EmbySessionOutput,
    queue: &PlaybackQueue,
) -> bool {
    let Some(loaded) = session.loaded.as_ref() else {
        return false;
    };
    Some(loaded.slot) == queue.active_slot_id()
        && presents_item(presented_observation(session), &loaded.item_id)
}

fn presented_loaded_slot<'a>(
    queue: &'a PlaybackQueue,
    output: &'a EmbySessionOutput,
    observed: &SessionInfo,
) -> Option<(mbv_queue::QueueSlotId, &'a mbv_queue::QueueSlot)> {
    let loaded: &EmbySessionLoadedItem = output
        .loaded
        .as_ref()
        .filter(|loaded| presents_item(Some(observed), &loaded.item_id))?;
    queue.slot(loaded.slot).map(|slot| (loaded.slot, slot))
}

/// The Session's stream lists and selected indexes as the shared track model
/// (design D10): each Emby stream index is the id the Owner's own track
/// commands address it by, and Emby's −1 "no subtitle" is the wire 0 (off).
fn apply_tracks(presented: &mut PlayerStatus, observed: &SessionInfo) {
    let media = &observed.media_info;
    presented.audio_tracks = media
        .audio_streams
        .iter()
        .map(|stream| (stream.index, stream_label(&stream.label, &stream.language)))
        .collect();
    presented.sub_tracks = media
        .subtitle_streams
        .iter()
        .map(|stream| {
            (
                stream.index,
                stream_label(&stream.label, &stream.language),
                stream.forced,
            )
        })
        .collect();
    presented.audio_id = observed.audio_index;
    presented.sub_id = subtitle_id(observed.sub_index);
    presented.audio_lang = selected_language(
        media
            .audio_streams
            .iter()
            .find(|stream| stream.index == observed.audio_index)
            .map(|stream| stream.language.as_str()),
    );
    presented.sub_lang = selected_language(
        media
            .subtitle_streams
            .iter()
            .find(|stream| stream.index == observed.sub_index)
            .map(|stream| stream.language.as_str()),
    );
    presented.video_height = video_height(media);
}

/// The wire subtitle id of an Emby subtitle index (design D9, D10): Emby
/// spells "no subtitle" as −1, the track model as 0.
fn subtitle_id(sub_index: i64) -> i64 {
    if sub_index < 0 { 0 } else { sub_index }
}

/// One track's label: the Session's own label, or its language code when the
/// payload carried no label.
fn stream_label(label: &str, language: &str) -> String {
    if label.is_empty() {
        language.to_string()
    } else {
        label.to_string()
    }
}

/// The raw language code of the selected stream, which the wire model leaves
/// empty for a selection that is off or unknown.
fn selected_language(language: Option<&str>) -> String {
    language.unwrap_or_default().to_string()
}

/// The video height the Session's media label carries (design D10): the
/// label's first token is a resolution — `1080p`, `720p`, `4K` — and a
/// Session that plays audio only presents no video.
fn video_height(media: &SessionMediaInfo) -> i64 {
    if media.audio_only {
        return 0;
    }
    media
        .video_label
        .split_whitespace()
        .next()
        .map_or(0, parse_height_label)
}

/// The pixel height one resolution token names: `4K` is 2160, and every other
/// token drops its `p` and parses. An unknown token presents no height.
fn parse_height_label(token: &str) -> i64 {
    if token.eq_ignore_ascii_case("4k") {
        return 2160;
    }
    token
        .strip_suffix(['p', 'P'])
        .unwrap_or(token)
        .parse()
        .unwrap_or(0)
}
