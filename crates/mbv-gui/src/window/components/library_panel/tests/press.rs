//! The panel's press contracts (`LibraryPanel.test.ts`, design D2 of
//! `gui-pointer-target-interaction`, D8, D15): the item menu's paint on a
//! destination that is not Music and the outside-press close, the library
//! menu's, and the collection dropdown's (gui-collection-dropdown design D7).
//! The presses are the panel's handler functions, tested against the mock core
//! link; a headless document cannot fire a DOM press.

use super::{Fixture, built, fixtures, press_in_panel, seed_idle_status};
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

/// Contract (user request 2026-11-20): in the Narrow arrangement the
/// right-click menu leads with View Detail, and View Detail is the only way
/// the Hero overlay opens — the menu closes and the overlay paints.
#[test]
fn view_detail_opens_the_narrow_hero_overlay() {
    let mut fixture = built(GuiArrangement::Narrow);
    let (list, detail) = fixtures::music_sections(Some(fixtures::music_hero()));
    fixture.carry(list, detail);
    seed_idle_status(&mut fixture);
    let state = fixture.state.clone();
    fixture.in_runtime(move || {
        state.open_library_row_menu(
            crate::gui_core::GuiLibraryMenuRow::Item("al1".to_owned()),
            40.0,
            60.0,
        );
    });
    fixture.carry_row_menu("al1");

    assert_eq!(fixture.text_of("[data-menu-entry]"), "View Detail");
    assert!(!fixture.presents("[data-library-region=\"heroOverlay\"]"));

    let state = fixture.state.clone();
    fixture.in_runtime(move || state.view_library_detail());

    assert!(fixture.presents("[data-library-region=\"heroOverlay\"]"));
    assert!(!fixture.presents("[data-row-menu]"));
}

/// Contract: the right-click item menu carries the pointer position it opened
/// at (the `RowMenu` state), and the card paints while it is up. Where the
/// card sits on screen is design presentation, not a contract: no test asserts
/// geometry (a viewport-pixel assert broke on presentation-only changes; user
/// decision 2026-11-19).
#[test]
fn the_item_menu_carries_the_pointer_it_opened_at() {
    let mut fixture = built(GuiArrangement::Wide);
    open_the_shows_item_menu(&mut fixture, 40.0, 60.0);

    assert!(fixture.presents("[data-row-menu]"), "the item menu paints");
    let menu = fixture
        .state
        .ui
        .peek()
        .row_menu
        .clone()
        .expect("the menu is up");
    assert_eq!((menu.x, menu.y), (40.0, 60.0));
}

/// Contract (gui-swap-queue-and-library-sides): in a Wide fixture with a
/// picked item, both the list region and the Hero region paint. Which side of
/// the window each sits on is design presentation, not a contract: no test
/// asserts geometry (user decision 2026-11-19).
#[test]
fn the_wide_list_and_hero_regions_paint() {
    let mut fixture = built(GuiArrangement::Wide);
    let (list, detail) = fixtures::music_sections(Some(fixtures::music_hero()));
    fixture.carry(list, detail);
    seed_idle_status(&mut fixture);

    assert!(fixture.presents("[data-library-region=\"list\"]"));
    assert!(fixture.presents("[data-library-region=\"hero\"]"));
}

/// Carry a Shows list and open its item menu at `x, y`, as the core's
/// right-click frame does.
fn open_the_shows_item_menu(fixture: &mut Fixture, x: f32, y: f32) {
    let (list, detail) = fixtures::shows_sections();
    fixture.carry(list, detail);
    seed_idle_status(fixture);
    let state = fixture.state.clone();
    fixture.in_runtime(move || {
        state.open_library_row_menu(
            crate::gui_core::GuiLibraryMenuRow::Item("s1".to_owned()),
            x,
            y,
        );
    });
    fixture.carry_row_menu("s1");
}

/// Contract: a press inside the panel while the menu is up closes it, and
/// sends the core nothing.
#[test]
fn a_press_inside_the_panel_closes_the_open_row_menu() {
    let mut fixture = built(GuiArrangement::Wide);
    let state = fixture.state.clone();
    fixture.in_runtime(move || {
        state.open_library_row_menu(
            crate::gui_core::GuiLibraryMenuRow::Item("s1".to_owned()),
            40.0,
            60.0,
        );
    });
    fixture.receiver.try_iter().last();

    let state = fixture.state.clone();
    fixture.in_runtime(move || press_in_panel(&state));

    assert!(fixture.state.ui.peek().row_menu.is_none());
    assert_eq!(fixture.library_commands(), []);
}

/// Contract (gui-collection-dropdown design D7, spec "A press outside closes
/// the dropdown"): a press inside the panel while the dropdown is open closes
/// it and clears its filter text, so the next open starts empty.
#[test]
fn a_press_inside_the_panel_closes_the_open_collection_dropdown() {
    let mut fixture = built(GuiArrangement::Wide);
    let (list, detail) = fixtures::music_sections(None);
    fixture.carry(list, detail);
    seed_idle_status(&mut fixture);
    let state = fixture.state.clone();
    fixture.in_runtime(move || {
        state.toggle_collections_dropdown();
        state.set_collections_filter("tifo".to_owned());
    });
    assert!(fixture.presents("[data-library-collections-menu]"));

    let state = fixture.state.clone();
    fixture.in_runtime(move || press_in_panel(&state));

    assert!(!fixture.state.ui.peek().collections.is_open());
    assert_eq!(fixture.state.ui.peek().collections.filter(), "");
    assert_eq!(fixture.library_commands(), []);
    assert!(!fixture.presents("[data-library-collections-menu]"));
}

/// Contract (gui-collection-dropdown design D6, D6a): the open popover paints,
/// and no header clips it. The selector row carried the pill row's
/// `overflow: hidden`, which cut the popover to the row's 52 px box and hid
/// every collection (review of 96f04ebef); the dropdown now rides the list
/// header's title line, and the header clips nothing. Where the popover sits
/// is design presentation, not a contract: no test asserts geometry (user
/// decision 2026-11-19).
#[test]
fn the_open_dropdown_paints_unclipped() {
    let mut fixture = built(GuiArrangement::Wide);
    let (list, detail) = fixtures::music_sections(None);
    fixture.carry(list, detail);
    seed_idle_status(&mut fixture);
    let state = fixture.state.clone();
    fixture.in_runtime(move || state.toggle_collections_dropdown());
    fixture.lay_out();

    assert!(
        fixture.presents("[data-library-collections-menu]"),
        "the popover paints"
    );
    assert!(
        !fixture.clips_overflow("[data-library-list-header]"),
        "the list header clips the popover"
    );
}
