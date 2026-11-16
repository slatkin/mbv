//! The Search field's key contracts of `search-keys.test.ts` (D16), over the
//! shared harness: the `Ctrl+K` chord arms and routes the field only while a
//! top bar mounts it, the field's typing answers `Text` and sends the core
//! nothing, Esc closes the field with the `searchClose` command, the arrows
//! and Enter act on the held row ids, and no search key acts while the field
//! does not hold the keyboard. A core overlay keeps the keyboard even from
//! the focused field, and the carried results close after the field lost it.

use super::*;
use crate::bridge::frame::library::{GuiSearchFrame, GuiSearchRowFrame};
use crate::bridge::sections::GuiSection;

/// One found row, as the frame carries it.
fn row_frame(id: &str, title: &str) -> GuiSearchRowFrame {
    GuiSearchRowFrame {
        id: id.to_owned(),
        item_type: "Artist".to_owned(),
        title: title.to_owned(),
        subtitle: "Artist".to_owned(),
    }
}

/// The found rows the fixtures carry.
fn rows() -> Vec<GuiSearchRowFrame> {
    vec![
        row_frame("r1", "Beastie Boys"),
        row_frame("r2", "Brand Nubian"),
        row_frame("r3", "Licensed To Ill"),
    ]
}

/// The carried results frame the keys act on.
fn results() -> GuiSearchFrame {
    GuiSearchFrame {
        query: "ba".to_owned(),
        loading: false,
        failed: false,
        results: rows(),
    }
}

/// Route one keydown against the window's own section stores and paint the
/// route outcome with the Search field's facts.
fn route_key(
    state: &LibraryState,
    facts: &Facts,
    name: &str,
    value: &str,
    shift: bool,
    ctrl: bool,
) {
    let dom = DomKey {
        key: value.to_owned(),
        code: String::new(),
        meta_key: false,
        repeat: false,
        modifiers: DomModifiers {
            shift_key: shift,
            ctrl_key: ctrl,
            alt_key: false,
        },
    };
    let outcome = route_library_key(
        &dom,
        Some(&state.sections),
        state,
        None,
        GuiArrangement::Wide,
        &PointerTarget {
            zone: Some(PointerZone::Library),
            list: None,
            row: None,
        },
    );
    let search = state.search.peek();
    facts.lock().expect("facts").push(format!(
        "{name}={outcome:?} focused={} mounted={} highlight={:?}",
        search.focused, search.mounted, search.highlight
    ));
}

/// Carry the Search section's results frame the Esc close and the highlight
/// read.
fn carry_search(state: &LibraryState, frame: GuiSearchFrame) {
    let batch = GuiSectionBatch {
        search: Some(GuiSection {
            revision: 1,
            value: Some(frame),
        }),
        ..GuiSectionBatch::default()
    };
    let mut sections = state.sections.clone();
    sections.apply_batch(&batch);
}

#[test]
fn ctrl_k_focuses_the_search_field_while_the_library_panel_shows() {
    let list = music_list(Vec::new());
    let (facts, rx) = run(move |state, facts| {
        state.mount_search_field();
        carry(state, list.clone(), music_detail(None));
        route_key(state, facts, "k", "k", false, true);
    });
    assert_eq!(
        facts,
        ["k=Handled focused=true mounted=true highlight=None"]
    );
    assert_eq!(commands(&rx), []);
}

// No Library section in the frame: the panel paints nothing and its keys
// are unwired, so the core's own Ctrl+K (`[keys]`) keeps the key.
#[test]
fn ctrl_k_out_of_a_queue_panel_frame_sends_the_key_to_the_core() {
    let (facts, rx) = run(move |state, facts| {
        route_key(state, facts, "k", "k", false, true);
    });
    assert_eq!(
        facts,
        ["k=Core(None) focused=false mounted=false highlight=None"]
    );
    assert_eq!(commands(&rx), []);
}

// Review of row 12.1, finding P1: the chord is a window key, but the top bar
// mounts the field only where the arrangement paints the bar, so arming the
// focus fact where no element holds it would leave the keyboard dead. The
// chord keeps its core effect until the bar mounts.
#[test]
fn ctrl_k_answers_the_core_while_no_top_bar_mounts_the_search_field() {
    let list = music_list(Vec::new());
    let (facts, rx) = run(move |state, facts| {
        carry(state, list.clone(), music_detail(None));
        route_key(state, facts, "k", "k", false, true);
    });
    assert_eq!(
        facts,
        ["k=Core(None) focused=false mounted=false highlight=None"]
    );
    assert_eq!(commands(&rx), []);
}

// Review of row 12.1, finding P1: the unmounted top bar retracts the focus
// fact with its mount fact, so the routing gate never traps the keyboard on
// the retracted field.
#[test]
fn a_key_meets_the_core_after_the_top_bar_unmounts_under_the_armed_field() {
    let list = music_list(Vec::new());
    let (facts, rx) = run(move |state, facts| {
        state.mount_search_field();
        state.focus_search_field();
        state.unmount_search_field();
        carry(state, list.clone(), music_detail(None));
        route_key(state, facts, "b", "b", false, false);
    });
    assert_eq!(
        facts,
        ["b=Core(None) focused=false mounted=false highlight=None"]
    );
    assert_eq!(commands(&rx), []);
}

#[test]
fn the_field_takes_its_typing_without_a_core_forward() {
    let list = music_list(Vec::new());
    let (facts, rx) = run(move |state, facts| {
        state.mount_search_field();
        state.focus_search_field();
        carry(state, list.clone(), music_detail(None));
        route_key(state, facts, "b", "b", false, false);
    });
    assert_eq!(facts, ["b=Text focused=true mounted=true highlight=None"]);
    // The forwarder sends the core nothing for a text answer.
    assert_eq!(commands(&rx), []);
}

#[test]
fn esc_closes_the_results_clears_the_field_and_sends_searchclose() {
    let list = music_list(Vec::new());
    let (facts, rx) = run(move |state, facts| {
        state.mount_search_field();
        state.focus_search_field();
        carry(state, list.clone(), music_detail(None));
        carry_search(state, results());
        route_key(state, facts, "escape", "Escape", false, false);
    });
    assert_eq!(
        facts,
        ["escape=Handled focused=false mounted=false highlight=None"]
    );
    assert_eq!(commands(&rx), [mbv::GuiLibraryCommand::SearchClose]);
}

#[test]
fn down_and_up_move_the_highlight_along_the_row_ids() {
    let list = music_list(Vec::new());
    let (facts, _rx) = run(move |state, facts| {
        state.mount_search_field();
        state.focus_search_field();
        carry(state, list.clone(), music_detail(None));
        carry_search(state, results());
        state.set_search_highlight(Some("r1".to_owned()));
        route_key(state, facts, "down", "ArrowDown", false, false);
        route_key(state, facts, "up", "ArrowUp", false, false);
    });
    assert_eq!(
        facts,
        [
            "down=Handled focused=true mounted=true highlight=Some(\"r2\")",
            "up=Handled focused=true mounted=true highlight=Some(\"r1\")",
        ]
    );
}

// The key-leak lesson's identity half (row 12.1): a refresh's results are
// fresh row values, so the movement must place the held id inside the new
// frame by id — r2 moves to r3 there.
#[test]
fn the_highlight_movement_walks_a_new_frame_by_id() {
    let list = music_list(Vec::new());
    let (facts, _rx) = run(move |state, facts| {
        state.mount_search_field();
        state.focus_search_field();
        carry(state, list.clone(), music_detail(None));
        carry_search(state, results());
        state.set_search_highlight(Some("r2".to_owned()));
        route_key(state, facts, "down", "ArrowDown", false, false);
    });
    assert_eq!(
        facts,
        ["down=Handled focused=true mounted=true highlight=Some(\"r3\")"]
    );
}

#[test]
fn enter_on_the_highlighted_row_sends_searchactivate_and_no_close() {
    let list = music_list(Vec::new());
    let (facts, rx) = run(move |state, facts| {
        state.mount_search_field();
        state.focus_search_field();
        carry(state, list.clone(), music_detail(None));
        carry_search(state, results());
        state.set_search_highlight(Some("r2".to_owned()));
        route_key(state, facts, "enter", "Enter", false, false);
    });
    assert_eq!(
        facts,
        ["enter=Handled focused=true mounted=true highlight=Some(\"r2\")"]
    );
    assert_eq!(
        commands(&rx),
        [mbv::GuiLibraryCommand::SearchActivate {
            id: "r2".to_owned(),
            item_type: "Artist".to_owned(),
        }]
    );
}

#[test]
fn enter_with_no_highlighted_row_sends_nothing() {
    let list = music_list(Vec::new());
    let (facts, rx) = run(move |state, facts| {
        state.mount_search_field();
        state.focus_search_field();
        carry(state, list.clone(), music_detail(None));
        carry_search(state, results());
        route_key(state, facts, "enter", "Enter", false, false);
    });
    assert_eq!(
        facts,
        ["enter=Handled focused=true mounted=true highlight=None"]
    );
    assert_eq!(commands(&rx), []);
}

// The Library list's own movement keys keep their job; the Search state holds
// no highlight and stays unfocused.
#[test]
fn no_search_key_acts_while_the_field_does_not_hold_the_keyboard() {
    let list = music_list(Vec::new());
    let (facts, rx) = run(move |state, facts| {
        carry(state, list.clone(), music_detail(None));
        carry_search(state, results());
        route_on_row(
            state,
            facts,
            "down",
            "ArrowDown",
            PointerList::Music,
            "nothing",
        );
        let search = state.search.peek();
        facts.lock().expect("facts").push(format!(
            "focused={} mounted={} highlight={:?}",
            search.focused, search.mounted, search.highlight
        ));
    });
    assert_eq!(
        facts,
        ["down=Handled", "focused=false mounted=false highlight=None"]
    );
    assert_eq!(commands(&rx), []);
}

// Review of row 12.1, finding P2: the dropdown paints while the frame
// carries the section, so Esc must close it even after a pointer takes the
// keyboard elsewhere — the routeEscapeKey arm runs before the panel keys and
// it is the only close the open results have.
#[test]
fn escape_closes_the_carried_results_after_the_field_lost_the_keyboard() {
    let list = music_list(Vec::new());
    let (facts, rx) = run(move |state, facts| {
        state.mount_search_field();
        carry(state, list.clone(), music_detail(None));
        carry_search(state, results());
        route_key(state, facts, "escape", "Escape", false, false);
    });
    assert_eq!(
        facts,
        ["escape=Handled focused=false mounted=false highlight=None"]
    );
    assert_eq!(commands(&rx), [mbv::GuiLibraryCommand::SearchClose]);
}

#[test]
fn a_core_overlay_keeps_the_keyboard_even_from_the_focused_search_field() {
    let list = music_list(Vec::new());
    let (facts, rx) = run(move |state, facts| {
        state.mount_search_field();
        state.focus_search_field();
        carry(state, list.clone(), music_detail(None));
        carry_search(state, results());
        open_core_overlay(state);
        route_key(state, facts, "escape", "Escape", false, false);
    });
    assert_eq!(
        facts,
        ["escape=Core(None) focused=true mounted=true highlight=None"]
    );
    assert_eq!(commands(&rx), []);
}
