//! The `window keys` contracts of `keys.test.ts` (D16, design D2 of
//! `gui-pointer-target-interaction`), over the shared harness: Esc closes what
//! the window opened wherever the pointer is, and a digit opens the destination
//! tab at its place.

use super::*;

// A digit names a carried tab, never a raw position (design D2, step 2): the
// presented destinations are the tabs the top bar paints.
#[test]
fn a_digit_opens_the_carried_tab_at_its_place() {
    let mut list = music_list(Vec::new());
    list.tabs = vec![
        GuiLibraryTabFrame {
            position: 1,
            label: "Music".to_owned(),
        },
        GuiLibraryTabFrame {
            position: 4,
            label: "Audiobooks".to_owned(),
        },
    ];
    let (facts, rx) = run(move |state, facts| {
        carry(state, list.clone(), music_detail(None));
        route(state, facts, "digit", "2", false);
    });
    assert_eq!(
        commands(&rx),
        [GuiLibraryCommand::OpenDestination { position: 4 }.into()]
    );
    assert_eq!(facts, ["digit=Handled"]);
}

#[test]
fn a_digit_past_the_carried_tabs_opens_nothing() {
    let list = music_list(Vec::new());
    let (facts, rx) = run(move |state, facts| {
        carry(state, list.clone(), music_detail(None));
        route(state, facts, "digit", "2", false);
    });
    assert_eq!(commands(&rx), []);
    assert_eq!(facts, ["digit=Handled"]);
}

// Esc closes the right-click row menu wherever the pointer is (design D2, step
// 1): the pointer rests over the Queue column, and the key still belongs to the
// window's open menu.
#[test]
fn esc_closes_the_open_row_menu_from_the_queue_zone() {
    let list = music_list(Vec::new());
    let (facts, rx) = run(move |state, facts| {
        carry(state, list.clone(), music_detail(None));
        state.open_library_row_menu("al1".to_owned(), 40.0, 60.0);
        route_under(state, facts, "escape", "Escape", PointerZone::Queue);
        facts.lock().expect("facts").push(ui_facts(state));
    });
    assert_eq!(facts[0], "escape=Handled");
    assert_eq!(
        facts[1],
        "filter_open=false filter=\"\" pill_select=None open=[] dismissed=true"
    );
    // Only the open's own command reached the core: the Esc sent nothing.
    assert_eq!(
        commands(&rx),
        [GuiLibraryCommand::RowMenu {
            item_id: "al1".to_owned(),
        }
        .into()]
    );
}

// Esc with nothing open is the core's key, and it names the Queue when the
// pointer rests over the Queue list box.
#[test]
fn esc_with_nothing_open_from_the_queue_zone_answers_the_queue() {
    let list = music_list(Vec::new());
    let (facts, rx) = run(move |state, facts| {
        carry(state, list.clone(), music_detail(None));
        route_under(state, facts, "escape", "Escape", PointerZone::Queue);
    });
    assert_eq!(facts, ["escape=Core(Queue)"]);
    assert_eq!(commands(&rx), []);
}

// Esc closes the Narrow Hero overlay on the arrangement the panel paints
// (design D4): the window hands the router the one value the panel is handed,
// so the first Narrow frame after a Wide one closes on its own Esc. Before the
// port read a report that landed after the paint, this Esc fell through to the
// core and left the overlay up (the Wide-to-Narrow stuck-overlay report).
#[test]
fn esc_closes_the_narrow_hero_overlay() {
    let artists = vec![beastie()];
    let list = music_list(artists);
    let detail = music_detail(Some(hero()));
    let (facts, rx) = run(move |state, facts| {
        carry(state, list.clone(), detail.clone());
        let mut ui = state.ui;
        ui.with_mut(|ui| ui.hero_dismissed = false);
        route_in(
            state,
            facts,
            "escape",
            "Escape",
            false,
            GuiArrangement::Narrow,
        );
        facts.lock().expect("facts").push(ui_facts(state));
    });
    assert_eq!(
        facts,
        vec![
            "escape=Handled",
            "filter_open=false filter=\"\" pill_select=None open=[] dismissed=true"
        ]
    );
    assert_eq!(commands(&rx), []);
}

// The overlay paints only in Narrow (design D4): the Esc that would close it
// elsewhere still belongs to the core's router.
#[test]
fn esc_with_no_overlay_or_menu_open_is_the_cores_key() {
    let artists = vec![beastie()];
    let list = music_list(artists);
    let detail = music_detail(Some(hero()));
    let (facts, rx) = run(move |state, facts| {
        carry(state, list.clone(), detail.clone());
        route_in(
            state,
            facts,
            "escape",
            "Escape",
            false,
            GuiArrangement::Wide,
        );
        facts.lock().expect("facts").push(ui_facts(state));
    });
    assert_eq!(
        facts,
        vec![
            "escape=Core(None)",
            "filter_open=false filter=\"\" pill_select=None open=[] dismissed=true"
        ]
    );
    assert_eq!(commands(&rx), []);
}
