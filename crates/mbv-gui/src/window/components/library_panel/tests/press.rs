//! The panel's press contracts (`LibraryPanel.test.ts`, design D4, D8, D15):
//! the take-focus press, the item menu's paint on a destination that is not
//! Music, the outside-press close, and the menu's own press. The presses are
//! the panel's handler functions, tested against the mock core link; a
//! headless document cannot fire a DOM press.

use super::{Fixture, built, fixtures, press_outside_menu, press_takes_focus, seed_idle_status};
use crate::bridge::frame::library::GuiShowsModeFrame;
use crate::window::arrangement::GuiArrangement;
use dioxus_native::prelude::*;

/// Contract (`LibraryPanel.test.ts`): a press inside the panel sends the
/// take-focus command (design D4), so the core sets the focus stack and
/// Panel focus together before the press acts on the row.
#[test]
fn a_press_inside_the_panel_sends_the_take_focus_command() {
    let mut fixture = built(GuiArrangement::Wide);

    let state = fixture.state.clone();
    fixture.in_runtime(move || press_takes_focus(&state));

    assert_eq!(
        fixture.library_commands(),
        [mbv::GuiLibraryCommand::TakeFocus]
    );
}

/// Contract (`LibraryPanel.test.ts`, design D8, D15): the right-click item
/// menu is the Library's, not one destination's. The row menu rides its own
/// section, so a Shows row's menu paints while Music carries no content —
/// before D15 the panel read the menu off the Music frame, so every other
/// destination's right-click opened nothing.
#[test]
fn the_item_menu_paints_on_a_destination_that_is_not_music() {
    let mut fixture = built(GuiArrangement::Wide);
    open_the_shows_item_menu(&mut fixture, 40.0, 60.0);

    assert!(fixture.presents("[data-row-menu]"), "the item menu paints");
    assert!(
        fixture.text_of("[data-menu-entry=\"0\"]").contains("Play"),
        "the menu entry paints"
    );
}

/// Contract: the right-click item menu opens at the pointer position in viewport
/// pixels. The card is `absolute` inside a zero-size `fixed` div at the
/// origin, so its `left`/`top` — the pointer coordinates the core sends — are
/// viewport pixels, not offsets into the column that paints it. Blitz places
/// a `fixed` box against the viewport, as the browser did, so the card lands
/// on the pointer and not on the column's own origin.
#[test]
fn the_item_menu_opens_at_the_pointer_in_viewport_pixels() {
    let mut fixture = built(GuiArrangement::Wide);
    open_the_shows_item_menu(&mut fixture, 40.0, 60.0);

    let menu = fixture.rect_of("[data-row-menu]");
    assert_eq!((menu.x, menu.y), (40.0, 60.0));
    // The column the card belongs to sits away from the origin, so an anchor
    // that resolved against it would miss the pointer position.
    let list = fixture.rect_of("[data-library-region=\"list\"]");
    assert_ne!(list.x, 0.0, "the list column is offset from the origin");
}

/// Carry a Shows list and open its item menu at `x, y`, as the core's
/// right-click frame does.
fn open_the_shows_item_menu(fixture: &mut Fixture, x: f32, y: f32) {
    let (list, detail) = fixtures::shows_sections(GuiShowsModeFrame::All);
    fixture.carry(list, detail);
    seed_idle_status(fixture);
    let state = fixture.state.clone();
    fixture.in_runtime(move || state.open_library_row_menu("s1".to_owned(), x, y));
    fixture.carry_row_menu("s1");
}

/// Contract: a press
/// inside the panel while the menu is up closes it, and the press still
/// takes focus.
#[test]
fn a_press_inside_the_panel_closes_the_open_row_menu_and_takes_focus() {
    let mut fixture = built(GuiArrangement::Wide);
    let state = fixture.state.clone();
    fixture.in_runtime(move || {
        state.open_library_row_menu("s1".to_owned(), 40.0, 60.0);
    });
    fixture.receiver.try_iter().last();

    let state = fixture.state.clone();
    fixture.in_runtime(move || press_outside_menu(&state));

    assert!(fixture.state.ui.peek().row_menu.is_none());
    assert_eq!(
        fixture.library_commands(),
        [mbv::GuiLibraryCommand::TakeFocus]
    );
}

/// Contract: the
/// menu's own press handler takes focus without the outside-press close —
/// the panel's close handler never runs for a press on the menu.
#[test]
fn a_press_on_the_menu_takes_focus_and_keeps_the_menu_open() {
    let mut fixture = built(GuiArrangement::Wide);
    let state = fixture.state.clone();
    fixture.in_runtime(move || {
        state.open_library_row_menu("s1".to_owned(), 40.0, 60.0);
    });
    fixture.receiver.try_iter().last();

    let state = fixture.state.clone();
    fixture.in_runtime(move || press_takes_focus(&state));

    assert!(fixture.state.ui.peek().row_menu.is_some());
    assert_eq!(
        fixture.library_commands(),
        [mbv::GuiLibraryCommand::TakeFocus]
    );
}
