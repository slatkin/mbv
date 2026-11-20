//! The Library list's window and scroll geometry (`add-gui-library-panel`
//! design D7, D9, D12; gui-delineated-rows design D7): the row contract every
//! list satisfies, the window of rows a viewport paints, the scroll offset a
//! movement key or a cursor reveal leaves, and the row set a `RowsInView`
//! report carries. The row models stay in [`super`]; this module reads them
//! only through [`GeometryRow`].

use crate::window::library::scroll::ListScroll;

/// The rows a `RowsInView` report adds above and below the painted window
/// (gui-delineated-rows design D7): the row art of the rows just outside the
/// viewport is fetched too, so a scroll of up to a screen paints no empty
/// thumb (user request 2026-11-20: 4 rows left empty boxes on a fast scroll).
pub const ROWS_IN_VIEW_OVERSCAN: usize = 16;

/// The geometry the scroll and window geometry read (`add-gui-library-panel`
/// design D7, D12): the stable id, the top edge from the list's first row, and
/// the painted height. The Library tree's rows and the Home list's rows both
/// satisfy it, so one window implementation serves both lists.
pub trait GeometryRow {
    /// The row's stable id.
    fn id(&self) -> &str;

    /// The id the row's pointer marker carries
    /// (`gui-pointer-target-interaction` D1), or `None` for a structural row
    /// that no key acts on: a letter heading names no item, so the pointer
    /// over it names no row.
    fn pointer_row(&self) -> Option<&str> {
        Some(self.id())
    }

    /// The id this row takes in a `RowsInView` report (gui-delineated-rows
    /// design D7), or `None` for a row whose kind has no art source. The core
    /// resolves the id with no coordinates, and a nested row names none — its
    /// kind paints no thumb. A row overrides this where the core's own
    /// encoding differs from its id (`HomeListRow`, `LibraryRow::Artist`).
    fn rows_in_view_id(&self) -> Option<String> {
        self.pointer_row().map(str::to_owned)
    }

    /// The row's top edge from the list's first row.
    fn top(&self) -> f32;

    /// The row's painted height.
    fn height(&self) -> f32;
}

impl<T: GeometryRow> GeometryRow for &T {
    fn id(&self) -> &str {
        T::id(self)
    }

    fn pointer_row(&self) -> Option<&str> {
        T::pointer_row(self)
    }

    fn rows_in_view_id(&self) -> Option<String> {
        T::rows_in_view_id(self)
    }

    fn top(&self) -> f32 {
        T::top(self)
    }

    fn height(&self) -> f32 {
        T::height(self)
    }
}

/// The painted height of the whole list: the bottom edge of its last row.
#[must_use]
pub fn rows_height(rows: &[impl GeometryRow]) -> f32 {
    rows.last().map_or(0.0, |row| row.top() + row.height())
}

/// A list movement the design's keys name (design D3): row by row for Up and
/// Down, one viewport for Page Up and Page Down, and the list's ends for Home
/// and End.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListMove {
    /// One row up.
    Up,
    /// One row down.
    Down,
    /// One viewport up.
    PageUp,
    /// One viewport down.
    PageDown,
    /// The list's first row.
    Home,
    /// The list's last row.
    End,
}

/// The index a movement leaves over `count` rows from `at` (design D7, D12):
/// the one clamp every list and the Settings panes share. `page` is the rows a
/// page spans.
#[must_use]
pub fn move_index(count: usize, at: usize, mv: ListMove, page: usize) -> usize {
    let step = page.max(1);
    let to = match mv {
        ListMove::Up => at.saturating_sub(1),
        ListMove::Down => at.saturating_add(1),
        ListMove::PageUp => at.saturating_sub(step),
        ListMove::PageDown => at.saturating_add(step),
        ListMove::Home => 0,
        ListMove::End => count.saturating_sub(1),
    };
    to.min(count.saturating_sub(1))
}

/// The half-open range of row indexes the viewport paints (design D7): the rows
/// that intersect `[scroll_top, scroll_top + viewport_height)`. An unmeasured
/// viewport paints nothing, so a long list never paints in full while it waits
/// for its first measurement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ListWindow {
    /// The first row in view.
    pub start: usize,
    /// One past the last row in view.
    pub end: usize,
}

/// The rows in view: only these paint, so a long list stays fast.
#[must_use]
pub fn list_window(rows: &[impl GeometryRow], scroll_top: f32, viewport_height: f32) -> ListWindow {
    if viewport_height <= 0.0 {
        return ListWindow { start: 0, end: 0 };
    }
    let top = scroll_top.max(0.0);
    let bottom = top + viewport_height;
    let Some(start) = rows.iter().position(|row| row.top() + row.height() > top) else {
        let len = rows.len();
        return ListWindow {
            start: len,
            end: len,
        };
    };
    let mut end = start;
    while end < rows.len() && rows[end].top() < bottom {
        end += 1;
    }
    ListWindow { start, end }
}

/// The stable ids a `RowsInView` report carries (gui-delineated-rows design
/// D7): the painted window's rows plus [`ROWS_IN_VIEW_OVERSCAN`] rows above and
/// below it. An unmeasured viewport paints nothing, so its list reports no rows
/// — the set arrives once the list is measured. A row whose kind paints no
/// thumb names no id.
#[must_use]
pub fn rows_in_view_ids<Row: GeometryRow>(rows: &[Row], window: ListWindow) -> Vec<String> {
    if window.end <= window.start {
        return Vec::new();
    }
    let start = window.start.saturating_sub(ROWS_IN_VIEW_OVERSCAN);
    let end = window
        .end
        .saturating_add(ROWS_IN_VIEW_OVERSCAN)
        .min(rows.len());
    rows[start..end]
        .iter()
        .filter_map(GeometryRow::rows_in_view_id)
        .collect()
}

/// A scroll offset held inside the list's scrollable range.
#[must_use]
pub fn clamp_scroll_top(rows: &[impl GeometryRow], scroll_top: f32, viewport_height: f32) -> f32 {
    let max = (rows_height(rows) - viewport_height.max(0.0)).max(0.0);
    scroll_top.max(0.0).min(max)
}

/// The scroll offset that keeps the cursor row fully visible (design D7, the
/// spec's "the list scrolls so that the cursor row is fully visible"): the row's
/// top when it sits above the viewport, its bottom edge when it sits below, and
/// an unchanged, clamped offset while it already shows. A cursor with no row
/// moves nothing.
#[must_use]
pub fn scroll_top_to_show_cursor<Row: GeometryRow>(
    rows: &[Row],
    cursor: Option<&str>,
    scroll_top: f32,
    viewport_height: f32,
) -> f32 {
    let row = cursor.and_then(|cursor| rows.iter().find(|row| row.id() == cursor));
    let top = scroll_top.max(0.0);
    // An unmeasured viewport scrolls nothing: the first measurement takes the
    // cursor's row (design D9).
    let Some(row) = row.filter(|_| viewport_height > 0.0) else {
        return clamp_scroll_top(rows, top, viewport_height);
    };
    if row.top() < top {
        clamp_scroll_top(rows, row.top(), viewport_height)
    } else if row.top() + row.height() > top + viewport_height {
        clamp_scroll_top(
            rows,
            row.top() + row.height() - viewport_height,
            viewport_height,
        )
    } else {
        clamp_scroll_top(rows, top, viewport_height)
    }
}

/// The scroll offset a movement key leaves (design D6 of
/// `gui-pointer-target-interaction`): Up and Down move by the height of the row
/// at the top edge, Page Up and Page Down by the viewport height, and Home and
/// End go to the list's ends. The value stays inside `[0, total − viewport]`,
/// so a key at an end moves nothing. `view` is the list's scroll offset and
/// measured viewport height, the fields every destination's scroll store
/// carries.
#[must_use]
pub fn scroll_step(rows: &[impl GeometryRow], view: ListScroll, mv: ListMove) -> f32 {
    let step = match mv {
        ListMove::Up => -top_row_height(rows, view.scroll_top),
        ListMove::Down => top_row_height(rows, view.scroll_top),
        ListMove::PageUp => -view.viewport_height,
        ListMove::PageDown => view.viewport_height,
        ListMove::Home => return clamp_scroll_top(rows, 0.0, view.viewport_height),
        ListMove::End => {
            return clamp_scroll_top(rows, rows_height(rows), view.viewport_height);
        }
    };
    clamp_scroll_top(rows, view.scroll_top + step, view.viewport_height)
}

/// The height of the row that covers the list's top edge: the row the first
/// visible pixel belongs to, the last row's height while the offset lies past
/// them, and no step while the list paints nothing.
#[must_use]
fn top_row_height(rows: &[impl GeometryRow], scroll_top: f32) -> f32 {
    let top = scroll_top.max(0.0);
    rows.iter()
        .find(|row| row.top() + row.height() > top)
        .or_else(|| rows.last())
        .map_or(0.0, GeometryRow::height)
}
