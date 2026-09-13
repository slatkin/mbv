//! Column geometry for the two-column library list layout, kept beside
//! `queue_column_width.rs` so the renderer and the cursor-movement code
//! derive cell geometry from one place.
//!
//! The column count derives from the *list pane* width (the width the
//! renderer receives after the queue column and shared gutters are removed),
//! not the terminal width, so widening/collapsing the queue column feeds
//! through live.

use ratatui::layout::Rect;

/// Columns of empty space between adjacent library list cells.
pub(super) const LIBRARY_COLUMN_GAP: u16 = 2;
/// Width of one cell when `content_area` holds `cols` columns:
/// `(content_width - gap * (cols - 1)) / cols`, floored. A width that does
/// not divide evenly leaves the leftover columns unpainted at the right edge
/// (they show the ordinary list background).
pub(super) fn library_cell_width(content_area: Rect, cols: usize) -> u16 {
    let cols = cols.max(1) as u16;
    content_area
        .width
        .saturating_sub(LIBRARY_COLUMN_GAP.saturating_mul(cols.saturating_sub(1)))
        / cols
}
