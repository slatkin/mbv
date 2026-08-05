//! Column geometry for the two-column library list layout, kept beside
//! `queue_column_width.rs` so the renderer and the cursor-movement code
//! derive cell geometry from one place.
//!
//! The column count derives from the *list pane* width (the width the
//! renderer receives after the queue column and shared gutters are removed),
//! not the terminal width, so widening/collapsing the queue column feeds
//! through live.

use ratatui::layout::Rect;

use super::POWER_TWO_COLUMN_THRESHOLD;

/// Narrowest width (columns) at which one library list cell is readable;
/// anchors the two-column cell sizing. Mirrors `POWER_LEFT_WIDTH_DEFAULT`
/// (`src/app/mod.rs`), the established narrowest comfortable width for a
/// media title row. The shared two-column threshold
/// (`POWER_TWO_COLUMN_THRESHOLD`) is `2 * LIBRARY_COLUMN_MIN_WIDTH +
/// LIBRARY_COLUMN_GAP`; the assert below pins the relationship at compile
/// time so they cannot drift.
pub(super) const LIBRARY_COLUMN_MIN_WIDTH: u16 = 40;
/// Columns of empty space between adjacent library list cells.
pub(super) const LIBRARY_COLUMN_GAP: u16 = 2;
/// Cap on the library list column count (two-column list; never more).
pub(super) const LIBRARY_MAX_COLUMNS: usize = 2;

/// Compile-time guard: the shared threshold must equal two min cells plus
/// one gap. If you change any of the three values, change all of them.
const _: () =
    assert!(POWER_TWO_COLUMN_THRESHOLD == 2 * LIBRARY_COLUMN_MIN_WIDTH + LIBRARY_COLUMN_GAP);

/// Column count for a list pane of the given width: two when the pane meets
/// `POWER_TWO_COLUMN_THRESHOLD` (the shared Power View two-column threshold
/// also used by the Home view's hero/list split), else one. Capped at
/// `LIBRARY_MAX_COLUMNS`.
pub(super) fn library_column_count(list_width: u16) -> usize {
    if list_width >= POWER_TWO_COLUMN_THRESHOLD {
        LIBRARY_MAX_COLUMNS
    } else {
        1
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn column_count_threshold_boundary_both_directions() {
        // Two cells at MIN_WIDTH plus one gap: 40 + 2 + 40 = 82.
        assert_eq!(library_column_count(81), 1, "just below the threshold");
        assert_eq!(library_column_count(82), 2, "exactly at the threshold");
        assert_eq!(library_column_count(40), 1);
        assert_eq!(library_column_count(200), 2, "very wide panes cap at 2");
    }
}
