//! The Queue panel's menu and drag contracts (`QueuePanel.test.ts`, the
//! Menus artboard): the open row menu's canvas chrome, hints, and
//! destructive entry, the drag's ghost and inset insertion line, and the
//! menu's anchor-row overlap.

use blitz_dom::local_name;

use super::harness::*;
use crate::bridge::frame::queue::{GuiQueueInsertion, GuiSemanticState};
use crate::window::components::queue_panel::geometry::{
    LIST_TOP_GAP_PX, MENU_OVERLAP_PX, ROW_GAP_PX, ROW_HEIGHT_PX,
};
use crate::window::theme::{EfToken, ef_var};

/// Contract (`QueuePanel.test.ts`, the Menus artboard B.2): the open row
/// menu paints the canvas chrome — a 230 px card in the menu fill, border,
/// radius, and shadow with 6 px padding and 2 px gap — 34 px entries at
/// 13 px, the muted mono hints with the paint-time `⏎` glyph for the core's
/// raw `Enter` chord, the cursor row in the selected fill, and the
/// destructive entry in the destructive tone above its divider.
#[test]
fn the_open_row_menu_paints_the_canvas_chrome_hints_and_destructive_entry() {
    let rows = vec![item(1)];
    let mut rows_frame = queue(rows);
    rows_frame.hover = Some(1);
    let fixture = built_with_frame(queue_panel_app, rows_frame, summary(), Some(row_menu()));

    let menu_style = fixture.style_of("[data-row-menu]");
    assert_eq!(
        fixture
            .attr_of("[data-row-menu]", local_name!("role"))
            .as_deref(),
        Some("menu")
    );
    assert_eq!(
        fixture
            .attr_of("[data-row-menu]", local_name!("aria-label"))
            .as_deref(),
        Some("Row actions")
    );
    assert!(menu_style.contains("width: 230px"), "{menu_style}");
    assert!(menu_style.contains("padding: 6px"), "{menu_style}");
    assert!(menu_style.contains("gap: 2px"), "{menu_style}");
    assert!(
        menu_style.contains("background: var(--canvas-menu)"),
        "{menu_style}"
    );
    assert!(
        menu_style.contains("border: 1px solid var(--canvas-menu-border)"),
        "{menu_style}"
    );
    assert!(menu_style.contains("border-radius: 8px"), "{menu_style}");
    assert!(
        menu_style.contains("box-shadow: var(--canvas-menu-shadow)"),
        "{menu_style}"
    );

    let entries = fixture.all("[data-menu-entry]");
    assert_eq!(entries.len(), 4);
    let first = fixture.text_of("[data-menu-entry=\"0\"]");
    assert!(first.contains("Play now"), "{first}");
    assert!(first.contains("⏎"), "{first}");
    assert!(!fixture.text().contains("Enter"));

    let first_style = fixture.style_of("[data-menu-entry=\"0\"]");
    assert!(first_style.contains("height: 34px"), "{first_style}");
    assert!(first_style.contains("padding: 0 10px"), "{first_style}");
    assert!(first_style.contains("font-size: 13px"), "{first_style}");
    let hint = fixture.style_of("[data-menu-entry=\"0\"] span");
    assert!(hint.contains("font-size: 11px"), "{hint}");
    assert!(hint.contains("color: var(--canvas-muted)"), "{hint}");

    let cursor_style = fixture.style_of("[data-menu-entry=\"1\"]");
    assert!(
        cursor_style.contains("background: var(--canvas-selected)"),
        "{cursor_style}"
    );
    let remove_style = fixture.style_of("[data-menu-entry=\"3\"]");
    assert!(
        remove_style.contains("color: var(--canvas-destructive)"),
        "{remove_style}"
    );
    let divider = fixture.style_of("[data-menu-divider]");
    assert!(
        divider.contains("background: var(--canvas-menu-border)"),
        "{divider}"
    );
}

/// Contract (`QueuePanel.test.ts`, the Menus artboard B.5): a drag paints
/// the ghost without an ordinal — its title in the heading tone and its
/// percent in the partial tone at the ghost's offset and overhang, in the
/// canvas's fill, radius, tilt, and lift — and the inset insertion line at
/// the row the anchor names, while the dragged row stays lit.
#[test]
fn a_drag_paints_the_ghost_its_percent_and_the_inset_insertion_line() {
    let ghost = in_state(
        renamed(item(1), "Dragged track"),
        GuiSemanticState::Active { progress: Some(5) },
    );
    let mut rows_frame = queue(vec![item(1), item(2), item(3)]);
    rows_frame.drag = Some(drag(ghost, GuiQueueInsertion::After { slot: 2 }));
    let fixture = built_with_frame(queue_panel_app, rows_frame, summary(), None);

    let ghost_style = fixture.style_of("[data-ghost]");
    assert!(fixture.text().contains("Dragged track"));
    assert!(!fixture.presents("[data-ghost] [data-ordinal]"));
    assert_eq!(fixture.text_of("[data-ghost-percent]"), "5%");
    assert!(ghost_style.contains("left: 38px"), "{ghost_style}");
    assert!(ghost_style.contains("right: -6px"), "{ghost_style}");
    let card = fixture.style_of("[data-ghost] div");
    assert!(card.contains("background: var(--canvas-ghost)"), "{card}");
    assert!(card.contains("border-radius: 6px"), "{card}");
    assert!(card.contains("transform: rotate(-1deg)"), "{card}");
    assert!(
        card.contains("box-shadow: var(--canvas-ghost-shadow)"),
        "{card}"
    );
    let ghost_title = fixture.style_of("[data-ghost] .truncate");
    assert!(
        ghost_title.contains(&format!("color: {}", ef_var(EfToken::Fg))),
        "{ghost_title}"
    );
    assert!(
        fixture.presents("[data-ghost] [data-progress]"),
        "the ghost paints the ring"
    );
    assert!(
        !fixture.presents("[data-ghost] [data-duration]"),
        "the ghost paints no right column"
    );

    let line = fixture.style_of("[data-insertion]");
    // The `after` anchor: the line pins to the row's bottom edge — on the
    // divider under it, where the drop lands — not to its top.
    assert!(line.contains("top: 40px;"), "{line}");
    assert!(!line.contains("top: 0;"), "{line}");
    assert!(line.contains("margin: 0 8px"), "{line}");
    assert!(line.contains("border-radius: 1px"), "{line}");
    assert!(
        line.contains("background: var(--canvas-insertion)"),
        "{line}"
    );
    assert!(fixture.presents("[data-row=\"2\"] [data-insertion]"));

    let dragged = fixture.style_of("[data-slot=\"1\"]");
    assert!(!dragged.contains("opacity"), "{dragged}");
}

/// Contract (`QueuePanel.test.ts`): the drag ghost paints no percent when
/// the dragged row has no progress.
#[test]
fn the_drag_ghost_paints_no_percent_when_the_dragged_row_has_no_progress() {
    let ghost = renamed(item(1), "Dragged track");
    let mut rows_frame = queue(vec![item(1), item(2)]);
    rows_frame.drag = Some(drag(ghost, GuiQueueInsertion::Before { slot: 2 }));
    let fixture = built_with_frame(queue_panel_app, rows_frame, summary(), None);

    assert!(fixture.presents("[data-ghost]"));
    assert!(!fixture.presents("[data-ghost-percent]"));
    assert!(!fixture.presents("[data-ghost] [data-progress]"));
}

/// Contract (`QueuePanel.test.ts`): the open menu overlaps the bottom edge
/// of the row its anchorSlot names — the second painted row — not the
/// hovered row's and not the cursor row's. The top counts the list box's gap
/// under the header (a 2026-11-19 fix: the offset had left it out, so the menu
/// sat 8 px high).
#[test]
fn the_open_menu_overlaps_the_bottom_edge_of_the_row_its_anchor_slot_names() {
    let rows = vec![item(1), item(2), item(3)];
    let mut rows_frame = queue(rows);
    rows_frame.hover = Some(3);
    rows_frame.cursor = Some(1);
    let fixture = built_with_frame(queue_panel_app, rows_frame, summary(), Some(row_menu()));

    let below = LIST_TOP_GAP_PX + 2 * (ROW_HEIGHT_PX + ROW_GAP_PX) - MENU_OVERLAP_PX;
    let menu_style = fixture.style_of("[data-row-menu]");
    assert!(
        menu_style.contains(&format!(
            "top: calc(var(--canvas-queue-header-height) + {below}px)"
        )),
        "{menu_style}"
    );
}
