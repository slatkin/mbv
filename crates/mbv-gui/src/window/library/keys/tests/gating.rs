//! The `gating` contracts of `keys.test.ts` (D16), over the shared harness.

use super::*;

#[test]
fn slash_opens_the_filter_and_sends_no_key() {
    let list = music_list(Vec::new());
    let (facts, rx) = run(move |state, facts| {
        carry(state, list.clone(), music_detail(None));
        let frame = frame(Some(
            &state.sections.library_list.peek().clone().expect("list"),
        ));
        route(state, facts, "slash", "/", false, &frame);
        facts.lock().expect("facts").push(ui_facts(state));
    });
    assert_eq!(
        facts,
        vec![
            "slash=Handled",
            "pane=Some(Pills) filter_open=true filter=\"\" focused_pill=None pill_select=None cursor=None open=[] track_cursor=None dismissed=false"
        ]
    );
    assert_eq!(commands(&rx), []);
}

// Queue Panel focus sends `/` to the core.
#[test]
fn queue_panel_focus_sends_slash_to_the_core() {
    let list = music_list(Vec::new());
    let (facts, rx) = run(move |state, facts| {
        carry(state, list.clone(), music_detail(None));
        let mut held = frame(Some(
            &state.sections.library_list.peek().clone().expect("list"),
        ));
        held.focus.panel = GuiPanel::Queue;
        route(state, facts, "slash", "/", false, &held);
        facts.lock().expect("facts").push(ui_facts(state));
    });
    assert_eq!(
        facts,
        vec![
            "slash=Core",
            "pane=Some(Pills) filter_open=false filter=\"\" focused_pill=None pill_select=None cursor=None open=[] track_cursor=None dismissed=false"
        ]
    );
    // The key command itself is the forwarder's; the router sends none.
    assert_eq!(commands(&rx), []);
}

// An armed prefix sends a Library key to the core.
#[test]
fn an_armed_prefix_sends_a_library_key_to_the_core() {
    let list = music_list(Vec::new());
    let (facts, _rx) = run(move |state, facts| {
        carry(state, list.clone(), music_detail(None));
        let mut held = frame(Some(
            &state.sections.library_list.peek().clone().expect("list"),
        ));
        held.status.prefix_armed = true;
        route(state, facts, "n", "n", false, &held);
    });
    assert_eq!(facts, ["n=Core"]);
}

// An open core overlay sends a Library key to the core.
#[test]
fn an_open_core_overlay_sends_a_library_key_to_the_core() {
    let list = music_list(Vec::new());
    let (facts, _rx) = run(move |state, facts| {
        carry(state, list.clone(), music_detail(None));
        let mut held = frame(Some(
            &state.sections.library_list.peek().clone().expect("list"),
        ));
        held.overlay = Some(GuiOverlayFrame::RowMenu {
            anchor_slot: 0,
            cursor: 0,
            entries: Vec::new(),
        });
        route(state, facts, "slash", "/", false, &held);
        facts.lock().expect("facts").push(ui_facts(state));
    });
    assert_eq!(
        facts,
        vec![
            "slash=Core",
            "pane=Some(Pills) filter_open=false filter=\"\" focused_pill=None pill_select=None cursor=None open=[] track_cursor=None dismissed=false"
        ]
    );
}

// The panel's presence is one fact for the window and the router (design D2,
// D10): a frame that carries no Library section takes no Library key, even
// while the window still holds the content of an earlier frame.
#[test]
fn a_frame_without_the_library_section_leaves_every_library_key_to_the_core() {
    let list = music_list(Vec::new());
    let (facts, rx) = run(move |state, facts| {
        carry(state, list.clone(), music_detail(None));
        let held = frame(None);
        route(state, facts, "slash", "/", false, &held);
        facts.lock().expect("facts").push(ui_facts(state));
    });
    assert_eq!(
        facts,
        vec![
            "slash=Core",
            "pane=Some(Pills) filter_open=false filter=\"\" focused_pill=None pill_select=None cursor=None open=[] track_cursor=None dismissed=false"
        ]
    );
    assert_eq!(commands(&rx), []);
}

// Space on a focused pill asks for its toggle and sends no key.
#[test]
fn the_open_filter_takes_a_typed_key() {
    let list = music_list(Vec::new());
    let (facts, rx) = run(move |state, facts| {
        carry(state, list.clone(), music_detail(None));
        let mut ui = state.ui;
        ui.with_mut(|ui| ui.filter_open = true);
        let frame = frame(Some(
            &state.sections.library_list.peek().clone().expect("list"),
        ));
        route(state, facts, "r", "r", false, &frame);
        facts.lock().expect("facts").push(ui_facts(state));
    });
    assert_eq!(
        facts,
        vec![
            "r=Text",
            "pane=Some(Pills) filter_open=true filter=\"\" focused_pill=None pill_select=None cursor=None open=[] track_cursor=None dismissed=false"
        ]
    );
    assert_eq!(commands(&rx), []);
}

// Esc closes the filter, clears its text, and sends no key.
#[test]
fn esc_closes_the_filter_clears_its_text() {
    let list = music_list(Vec::new());
    let (facts, rx) = run(move |state, facts| {
        carry(state, list.clone(), music_detail(None));
        let mut ui = state.ui;
        ui.with_mut(|ui| {
            ui.filter_open = true;
            ui.filter = "roc".to_owned();
        });
        let frame = frame(Some(
            &state.sections.library_list.peek().clone().expect("list"),
        ));
        route(state, facts, "escape", "Escape", false, &frame);
        facts.lock().expect("facts").push(ui_facts(state));
    });
    assert_eq!(
        facts,
        vec![
            "escape=Handled",
            "pane=Some(Pills) filter_open=false filter=\"\" focused_pill=None pill_select=None cursor=None open=[] track_cursor=None dismissed=false"
        ]
    );
    assert_eq!(commands(&rx), []);
}
