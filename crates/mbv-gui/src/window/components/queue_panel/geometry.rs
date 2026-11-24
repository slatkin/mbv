//! The Queue panel's layout constants (gui-queue-panel-redesign design D10):
//! the header's bar and control sizes, the row height and the list's capacity
//! in whole rows, the row's insets and ordinal column, the drag's insertion
//! line and its ghost offset, and the row menu's top edge. They moved out of
//! `queue_panel.rs` so that file stays under the 800-line limit, and the whole
//! queue column reads its pixel sizes from here. The rows are the delineated
//! 40 px rows with no gap between them (design D3 of `gui-delineated-rows`), so
//! a stacked row starts one 40 px stride below its neighbour.

use crate::bridge::frame::queue::{GuiQueue, GuiQueueInsertion, GuiQueueRow};

/// The Queue header bar's height in logical pixels (design D7): one row of
/// 26 px controls in a 48 px bar. `assets/app.css` defines the same value as
/// `--canvas-queue-header-height`, because the stylesheet must carry it before
/// the first theme writes `:root`; `queue_panel/tests/header.rs` keeps the two
/// equal.
pub(in crate::window::components) const HEADER_HEIGHT_PX: u16 = 48;

/// The Queue header's control size in logical pixels (user request 2026-11-24,
/// tightened from the canvas's 32 px): the icon buttons are square at this size
/// and the Queue switcher and the playback-target control are this tall (user
/// request 2026-11-24: the two dropdowns paint one height), so the bar's
/// controls take less of the row's width.
pub(in crate::window::components) const CONTROL_PX: u16 = 26;

/// The canvas's fixed queue row height in logical pixels (design D3).
pub(in crate::window::components) const ROW_HEIGHT_PX: u16 = 40;

/// The gap between two stacked rows in logical pixels (design D3): the rows
/// sit flush and their own 1 px dividers separate them.
pub(in crate::window::components) const ROW_GAP_PX: u16 = 0;

/// The space between the queue header and the first row. It is the list box's
/// top margin, not its padding: the list's measured height (the capacity) and
/// the absolute ghost and row-menu offsets then stay relative to the first
/// row's top edge.
pub(super) const LIST_TOP_GAP_PX: u16 = 8;

/// The distance from one row's top edge to the next's: the row plus its gap.
const ROW_STRIDE_PX: u16 = ROW_HEIGHT_PX + ROW_GAP_PX;

/// The row's inset from its leading and trailing edges, in logical
/// pixels (design D3).
pub(super) const ROW_INSET_PX: u16 = 10;

/// The gap between a row's cells, in logical pixels (design D3): the ordinal,
/// the title, and the right column.
pub(super) const ROW_CELL_GAP_PX: u16 = 10;

/// The queue ordinal's column width in logical pixels (design D3): the
/// two-digit number every row leads with.
pub(in crate::window::components) const ORDINAL_COL_PX: u16 = 28;

/// The row's right column width in logical pixels (design D3): the item's
/// duration, right-aligned, which every row state paints.
pub(super) const RIGHT_COL_PX: u16 = 96;

/// The row's title's inset from its leading edge: the row's inset, the ordinal
/// column, and the cell gap.
const TITLE_LEFT_PX: u16 = ROW_INSET_PX + ORDINAL_COL_PX + ROW_CELL_GAP_PX;

/// The drag ghost's leading inset: the title's inset less the ghost card's own
/// padding, so the ghost's title lines up with the row's (design D9).
pub(super) const GHOST_LEFT_PX: u16 = TITLE_LEFT_PX - ROW_INSET_PX;

/// The drag ghost's overhang past the row's trailing edge, in logical pixels
/// (design D9): the card paints its own inset, so its title lines up with the
/// row's too.
pub(super) const GHOST_RIGHT_OVERHANG_PX: i16 = -6;

/// The Queue chooser card's drop under the switcher, in logical pixels (design
/// D8).
pub(in crate::window::components) const MENU_DROP_PX: u16 = 4;

/// The canvas's row-menu overlap into its anchor row, in logical pixels (the
/// `Menus.dc.html` menu sits over the row's bottom edge, not under it).
pub(super) const MENU_OVERLAP_PX: u16 = 6;

/// The painted index of the item row holding `slot`, if it is a painted row.
fn slot_index(queue: &GuiQueue, slot: u64) -> Option<usize> {
    queue
        .rows
        .iter()
        .position(|row| matches!(row, GuiQueueRow::Item { slot: held, .. } if *held == slot))
}

/// The drag's insertion line resolved against the painted rows: the anchor
/// slot's row index, or `None` while no drag runs or its anchor is not a
/// painted row.
pub(super) struct Insertion {
    pub(super) before: bool,
    pub(super) slot: u64,
    index: usize,
}

/// The drag's insertion line resolved against the painted rows.
pub(super) fn insertion(queue: &GuiQueue) -> Option<Insertion> {
    let drag = queue.drag.as_ref()?;
    let (before, slot) = match drag.insertion {
        GuiQueueInsertion::Before { slot } => (true, slot),
        GuiQueueInsertion::After { slot } => (false, slot),
    };
    let index = slot_index(queue, slot)?;
    Some(Insertion {
        before,
        slot,
        index,
    })
}

/// The ghost's top edge in the panel's pixels: the row top the drop would
/// insert at — the anchor row's top edge for `before`, the next row's top edge
/// for `after` — measured from the list box's first row, so the list's top gap
/// comes first.
pub(super) fn ghost_top(insertion: &Insertion) -> usize {
    let stride = usize::from(ROW_STRIDE_PX);
    let row = if insertion.before {
        insertion.index * stride
    } else {
        (insertion.index + 1) * stride
    };
    usize::from(LIST_TOP_GAP_PX) + row
}

/// The open row menu's top edge: the header row's height plus the bottom
/// edge of the row the menu opened on — the overlay's `anchorSlot`, which
/// the core names (hover and cursor can move while the menu is up) — less
/// the canvas's 6 px overlap into that row (`Menus.dc.html` sits the 230 px
/// menu over its anchor row's bottom edge, not under it). A `0` fallback
/// holds the menu at the header while the anchor is not a painted row.
pub(super) fn menu_top(queue: &GuiQueue, anchor_slot: u64) -> String {
    let stride = usize::from(ROW_STRIDE_PX);
    let overlap = usize::from(MENU_OVERLAP_PX);
    let below = slot_index(queue, anchor_slot).map_or(0, |index| {
        usize::from(LIST_TOP_GAP_PX) + (index + 1) * stride - overlap
    });
    format!("calc(var(--canvas-queue-header-height) + {below}px)")
}

/// The whole rows a list box of `height` CSS pixels fits (the former client
/// report's `wholeRows`): the capacity the core's queue viewport reads. N rows
/// need `N` row heights and the `N - 1` gaps between them, so a box fits
/// `floor((height + gap) / stride)` of them — with a 0 px gap, 80 px holds two
/// 40 px rows, and only a box shorter than a row fits none.
pub(super) fn whole_rows(height: f32) -> usize {
    let stride = f32::from(ROW_STRIDE_PX);
    let gap = f32::from(ROW_GAP_PX);
    let rows =
        mbv_emby_model::saturating_i64_from_f64(f64::from(((height + gap) / stride).floor()));
    usize::try_from(rows).unwrap_or(0)
}
