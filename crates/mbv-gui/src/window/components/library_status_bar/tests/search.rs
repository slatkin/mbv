//! The Search field's contracts at the Library Status bar (design D9 of
//! `move-playback-header-into-queue-footer`): the field and its results
//! dropdown moved here from the top bar's suite with their tests. The bar
//! mounts the field at its left, the dropdown opens upward, and the field's
//! mount, focus, and highlight facts follow the same store. The harness is
//! `super`'s.

use blitz_dom::Document as _;
use dioxus_native::prelude::*;

use super::{built, idle, music_props, results, row};
use crate::bridge::frame::library::GuiSearchFrame;

/// Contract (`LibraryTopBar.test.ts`, moved with the field, design D9): a
/// closed Search section paints the field and no dropdown.
#[test]
fn a_closed_search_section_paints_the_field_and_no_dropdown() {
    let doc = built(music_props(idle()));

    assert!(
        doc.presents("[data-library-search-input]"),
        "the field paints"
    );
    assert!(
        !doc.presents("[data-library-search-results]"),
        "no dropdown paints"
    );
}

/// Contract (`LibraryTopBar.test.ts`, moved with the field): the dropdown
/// paints the rows in the list-row look, and a first landing highlights the
/// first row.
#[test]
fn the_dropdown_paints_the_rows_with_the_first_row_highlighted() {
    let mut fixture = built(music_props(idle()));
    fixture.set_search_frame(Some(results()));

    // The row paints its title and its subtitle; the former markup's
    // inter-element space is formatting, so the joined text asserts both
    // parts.
    let first = fixture.text_of("[data-library-search-row=\"r1\"]");
    assert!(first.contains("Beastie Boys"), "{first}");
    assert!(first.contains("Artist"), "{first}");
    let second = fixture.text_of("[data-library-search-row=\"r2\"]");
    assert!(second.contains("Licensed To Ill"), "{second}");
    assert!(second.contains("Beastie Boys"), "{second}");
    assert_eq!(
        fixture.state.search.peek().highlight.clone(),
        Some("r1".to_owned())
    );
    assert_eq!(
        fixture.attrs_of(
            "[data-library-search-highlight]",
            &blitz_dom::LocalName::from("data-library-search-row")
        ),
        [Some("r1".to_owned())]
    );
}

/// Contract (design D9, the spec's "The Search field SHALL search all
/// libraries, show the results above the field"): the results dropdown paints
/// while the frame carries rows. Where it sits relative to the field is design
/// presentation, not a contract: no test asserts geometry (user decision
/// 2026-11-19).
#[test]
fn the_results_dropdown_paints() {
    let mut fixture = built(music_props(idle()));
    fixture.set_search_frame(Some(results()));

    assert!(
        fixture.presents("[data-library-search-results]"),
        "the dropdown paints"
    );
    assert!(fixture.presents("[data-library-search]"), "the field stays");
}

/// Contract (`LibraryTopBar.test.ts`, moved with the field): the highlight
/// survives a rerendered results frame by row id, not object identity.
#[test]
fn the_highlight_survives_a_rerendered_results_frame_by_row_id() {
    let mut fixture = built(music_props(idle()));
    let state = fixture.state.clone();
    fixture.in_runtime(move || state.set_search_highlight(Some("r2".to_owned())));
    // A new frame: fresh row values, same ids, same highlight.
    fixture.set_search_frame(Some(results()));

    assert_eq!(
        fixture.state.search.peek().highlight.clone(),
        Some("r2".to_owned())
    );
    assert_eq!(
        fixture.attrs_of(
            "[data-library-search-highlight]",
            &blitz_dom::LocalName::from("data-library-search-row")
        ),
        [Some("r2".to_owned())]
    );
}

/// Contract (`LibraryTopBar.test.ts`, moved with the field): the highlight
/// falls to the first row when a new frame drops its id, and to none when the
/// frame carries no rows.
#[test]
fn the_highlight_falls_to_the_first_row_when_a_new_frame_drops_its_id() {
    let mut fixture = built(music_props(idle()));
    let state = fixture.state.clone();
    fixture.in_runtime(move || state.set_search_highlight(Some("r2".to_owned())));
    let dropped = GuiSearchFrame {
        results: vec![row("r1", "Beastie Boys", "Artist")],
        ..results()
    };
    fixture.set_search_frame(Some(dropped.clone()));
    assert_eq!(
        fixture.state.search.peek().highlight.clone(),
        Some("r1".to_owned())
    );

    fixture.set_search_frame(Some(GuiSearchFrame {
        results: Vec::new(),
        ..dropped
    }));
    assert_eq!(fixture.state.search.peek().highlight.clone(), None);
    assert!(
        !fixture.presents("[data-library-search-highlight]"),
        "no row paints the highlight"
    );
}

/// Contract (`LibraryTopBar.test.ts`, moved with the field): the dropdown
/// paints the loading and failed states while the query lands.
#[test]
fn the_dropdown_paints_the_loading_and_failed_states() {
    let mut fixture = built(music_props(idle()));
    fixture.set_search_frame(Some(GuiSearchFrame {
        loading: true,
        ..results()
    }));
    assert!(
        fixture.presents("[data-library-search-loading]"),
        "the loading state paints"
    );

    fixture.set_search_frame(Some(GuiSearchFrame {
        loading: false,
        failed: true,
        ..results()
    }));
    assert!(
        fixture.presents("[data-library-search-failed]"),
        "the failed state paints"
    );
}

/// Contract (`LibraryTopBar.test.ts`, the review-of-row-12.1 finding P1,
/// moved with the field): unmounting the bar retracts the mount and focus
/// facts, so an arrangement change never leaves the keyboard armed on a field
/// whose element nothing paints.
#[test]
fn unmounting_the_bar_retracts_the_mount_and_focus_facts() {
    let mut fixture = built(music_props(idle()));
    assert!(fixture.state.search.peek().mounted);

    let state = fixture.state.clone();
    fixture.in_runtime(move || state.focus_search_field());
    assert!(fixture.state.search.peek().focused);

    let mut show = fixture.show;
    fixture.in_runtime(move || show.set(false));

    let held = fixture.state.search.peek().clone();
    assert!(!held.mounted);
    assert!(!held.focused);
}

/// Contract (`LibraryTopBar.test.ts`, moved with the field): the field
/// follows its focus fact into DOM focus — the key router's `Ctrl+K` fact
/// moves the DOM focus, and the field's own blur event checks the fact back
/// out (the blur direction is the state layer's contract,
/// `window/library/search/tests.rs`).
#[test]
fn the_field_follows_its_focus_fact_into_dom_focus() {
    let mut fixture = built(music_props(idle()));

    let state = fixture.state.clone();
    fixture.in_runtime(move || state.focus_search_field());

    let dom = fixture.doc.inner();
    let focused = dom.get_focussed_node_id();
    let input = dom
        .query_selector("[data-library-search-input]")
        .expect("the selector is valid");
    assert_eq!(focused, input, "the field holds the DOM focus");
}
