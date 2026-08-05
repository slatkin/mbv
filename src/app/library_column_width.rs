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

/// Rect of column `col` within `content_area`: `cols` equal cells separated
/// by `LIBRARY_COLUMN_GAP`, cell `c` starting at `c * (cell_width + gap)`.
/// The returned rect's `y`/`height` span the whole content area; callers
/// that paint a fixed number of rows use only `x`/`width`.
pub(super) fn library_cell_rect(content_area: Rect, cols: usize, col: usize) -> Rect {
    let cell_w = library_cell_width(content_area, cols);
    Rect {
        x: content_area.x + (col as u16).saturating_mul(cell_w + LIBRARY_COLUMN_GAP),
        y: content_area.y,
        width: cell_w,
        height: content_area.height,
    }
}

/// Slot rect for painting the notched selected block, like
/// `library_cell_rect` but with the rightmost cell's width extended to
/// cover the trailing remainder column. When the content width minus the
/// inter-column gap does not divide evenly by `cols` (e.g. width=149
/// gives cells of 73+2+73=148 with 1 leftover col), the right cell
/// absorbs that leftover so its tab joins the full-width panel below at
/// the content area's right edge instead of leaving a 1-col strip of
/// ordinary background between them.
///
/// The notched block this sized is gone with the hero-on-top change (the
/// selected cell is a `▌`/`##` marker now), but the slot geometry stays
/// for future cell work.
#[allow(dead_code)]
pub(super) fn library_cell_slot(content_area: Rect, cols: usize, col: usize) -> Rect {
    let rect = library_cell_rect(content_area, cols, col);
    if col + 1 == cols {
        let right_edge = content_area.x + content_area.width;
        Rect {
            x: rect.x,
            y: rect.y,
            width: right_edge.saturating_sub(rect.x),
            height: rect.height,
        }
    } else {
        rect
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::layout::Rect;

    fn area(width: u16) -> Rect {
        Rect::new(0, 0, width, 10)
    }

    #[test]
    fn column_count_threshold_boundary_both_directions() {
        // Two cells at MIN_WIDTH plus one gap: 40 + 2 + 40 = 82.
        assert_eq!(library_column_count(81), 1, "just below the threshold");
        assert_eq!(library_column_count(82), 2, "exactly at the threshold");
        assert_eq!(library_column_count(40), 1);
        assert_eq!(library_column_count(200), 2, "very wide panes cap at 2");
    }

    #[test]
    fn one_column_cell_spans_the_full_content_area() {
        let rect = library_cell_rect(area(60), 1, 0);
        assert_eq!((rect.x, rect.width), (0, 60));
        assert_eq!((rect.y, rect.height), (0, 10));
    }

    #[test]
    fn two_column_cell_rect_arithmetic() {
        // 82 - 2 gap = 80, /2 = 40 per cell.
        let c0 = library_cell_rect(area(82), 2, 0);
        assert_eq!((c0.x, c0.width), (0, 40));
        let c1 = library_cell_rect(area(82), 2, 1);
        assert_eq!(
            (c1.x, c1.width),
            (42, 40),
            "cell 1 starts after cell 0 + gap"
        );
    }

    #[test]
    fn cell_rect_with_non_dividing_width_floors_cell_width() {
        // 83 - 2 = 81, /2 = 40 (floored); one column is left over at the
        // right edge and cell 1 still starts at 0 + 40 + 2 = 42.
        let c0 = library_cell_rect(area(83), 2, 0);
        assert_eq!((c0.x, c0.width), (0, 40));
        let c1 = library_cell_rect(area(83), 2, 1);
        assert_eq!((c1.x, c1.width), (42, 40));
    }

    #[test]
    fn cell_rect_respects_content_area_origin() {
        let area = Rect::new(5, 3, 84, 12);
        let c1 = library_cell_rect(area, 2, 1);
        assert_eq!(
            (c1.x, c1.width),
            (5 + 41 + 2, 41),
            "(84-2)/2 = 41, cell 1 starts after cell 0 + gap"
        );
    }

    #[test]
    fn cell_slot_left_cell_matches_cell_rect() {
        // The leftmost cell never absorbs a remainder, so its slot is the
        // plain cell rect.
        let slot = library_cell_slot(area(82), 2, 0);
        let rect = library_cell_rect(area(82), 2, 0);
        assert_eq!((slot.x, slot.width), (rect.x, rect.width));
    }

    #[test]
    fn cell_slot_right_cell_absorbs_trailing_remainder() {
        // Width 83: cells of 40+2+40 = 82 leaves 1 trailing col. The right
        // cell's slot extends to absorb it so the tab joins the panel.
        let slot = library_cell_slot(area(83), 2, 1);
        assert_eq!(
            (slot.x, slot.width),
            (42, 41),
            "right slot extends 1 col past the cell to the content area's right edge"
        );
    }

    #[test]
    fn cell_slot_even_width_unchanged() {
        // Width 82 divides evenly: no trailing col, slot matches cell rect.
        let slot = library_cell_slot(area(82), 2, 1);
        let rect = library_cell_rect(area(82), 2, 1);
        assert_eq!((slot.x, slot.width), (rect.x, rect.width));
    }
}
