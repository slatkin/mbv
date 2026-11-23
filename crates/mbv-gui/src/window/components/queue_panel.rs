//! The Queue panel (`replace-tauri-with-blitz` task 5.5, the port of
//! `QueuePanel`): the canvas header (`QueueHeader`), the delineated fixed
//! 40 px rows, flush against one another, with no native DOM scroll — the core
//! owns scroll and sends the viewport rows to paint — and the canvas footer
//! line is `QueueFooter`'s. The component measures the list and reports the
//! whole rows it fits plus the painted regions through its one region set
//! (design D1 of `gui-tech-debt-simplification`).
//!
//! The rows paint the canvas's states from the frame's per-row
//! `GuiSemanticState` (design D9 of `gui-queue-panel-redesign`, restyled by
//! design D3 of `gui-delineated-rows`): a watched (`played`) row strikes its
//! grey title through, the now-playing row fills green and paints its title in
//! green at weight 600, and a row with progress — a partial row, and the
//! playing row, which follows the playhead — carries the progress ring after
//! its title. The ring is the row's only progress mark and adds no height. A
//! 1 px `--canvas-border` divider runs under every row, so the flush rows read
//! apart without a gap. Every row leads with its padded queue ordinal, carries
//! one title line, and paints the item's duration in its right column in every
//! state — no second line, no date gutter, no time left, and no state glyph.
//! The rows are square-cornered, the now-playing row included (user request
//! 2026-11-20). The rows paint three fills: the now-playing row's green fill, a
//! marked row's raised fill, and the hovered row's own fill; every other row
//! paints none. The cursor slot paints nothing: only a multi-selection shows as
//! selected. A marked row's look needs no focus fact: the window holds no focus,
//! and the pointer resting on a marked row leaves its fill in place.
//!
//! The row hover and the drag paint from the frame too — the window holds
//! neither (the `gui-client` spec): `queue.hover` names the row that paints
//! the hover fill (the row's context menu opens on a right-click, and a drag
//! starts on motion after a press anywhere in the row — user request
//! 2026-11-23; the window paints no hover chrome), and `queue.drag`
//! carries the drag ghost's content and the insertion
//! line's stable-slot anchor. The ghost floats at the line the drop would
//! insert at, with no ordinal, no right column, and the dragged row's ring and
//! percent when it has progress; the dragged row no longer dims in place (the
//! canvas keeps it lit). Every
//! pointer press, move, and release forwards through the shared input
//! installer — the core owns the gesture, the drop index, and the move
//! request.
//!
//! The regions report in paint order: the header's controls resolve to their
//! `queueChooser`, `newQueue`, `closeQueue`, and `filter` targets, then the
//! rows resolve to their stable slots. The switcher always paints and
//! so always claims a pointer; the close control claims one only while it acts,
//! so the panel marks it only then. The open row menu's entries
//! (`QueueRowMenu`, presentation-only) report their `rowMenuEntry` indices
//! through this panel's region set's overlay 0, and the open Queue chooser's
//! entries (`QueueChooserMenu`, presentation-only) report through overlay 1 —
//! the same technique `TrackChips` uses for its popover rows — so they land
//! after every region the panel reports and win the hit map where the menu or
//! the dropdown paints over a row beneath it. Each chooser entry names the
//! identity the window painted: the local queue's own, or the peer entry.
//!
//! The open Queue chooser's dropdown (task 8.3) is `QueueChooserMenu`'s: the
//! panel builds it from the overlay and mounts it in the header's switcher
//! anchor (the header carries no chooser facts). The footer totals are the
//! Queue footer line's (task 8.5).
//!
//! The row paint is the redesign's (design D9): the row states come through
//! the queue column's `--ef-*` tokens (design D1), so no theme slot or TUI
//! palette value moves (U13). Three fills still name role values because the
//! frame carries roles and the chrome fills, not surface fills (design D7):
//! the recessed box names the chrome's `queuePanel` resting fill —
//! `Surface::QueuePanel` — and the drag's insertion line and ghost name the
//! `insertion` and `ghost` canvas tokens. None of the box's follows any focus
//! fact: the GUI holds one fill for the queue at every state (user decision
//! 2026-11-15), unlike the TUI's focus-reactive `queue_row_background(focused)`.
//! The box's name is not part of the window's `--mbv-*` token set, so its
//! `var()` is invalid at computed-value time and the fill stays transparent,
//! as it painted in the reference build (`window/theme.rs::write_chrome` holds
//! the rule and the reason); the queue column's own backdrop shows through.

use dioxus_native::prelude::*;

use crate::bridge::frame::overlay::GuiOverlayFrame;
use crate::bridge::frame::theme::GuiRole;
use crate::bridge::layout::{ListId, QueueTargetId, RegionTarget};
use crate::window::components::queue_chooser_menu::{QueueChooserMenu, QueueChooserMenuState};
use crate::window::components::queue_header::QueueHeader;
use crate::window::components::queue_row_menu::QueueRowMenu;
use crate::window::layout::region_set::{MarkHandler, use_region_set};
use crate::window::pointer_target::PointerZone;
use crate::window::sections::Sections;
use crate::window::theme::role_var;

pub(super) mod geometry;
mod row;
mod row_cells;

use geometry::{
    GHOST_LEFT_PX, GHOST_RIGHT_OVERHANG_PX, LIST_TOP_GAP_PX, ROW_GAP_PX, ROW_HEIGHT_PX, ghost_top,
    insertion, menu_top, whole_rows,
};
use row::queue_rows;
use row_cells::{ItemRow, RowCells, RowChrome, row_look};

#[cfg(test)]
mod tests;

/// The Queue panel (task 5.5). The panel reads the `queue`, `summary`,
/// and `overlay` sections and the playhead's prediction; its one region set
/// reports each paint that moved a region, and the layout epoch — the
/// `ResizeObserver` the former effect watched (D9) — re-reports on every layout
/// change.
#[component]
pub fn QueuePanel() -> Element {
    let sections = consume_context::<Sections>();

    // Each paint that moved a region reports (design D1): the panel's one region
    // set watches the `queue`, `summary`, and `overlay` sections, and the
    // layout epoch through the set's own read. Reading the overlay registers
    // the menu's entries, which appear and disappear with that frame section,
    // so a closed menu's overlay retracts. The header's controls and the rows
    // are the base contribution, the open row menu's entries overlay 0, and the
    // open Queue chooser's entries overlay 1, in that order.
    let watch_queue = sections.queue;
    let watch_summary = sections.summary;
    let watch_overlay = sections.overlay;
    let region_set = use_region_set(2, move || {
        let _queue = watch_queue.cloned();
        let _summary = watch_summary.cloned();
        let _overlay = watch_overlay.cloned();
    });

    // The queue section travels as one handle (`share-gui-sections-by-rc`
    // design D3): a render clones the handle, never the rows.
    let queue = sections.queue.cloned();
    let summary = sections.summary.cloned();
    let overlay = sections.overlay.cloned();

    // The open row menu, or `None` while another overlay (or none) is up.
    let menu = match overlay.as_ref() {
        Some(frame @ GuiOverlayFrame::RowMenu { .. }) => Some(frame.clone()),
        _ => None,
    };
    let insertion = insertion(&queue);
    // The drag ghost's content, `None` while no drag runs or the ghost row is
    // structural (the core builds it from an item row).
    let ghost = queue
        .drag
        .as_ref()
        .and_then(|drag| ItemRow::of(&drag.ghost));
    let ghost_top = insertion.as_ref().map(ghost_top);
    let menu_open = menu.clone().and_then(|frame| match frame {
        GuiOverlayFrame::RowMenu {
            cursor, entries, ..
        } => Some((cursor, entries)),
        _ => None,
    });
    let menu_top = menu.as_ref().map(|frame| match frame {
        GuiOverlayFrame::RowMenu { anchor_slot, .. } => menu_top(&queue, *anchor_slot),
        _ => String::new(),
    });
    // The open Queue chooser, or `None` while another overlay (or none) is up:
    // the core's cursor, the entries in paint order, and the rename draft. The
    // header's switcher anchor paints the card from these facts; the entries'
    // regions stay this panel's to report (design D8).
    let chooser_menu = QueueChooserMenuState::from_overlay(overlay.as_ref());
    // The recessed box's fill: the chrome's `queuePanel` resting fill at
    // every state (user decision 2026-11-15). The property is undefined
    // on purpose (`window/theme.rs::write_chrome`), so the box paints no fill
    // of its own and the backdrops behind it show.
    let box_background = "var(--mbv-queuePanel)";

    // The marks, in the report's order (design D1): the header's four controls
    // first — the Queue switcher, the new-queue control, the close control, and
    // the filter (design D7 of `gui-queue-panel-redesign`) — then the rows.
    // The switcher and the filter always paint, so they always report; a
    // control that does not act claims no pointer, so the close control is
    // marked only while it acts, and the header attaches its mark only then.
    let chooser_mark = region_set.mark(RegionTarget::Queue(QueueTargetId::QueueChooser));
    let new_queue_mark = region_set.mark(RegionTarget::Queue(QueueTargetId::NewQueue));
    let close_mark = (!summary.header.close_disabled)
        .then(|| region_set.mark(RegionTarget::Queue(QueueTargetId::CloseQueue)));
    let rename_mark = (!summary.header.rename_disabled)
        .then(|| region_set.mark(RegionTarget::Queue(QueueTargetId::RenameQueue)));
    let filter_mark = region_set.mark(RegionTarget::Queue(QueueTargetId::Filter));
    // The painted rows' slot marks, in paint order.
    let slot_marks: Vec<(u64, MarkHandler)> = queue
        .rows
        .iter()
        .filter_map(ItemRow::of)
        .map(|item| {
            let slot = item.slot;
            let mark = region_set.mark(RegionTarget::Queue(QueueTargetId::Slot(slot)));
            (slot, mark)
        })
        .collect();
    // The open row menu's entries mark overlay 0, one per painted row in row
    // order, and the open Queue chooser's entries mark overlay 1 the same way,
    // each naming the identity the window paints — a local queue's own, or the
    // peer entry. A closed menu marks none, so its overlay retracts.
    let menu_marks = menu_open.as_ref().map_or_else(Vec::new, |(_, entries)| {
        (0..entries.len())
            .map(|index| {
                region_set.mark_over(0, RegionTarget::Queue(QueueTargetId::RowMenuEntry(index)))
            })
            .collect()
    });
    let chooser_marks = chooser_menu.as_ref().map_or_else(Vec::new, |menu| {
        menu.entries
            .iter()
            .map(|entry| {
                let target = match entry.queue {
                    Some(raw) => QueueTargetId::QueueChooserEntry(raw),
                    None => QueueTargetId::QueueChooserPeer,
                };
                region_set.mark_over(1, RegionTarget::Queue(target))
            })
            .collect()
    });

    rsx! {
        // The TUI's queue panel arrangement (the `queue_panel_geometry`
        // structure), in cell units, with the canvas's queue header inside
        // the recessed box: the placement spans the full queue column, and
        // the box (`QueuePanel` fill, with the header row at its top and its
        // one-row bottom spacer) sits two columns inside it —
        // `queue_list_box` insets from `placement.x + 2`, the same +2 columns
        // the playback band's `queue_panel_inset` sits at, so every band edge
        // aligns. The canvas footer line below the box is `QueueFooter`'s.
        div {
            class: "flex h-full flex-col",
            div {
                class: "relative flex min-h-0 flex-1 flex-col",
                style: "margin-inline: calc(var(--mbv-cell-col) * 2); background: {box_background};",
                "data-queue-box": "",
                // The Queue zone (`gui-pointer-target-interaction` D1): the
                // header, the filter, and the rows. The footer band under the
                // box carries the same marker (`queue_footer.rs`), and the
                // player block above it carries none — a key over the player
                // block keeps only the global chords.
                "data-pointer-zone": PointerZone::Queue.token(),
                QueueHeader {
                    summary: summary.clone(),
                    chooser_open: chooser_menu.is_some(),
                    chooser: chooser_mark,
                    filter: filter_mark,
                    new_queue: new_queue_mark,
                    close: close_mark,
                    rename: rename_mark,
                    if let Some(menu) = chooser_menu {
                        QueueChooserMenu {
                            cursor: menu.cursor,
                            entries: menu.entries,
                            renaming: menu.renaming,
                            row_marks: chooser_marks,
                        }
                    }
                }
                div {
                    class: "relative min-h-0 flex-1 overflow-hidden",
                    style: "margin-top: {LIST_TOP_GAP_PX}px; padding-inline: 8px; display: flex; flex-direction: column; gap: {ROW_GAP_PX}px;",
                    "data-queue-list": "",
                    onmounted: region_set.mark_list(ListId::Queue, whole_rows),
                    {queue_rows(&queue, insertion.as_ref(), &slot_marks)}
                }
                if let Some(scroll) = queue.scroll {
                    // The scroll bar: a readout of the core's scroll, set
                    // halfway between the rows' end and the queue panel's
                    // edge (the box's two-column inset, half of it outside
                    // the box), so it never crowds a row. It spans the
                    // list's rows, under the header and over the spacer.
                    // The thumb is the viewport's share of the list; the
                    // pointer passes through it.
                    {
                        let (thumb_top, thumb_length) = scroll.thumb_percent();
                        rsx! {
                            div {
                                class: "pointer-events-none absolute",
                                style: "top: var(--canvas-queue-header-height); bottom: var(--mbv-cell-row); right: calc(var(--mbv-cell-col) * -1 + 0.5px); width: 6px;",
                                "data-scrollbar": "",
                                div {
                                    class: "absolute",
                                    style: "left: 0; right: 0; top: {thumb_top}%; height: {thumb_length}%; min-height: 16px; background: {role_var(GuiRole::Scrollbar)}; border-radius: 3px;",
                                    "data-scrollbar-thumb": "",
                                }
                            }
                        }
                    }
                }
                if let (Some(top), Some(ghost_row)) = (ghost_top, ghost.as_ref()) {
                    // The drag ghost (the Menus artboard): the dragged row's
                    // title and its percent in a floating card at the
                    // insertion line, `pointer-events-none` so the pointer
                    // keeps resolving the regions beneath it. It sits outside
                    // the clipped list so its canvas overhang past the row's
                    // right edge paints, keeps the row's own insets so its
                    // cells line up with the row beneath, and paints no
                    // ordinal. The core owns the drag; the release's move
                    // request is its.
                    div {
                        class: "pointer-events-none absolute z-20",
                        style: format!(
                            "top: calc(var(--canvas-queue-header-height) + {top}px); left: {GHOST_LEFT_PX}px; right: {GHOST_RIGHT_OVERHANG_PX}px;"
                        ),
                        "data-ghost": "",
                        div {
                            class: "flex items-center overflow-hidden whitespace-nowrap",
                            style: "height: {ROW_HEIGHT_PX}px; padding-inline: 8px; gap: 10px; background: var(--canvas-ghost); border-radius: 6px; box-shadow: var(--canvas-ghost-shadow); transform: rotate(-1deg);",
                            RowCells {
                                row: ghost_row.clone(),
                                look: row_look(ghost_row, &queue),
                                chrome: RowChrome::Ghost,
                            }
                        }
                    }
                }
                // The recessed box's one-row bottom spacer: its own fill, so
                // the rows never touch the box's edges.
                div { style: "height: var(--mbv-cell-row);" }
                if let Some((cursor, entries)) = menu_open {
                    // The open row menu (design D15): the frame's entries and
                    // key hints, dropped over the bottom edge of the row the
                    // menu opened on, as the canvas places it. Its entries
                    // report through the set's overlay 0, last over the rows
                    // beneath (the module comment names why).
                    QueueRowMenu {
                        cursor: Some(cursor),
                        entries,
                        top: menu_top.unwrap_or_default(),
                        row_marks: menu_marks,
                    }
                }
            }
        }
    }
}
