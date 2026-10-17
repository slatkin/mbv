//! Seekbar pointer geometry shared by the playback panels (ADR 0024): the
//! seek fraction a column resolves against the painted bar, and the scrub
//! admission for a left-drag anchored inside it. A drag reports every motion
//! event, so admission is throttled to one scrub per interval.

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

/// The seek fraction a pointer column resolves against a painted bar: its
/// fraction of the bar span, with columns before the bar resolving 0. `None`
/// when the panel painted no bar (`bar.width == 0`). The zero-width guard and
/// the column clamp live here, so a fix to either lands on every bar that
/// seeks.
#[must_use]
pub fn seek_fraction(bar: Rect, column: u16) -> Option<f64> {
    (bar.width > 0).then(|| f64::from(column.saturating_sub(bar.x)) / f64::from(bar.width))
}

/// Admits one scrub of `bar` for a recognized left-drag and returns the seek
/// fraction the pointer column resolves to: [`admit_seekbar_scrub`]'s
/// admission, then [`seek_fraction`]'s fraction.
pub fn scrub_fraction(
    bar: Rect,
    last_scrub: &mut Option<Instant>,
    gesture: Option<MouseGesture>,
) -> Option<f64> {
    seek_fraction(bar, admit_seekbar_scrub(bar, last_scrub, gesture)?)
}
