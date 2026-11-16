//! The Queue panel (`replace-tauri-with-blitz` task 5.5, the port of
//! `QueuePanel`): the canvas header (`QueueHeader`), fixed 30 px rows
//! with no native DOM scroll — the core owns scroll and sends the viewport
//! rows to paint — and the canvas footer line is `QueueFooter`'s. The
//! component measures the list and reports the whole 30 px rows it fits
//! plus the painted regions through the shared layout reporter.
//!
//! The rows paint the canvas's states from the frame's per-row
//! `GuiSemanticState`: a watched (`played`) row strikes its text through,
//! the now-playing row shows a play glyph beside its title (the highlight is
//! for focus only, so the row keeps its own fill), and a partial-progress
//! row (and the playing row) draws a thin progress track on the title's
//! line. Every row leads with its padded queue ordinal and carries one
//! title line and the right-column value — no second line and no date
//! gutter, as the canvas's row draws. The rows paint four looks (design D7):
//! a hovered row paints the window's hover veil and no fill of its own, the
//! active row (the cursor slot) paints the canvas's picked fill with a 4 px
//! leading accent edge, a marked row paints the picked fill with no edge,
//! and every other row paints the zebra stripe or nothing. The active row's
//! look needs no focus fact: the window holds no focus, and the pointer
//! resting on the active row leaves its fill and edge in place. The right
//! column paints the value the summary's mode names: "Length" or "Ends at".
//!
//! The row hover and the drag paint from the frame too — the window holds
//! neither (the `gui-client` spec): `queue.hover` names the row that shows
//! the six-dot grip in its ordinal's slot and the canvas's 30 px actions
//! button in its right value's slot — the two chrome pieces stay siblings of
//! the row's one button, over slots the row reserves, so no control nests in
//! it — and `queue.drag` carries the drag ghost's content and the insertion
//! line's stable-slot anchor. The ghost floats at the line the drop would
//! insert at, with no ordinal and the dragged row's percent when it has one;
//! the dragged row no longer dims in place (the canvas keeps it lit). Every
//! pointer press, move, and release forwards through the shared input
//! installer — the core owns the gesture, the drop index, and the move
//! request.
//!
//! The regions report in paint order: the header's Length / Ends-at switch
//! and filter resolve to their `rightColumn` and `filter` targets first —
//! the switch's inactive segment's mounted handle, by the summary's mode,
//! and the filter's — then the rows resolve to their stable slots, then the
//! hovered row's `rowGrip` and `rowActions`
//! — pushed after the row's `slot` region so the chrome wins where it paints
//! over the row (the hit map's last push wins). The open row menu's entries
//! (`QueueRowMenu`, presentation-only) report their `rowMenuEntry` indices
//! under a second report key this component inserts when the menu opens —
//! the same technique `TrackChips` uses for its popover rows — so they land
//! after every region the panel reports and win the hit map where the menu
//! paints over a row beneath it.
//!
//! The row paint is the canvas's (U4.2): its hexes come through the
//! queue-local `--canvas-*` tokens, so no theme slot or TUI palette value
//! moves (U13). Two fills still name role values because the frame carries
//! roles and the chrome fills, not surface fills (design D7): the recessed
//! box names the chrome's `queuePanel` resting fill — `Surface::QueuePanel`
//! — and the zebra stripe the `queueColumn` resting fill. Neither follows
//! any focus fact: the GUI holds one fill for the queue at every state (user
//! decision 2026-11-15), unlike the TUI's focus-reactive
//! `queue_row_background(focused)`. Their names are not part of the
//! window's `--mbv-*` token set, so each `var()` is invalid at
//! computed-value time and the fill stays transparent, as it painted in the
//! reference build (`window/theme.rs::write_chrome` holds the rule and the
//! reason); the queue column's own backdrop shows through.

use std::fmt::Write as _;

use dioxus_native::prelude::*;

use crate::bridge::frame::overlay::GuiOverlayFrame;
use crate::bridge::frame::queue::{GuiQueue, GuiQueueInsertion, GuiQueueRow, GuiRightColumn};
use crate::bridge::frame::theme::GuiRole;
use crate::bridge::layout::{LayoutRegion, ListId, ListRows, QueueTargetId, RegionTarget};
use crate::window::components::queue_header::QueueHeader;
use crate::window::components::queue_row_menu::QueueRowMenu;
use crate::window::icons::{Ellipsis, GripVertical};
use crate::window::layout::{
    LayoutContribution, LayoutEpoch, LayoutKey, LayoutReporter, element_region,
};
use crate::window::pointer_target::PointerZone;
use crate::window::sections::Sections;
use crate::window::theme::{canvas_var, role_var};

mod row_cells;

use row_cells::{ItemRow, RowCells, RowChrome, tone};

#[cfg(test)]
mod tests;

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
const GRIP_SIZE_PX: u16 = 14;
const GRIP_TOP_PX: f32 = (ROW_HEIGHT - GRIP_SIZE_PX as f32) / 2.0;

/// The empty-queue text, as the TUI's local queue shows it. The frame
/// carries no queue link, so the remote-queue text cannot be selected; the
/// first slice attaches only to a local Owner (design D9). The canvas draws
/// no empty-queue state (design D15), so the placeholder paints in the
/// canvas's type — a display-sized medium weight in the `emptyQueueFg` role
/// — as a missing visual does.
const EMPTY_QUEUE_TEXT: &str = "¯\\_(ツ)_/¯";

/// The active row's leading edge: its width in logical pixels, and the radius
/// that follows the row's own leading corners.
const ACTIVE_EDGE_PX: u16 = 4;

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
fn row_background(row: &ItemRow, queue: &GuiQueue) -> Option<String> {
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
struct Insertion {
    before: bool,
    slot: u64,
    index: usize,
}

fn insertion(queue: &GuiQueue) -> Option<Insertion> {
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
fn ghost_top(insertion: &Insertion) -> usize {
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
fn menu_top(queue: &GuiQueue, anchor_slot: u64) -> String {
    let index = queue.rows.iter().position(
        |row| matches!(row, GuiQueueRow::Item { slot: held, .. } if *held == anchor_slot),
    );
    let below = index.map_or(0, |index| (index + 1) * ROW_HEIGHT_PX - 6);
    format!("calc(var(--canvas-queue-header-height) + {below}px)")
}

/// The whole 30 px rows a list box of `height` CSS pixels fits (the former client
/// report's `wholeRows`): the capacity the core's queue viewport reads.
fn whole_rows(height: f32) -> usize {
    let rows = mbv_emby_model::saturating_i64_from_f64(f64::from(height / ROW_HEIGHT).floor());
    usize::try_from(rows).unwrap_or(0)
}

/// The Queue panel (task 5.5). The panel reads the `queue`, `summary`,
/// and `overlay` sections and the playhead's prediction; each paint
/// that moved a region reports, and the layout epoch — the `ResizeObserver`
/// the former effect watched (D9) — re-reports on every layout change.
#[component]
pub fn QueuePanel() -> Element {
    let sections = consume_context::<Sections>();
    let epoch = consume_context::<LayoutEpoch>();
    let reporter = consume_context::<LayoutReporter>();
    let key = use_hook(LayoutKey::next);
    let menu_key = use_hook(LayoutKey::next);

    let mut list_mounted: Signal<Option<Event<MountedData>>> = use_signal(|| None);
    // The header's controls' handles: the two
    // switch segments' mounted events, keyed by the mode each paints, and
    // the filter control's mounted event. The panel picks the inactive
    // segment's handle from the summary's mode.
    let right_column_handles: Signal<Vec<(GuiRightColumn, Event<MountedData>)>> =
        use_signal(Vec::new);
    let filter_handle: Signal<Option<Event<MountedData>>> = use_signal(|| None);
    // The painted rows' mounted events, by their stable slot — the handles
    // the row regions read (the former client `querySelectorAll('[data-slot]')`).
    let slot_handles: Signal<Vec<(u64, Event<MountedData>)>> = use_signal(Vec::new);
    // The hovered row's chrome handles: the grip and the actions button, one
    // painted at a time (the former client `querySelectorAll('[data-grip]')` and
    // `'[data-actions]'`).
    let grip_mounted: Signal<Option<(u64, Event<MountedData>)>> = use_signal(|| None);
    let actions_mounted: Signal<Option<(u64, Event<MountedData>)>> = use_signal(|| None);
    // The open row menu's painted entries, by their entry index (the former client
    // `querySelectorAll('[data-menu-entry]')`).
    let mut menu_rows: Signal<Vec<Option<Event<MountedData>>>> = use_signal(Vec::new);

    // No region outlives the panel: the contributions retract on unmount, as
    // the former client `onMount` teardown did.
    let mut drop_reporter = reporter.clone();

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
    // The recessed box's fill: the chrome's `queuePanel` resting fill at
    // every state (user decision 2026-11-15). The property is undefined
    // on purpose (`window/theme.rs::write_chrome`), so the box paints no fill
    // of its own and the backdrops behind it show.
    let box_background = "var(--mbv-queuePanel)";

    // Each paint that moved a region reports (design D4): this effect runs
    // after the DOM update, and the layout epoch reports layout-driven moves.
    // Reading the overlay registers the menu's entries, which appear and
    // disappear with that frame section; the menu's key retracts while none
    // is open. The reporter dedupes an unchanged contribution, so a frame
    // that moves only the cursor sends nothing.
    let watch_queue = sections.queue;
    let watch_summary = sections.summary;
    let watch_overlay = sections.overlay;
    use_effect(move || {
        // The effect reads its own handle to the queue, so a batch that wrote
        // the section re-reports while a render that read it again would not.
        let queue = watch_queue.cloned();
        // The summary's changes move the header's controls: reading it keeps
        // the re-report on their paint, and its mode picks which switch
        // segment reports.
        let summary = watch_summary.cloned();
        let overlay_open = watch_overlay.cloned();
        let _epoch = epoch.0.cloned();
        let list = list_mounted.cloned();
        let slots = slot_handles.cloned();
        let grip = grip_mounted.cloned();
        let actions = actions_mounted.cloned();
        let menu_row_handles = menu_rows.cloned();
        let right_column = right_column_handles.cloned();
        let filter = filter_handle.cloned();
        let menu = match overlay_open.as_ref() {
            Some(frame @ GuiOverlayFrame::RowMenu { .. }) => Some(frame.clone()),
            _ => None,
        };
        let mut effect_reporter = reporter.clone();
        spawn(async move {
            // A shrinking entries list drops the stale row handles (the
            // track chips' idiom).
            let entry_count = menu.as_ref().map_or(0, |frame| match frame {
                GuiOverlayFrame::RowMenu { entries, .. } => entries.len(),
                _ => 0,
            });
            if menu_rows.peek().len() > entry_count {
                menu_rows.write().truncate(entry_count);
            }
            let mut regions: Vec<LayoutRegion> = Vec::new();
            // The queue header's `rightColumn` and `filter` segments report
            // first: the switch's inactive segment — the active one reports
            // no target, a press on it would toggle away from the mode it
            // shows — and the filter control.
            if let Some((_, mounted)) = right_column
                .iter()
                .find(|(mode, _)| *mode != summary.right_column)
            {
                regions.extend(
                    element_region(mounted, RegionTarget::Queue(QueueTargetId::RightColumn)).await,
                );
            }
            if let Some(mounted) = filter.as_ref() {
                regions.extend(
                    element_region(mounted, RegionTarget::Queue(QueueTargetId::Filter)).await,
                );
            }
            for slot in queue
                .rows
                .iter()
                .filter_map(|row| ItemRow::of(row).map(|item| item.slot))
            {
                if let Some((_, mounted)) = slots.iter().find(|(held, _)| *held == slot) {
                    regions.extend(
                        element_region(mounted, RegionTarget::Queue(QueueTargetId::Slot(slot)))
                            .await,
                    );
                }
            }
            if let Some(hover) = queue.hover {
                for (chrome, id) in [
                    (grip.as_ref(), QueueTargetId::RowGrip(hover)),
                    (actions.as_ref(), QueueTargetId::RowActions(hover)),
                ] {
                    if let Some((held, mounted)) = chrome
                        && *held == hover
                    {
                        regions.extend(element_region(mounted, RegionTarget::Queue(id)).await);
                    }
                }
            }
            let rows = match list {
                Some(list) => list.get_client_rect().await.map_or(0, |rect| {
                    whole_rows(crate::window::input::f32_from_f64(rect.height()))
                }),
                None => 0,
            };
            effect_reporter.report_layout(
                key,
                LayoutContribution {
                    lists: vec![ListRows {
                        list: ListId::Queue,
                        whole_rows: rows,
                    }],
                    regions,
                },
            );
            if menu.is_some() {
                let mut menu_regions: Vec<LayoutRegion> = Vec::new();
                for (index, mounted) in menu_row_handles.iter().enumerate() {
                    if let Some(mounted) = mounted {
                        menu_regions.extend(
                            element_region(
                                mounted,
                                RegionTarget::Queue(QueueTargetId::RowMenuEntry(index)),
                            )
                            .await,
                        );
                    }
                }
                effect_reporter.report_layout(
                    menu_key,
                    LayoutContribution {
                        lists: Vec::new(),
                        regions: menu_regions,
                    },
                );
            } else {
                effect_reporter.retract_layout(menu_key);
            }
        });
    });

    use_drop(move || {
        drop_reporter.retract_layout(key);
        drop_reporter.retract_layout(menu_key);
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
                    right_column: right_column_handles,
                    filter: filter_handle,
                }
                div {
                    class: "relative min-h-0 flex-1 overflow-hidden",
                    style: "padding-inline: 8px;",
                    "data-queue-list": "",
                    onmounted: move |event| list_mounted.set(Some(event)),
                    if queue.rows.is_empty() {
                        // The empty-queue placeholder (design D15's
                        // missing-visual rule): the TUI's shrug set as the
                        // canvas's display type — display-sized, medium
                        // weight, wide tracking — in the `emptyQueueFg` role,
                        // centred in the list box.
                        div {
                            class: "flex h-full items-center justify-center",
                            span {
                                class: "text-2xl font-medium tracking-widest",
                                style: "color: {role_var(GuiRole::EmptyQueueFg)};",
                                "{EMPTY_QUEUE_TEXT}"
                            }
                        }
                    } else {
                        for (index, row) in queue.rows.iter().enumerate() {
                            { match ItemRow::of(row) {
                                Some(item) => {
                                    let hovered = queue.hover == Some(item.slot);
                                    let slot = item.slot;
                                    // The active row (design D7): the cursor slot,
                                    // which carries the leading edge at all times.
                                    let active_row = queue.cursor == Some(slot);
                                    let mut button_style = format!(
                                        "height: 30px; padding-inline: {}; gap: 10px; border-radius: 4px;",
                                        if hovered { "4px 2px" } else { "8px" }
                                    );
                                    if let Some(background) = row_background(&item, &queue) {
                                        let _ = write!(button_style, " background: {background};");
                                    }
                                    let line = insertion
                                        .as_ref()
                                        .filter(|line| line.slot == slot)
                                        .map(|line| {
                                            (
                                                if line.before { "top: 0;" } else { "bottom: 0;" },
                                                if line.before { "before" } else { "after" },
                                            )
                                        });
                                    let cells_row = item.clone();
                                    let mut slot_slot = slot_handles;
                                    let mut grip_slot = grip_mounted;
                                    let mut actions_slot = actions_mounted;
                                    rsx! {
                                        // The row is one button; its hover chrome and the
                                        // drag's insertion line are siblings inside the
                                        // positioning box, so no control nests in the row's
                                        // own button.
                                        div {
                                            key: "{slot}",
                                            class: "relative",
                                            "data-row": "{slot}",
                                            button {
                                                "type": "button",
                                                class: "flex w-full items-center overflow-hidden text-left whitespace-nowrap",
                                                style: "{button_style}",
                                                "data-slot": "{slot}",
                                                onmounted: move |event| {
                                                    let mut handles = slot_slot.write();
                                                    if let Some(entry) =
                                                        handles.iter_mut().find(|(held, _)| *held == slot)
                                                    {
                                                        entry.1 = event;
                                                    } else {
                                                        handles.push((slot, event));
                                                    }
                                                },
                                                RowCells {
                                                    row: cells_row,
                                                    cell_tone: tone(&item, queue.hover),
                                                    chrome: if hovered { RowChrome::Hover } else { RowChrome::Row },
                                                    right_mode: summary.right_column,
                                                }
                                            }
                                            if active_row {
                                                // The active row's leading accent edge (design
                                                // D7): 4 px inside the row's leading edge,
                                                // following its rounded corners. It paints over
                                                // the row's fill, so the active row reads as
                                                // active under the pointer too.
                                                span {
                                                    class: "pointer-events-none absolute",
                                                    style: "top: 0; bottom: 0; left: 0; width: {ACTIVE_EDGE_PX}px; border-radius: 4px 0 0 4px; background: var(--canvas-accent);",
                                                    "data-active-edge": "{slot}",
                                                }
                                            }
                                            if let Some((line_position, line_at)) = line {
                                                // The drag's insertion line, anchored on this
                                                // row's top edge (`before`) or bottom edge
                                                // (`after`), inset from the row's edges as the
                                                // canvas draws it.
                                                span {
                                                    class: "pointer-events-none absolute right-0 left-0 z-10",
                                                    style: "height: 2px; {line_position} margin: 0 8px; border-radius: 1px; background: {canvas_var(\"insertion\")};",
                                                    "data-insertion": "{line_at}",
                                                }
                                            }
                                            if hovered {
                                                // The hover chrome (the Menus artboard): the
                                                // six-dot grip over the leading slot the row
                                                // reserves in place of its ordinal, and the
                                                // 30 px actions button over the trailing slot
                                                // in place of its right value. Each is its own
                                                // region, pushed after the row's `slot` region
                                                // so the press resolves to the chrome. They
                                                // stay siblings of the row's button, so no
                                                // control nests in it.
                                                span {
                                                    class: "absolute flex items-center",
                                                    style: "left: 4px; width: {GRIP_SIZE_PX}px; top: {GRIP_TOP_PX}px;",
                                                    "data-grip": "{slot}",
                                                    onmounted: move |event| grip_slot.set(Some((slot, event))),
                                                    GripVertical { size: GRIP_SIZE_PX, color: canvas_var("muted") }
                                                }
                                                button {
                                                    "type": "button",
                                                    class: "absolute flex items-center justify-center",
                                                    style: "top: 0; right: 2px; width: 30px; height: 30px; border-radius: 4px; background: var(--canvas-action);",
                                                    "data-actions": "{slot}",
                                                    "aria-label": "Row actions",
                                                    onmounted: move |event| actions_slot.set(Some((slot, event))),
                                                    Ellipsis {
                                                        size: 14,
                                                        fill: canvas_var("heading"),
                                                        color: canvas_var("heading"),
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                None => match row {
                                    GuiQueueRow::Heading { text } => {
                                        let heading = text.clone();
                                        rsx! {
                                            div {
                                                key: "heading-{index}",
                                                class: "flex items-center overflow-hidden uppercase",
                                                style: "height: 30px; padding-inline: calc(var(--mbv-cell-col) * 2); color: {role_var(GuiRole::GroupHeadingFg)};",
                                                "{heading}"
                                            }
                                        }
                                    }
                                    _ => rsx! {
                                        div { key: "spacer-{index}", style: "height: 30px;" }
                                    },
                                },
                            } }
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
                        style: "top: calc(var(--canvas-queue-header-height) + {top}px); left: 28px; right: -6px;",
                        "data-ghost": "",
                        div {
                            class: "flex items-center overflow-hidden whitespace-nowrap",
                            style: "height: 30px; padding-inline: 8px; gap: 10px; background: var(--canvas-ghost); border-radius: 4px; box-shadow: var(--canvas-ghost-shadow); transform: rotate(-1deg);",
                            RowCells {
                                row: ghost_row.clone(),
                                cell_tone: tone(ghost_row, queue.hover),
                                chrome: RowChrome::Ghost,
                                right_mode: summary.right_column,
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
                    // report their own regions under the component's key, last
                    // over the rows beneath (the module comment names why).
                    QueueRowMenu {
                        cursor: Some(cursor),
                        entries,
                        top: menu_top.unwrap_or_default(),
                        row_slot: menu_rows,
                    }
                }
            }
        }
    }
}
