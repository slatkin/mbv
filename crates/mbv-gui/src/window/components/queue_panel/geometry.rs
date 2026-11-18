//! The Queue panel's row geometry (gui-queue-panel-redesign design D10): the
//! row height and the list's capacity in whole rows, the row's painted
//! background, the drag's insertion line and its ghost offset, and the row
//! menu's top edge. They moved out of `queue_panel.rs` so that file stays
//! under the 800-line limit; the row height is still the 30 px row the canvas
//! draws (design D9's 34 px row arrives in its own task).

use super::row_cells::ItemRow;
use crate::bridge::frame::queue::{GuiQueue, GuiQueueInsertion, GuiQueueRow};
use crate::window::theme::canvas_var;

/// The canvas's fixed queue row height in logical pixels (the spec's 30 px
/// rows). Capacity is whole rows.
const ROW_HEIGHT: f32 = 30.0;

/// The same row height as a whole-row count unit (the spec's 30 px rows).
const ROW_HEIGHT_PX: usize = 30;

/// The hovered row's six-dot grip: the icon's size in logical pixels, and the
/// row offset that centres it in the 30 px row. The centring is a layout
/// offset, not a `-translate-y-1/2` (task 11.1): Blitz lays out without the
/// `translate` property and applies it at paint only, so a translated grip
/// would paint centred while the `rowGrip` region its parent reports — the
/// region the core's hit map holds — stayed half a grip low. The layout box
/// is the box the grip paints, so the region is the control.
pub(super) const GRIP_SIZE_PX: u16 = 14;
pub(super) const GRIP_TOP_PX: f32 = (ROW_HEIGHT - GRIP_SIZE_PX as f32) / 2.0;

/// Whether one item row paints the picked fill: the cursor slot — the GUI's
/// active row, which bars at all times now that the window holds no focus
/// fact — and every marked row do (design D7).
fn picked(queue: &GuiQueue, slot: u64) -> bool {
    queue.marks.contains(&slot) || queue.cursor == Some(slot)
}

/// The row's painted background: the active or marked row's fill, then the
/// zebra stripe. A hovered row paints no fill of its own — the veil every
/// hovered `button` paints (`assets/app.css`) is its whole hover look, so the
/// active row keeps its fill and edge under the pointer (design D7).
pub(super) fn row_background(row: &ItemRow, queue: &GuiQueue) -> Option<String> {
    if picked(queue, row.slot) {
        return Some(canvas_var("selected"));
    }
    if queue.hover == Some(row.slot) {
        return None;
    }
    if row.striped {
        return Some("var(--mbv-queueColumn)".to_owned());
    }
    None
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
    let index = queue
        .rows
        .iter()
        .position(|row| matches!(row, GuiQueueRow::Item { slot: held, .. } if *held == slot))?;
    Some(Insertion {
        before,
        slot,
        index,
    })
}

/// The ghost's top edge in list pixels: the row top the drop would insert at
/// — the anchor row's top edge for `before`, its bottom edge for `after`.
pub(super) fn ghost_top(insertion: &Insertion) -> usize {
    if insertion.before {
        insertion.index * ROW_HEIGHT_PX
    } else {
        (insertion.index + 1) * ROW_HEIGHT_PX
    }
}

/// The open row menu's top edge: the header row's height plus the bottom
/// edge of the row the menu opened on — the overlay's `anchorSlot`, which
/// the core names (hover and cursor can move while the menu is up) — less
/// the canvas's 6 px overlap into that row (`Menus.dc.html` sits the 230 px
/// menu over its anchor row's bottom edge, not under it). A `0` fallback
/// holds the menu at the header while the anchor is not a painted row.
pub(super) fn menu_top(queue: &GuiQueue, anchor_slot: u64) -> String {
    let index = queue.rows.iter().position(
        |row| matches!(row, GuiQueueRow::Item { slot: held, .. } if *held == anchor_slot),
    );
    let below = index.map_or(0, |index| (index + 1) * ROW_HEIGHT_PX - 6);
    format!("calc(var(--canvas-queue-header-height) + {below}px)")
}

/// The whole 30 px rows a list box of `height` CSS pixels fits (the former client
/// report's `wholeRows`): the capacity the core's queue viewport reads.
pub(super) fn whole_rows(height: f32) -> usize {
    let rows = mbv_emby_model::saturating_i64_from_f64(f64::from(height / ROW_HEIGHT).floor());
    usize::try_from(rows).unwrap_or(0)
}
