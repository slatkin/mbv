//! The GUI's cell unit (D6 of `replace-tauri-with-blitz`): the port of
//! `lib/bridge/cells.ts`. The one TUI terminal cell the queue column's
//! arrangement is expressed in. One row is 28px — the Queue list's row
//! height, the fixed-row column the shared core addresses — and one column
//! is 14px, the terminal cell's 1:2 aspect at that row height. Every TUI
//! row/column offset the queue column paints (the bands' two-column inset,
//! the one-row pads and gaps, the list's two-column indent and right inset,
//! the six-column date gutter) is a multiple of these two, and every
//! component writes its offsets with `calc()` over the `--mbv-cell-row` and
//! `--mbv-cell-col` properties [`cell_unit_rule`] writes, so a unit change
//! is one edit here.

/// One TUI text row in logical pixels: the Queue list's row height.
pub const CELL_ROW_PX: u32 = 28;

/// One TUI text column in logical pixels: the terminal cell's 1:2 aspect at
/// the row height.
pub const CELL_COL_PX: u32 = 14;

/// The CSS custom property one row writes to.
pub const CELL_ROW_PROPERTY: &str = "--mbv-cell-row";

/// The CSS custom property one column writes to.
pub const CELL_COL_PROPERTY: &str = "--mbv-cell-col";

/// The `:root` rule that writes the two properties (D6): the same write
/// `applyCellUnit` makes before the window mounts. The root renders it in
/// the same `style` element as the stylesheet, before any other child, so
/// the first paint resolves every `calc()` over the two properties.
#[must_use]
pub fn cell_unit_rule() -> String {
    format!(
        ":root {{ {CELL_ROW_PROPERTY}: {CELL_ROW_PX}px; {CELL_COL_PROPERTY}: {CELL_COL_PX}px; }}"
    )
}
