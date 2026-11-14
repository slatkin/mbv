//! The Narrow Hero overlay's contracts (`LibraryPanel.test.ts`, the
//! review-of-row-4.5 finding P1): the overlay paints the same Hero painter
//! the Wide pane mounts, over the list, and closes on a press on its
//! backdrop (design D3, D4) — the backdrop's close is the interaction
//! state's own close, driven here through the painted overlay's reaction.

use super::{built, fixtures, seed_idle_status};
use crate::bridge::frame::library::{GuiAudiobooksChapterRowFrame, GuiAudiobooksWorkspaceFrame};
use crate::window::arrangement::GuiArrangement;

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

    assert!(
        fixture
            .presents("[data-library-region=\"heroOverlay\"] [data-panel-child=\"LibraryHero\"]"),
        "the overlay paints the Hero"
    );

    let state = fixture.state.clone();
    fixture.in_runtime(move || state.close_library_hero());

    assert!(
        !fixture.presents("[data-library-region=\"heroOverlay\"]"),
        "the overlay closed"
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
        fixture.presents("[data-library-region=\"hero\"] [data-panel-child=\"LibraryHero\"]"),
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
