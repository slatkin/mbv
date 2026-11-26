//! The destination reset's window contract (`gui-pointer-target-interaction`
//! design D2): the core cannot reach the window state an open clears, so the
//! panel runs the reset when the carried active tab changes. A tab press, a
//! window digit, a router tab step, and the launch restore all change that
//! value, so they share one reset.

use dioxus_native::prelude::*;

use super::{built, fixtures, seed_idle_status};
use crate::bridge::frame::library::GuiLibraryTabFrame;
use crate::window::arrangement::GuiArrangement;

/// Contract (design D2): a carried `library_list` section whose
/// `current_tab` is new closes an open row menu and dismisses the Narrow Hero
/// overlay, so a router open paints the new destination's list exactly as a
/// tab press does.
#[test]
fn a_new_carried_current_tab_closes_the_row_menu_and_dismisses_the_narrow_hero() {
    let mut fixture = built(GuiArrangement::Narrow);
    let (list, detail) = fixtures::music_sections(Some(fixtures::music_hero()));
    fixture.carry(list, detail);
    seed_idle_status(&mut fixture);
    fixture.pick();
    let state = fixture.state.clone();
    fixture.in_runtime(move || {
        state.open_library_row_menu(
            crate::gui_core::GuiLibraryMenuRow::Item("al1".to_owned()),
            10.0,
            12.0,
        );
    });
    assert!(
        fixture.presents("[data-library-region=\"heroOverlay\"]"),
        "the pick paints the overlay before the open"
    );
    assert!(
        fixture.state.ui.peek().row_menu.is_some(),
        "the row menu is open before the open"
    );

    let (list, detail) = fixtures::music_sections(Some(fixtures::music_hero()));
    let next = crate::bridge::frame::library::GuiLibraryList {
        refreshing: false,
        tabs: vec![GuiLibraryTabFrame {
            position: 2,
            label: "Shows".to_owned(),
        }],
        current_tab: Some(2),
        destination: list.destination,
    };
    fixture.carry(next, detail);

    let ui = fixture.state.ui.peek().clone();
    assert!(ui.row_menu.is_none(), "the open row menu closed");
    assert!(ui.hero_dismissed, "the Narrow Hero overlay went");
    assert!(
        !fixture.presents("[data-library-region=\"heroOverlay\"]"),
        "the overlay paints nothing"
    );
}
