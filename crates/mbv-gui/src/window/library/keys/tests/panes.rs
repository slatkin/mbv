//! The `panes` contracts of `keys.test.ts` (D16), over the shared harness.

use super::*;

#[test]
fn tab_with_queue_panel_focus_takes_library_focus() {
    let list = music_list(Vec::new());
    let (facts, rx) = run(move |state, facts| {
        carry(state, list.clone(), music_detail(None));
        queue_panel_focus(state);
        route(state, facts, "tab", "Tab", false);
        facts.lock().expect("facts").push(ui_facts(state));
    });
    assert_eq!(commands(&rx), vec![GuiLibraryCommand::TakeFocus.into()]);
    assert!(facts[1].contains("pane=Some(Pills)"));
}

// The walks cross the panel boundary at their ends (design D3, D4): Tab past
// the Hero and Shift+Tab before the pill row give the Queue panel Panel focus.
#[test]
fn tab_past_the_last_library_pane_takes_queue_panel_focus() {
    let list = music_list(Vec::new());
    let (facts, rx) = run(move |state, facts| {
        carry(state, list.clone(), music_detail(None));
        let mut ui = state.ui;
        ui.with_mut(|ui| ui.pane = Some(LibraryPane::Hero));
        route(state, facts, "tab", "Tab", false);
    });
    assert_eq!(
        commands(&rx),
        vec![GuiLibraryCommand::TakeQueueFocus.into()]
    );
    assert_eq!(facts, ["tab=Handled"]);
}
#[test]
fn shift_tab_before_the_first_library_pane_takes_queue_panel_focus() {
    let list = music_list(Vec::new());
    let (facts, rx) = run(move |state, facts| {
        carry(state, list.clone(), music_detail(None));
        route(state, facts, "shift_tab", "Tab", true);
    });
    assert_eq!(
        commands(&rx),
        vec![GuiLibraryCommand::TakeQueueFocus.into()]
    );
    assert_eq!(facts, ["shift_tab=Handled"]);
}

// A digit names a carried tab, never a raw position (design D3, step 3): the
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

/// The Shows content the destination-switch contracts read (design D12): a
/// frame whose detail paints no Hero, so the walk loses the Hero pane.
fn shows_list() -> GuiLibraryList {
    GuiLibraryList {
        tabs: vec![GuiLibraryTabFrame {
            position: 1,
            label: "Music".to_owned(),
        }],
        current_tab: None,
        destination: GuiDestinationList::Shows(GuiShowsList {
            lib_idx: 0,
            pills: Vec::new(),
            collection_names: Vec::new(),
            loading: false,
            series: Vec::new(),
            upcoming_failed: false,
            rows: Vec::new(),
        }),
    }
}

/// The Shows detail part with no Hero.
fn shows_detail() -> GuiLibraryDetail {
    GuiLibraryDetail::Shows(GuiShowsDetail {
        workspace_loading: false,
        selected: None,
        hero: None,
    })
}

// A pane the active destination does not paint is not a boundary crossing
// (`keys.ts`'s `at`): after a destination switch leaves the last
// destination's pane, Tab walks onto the new destination's list instead of
// giving the Queue panel Panel focus.
#[test]
fn tab_from_a_pane_the_new_destination_does_not_paint_walks_its_list() {
    let list = music_list(Vec::new());
    let (facts, rx) = run(move |state, facts| {
        carry(state, list.clone(), music_detail(Some(hero())));
        // Two Tabs: the pill row takes the walk, then the Hero.
        route(state, facts, "tab1", "Tab", false);
        route(state, facts, "tab2", "Tab", false);
        facts.lock().expect("facts").push(ui_facts(state));
        switch_destination(state, shows_list(), shows_detail());
        route(state, facts, "tab3", "Tab", false);
        facts.lock().expect("facts").push(ui_facts(state));
    });
    // The Music walk reached the Hero, and the third Tab landed on the Shows
    // walk's list, not the Queue panel's boundary.
    assert!(facts[2].contains("pane=Some(Hero)"));
    assert!(facts[4].contains("pane=Some(List)"));
    assert_eq!(commands(&rx), []);
}

// The open filter takes a typed key without preventing its default: the
// browser types the key, and the core gets none.
#[test]
fn tab_onto_the_hero_in_narrow_paints_its_overlay_again() {
    let artists = vec![beastie()];
    let list = music_list(artists);
    let detail = music_detail(Some(hero()));
    let (facts, rx) = run(move |state, facts| {
        carry(state, list.clone(), detail.clone());
        let mut ui = state.ui;
        ui.with_mut(|ui| {
            ui.pane = Some(LibraryPane::List);
            ui.hero_dismissed = true;
        });
        route_in(state, facts, "tab", "Tab", false, GuiArrangement::Narrow);
        facts.lock().expect("facts").push(ui_facts(state));
    });
    assert!(facts[1].contains("pane=Some(Hero)"));
    assert!(facts[1].contains("dismissed=false"));
    assert_eq!(commands(&rx), []);
}

// The other half of the same invariant: closing the overlay while the Hero
// pane held the focus moves the focus back to the painted list.
#[test]
fn esc_with_the_hero_pane_focused_returns_the_pane_focus_to_the_list() {
    let artists = vec![beastie()];
    let list = music_list(artists);
    let detail = music_detail(Some(hero()));
    let (facts, rx) = run(move |state, facts| {
        carry(state, list.clone(), detail.clone());
        let mut ui = state.ui;
        ui.with_mut(|ui| {
            ui.pane = Some(LibraryPane::Hero);
            ui.hero_dismissed = false;
        });
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
            "pane=Some(List) filter_open=false filter=\"\" focused_pill=None pill_select=None cursor=None open=[] track_cursor=None dismissed=true"
        ]
    );
    assert_eq!(commands(&rx), []);
}

// Esc closes the Narrow Hero overlay on the arrangement the panel paints
// (design D4): the window hands the router the one value the panel is handed,
// so the first Narrow frame after a Wide one closes on its own Esc. Before the
// port read a report that landed after the paint, this Esc fell through to the
// core and left the overlay up (the Wide-to-Narrow stuck-overlay report).
// Review of row 4.7, finding P2-b: the cursor was seeded only when a pane
// took the focus, so a tracklist whose cache landed afterwards painted no
// cursor and Enter did nothing. The cursor now derives from the tracks that
// paint.
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
            "pane=Some(Pills) filter_open=false filter=\"\" focused_pill=None pill_select=None cursor=None open=[] track_cursor=None dismissed=true"
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
            "pane=Some(Pills) filter_open=false filter=\"\" focused_pill=None pill_select=None cursor=None open=[] track_cursor=None dismissed=true"
        ]
    );
    assert_eq!(commands(&rx), []);
}

// add-gui-library-panel row 6.2: the Home keys route with their port (task
// 6.4). The pane-walk contracts of the Home section need only the state's
// walk facts, so they hold now.

// The pane walk skips the panes Home does not paint (design D12): no pill
// row, so Tab from the Queue panel focuses the list.
#[test]
fn tab_from_the_queue_panel_focuses_the_list_pane_on_home() {
    let list = home_list();
    let (facts, rx) = run(move |state, facts| {
        carry(
            state,
            list.clone(),
            GuiLibraryDetail::Home(crate::bridge::frame::library::GuiHomeDetail {
                selected: None,
                hero: None,
            }),
        );
        queue_panel_focus(state);
        route(state, facts, "tab", "Tab", false);
        facts.lock().expect("facts").push(ui_facts(state));
    });
    assert_eq!(commands(&rx), vec![GuiLibraryCommand::TakeFocus.into()]);
    assert!(facts[1].contains("pane=Some(List)"));
}
#[test]
fn tab_past_the_hero_on_home_takes_queue_panel_focus() {
    let list = home_list();
    let (facts, rx) = run(move |state, facts| {
        carry(
            state,
            list.clone(),
            GuiLibraryDetail::Home(crate::bridge::frame::library::GuiHomeDetail {
                selected: None,
                hero: None,
            }),
        );
        let mut ui = state.ui;
        ui.with_mut(|ui| ui.pane = Some(LibraryPane::Hero));
        route(state, facts, "tab", "Tab", false);
    });
    assert_eq!(
        commands(&rx),
        vec![GuiLibraryCommand::TakeQueueFocus.into()]
    );
    assert_eq!(facts, ["tab=Handled"]);
}
