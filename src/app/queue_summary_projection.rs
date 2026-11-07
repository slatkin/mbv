//! The Queue summary projection every Client consumes: the queue header's
//! remaining count and finish clock, and each pending slot's finish clock for
//! the right column's "Ends at" mode (design D4a's derived wall-clock rows).
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
use mbv_queue::{QueueSlot, QueueSlotId};
use mbv_ui_model::ui_util::fmt_wall_clock;
use time::{OffsetDateTime, UtcOffset};

use crate::app::App;

/// Which value the queue rows' right column paints (add-tauri-gui-client row
/// 6.2, design D4): the queue header's Length / Ends-at switch. The GUI
/// Client toggles it; the TUI Client paints the length value only, so it
/// never reads the state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum QueueRightColumn {
    /// Each row paints its total runtime.
    #[default]
    Length,
    /// Each row paints the clock time at which it ends, from the current
    /// position onward.
    EndsAt,
}

/// The Queue summary's facts for one read: the header's remaining count and
/// finish clock, the next item's title, plus each pending slot's end-of-play
/// wall clock.
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
    /// Each pending slot's wall-clock end time, keyed by its stable slot id,
    /// formatted in the local zone: the right column's "Ends at" value. The
    /// active slot ends at the playback projection's finish clock; a slot
    /// before the playhead carries no clock, and the chain stops at the first
    /// pending slot whose runtime is unknown.
    pub row_finish_clocks: Vec<(QueueSlotId, String)>,
    /// Which value the rows' right column paints: the Client-core toggle's
    /// state (add-tauri-gui-client row 6.2).
    pub right_column: QueueRightColumn,
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
        let right_column = self.queue_right_column;
        let filter = self.queue_filter.clone();
        let filter_active = self.queue_filter_active;
        let active_index = state
            .active
            .then_some(state.active_idx)
            .flatten()
            .filter(|index| *index < slots.len());
        let offset = UtcOffset::current_local_offset().unwrap_or(UtcOffset::UTC);
        let Some(index) = active_index else {
            return QueueSummaryProjection {
                remaining: slots.len(),
                next_title: None,
                finish_clock: None,
                row_finish_clocks: Vec::new(),
                right_column,
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
                row_finish_clocks: Vec::new(),
                right_column,
                filter,
                filter_active,
            };
        };
        let (ends, finish) = finish_chain(slots, index, anchor);
        QueueSummaryProjection {
            remaining: slots.len() - 1,
            next_title,
            finish_clock: finish.map(|secs| queue_finish_clock(secs, now, offset)),
            row_finish_clocks: ends
                .into_iter()
                .map(|(slot, secs)| (slot, fmt_wall_clock(secs, offset)))
                .collect(),
            right_column,
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

/// The pending slots' wall-clock end epochs from one anchor: the anchor ends
/// the active slot, and each later slot's known runtime extends the chain.
/// Returns the per-slot ends in queue order and the shown queue's finish —
/// the last slot's end when the whole chain resolved, `None` when a pending
/// runtime broke it.
#[must_use]
fn finish_chain(
    slots: &[QueueSlot],
    active_index: usize,
    anchor_epoch: i64,
) -> (Vec<(QueueSlotId, i64)>, Option<i64>) {
    let mut ends = Vec::with_capacity(slots.len() - active_index);
    let mut end = anchor_epoch;
    for (offset, slot) in slots[active_index..].iter().enumerate() {
        if offset > 0 {
            let runtime_secs = slot.item.runtime_ticks() / TICKS_PER_SECOND;
            if runtime_secs <= 0 {
                return (ends, None);
            }
            end += runtime_secs;
        }
        ends.push((slot.slot_id, end));
    }
    (ends, Some(end))
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
