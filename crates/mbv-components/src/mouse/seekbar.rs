//! Seekbar scrub admission shared by the playback panels (ADR 0024).
//!
//! A left-drag that anchors inside a seekbar scrubs it. A drag reports every
//! motion event, so admission is throttled to one scrub per interval.

use std::time::{Duration, Instant};

use ratatui::layout::Rect;

use super::gesture::MouseGesture;

/// Seekbar scrub throttle. A drag reports every motion event, so the panel
/// admits at most one scrub seek per interval (the legacy 150 ms cadence);
/// without it one pointer sweep would queue a seek per terminal event.
pub const SEEKBAR_SCRUB_INTERVAL: Duration = Duration::from_millis(150);

/// Admits one scrub of `bar` for a recognized left-drag and returns the pointer
/// column the scrub seeks to. The drag must have anchored inside `bar`, and the
/// pointer must still be over it: a drag that merely crossed the bar is not a
/// scrub of this bar. `last_scrub` records the last admitted scrub; `None`
/// admits the next one.
pub fn admit_seekbar_scrub(
    bar: Rect,
    last_scrub: &mut Option<Instant>,
    gesture: Option<MouseGesture>,
) -> Option<u16> {
    let MouseGesture::Drag { from, to } = gesture? else {
        return None;
    };
    if !bar.contains(from) || !bar.contains(to) {
        return None;
    }
    let now = Instant::now();
    if last_scrub.is_some_and(|at| now.duration_since(at) < SEEKBAR_SCRUB_INTERVAL) {
        return None;
    }
    *last_scrub = Some(now);
    Some(to.x)
}
