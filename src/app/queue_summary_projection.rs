//! The Queue summary projection every Client consumes: the queue header's
//! remaining count and finish clock (design D4a's derived wall-clock facts).
//! The arithmetic is new projection work over held facts — the wall clock,
//! the displayed playback state's position and runtime, and the shown queue's
//! slot runtimes — so no datum crosses an Owner edge.
//!
//! The clocks are read per projection call, never adopted into row content:
//! a seek moves the anchor of every pending slot at once, and the row
//! projection's single-row patch would leave the other rows' clocks stale
//! until the next full row rebuild.

use std::time::{SystemTime, UNIX_EPOCH};

use mbv_emby_model::TICKS_PER_SECOND;
use mbv_queue::QueueSlot;
use mbv_ui_model::ui_util::fmt_wall_clock;
use time::{OffsetDateTime, UtcOffset};

use crate::app::App;

/// The Queue summary's facts for one read: the header's remaining count and
/// finish clock and the next item's title.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct QueueSummaryProjection {
    /// The slots that do not carry the playhead: the canvas's remaining
    /// count counts every non-current row, rows before the playhead
    /// included. The whole queue's slot count while no slot carries the
    /// playhead — idle playback, slotless output, or a playhead the shown
    /// queue does not know yet.
    pub remaining: usize,
    /// The title of the first slot after the playhead (review of
    /// add-tauri-gui-client row 6.6, finding P2): the compact strip's
    /// next-item line, read from the queue rather than from the frame's
    /// viewport rows, which need not hold the playhead. `None` while no slot
    /// carries the playhead or none follows it.
    pub next_title: Option<String>,
    /// The wall-clock time the shown queue finishes, formatted in the local
    /// zone: the active slot's remaining time plus every pending slot's
    /// runtime after it. `None` while no slot carries the playhead or a
    /// pending runtime in the chain is unknown.
    pub finish_clock: Option<String>,
    /// The queue filter's text (design D4): the header control's content,
    /// which narrows the projected rows locally — never the canonical queue.
    /// Empty text filters nothing.
    pub filter: String,
    /// Whether the filter's text entry holds the keyboard: the fact that
    /// suppresses the global bindings while the user types, and the caret
    /// fact the GUI's filter control paints.
    pub filter_active: bool,
}

impl App {
    /// Project the queue summary (design D4a's derived rows). Read-only.
    #[must_use]
    pub fn queue_summary_projection(&self) -> QueueSummaryProjection {
        let slots = self.playback_queue().slots();
        let state = self.displayed_playback_state();
        let filter = self.queue_filter.clone();
        let filter_active = self.queue_filter_active;
        let active_index = self.displayed_active_index(slots);
        let offset = UtcOffset::current_local_offset().unwrap_or(UtcOffset::UTC);
        let Some(index) = active_index else {
            return QueueSummaryProjection {
                remaining: slots.len(),
                next_title: None,
                finish_clock: None,
                filter,
                filter_active,
            };
        };
        let next_title = self.next_slot_title(slots, index);
        let now = now_unix_secs();
        let anchor = playback_finish_epoch(now, state.position_ticks, state.runtime_ticks);
        let Some(anchor) = anchor else {
            return QueueSummaryProjection {
                remaining: slots.len() - 1,
                next_title,
                finish_clock: None,
                filter,
                filter_active,
            };
        };
        let finish = finish_chain(slots, index, anchor);
        QueueSummaryProjection {
            remaining: slots.len() - 1,
            next_title,
            finish_clock: finish.map(|secs| queue_finish_clock(secs, now, offset)),
            filter,
            filter_active,
        }
    }

    /// The title of the slot after the playhead: the compact strip's
    /// next-item line (review of add-tauri-gui-client row 6.6, finding P2).
    /// `None` while no slot follows the playhead.
    fn next_slot_title(&self, slots: &[QueueSlot], active_index: usize) -> Option<String> {
        slots
            .get(active_index + 1)
            .map(|slot| self.playback_title_parts(&slot.item).title.text)
    }

    /// The index of the shown queue's slot that carries the playhead, or
    /// `None` while the displayed playback state names no slot of that queue
    /// (idle playback, or a playhead the shown queue does not know yet).
    fn displayed_active_index(&self, slots: &[QueueSlot]) -> Option<usize> {
        let state = self.displayed_playback_state();
        state
            .active
            .then_some(state.active_idx)
            .flatten()
            .filter(|index| *index < slots.len())
    }

    /// The shown queue's tail the folded queue and the queue header derive
    /// their finish clock from (`gui-change-driven-sections` design D5): the
    /// last pending slot's end as seconds after the end of the active item.
    /// The window adds the anchor — the wall clock now plus the active item's
    /// remaining time — and the local zone itself, so no clock string crosses
    /// the Owner edge and a paused window's time still moves.
    #[must_use]
    pub fn gui_queue_end_offsets(&self) -> QueueEndOffsets {
        let slots = self.playback_queue().slots();
        let Some(index) = self.displayed_active_index(slots) else {
            return QueueEndOffsets::default();
        };
        QueueEndOffsets {
            tail: queue_end_tail(slots, index),
        }
    }
}

/// The shown queue's end offset for one read (design D5): the GUI sections'
/// duration fact for the queue finish.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct QueueEndOffsets {
    /// The shown queue's tail: the last pending slot's end as seconds after
    /// the end of the active item. `None` when a pending runtime breaks the
    /// chain, or no slot carries the playhead.
    pub tail: Option<i64>,
}

/// The wall-clock epoch at which the playhead's item ends, from `now_secs`:
/// the item's remaining runtime added to the clock. `None` when the runtime
/// is unknown (zero); the position clamps into the runtime, so a playhead
/// that overshot ends now.
#[must_use]
pub(in crate::app) fn playback_finish_epoch(
    now_secs: i64,
    position_ticks: i64,
    runtime_ticks: i64,
) -> Option<i64> {
    (runtime_ticks > 0).then(|| {
        let remaining_ticks = runtime_ticks - position_ticks.clamp(0, runtime_ticks);
        now_secs + remaining_ticks / TICKS_PER_SECOND
    })
}

/// The queue's finish clock with the canvas's next-day suffix: the
/// zero-padded wall clock, plus ` +1d` when the finish epoch lies past the
/// local day `now_secs` falls on. Only the queue finish carries the suffix;
/// row ends and the scrubber's finish clock paint the bare clock.
#[must_use]
fn queue_finish_clock(finish_secs: i64, now_secs: i64, offset: UtcOffset) -> String {
    let clock = fmt_wall_clock(finish_secs, offset);
    if clock.is_empty() {
        return clock;
    }
    let after_midnight = OffsetDateTime::from_unix_timestamp(finish_secs)
        .ok()
        .zip(OffsetDateTime::from_unix_timestamp(now_secs).ok())
        .is_some_and(|(finish, now)| {
            finish.to_offset(offset).date() > now.to_offset(offset).date()
        });
    if after_midnight {
        format!("{clock} +1d")
    } else {
        clock
    }
}

/// The shown queue's tail: each pending slot's known runtime extends the
/// chain from the active slot's end, so the tail is the sum of the runtimes
/// that follow the active slot. `None` when a pending runtime is unknown (a
/// zero runtime is the codebase's unknown marker), because the end then
/// cannot be named. A playhead on the last slot has a tail of `0`.
#[must_use]
fn queue_end_tail(slots: &[QueueSlot], active_index: usize) -> Option<i64> {
    let mut tail = 0;
    for slot in &slots[active_index + 1..] {
        let runtime_secs = slot.item.runtime_ticks() / TICKS_PER_SECOND;
        if runtime_secs <= 0 {
            return None;
        }
        tail += runtime_secs;
    }
    Some(tail)
}

/// The shown queue's finish epoch from one anchor: [`queue_end_tail`] gives
/// the tail as seconds after the active item's end, and the anchor turns it
/// into an epoch. `None` when the tail is unknown.
#[must_use]
fn finish_chain(slots: &[QueueSlot], active_index: usize, anchor_epoch: i64) -> Option<i64> {
    queue_end_tail(slots, active_index).map(|tail| anchor_epoch + tail)
}

/// The wall clock's unix seconds, saturated to `i64`.
pub(in crate::app) fn now_unix_secs() -> i64 {
    i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_secs()),
    )
    .unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests;
