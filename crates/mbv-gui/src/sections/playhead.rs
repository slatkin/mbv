//! The playhead anchor compare (design D5). The ledger keeps the last sent
//! anchor and the `Instant` of its send, and each collect compares the tick's
//! one playback projection read against them: a changed `active`,
//! `activeSlot`, `paused`, or `runtimeTicks`, or a new sample that drifts
//! past the tolerance from the prediction, sends a new anchor. Only a new
//! sample is compared, so an output that reports its position from time to
//! time does not re-anchor between reports, and a rate change is no trigger —
//! mbv has no playback-rate fact.
//!
//! Sync does not replay the anchor either: a replay would rewind the window's
//! position by the anchor's age, because the front end stamps the anchor on
//! receipt and has no other clock for it. `release_all` forgets the anchor,
//! and the next collect takes the first-send path — a fresh anchor at a new
//! revision.

use std::time::{Duration, Instant};

use mbv::QueuePlaybackProjection;
use mbv_emby_model::{TICKS_PER_SECOND, seconds_to_ticks};
use mbv_surfaces::queue::QueueSurface;

use super::Slot;
use crate::bridge::frame::playback::GuiPlayhead;

/// The drift a new sample may carry before it re-anchors (design D5): one
/// second, in ticks.
const PLAYHEAD_TOLERANCE: i64 = TICKS_PER_SECOND;

/// The anchor compare's own state (design D5): the last reported position,
/// which names a new sample, and the `Instant` the last anchor went out,
/// which the prediction advances from.
#[derive(Debug)]
pub(super) struct PlayheadAnchor {
    last_observed: Option<i64>,
    sent_at: Instant,
}

impl PlayheadAnchor {
    /// The sampler at construction: the first collect's prediction starts
    /// from here.
    pub(super) fn new(now: Instant) -> Self {
        Self {
            last_observed: None,
            sent_at: now,
        }
    }

    /// Compare the tick's playback read against the last sent anchor and
    /// adopt a new one when a trigger fires (design D5). The Queue Surface
    /// state names the anchor's active slot, the same whole-list now-playing
    /// marker the `queue` section reads.
    pub(super) fn sample(
        &mut self,
        slot: &mut Slot<GuiPlayhead>,
        playback: &QueuePlaybackProjection,
        queue: &QueueSurface,
        now: Instant,
    ) {
        let anchor = GuiPlayhead::from_surface(playback, queue);
        let new_sample = self.last_observed != Some(anchor.position_ticks);
        self.last_observed = Some(anchor.position_ticks);
        let Some(sent) = slot.value.as_ref() else {
            self.send(slot, anchor, now);
            return;
        };
        let fact_changed = sent.active != anchor.active
            || sent.active_slot != anchor.active_slot
            || sent.paused != anchor.paused
            || sent.runtime_ticks != anchor.runtime_ticks;
        if fact_changed {
            self.send(slot, anchor, now);
            return;
        }
        if new_sample {
            // The prediction: the anchor's position advanced by the elapsed
            // time since its send, or held while the anchor was paused.
            let elapsed = now.saturating_duration_since(self.sent_at);
            let advance = if sent.paused {
                0
            } else {
                seconds_to_ticks(elapsed.as_secs_f64())
            };
            let predicted = sent.position_ticks + advance;
            if (anchor.position_ticks - predicted).abs() > PLAYHEAD_TOLERANCE {
                self.send(slot, anchor, now);
            }
        }
    }

    /// Adopt the anchor and stamp the send's clock.
    fn send(&mut self, slot: &mut Slot<GuiPlayhead>, anchor: GuiPlayhead, now: Instant) {
        slot.adopt(anchor);
        self.sent_at = now;
    }

    /// Drop the anchor state's memory (design D5's sync rule): no last
    /// observed position and a re-stamped send clock, so the next collect
    /// predicts nothing and takes the first-send path.
    pub(super) fn reset(&mut self) {
        self.last_observed = None;
        self.sent_at = Instant::now();
    }

    /// Move the send instant back by `by`, so a core-thread test's next
    /// collect predicts from an older anchor with no real wait (the drift
    /// rule's time injection).
    pub(super) fn backdate_for_test(&mut self, by: Duration) {
        self.sent_at = self
            .sent_at
            .checked_sub(by)
            .expect("the clock reaches back by the test's drift window");
    }
}
