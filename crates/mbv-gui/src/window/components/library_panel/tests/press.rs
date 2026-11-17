//! The panel's press contracts (`LibraryPanel.test.ts`, design D2 of
//! `gui-pointer-target-interaction`, D8, D15): the item menu's paint on a
//! destination that is not Music and the outside-press close. The presses are
//! the panel's handler functions, tested against the mock core link; a headless
//! document cannot fire a DOM press.

use super::{Fixture, built, fixtures, press_outside_menu, seed_idle_status};
use crate::bridge::frame::library::GuiShowsModeFrame;
use crate::window::arrangement::GuiArrangement;
use dioxus_native::prelude::*;

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

/// Contract (gui-swap-queue-and-library-sides, the scenario "The Wide list is
/// at the left window edge"): in a Wide fixture with a picked item, the list
/// region's box ends at or left of the Hero region's box start — the list is
/// at the left window edge and the Hero pane to its right.
#[test]
fn the_wide_list_region_ends_left_of_the_hero_region() {
    let mut fixture = built(GuiArrangement::Wide);
    let (list, detail) = fixtures::music_sections(Some(fixtures::music_hero()));
    fixture.carry(list, detail);
    seed_idle_status(&mut fixture);

    let list = fixture.rect_of("[data-library-region=\"list\"]");
    let hero = fixture.rect_of("[data-library-region=\"hero\"]");
    assert!(list.x + list.width <= hero.x);
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

/// Contract: a press inside the panel while the menu is up closes it, and
/// sends the core nothing.
#[test]
fn a_press_inside_the_panel_closes_the_open_row_menu() {
    let mut fixture = built(GuiArrangement::Wide);
    let state = fixture.state.clone();
    fixture.in_runtime(move || {
        state.open_library_row_menu("s1".to_owned(), 40.0, 60.0);
    });
    fixture.receiver.try_iter().last();

    let state = fixture.state.clone();
    fixture.in_runtime(move || press_outside_menu(&state));

    assert!(fixture.state.ui.peek().row_menu.is_none());
    assert_eq!(fixture.library_commands(), []);
}
