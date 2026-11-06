//! The seekbar's seek rules (extract-client-core row 4.5): the fraction a
//! pointer position resolves against a painted bar, and the scrub admission
//! for a left-drag anchored inside it. Moved from the TUI adapter's
//! `mouse/seekbar.rs` so both Clients resolve the same seek target.
//!
//! The bar is the Client's painted geometry, handed in per call like a hit
//! region; the rules here never compute layout.

use std::time::{Duration, Instant};

use mbv_ui_msg::Position;

use crate::HitRegion;

/// Seekbar scrub throttle. A drag reports every motion event, so the panel
/// admits at most one scrub seek per interval (the legacy 150 ms cadence);
/// without it one pointer sweep would queue a seek per terminal event.
pub const SEEKBAR_SCRUB_INTERVAL: Duration = Duration::from_millis(150);

/// The seek fraction a pointer column resolves against a painted bar: its
/// fraction of the bar span, with columns before the bar resolving 0. `None`
/// when the Client painted no bar (zero width). The zero-width guard and the
/// column clamp live here, so a fix to either lands on every bar that seeks.
/// A leading partial cell is an ordinary seek target; the rule carries no
/// upper clamp, matching the painted-bar contract the Clients resolve
/// against.
#[must_use]
pub fn seek_fraction(bar: HitRegion, column: f32) -> Option<f64> {
    (bar.width > 0.0).then(|| {
        let offset = (f64::from(column) - f64::from(bar.x)).max(0.0);
        offset / f64::from(bar.width)
    })
}

/// Admit one scrub of `bar` for a left-drag from `from` to `to` and return
/// the seek fraction the drag's pointer end resolves to. The drag must have
/// anchored inside the bar span and the pointer must still be over it — a
/// drag that merely crossed the bar is not a scrub of this bar. `last_scrub`
/// records the last admitted scrub; `None` admits the next one. `now` is the
/// caller's clock, so a test injects time instead of sleeping.
#[must_use]
pub fn scrub_fraction(
    bar: HitRegion,
    last_scrub: &mut Option<Instant>,
    from: Position,
    to: Position,
    now: Instant,
) -> Option<f64> {
    if !bar.contains(from) || !bar.contains(to) {
        return None;
    }
    if last_scrub.is_some_and(|at| now.duration_since(at) < SEEKBAR_SCRUB_INTERVAL) {
        return None;
    }
    *last_scrub = Some(now);
    seek_fraction(bar, to.x)
}
