use crate::app::components::media_list::{
    ActiveProgress, MediaKind, MediaListRow, MediaSemanticState,
};
use crate::app::state::types::playback::PlaybackState;
use crate::app::ui_util::fmt_duration_short;
use mbv_core::api::TICKS_PER_SECOND;
use mbv_core::playback_queue::{QueueItem, QueueSlot, QueueSlotId};

/// Project Queue slots into the canonical provider-neutral row vocabulary
/// (migrate-queue-to-canonical-list D2): a stable `QueueSlotId` target, the
/// slot title, duration/elapsed metadata, and semantic active state whose
/// progress is clamped to `0..=100` at this projection boundary. No ticks,
/// runtime, source, credentials, callbacks, or effects cross the child edge.
pub(in crate::app) fn queue_media_rows(
    slots: &[QueueSlot],
    playback: PlaybackState,
    pending_slot: Option<QueueSlotId>,
) -> Vec<MediaListRow<QueueSlotId>> {
    slots
        .iter()
        .enumerate()
        .map(|(index, slot)| queue_media_row_at(slot, index, playback, pending_slot))
        .collect()
}

pub(in crate::app) fn queue_media_row(
    slot: &QueueSlot,
    index: usize,
    playback: PlaybackState,
    pending_slot: Option<QueueSlotId>,
) -> MediaListRow<QueueSlotId> {
    queue_media_row_at(slot, index, playback, pending_slot)
}

fn queue_media_row_at(
    slot: &QueueSlot,
    index: usize,
    playback: PlaybackState,
    pending_slot: Option<QueueSlotId>,
) -> MediaListRow<QueueSlotId> {
    // A pending selection moves the now-playing state to the selected slot
    // before the owner confirms it: the outgoing row, still playing, drops the
    // highlight immediately rather than sharing it
    // (queue-canonical-list, "Selecting a different item to play").
    let superseded = pending_slot.is_some_and(|target| target != slot.slot_id);
    let is_active = playback.active && playback.active_idx == Some(index) && !superseded;
    let is_pending = pending_slot == Some(slot.slot_id) && !is_active;
    let (title, pos_ticks, duration_ticks) = queue_row_fields(&slot.item, playback, is_active);
    let semantic_state = if is_pending {
        MediaSemanticState::NowPlaying { progress: None }
    } else if is_active {
        let progress = (pos_ticks > 0 && duration_ticks > 0).then(|| {
            u16::try_from((pos_ticks * 100 / duration_ticks).clamp(0, 100)).unwrap_or(u16::MAX)
        });
        MediaSemanticState::NowPlaying {
            progress: progress.map(ActiveProgress::new),
        }
    } else {
        // The one canonical state derivation: a played slot paints the
        // shared played colour, an in-progress slot its resume percentage.
        MediaSemanticState::from_queue_item(&slot.item)
    };
    MediaListRow::Item {
        target: slot.slot_id,
        primary: title,
        secondary: None,
        trailing: None,
        // The now-playing row shows its total duration in every state: a
        // pending selection is not playing yet, but it still has a known
        // runtime, and blanking the slot reads as a glitch.
        duration: {
            let time_text = queue_row_time_text(pos_ticks, duration_ticks, false);
            (!time_text.is_empty()).then_some(time_text)
        },
        kind: MediaKind::Media,
        semantic_state,
    }
}

/// The title and (position, duration) ticks a Queue row paints, resolved per
/// item kind and overridden with live playback ticks for the active row.
fn queue_row_fields(
    item: &QueueItem,
    playback: PlaybackState,
    is_active: bool,
) -> (String, i64, i64) {
    // Every kind carries its stored resume position, and the active row's live
    // ticks win over it. Feeds and Audiobookshelf used to paint 0 on a
    // non-active row, so only the playing row ever showed progress.
    let stored_ticks = item.playback_position_ticks();
    match item {
        QueueItem::Emby(item) => {
            let (pos, runtime) = if is_active {
                (
                    if playback.position_ticks > 0 {
                        playback.position_ticks
                    } else {
                        stored_ticks
                    },
                    playback.runtime_ticks,
                )
            } else {
                (stored_ticks, item.runtime_ticks)
            };
            (item.name.clone(), pos, runtime)
        }
        QueueItem::Feed(entry) => (
            entry.title.clone(),
            if is_active {
                playback.position_ticks
            } else {
                stored_ticks
            },
            i64::try_from(entry.duration_ticks.unwrap_or(0)).unwrap_or(i64::MAX),
        ),
        QueueItem::Audiobookshelf(mbv_core::playback_queue::AudiobookshelfItem::Episode(ep)) => (
            ep.title.clone(),
            if is_active {
                playback.position_ticks
            } else {
                stored_ticks
            },
            i64::try_from(ep.duration_ticks.unwrap_or(0)).unwrap_or(i64::MAX),
        ),
        QueueItem::Audiobookshelf(mbv_core::playback_queue::AudiobookshelfItem::Book(book)) => (
            book.title.clone(),
            if is_active {
                playback.position_ticks
            } else {
                stored_ticks
            },
            i64::try_from(book.duration_ticks.unwrap_or(0)).unwrap_or(i64::MAX),
        ),
    }
}

fn queue_row_time_text(pos_ticks: i64, dur_ticks: i64, show_elapsed: bool) -> String {
    let dur_s = dur_ticks / TICKS_PER_SECOND;
    if dur_s <= 0 {
        return String::new();
    }
    if show_elapsed {
        format!(
            "{} / {}",
            fmt_duration_short(pos_ticks / TICKS_PER_SECOND),
            fmt_duration_short(dur_s)
        )
    } else {
        fmt_duration_short(dur_s)
    }
}
