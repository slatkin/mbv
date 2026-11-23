//! One painted queue row's card (the row `QueuePanel` paints per slot): the row
//! button with its canvas cells, the 1 px divider under it, and the drag's
//! insertion line. The row paints no chrome of its own: the hover names the
//! row through its fill only, and a drag starts on motion after a press
//! anywhere in the row (user request 2026-11-23; the former six-dot grip is
//! gone). The structural rows (a group heading
//! and the plain spacer) paint here too.
//!
//! The panel owns every mark and hands the row's closure down (design D1 of
//! `gui-tech-debt-simplification`): the `slot` mark the row attaches to its
//! button. A
//! plain function assembles each row, not a component: the panel's marks run
//! in the panel's own render, and the paint must stay the same element tree
//! with no extra wrapper.

use std::fmt::Write as _;

use dioxus_native::prelude::*;

use crate::bridge::frame::queue::{GuiQueue, GuiQueueRow};
use crate::bridge::frame::theme::GuiRole;
use crate::window::layout::region_set::MarkHandler;
use crate::window::theme::{canvas_var, role_var};

use super::geometry::{Insertion, ROW_CELL_GAP_PX, ROW_HEIGHT_PX, ROW_INSET_PX};
use super::row_cells::{ItemRow, RowCells, RowChrome, row_look};

/// The row's corner radius in logical pixels (design D3; user request
/// 2026-11-20): every row is square, the now-playing row included.
const ROW_RADIUS_PX: u16 = 0;

/// The empty-queue text, as the TUI's local queue shows it. The frame
/// carries no queue link, so the remote-queue text cannot be selected; the
/// first slice attaches only to a local Owner (design D9). The canvas draws
/// no empty-queue state (design D15), so the placeholder paints in the
/// canvas's type — a display-sized medium weight in the `emptyQueueFg` role
/// — as a missing visual does.
const EMPTY_QUEUE_TEXT: &str = "¯\\_(ツ)_/¯";

/// One painted item row: the item's extracted facts, the queue the row reads
/// its fill and hover from, the drag's insertion line, and the mark the
/// panel made for it.
pub(super) struct RowPaint<'a> {
    pub(super) item: ItemRow,
    pub(super) queue: &'a GuiQueue,
    /// The drag's insertion line, `None` while no drag runs; the row paints a
    /// line only while the line's anchor is its own slot.
    pub(super) insertion: Option<&'a Insertion>,
    /// The row button's `slot` mark.
    pub(super) slot_mark: MarkHandler,
}

/// The row's inline size: the fixed row height, the square corners the canvas
/// draws, and the row's own fill where it paints one.
fn row_button_style(background: Option<&'static str>) -> String {
    let mut style = format!(
        "height: {ROW_HEIGHT_PX}px; padding-inline: {ROW_INSET_PX}px; gap: {ROW_CELL_GAP_PX}px; border-radius: {ROW_RADIUS_PX}px;"
    );
    if let Some(background) = background {
        let _ = write!(style, " background: {background};");
    }
    style
}

/// The drag's insertion line resolved against the row holding `slot`: its
/// position on the row's top edge (`before`) or in the gap under its bottom
/// edge (`after`), where the ghost's top lands, and the line's `data-insertion`
/// value. `None` while no line anchors on this row.
fn insertion_line(insertion: Option<&Insertion>, slot: u64) -> Option<(String, &'static str)> {
    let line = insertion.filter(|line| line.slot == slot)?;
    let position = if line.before {
        "top: 0;".to_owned()
    } else {
        format!("top: {ROW_HEIGHT_PX}px;")
    };
    let at = if line.before { "before" } else { "after" };
    Some((position, at))
}

/// One item row's card: the row button with its cells, the divider under it,
/// and the drag's insertion line. The insertion line is a sibling of the row's
/// button inside the positioning box, so no control nests in the row's own
/// button.
pub(super) fn queue_row_card(paint: RowPaint<'_>) -> Element {
    let RowPaint {
        item,
        queue,
        insertion,
        slot_mark,
    } = paint;
    let slot = item.slot;
    let look = row_look(&item, queue);
    let button_style = row_button_style(look.background);
    let line = insertion_line(insertion, slot);
    rsx! {
        div {
            key: "{slot}",
            class: "relative",
            "data-row": "{slot}",
            button {
                "type": "button",
                class: "relative flex w-full items-center overflow-hidden text-left whitespace-nowrap",
                style: "{button_style}",
                "data-slot": "{slot}",
                onmounted: slot_mark,
                RowCells { row: item, look, chrome: RowChrome::Row }
            }
            // The 1 px divider under the row (design D3): the rows sit flush
            // (`ROW_GAP_PX` 0), so the line is the row's own bottom edge and
            // adds no height to it.
            span {
                class: "pointer-events-none absolute right-0 left-0",
                style: "bottom: 0; height: 1px; background: {canvas_var(\"border\")};",
                "data-divider": "",
            }
            if let Some((line_position, line_at)) = line {
                // The drag's insertion line, anchored on this row's top edge
                // (`before`) or under its bottom edge (`after`), inset from the
                // row's edges as the canvas draws it.
                span {
                    class: "pointer-events-none absolute right-0 left-0 z-10",
                    style: "height: 2px; {line_position} margin: 0 8px; border-radius: 1px; background: {canvas_var(\"insertion\")};",
                    "data-insertion": "{line_at}",
                }
            }
        }
    }
}

/// One structural row: the canvas's group heading, or the row-height spacer
/// every other structural row paints.
pub(super) fn structural_row(row: &GuiQueueRow, index: usize) -> Element {
    match row {
        GuiQueueRow::Heading { text } => {
            let heading = text.clone();
            rsx! {
                div {
                    key: "heading-{index}",
                    class: "flex items-center overflow-hidden uppercase",
                    style: "height: {ROW_HEIGHT_PX}px; padding-inline: calc(var(--mbv-cell-col) * 2); color: {role_var(GuiRole::GroupHeadingFg)};",
                    "{heading}"
                }
            }
        }
        _ => rsx! {
            div { key: "spacer-{index}", style: "height: {ROW_HEIGHT_PX}px;" }
        },
    }
}

/// The mark of the row the panel painted for `slot`: every painted item row
/// marked its slot this render, and the rows look their mark up by slot. The
/// tripwire stays loud, because a row that mounted without its mark would
/// report no region.
fn slot_mark_for(marks: &[(u64, MarkHandler)], slot: u64) -> MarkHandler {
    marks
        .iter()
        .find(|(held, _)| *held == slot)
        .map(|(_, mark)| *mark)
        .expect("a painted item row marked its slot")
}

/// The list's rows: the empty-queue placeholder, or one card per projected
/// row in paint order, each resolving its own mark from the panel's slot marks.
pub(super) fn queue_rows(
    queue: &GuiQueue,
    insertion: Option<&Insertion>,
    slot_marks: &[(u64, MarkHandler)],
) -> Element {
    if queue.rows.is_empty() {
        // The empty-queue placeholder (design D15's missing-visual rule): the
        // TUI's shrug set as the canvas's display type — display-sized, medium
        // weight, wide tracking — in the `emptyQueueFg` role, centred in the
        // list box.
        return rsx! {
            div {
                class: "flex h-full items-center justify-center",
                span {
                    class: "text-2xl font-medium tracking-widest",
                    style: "color: {role_var(GuiRole::EmptyQueueFg)};",
                    "{EMPTY_QUEUE_TEXT}"
                }
            }
        };
    }
    rsx! {
        for (index, row) in queue.rows.iter().enumerate() {
            { match ItemRow::of(row) {
                Some(item) => {
                    let slot = item.slot;
                    queue_row_card(RowPaint {
                        item,
                        queue,
                        insertion,
                        slot_mark: slot_mark_for(slot_marks, slot),
                    })
                }
                None => structural_row(row, index),
            } }
        }
    }
}
