//! Seekbar pointer geometry shared by the playback panels (ADR 0024): the
//! TUI translation over the Surface state's seek rules (add-gui-front-end
//! row 4.5). The admission throttle and the fraction rule live once in
//! `mbv_surfaces::seekbar`; this module converts the TUI's painted cell
//! rectangles and gesture positions and delegates, so a fix to either rule
//! lands on every Client.

use std::time::Instant;

use ratatui::layout::Rect;

use crate::media_list::hit_region;
use mbv_surfaces::gesture::PointerGesture;
use mbv_ui_msg::Position;

pub use mbv_surfaces::seekbar::SEEKBAR_SCRUB_INTERVAL;

/// The seek fraction a pointer column resolves against a painted bar: its
/// fraction of the bar span, with columns before the bar resolving 0. `None`
/// when the panel painted no bar (`bar.width == 0`). One cell is one logical
/// unit.
#[must_use]
pub fn seek_fraction(bar: Rect, column: u16) -> Option<f64> {
    mbv_surfaces::seekbar::seek_fraction(hit_region(bar), f32::from(column))
}

/// Admit one scrub of `bar` for a recognized left-drag and return the seek
/// fraction the pointer column resolves to: the drag must have anchored
/// inside the bar span, the pointer must still be over it, and at most one
/// scrub is admitted per [`SEEKBAR_SCRUB_INTERVAL`].
#[must_use]
pub fn scrub_fraction(
    bar: Rect,
    last_scrub: &mut Option<Instant>,
    gesture: Option<PointerGesture<ratatui::layout::Position>>,
) -> Option<f64> {
    let PointerGesture::Drag { from, to } = gesture? else {
        return None;
    };
    mbv_surfaces::seekbar::scrub_fraction(
        hit_region(bar),
        last_scrub,
        neutral_position(from),
        neutral_position(to),
        Instant::now(),
    )
}

/// One cell is one logical unit (design D6).
fn neutral_position(position: ratatui::layout::Position) -> Position {
    Position {
        x: f32::from(position.x),
        y: f32::from(position.y),
    }
}
