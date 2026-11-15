//! The `pills` contracts of `keys.test.ts` (D16), over the shared harness.

use super::*;

#[test]
fn space_on_a_focused_pill_asks_for_its_toggle() {
    let list = music_list(Vec::new());
    let (facts, rx) = run(move |state, facts| {
        carry(state, list.clone(), music_detail(None));
        let mut ui = state.ui;
        ui.with_mut(|ui| ui.focused_pill = Some("g2".to_owned()));
        route(state, facts, "space", " ", false);
        facts.lock().expect("facts").push(ui_facts(state));
    });
    assert_eq!(facts[0], "space=Handled");
    assert!(facts[1].contains("pill_select=Some(\"g2\")"));
    assert_eq!(commands(&rx), []);
}

// Space with no focused pill is sent to the core.
#[test]
fn space_with_no_focused_pill_is_sent_to_the_core() {
    let list = music_list(Vec::new());
    let (facts, _rx) = run(move |state, facts| {
        carry(state, list.clone(), music_detail(None));
        route(state, facts, "space", " ", false);
        facts.lock().expect("facts").push(ui_facts(state));
    });
    assert_eq!(facts[0], "space=Core");
    assert!(facts[1].contains("pill_select=None"));
}

// Right with no focused pill focuses the first pill and sends no key.
#[test]
fn right_with_no_focused_pill_focuses_the_first_pill() {
    let list = music_list(Vec::new());
    let (facts, rx) = run(move |state, facts| {
        carry(state, list.clone(), music_detail(None));
        route(state, facts, "right", "ArrowRight", false);
        facts.lock().expect("facts").push(ui_facts(state));
    });
    assert_eq!(facts[0], "right=Handled");
    assert!(facts[1].contains("focused_pill=Some(\"g1\")"));
    assert_eq!(commands(&rx), []);
}

// Enter on a focused pill asks for its toggle and sends no key.
#[test]
fn enter_on_a_focused_pill_asks_for_its_toggle() {
    let list = music_list(Vec::new());
    let (facts, rx) = run(move |state, facts| {
        carry(state, list.clone(), music_detail(None));
        let mut ui = state.ui;
        ui.with_mut(|ui| ui.focused_pill = Some("g2".to_owned()));
        route(state, facts, "enter", "Enter", false);
        facts.lock().expect("facts").push(ui_facts(state));
    });
    assert_eq!(facts[0], "enter=Handled");
    assert!(facts[1].contains("pill_select=Some(\"g2\")"));
    assert_eq!(commands(&rx), []);
}

// Tab with Queue Panel focus takes Library focus and sends no key.
