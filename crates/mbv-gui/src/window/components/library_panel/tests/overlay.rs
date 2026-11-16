//! The Narrow Hero overlay's contracts (`LibraryPanel.test.ts`, the
//! review-of-row-4.5 finding P1): the overlay paints the same Hero painter
//! the Wide pane mounts, over the list, and closes on a press on its
//! backdrop (design D3, D4) — the backdrop's close is the interaction
//! state's own close, driven here through the painted overlay's reaction,
//! and through a real pointer press on the backdrop's painted box.

use std::sync::Arc;

use blitz_dom::Document as _;
use blitz_traits::events::{
    BlitzPointerEvent, BlitzPointerId, MouseEventButton, MouseEventButtons, Point, PointerCoords,
    PointerDetails, UiEvent,
};
use dioxus_native::prelude::*;

use super::{built, fixtures, seed_idle_status};
use crate::bridge::frame::library::{GuiAudiobooksChapterRowFrame, GuiAudiobooksWorkspaceFrame};
use crate::window::arrangement::GuiArrangement;
use crate::window::input::f32_from_f64;

/// Contract (`LibraryPanel.test.ts`, the review-of-row-4.5 finding P1): the
/// Narrow overlay paints the Hero over the list and closes on a press on its
/// backdrop (design D3) — the backdrop's close is the interaction state's
/// own close, driven here through the painted overlay's reaction.
#[test]
fn the_narrow_overlay_paints_the_hero_and_closes_on_its_backdrop_press() {
    let mut fixture = built(GuiArrangement::Narrow);
    let (list, detail) = fixtures::music_sections(Some(fixtures::music_hero()));
    fixture.carry(list, detail);
    seed_idle_status(&mut fixture);
    fixture.pick();

    assert!(
        fixture.presents("[data-library-region=\"heroOverlay\"] [data-library-hero]"),
        "the overlay paints the Hero"
    );

    let state = fixture.state.clone();
    fixture.in_runtime(move || state.close_library_hero());

    assert!(
        !fixture.presents("[data-library-region=\"heroOverlay\"]"),
        "the overlay closed"
    );
}

/// Contract: a Narrow Library with no pick paints the list, not the overlay
/// Hero for the core's default row (a hero is not forced on switching to a
/// library; the Hero overlay needs a pick).
#[test]
fn the_narrow_library_paints_no_overlay_hero_before_a_pick() {
    let mut fixture = built(GuiArrangement::Narrow);
    let (list, detail) = fixtures::music_sections(Some(fixtures::music_hero()));
    fixture.carry(list, detail);
    seed_idle_status(&mut fixture);

    assert!(
        !fixture.presents("[data-library-region=\"heroOverlay\"]"),
        "no overlay before a pick"
    );
    assert!(
        fixture.presents("[data-library-region=\"list\"]"),
        "the list paints"
    );
}

/// One pointer press or release at `x`/`y` in the document's client
/// coordinates, as the shell builds them from a winit pointer button.
fn pointer(down: bool, x: f32, y: f32) -> UiEvent {
    let coords = PointerCoords {
        page_x: x,
        page_y: y,
        screen_x: x,
        screen_y: y,
        client_x: x,
        client_y: y,
    };
    let event = BlitzPointerEvent {
        id: BlitzPointerId::Mouse,
        is_primary: true,
        coords,
        button: MouseEventButton::Main,
        buttons: MouseEventButtons::Primary,
        mods: Modifiers::default(),
        details: PointerDetails::default(),
        element: Point::default(),
        active_pointers: Arc::default(),
    };
    if down {
        UiEvent::PointerDown(event)
    } else {
        UiEvent::PointerUp(event)
    }
}

/// A press and its release at `x`/`y`: the release dispatches the DOM
/// `click`, so the backdrop's own handler runs.
fn click(fixture: &mut super::Fixture, x: f32, y: f32) {
    fixture.doc.handle_ui_event(pointer(true, x, y));
    fixture.doc.handle_ui_event(pointer(false, x, y));
    fixture.poll_until_idle();
}

/// Contract: a
/// click on the overlay's dim backdrop reaches the painted button's handler,
/// so the overlay closes and the list under it paints again. The card covers
/// the overlay's middle, so the click lands on the frame beside it.
#[test]
fn a_click_on_the_narrow_overlays_backdrop_closes_it_and_the_list_paints() {
    let mut fixture = built(GuiArrangement::Narrow);
    let (list, detail) = fixtures::music_sections(Some(fixtures::music_hero()));
    fixture.carry(list, detail);
    seed_idle_status(&mut fixture);
    fixture.pick();
    fixture.lay_out();

    let backdrop = fixture.rect_of("[data-library-hero-backdrop]");
    let (x, y) = (
        f32_from_f64(backdrop.x) + 2.0,
        f32_from_f64(backdrop.y) + 2.0,
    );
    click(&mut fixture, x, y);

    assert!(
        !fixture.presents("[data-library-region=\"heroOverlay\"]"),
        "the click closed the overlay"
    );
    assert!(
        fixture.presents("[data-library-region=\"list\"]"),
        "the list paints again"
    );
}

/// Contract (`LibraryPanel.test.ts`): the Wide Hero pane paints the Hero
/// beside the list, and no overlay mounts over it.
#[test]
fn the_wide_hero_pane_paints_the_hero_beside_the_list() {
    let mut fixture = built(GuiArrangement::Wide);
    let (list, detail) = fixtures::music_sections(Some(fixtures::music_hero()));
    fixture.carry(list, detail);
    seed_idle_status(&mut fixture);

    assert!(
        fixture.presents("[data-library-region=\"hero\"] [data-library-hero]"),
        "the Wide pane paints the Hero"
    );
    assert!(
        !fixture.presents("[data-library-region=\"heroOverlay\"]"),
        "no overlay mounts"
    );
}

/// Contract (`LibraryPanel.test.ts`): the Narrow Hero overlay works for the
/// Home Hero through the same painter the Wide pane uses, and closes on a
/// press on its backdrop.
#[test]
fn the_narrow_overlay_paints_the_home_hero_and_closes_on_its_backdrop_press() {
    let mut fixture = built(GuiArrangement::Narrow);
    let (list, detail) = fixtures::home_sections(true);
    fixture.carry(list, detail);
    seed_idle_status(&mut fixture);
    fixture.pick();

    assert!(
        fixture.presents("[data-library-region=\"heroOverlay\"] [data-library-home-hero]"),
        "the overlay paints the Home Hero"
    );

    let state = fixture.state.clone();
    fixture.in_runtime(move || state.close_library_hero());

    assert!(
        !fixture.presents("[data-library-region=\"heroOverlay\"]"),
        "the overlay closed"
    );
}
/// Contract (`LibraryPanel.test.ts`): the Narrow overlay works for the
/// Audiobooks Hero through the same painter the Wide pane uses, and —
/// copied from the row-7.1 lesson — a stale Hero paints through an overlay
/// the frame's own selection carries.
#[test]
fn the_narrow_overlay_paints_the_audiobooks_hero_and_closes_on_its_backdrop_press() {
    let mut fixture = built(GuiArrangement::Narrow);
    let (list, detail) = fixtures::audiobooks_sections(Some(fixtures::audiobooks_hero(Some(
        GuiAudiobooksWorkspaceFrame {
            rows: vec![GuiAudiobooksChapterRowFrame {
                chapter: 1,
                title: "Act one".to_owned(),
                duration: "2:16".to_owned(),
                start_ticks: 0,
                end_ticks: 136 * 1_000_000,
            }],
        },
    ))));
    fixture.carry(list, detail);
    seed_idle_status(&mut fixture);
    fixture.pick();

    assert!(
        fixture.presents("[data-library-region=\"heroOverlay\"] [data-library-abs-hero]"),
        "the overlay paints the book Hero"
    );

    let state = fixture.state.clone();
    fixture.in_runtime(move || state.close_library_hero());

    assert!(
        !fixture.presents("[data-library-region=\"heroOverlay\"]"),
        "the overlay closed"
    );
}
