//! The collection dropdown's key contracts (gui-collection-dropdown design D7,
//! spec "Open text fields and menus keep the keys"): a closed dropdown takes no
//! key, so Space over its trigger still plays or pauses; the open dropdown's
//! filter takes the typed keys and Backspace wherever the pointer is; and Esc
//! closes the dropdown and clears its filter text.

use super::*;

// Space with the pointer over the Library and the dropdown closed is the
// core's key (design D7): the trigger holds no keyboard focus, so Space keeps
// its play and pause, and the selection does not change.
#[test]
fn space_with_the_closed_dropdown_is_the_cores_key() {
    let list = music_list(Vec::new());
    let (facts, rx) = run(move |state, facts| {
        carry(state, list.clone(), music_detail(None));
        route(state, facts, "space", " ", false);
        facts.lock().expect("facts").push(ui_facts(state));
    });
    assert_eq!(facts[0], "space=Core(None)");
    assert!(facts[1].contains("collections_open=false"));
    assert_eq!(commands(&rx), []);
}

// The open dropdown's filter keeps the typed keys wherever the pointer is (the
// spec's "The dropdown filter keeps the keys"): with the pointer over the
// Queue, the typed keys and Backspace are the field's, and the Queue gets none.
#[test]
fn the_open_dropdown_filter_keeps_the_keys_over_the_queue() {
    let list = music_list(Vec::new());
    let (facts, rx) = run(move |state, facts| {
        carry(state, list.clone(), music_detail(None));
        let mut ui = state.ui;
        ui.with_mut(|ui| ui.collections = Some(String::new()));
        route_under(state, facts, "t", "t", PointerZone::Queue);
        route_under(state, facts, "i", "i", PointerZone::Queue);
        route_under(state, facts, "backspace", "Backspace", PointerZone::Queue);
        facts.lock().expect("facts").push(ui_facts(state));
    });
    assert_eq!(facts[0], "t=Text");
    assert_eq!(facts[1], "i=Text");
    assert_eq!(facts[2], "backspace=Text");
    assert_eq!(commands(&rx), []);
}

// Esc closes the open dropdown and clears its filter text wherever the pointer
// is (design D7), and the Queue gets no key.
#[test]
fn esc_closes_the_open_dropdown_over_the_queue() {
    let list = music_list(Vec::new());
    let (facts, rx) = run(move |state, facts| {
        carry(state, list.clone(), music_detail(None));
        let mut ui = state.ui;
        ui.with_mut(|ui| ui.collections = Some("tifo".to_owned()));
        route_under(state, facts, "escape", "Escape", PointerZone::Queue);
        facts.lock().expect("facts").push(ui_facts(state));
    });
    assert_eq!(facts[0], "escape=Handled");
    assert!(facts[1].contains("collections_open=false"));
    assert!(facts[1].contains("collections_filter=\"\""));
    assert_eq!(commands(&rx), []);
}

// The Shows view dropdown reuses the open fact but mounts no filter field
// (design D8), so the typed keys stay the panel's and the core's while it is
// open: Space keeps its play and pause and `n` reaches the core.
#[test]
fn the_shows_view_popover_takes_no_typed_key() {
    let list = shows_list();
    let (facts, rx) = run(move |state, facts| {
        carry(state, list.clone(), shows_detail());
        let mut ui = state.ui;
        ui.with_mut(|ui| ui.collections = Some(String::new()));
        route_under(state, facts, "space", " ", PointerZone::Queue);
        route_under(state, facts, "n", "n", PointerZone::Queue);
        facts.lock().expect("facts").push(ui_facts(state));
    });
    assert_eq!(facts[0], "space=Core(Queue)");
    assert_eq!(facts[1], "n=Core(Queue)");
    assert!(facts[2].contains("collections_open=true"));
    assert_eq!(commands(&rx), []);
}

// Esc closes the Shows view popover wherever the pointer is, as it closes the
// collection dropdown (design D7, D8): the popover with no filter still binds
// its close.
#[test]
fn esc_closes_the_shows_view_popover() {
    let list = shows_list();
    let (facts, rx) = run(move |state, facts| {
        carry(state, list.clone(), shows_detail());
        let mut ui = state.ui;
        ui.with_mut(|ui| ui.collections = Some(String::new()));
        route_under(state, facts, "escape", "Escape", PointerZone::Queue);
        facts.lock().expect("facts").push(ui_facts(state));
    });
    assert_eq!(facts[0], "escape=Handled");
    assert!(facts[1].contains("collections_open=false"));
    assert_eq!(commands(&rx), []);
}

// A typed key with the dropdown closed is the core's, for the panel the
// pointer names (design D7): the dropdown takes no key while it is closed.
#[test]
fn a_closed_dropdown_takes_no_typed_key() {
    let list = music_list(Vec::new());
    let (facts, rx) = run(move |state, facts| {
        carry(state, list.clone(), music_detail(None));
        route_under(state, facts, "t", "t", PointerZone::Queue);
        facts.lock().expect("facts").push(ui_facts(state));
    });
    assert_eq!(facts[0], "t=Core(Queue)");
    assert_eq!(commands(&rx), []);
}
