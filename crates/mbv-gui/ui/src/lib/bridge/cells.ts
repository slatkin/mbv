// The GUI's cell unit (design D8's geometry basis): the one TUI terminal
// cell the queue column's arrangement is expressed in. One row is 28px —
// the Queue list's row height, the fixed-row column the shared core
// addresses — and one column is 14px, the terminal cell's 1:2 aspect at
// that row height. Every TUI row/column offset the queue column paints
// (the bands' two-column inset, the one-row pads and gaps, the list's
// two-column indent and right inset, the six-column date gutter) is a
// multiple of these two, and every component writes its offsets with
// `calc()` over the `--mbv-cell-row` and `--mbv-cell-col` properties
// `applyCellUnit` writes, so a unit change is one edit here.

/** One TUI text row in logical pixels: the Queue list's row height. */
export const CELL_ROW_PX = 28;

/** One TUI text column in logical pixels: the terminal cell's 1:2 aspect
 * at the row height. */
export const CELL_COL_PX = 14;

/** The CSS custom property one row writes to. */
export const CELL_ROW_PROPERTY = '--mbv-cell-row';

/** The CSS custom property one column writes to. */
export const CELL_COL_PROPERTY = '--mbv-cell-col';

/** Writes the cell unit to `--mbv-cell-row` and `--mbv-cell-col` on
 * `root`. The GUI calls it once before the window mounts, so the first
 * paint resolves every `calc()` over them. */
export function applyCellUnit(root: HTMLElement = document.documentElement): void {
	root.style.setProperty(CELL_ROW_PROPERTY, `${CELL_ROW_PX}px`);
	root.style.setProperty(CELL_COL_PROPERTY, `${CELL_COL_PX}px`);
}
