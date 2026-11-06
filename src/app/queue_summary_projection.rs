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
use time::UtcOffset;

use crate::app::App;

/// The Queue summary's facts for one read: the header's remaining count and
/// finish clock, plus each pending slot's end-of-play wall clock.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct QueueSummaryProjection {
    /// The slots after the active index: the queue's remaining play. The
    /// whole queue's slot count while no slot carries the playhead — idle
    /// playback, slotless output, or a playhead the shown queue does not
    /// know yet.
    pub remaining: usize,
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
}

impl App {
    /// Project the queue summary (design D4a's derived rows). Read-only.
    #[must_use]
    pub fn queue_summary_projection(&self) -> QueueSummaryProjection {
        let slots = self.playback_queue().slots();
        let state = self.displayed_playback_state();
        let active_index = state
            .active
            .then_some(state.active_idx)
            .flatten()
            .filter(|index| *index < slots.len());
        let offset = UtcOffset::current_local_offset().unwrap_or(UtcOffset::UTC);
        let Some(index) = active_index else {
            return QueueSummaryProjection {
                remaining: slots.len(),
                finish_clock: None,
                row_finish_clocks: Vec::new(),
            };
        };
        let anchor = playback_finish_epoch(now_unix_secs(), state.position_ticks, state.runtime_ticks);
        let Some(anchor) = anchor else {
            return QueueSummaryProjection {
                remaining: slots.len() - index - 1,
                finish_clock: None,
                row_finish_clocks: Vec::new(),
            };
        };
        let (ends, finish) = finish_chain(slots, index, anchor);
        QueueSummaryProjection {
            remaining: slots.len() - index - 1,
            finish_clock: finish.map(|secs| fmt_wall_clock(secs, offset)),
            row_finish_clocks: ends
                .into_iter()
                .map(|(slot, secs)| (slot, fmt_wall_clock(secs, offset)))
                .collect(),
        }
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
