//! The Queue panel's row geometry (gui-queue-panel-redesign design D10): the
//! row height and the list's capacity in whole rows, the row's painted
//! background, the drag's insertion line and its ghost offset, and the row
//! menu's top edge. They moved out of `queue_panel.rs` so that file stays
//! under the 800-line limit. The rows are the redesign's 34 px rows, 2 px
//! apart (design D9), so a stacked row starts one 36 px stride below its
//! neighbour.

use crate::bridge::frame::queue::{GuiQueue, GuiQueueInsertion, GuiQueueRow};

/// The canvas's fixed queue row height in logical pixels (design D9).
pub(in crate::window::components) const ROW_HEIGHT_PX: u16 = 34;

/// The gap between two stacked rows in logical pixels (design D9).
pub(in crate::window::components) const ROW_GAP_PX: u16 = 2;

/// The space between the queue header and the first row. It is the list box's
/// top margin, not its padding: the list's measured height (the capacity) and
/// the absolute ghost and row-menu offsets then stay relative to the first
/// row's top edge.
pub(super) const LIST_TOP_GAP_PX: u16 = 8;

/// The distance from one row's top edge to the next's: the row plus its gap.
const ROW_STRIDE_PX: u16 = ROW_HEIGHT_PX + ROW_GAP_PX;

/// The row height as the capacity divides by, in logical pixels.
const ROW_HEIGHT: f32 = ROW_HEIGHT_PX as f32;

/// The hovered row's six-dot grip: the icon's size in logical pixels, and the
/// row offset that centres it in the row. The centring is a layout offset,
/// not a `-translate-y-1/2` (task 11.1): Blitz lays out without the
/// `translate` property and applies it at paint only, so a translated grip
/// would paint centred while the `rowGrip` region its parent reports — the
/// region the core's hit map holds — stayed half a grip low. The layout box
/// is the box the grip paints, so the region is the control.
pub(super) const GRIP_SIZE_PX: u16 = 14;
pub(super) const GRIP_TOP_PX: f32 = (ROW_HEIGHT - GRIP_SIZE_PX as f32) / 2.0;

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

/// The ghost's top edge in list pixels: the row top the drop would insert at
/// — the anchor row's top edge for `before`, the next row's top edge for
/// `after`.
pub(super) fn ghost_top(insertion: &Insertion) -> usize {
    let stride = usize::from(ROW_STRIDE_PX);
    if insertion.before {
        insertion.index * stride
    } else {
        (insertion.index + 1) * stride
    }
}

/// The open row menu's top edge: the header row's height plus the bottom
/// edge of the row the menu opened on — the overlay's `anchorSlot`, which
/// the core names (hover and cursor can move while the menu is up) — less
/// the canvas's 6 px overlap into that row (`Menus.dc.html` sits the 230 px
/// menu over its anchor row's bottom edge, not under it). A `0` fallback
/// holds the menu at the header while the anchor is not a painted row.
pub(super) fn menu_top(queue: &GuiQueue, anchor_slot: u64) -> String {
    let stride = usize::from(ROW_STRIDE_PX);
    let below = slot_index(queue, anchor_slot).map_or(0, |index| (index + 1) * stride - 6);
    format!("calc(var(--canvas-queue-header-height) + {below}px)")
}

/// The whole rows a list box of `height` CSS pixels fits (the former client
/// report's `wholeRows`): the capacity the core's queue viewport reads. N rows
/// need `N` row heights and the `N - 1` gaps between them, so a box fits
/// `floor((height + gap) / stride)` of them — 70 px holds two 34 px rows and
/// their 2 px gap, and only a box shorter than a row fits none.
pub(super) fn whole_rows(height: f32) -> usize {
    let stride = f32::from(ROW_STRIDE_PX);
    let gap = f32::from(ROW_GAP_PX);
    let rows =
        mbv_emby_model::saturating_i64_from_f64(f64::from(((height + gap) / stride).floor()));
    usize::try_from(rows).unwrap_or(0)
}
