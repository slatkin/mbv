//! Headless tests of the plain Emby-library keys (D16): the contracts of
//! `emby-keys.test.ts` — the list's cursor moves, Enter and the double-click's
//! play command play every row (a collection row too), `n` and `a` play next
//! and queue from every pane, the feed-group pills walk their groups, `/` is
//! taken, and the pane walk skips the pill row a plain library does not paint
//! and the Hero a frame without one does not. The router under test is the
//! panel's one routing site, [`super::super::keys::route_library_key`], so
//! these assertions cover the Emby dispatch it hands to [`super::
//! route_emby_panel_key`]. The harness builds a `DioxusDocument` whose root
//! creates the store and runs each scenario in one effect over a recording
//! core link, so no test opens a window or sleeps, and no IPC runs.

use std::sync::{Arc, Mutex, mpsc};

use blitz_dom::Document as _;
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use crate::bridge::frame::focus::GuiPanel;
use crate::bridge::frame::library::{
    GuiDestinationList, GuiEmbyDetail, GuiEmbyHeroFrame, GuiEmbyList, GuiEmbyPillFrame,
    GuiEmbyRowFrame, GuiEmbyRowKindFrame, GuiLibraryDetail, GuiLibraryList, GuiLibraryTabFrame,
    GuiMetaKindFrame, GuiMetaSegmentFrame,
};
use crate::bridge::keys::{DomKey, DomModifiers};
use crate::bridge::sections::GuiSectionBatch;
use crate::core_thread::CoreInput;
use crate::window::channel::CoreLink;
use crate::window::library::state::{LibraryPane, LibraryState};
use crate::window::sections::Sections;

/// The facts one scenario recorded, in its run order.
type Facts = Arc<Mutex<Vec<String>>>;

/// One scenario the harness runs against the built store.
type Scenario = std::rc::Rc<dyn Fn(&LibraryState, &Facts)>;

/// The root: creates the store once and runs the scenario in one effect.
fn harness((tx, facts, scenario): (mpsc::Sender<CoreInput>, Facts, Scenario)) -> Element {
    let state = use_hook(|| {
        // The playhead position the Audiobooks Workspace reads: the tests
        // hold it at 0, which marks no chapter but feeds the resolution.
        let playhead_position = Memo::new(|| 0.0);
        LibraryState::new(Sections::new(), CoreLink::new(tx), playhead_position)
    });
    use_effect(move || {
        scenario(&state, &facts);
    });
    rsx! {
        div { "emby-keys-harness" }
    }
}

/// Run one scenario against a fresh store and return the facts it recorded
/// and the commands the core link received. Bounded by construction: the
/// effect runs once and the poll loop stops when the document goes idle.
fn run(
    scenario: impl Fn(&LibraryState, &Facts) + 'static,
) -> (Vec<String>, mpsc::Receiver<CoreInput>) {
    let (tx, rx) = mpsc::channel::<CoreInput>();
    let facts: Facts = Arc::new(Mutex::new(Vec::new()));
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(
            harness,
            (
                tx,
                Arc::clone(&facts),
                std::rc::Rc::new(move |state: &LibraryState, recorded: &Facts| {
                    scenario(state, recorded);
                }),
            ),
        ),
        DocumentConfig::default(),
    );
    doc.initial_build();
    poll_until_idle(&mut doc);
    let recorded = facts.lock().expect("facts").clone();
    (recorded, rx)
}

/// Poll the document until its effects have run and their re-renders landed.
fn poll_until_idle(doc: &mut DioxusDocument) {
    for _ in 0..8 {
        if !doc.poll(None) {
            return;
        }
    }
    panic!("document did not become idle");
}

/// The Library commands the scenario sent, as the core thread receives them.
fn commands(rx: &mpsc::Receiver<CoreInput>) -> Vec<mbv::GuiLibraryCommand> {
    let mut commands = Vec::new();
    while let Ok(input) = rx.try_recv() {
        if let CoreInput::Library(command) = input {
            commands.push(command);
        }
    }
    commands
}

/// One keydown as the DOM names it.
fn key(value: &str, shift: bool) -> DomKey {
    DomKey {
        key: value.to_owned(),
        code: String::new(),
        meta_key: false,
        repeat: false,
        modifiers: DomModifiers {
            shift_key: shift,
            ctrl_key: false,
            alt_key: false,
        },
    }
}

/// Route one keydown against the given section stores and paint the outcome.
fn route_on(
    state: &LibraryState,
    facts: &Facts,
    name: &str,
    value: &str,
    shift: bool,
    sections: &Sections,
) {
    let outcome = super::super::keys::route_library_key(&key(value, shift), Some(sections), state);
    facts
        .lock()
        .expect("facts")
        .push(format!("{name}={outcome:?}"));
}

/// Route one keydown against the window's own section stores and paint the
/// outcome.
fn route(state: &LibraryState, facts: &Facts, name: &str, value: &str, shift: bool) {
    route_on(state, facts, name, value, shift, &state.sections);
}

/// The Library's own keys: the movements, Right and Left's open and close,
/// Enter, and the item keys `n` and `a` (design D3, step 4).
const LIBRARY_KEYS: &[&str] = &[
    "Enter",
    "ArrowUp",
    "ArrowDown",
    "ArrowLeft",
    "ArrowRight",
    "PageUp",
    "PageDown",
    "Home",
    "End",
    "n",
    "a",
];

/// The plain mode's name-sorted rows: two `A` movies and one `B` movie.
fn movies() -> Vec<GuiEmbyRowFrame> {
    vec![
        GuiEmbyRowFrame {
            id: "m1".to_owned(),
            title: "Alien".to_owned(),
            subtitle: Some("1979".to_owned()),
            date: None,
            sort_letter: Some('A'),
            kind: GuiEmbyRowKindFrame::Media,
        },
        GuiEmbyRowFrame {
            id: "m2".to_owned(),
            title: "Amélie".to_owned(),
            subtitle: Some("2001".to_owned()),
            date: None,
            sort_letter: Some('A'),
            kind: GuiEmbyRowKindFrame::Media,
        },
        GuiEmbyRowFrame {
            id: "m3".to_owned(),
            title: "Brazil".to_owned(),
            subtitle: Some("1985".to_owned()),
            date: None,
            sort_letter: Some('B'),
            kind: GuiEmbyRowKindFrame::Media,
        },
    ]
}

/// The feed-group mode's date-ordered rows, with a folder as the first.
fn feed_rows() -> Vec<GuiEmbyRowFrame> {
    vec![
        GuiEmbyRowFrame {
            id: "f1".to_owned(),
            title: "Family".to_owned(),
            subtitle: None,
            date: Some("1998".to_owned()),
            sort_letter: None,
            kind: GuiEmbyRowKindFrame::Collection,
        },
        GuiEmbyRowFrame {
            id: "f2".to_owned(),
            title: "Fargo".to_owned(),
            subtitle: None,
            date: Some("1996".to_owned()),
            sort_letter: None,
            kind: GuiEmbyRowKindFrame::Media,
        },
    ]
}

/// The selected row's Hero facts: a plain Emby-library Hero with no
/// Workspace.
fn emby_hero() -> GuiEmbyHeroFrame {
    GuiEmbyHeroFrame {
        kind: "movie".to_owned(),
        title: "Alien".to_owned(),
        meta_rows: vec![vec![GuiMetaSegmentFrame {
            text: "1979 · 1 h 57 m".to_owned(),
            kind: GuiMetaKindFrame::Time,
        }]],
        duration_row: Some(0),
        progress_row: None,
        overview: None,
        art_shape: crate::bridge::frame::library::GuiArtShapeFrame::Landscape,
        art: None,
        credits: Vec::new(),
        imdb_url: None,
    }
}

/// The carried content while an Emby library is active (row 8.1): no Music,
/// Home, or Shows content, the mode's rows, and the Hero the frame paints.
fn emby_content(
    feed_group: bool,
    rows: &[GuiEmbyRowFrame],
    hero: Option<GuiEmbyHeroFrame>,
) -> (GuiLibraryList, GuiLibraryDetail) {
    let list = GuiLibraryList {
        tabs: vec![
            GuiLibraryTabFrame {
                position: 1,
                label: "Music".to_owned(),
            },
            GuiLibraryTabFrame {
                position: 3,
                label: "Movies".to_owned(),
            },
        ],
        current_tab: Some(3),
        destination: GuiDestinationList::Emby(GuiEmbyList {
            lib_idx: 3,
            library_name: "Movies".to_owned(),
            pills: if feed_group {
                vec![
                    GuiEmbyPillFrame {
                        group: crate::bridge::frame::library::GuiEmbyFeedGroupFrame::All,
                        name: "All".to_owned(),
                        selected: true,
                        collection: false,
                    },
                    GuiEmbyPillFrame {
                        group: crate::bridge::frame::library::GuiEmbyFeedGroupFrame::Folder(
                            "folder-1".to_owned(),
                        ),
                        name: "Family".to_owned(),
                        selected: false,
                        collection: false,
                    },
                ]
            } else {
                Vec::new()
            },
            collection_names: if feed_group {
                vec!["All".to_owned()]
            } else {
                Vec::new()
            },
            feed_group,
            loading: false,
            failed: false,
            rows: rows.to_owned(),
        }),
    };
    let detail = GuiLibraryDetail::Emby(GuiEmbyDetail {
        selected: rows.first().map(|row| row.id.clone()),
        hero,
    });
    (list, detail)
}

/// The Library Panel focus, as the routed keys read it: the Section's focus
/// store starts on the Queue panel, so the harness carries it first.
fn carry_panel_focus(sections: &Sections) {
    let mut focus = sections.focus;
    focus.write().panel = GuiPanel::Library;
}

/// Carry the Emby content and focus the pane the key acts on.
fn carry_emby(
    state: &LibraryState,
    content: (GuiLibraryList, GuiLibraryDetail),
    pane: LibraryPane,
    cursor: Option<&str>,
) {
    let (list, detail) = content;
    let batch = GuiSectionBatch {
        library_list: Some(crate::bridge::sections::GuiSection {
            revision: 1,
            value: Some(list),
        }),
        library_detail: Some(crate::bridge::sections::GuiSection {
            revision: 1,
            value: Some(detail),
        }),
        ..GuiSectionBatch::default()
    };
    let mut sections = state.sections.clone();
    sections.apply_batch(&batch);
    carry_panel_focus(&sections);
    let mut ui = state.ui;
    ui.with_mut(|ui| ui.pane = Some(pane));
    let mut emby_ui = state.emby_ui;
    emby_ui.with_mut(|ui| {
        ui.cursor = cursor.map(str::to_owned);
        ui.focused_pill = None;
        ui.pill_select = None;
        ui.filter.clear();
    });
}

/// The pane focus and filter facts the assertions read.
fn ui_facts(state: &LibraryState) -> String {
    let ui = state.ui.peek().clone();
    let emby = state.emby_ui.peek().clone();
    format!(
        "pane={:?} filter_open={} emby_filter={:?} focused_pill={:?} pill_select={:?} cursor={:?}",
        ui.pane, ui.filter_open, emby.filter, emby.focused_pill, emby.pill_select, emby.cursor,
    )
}

// Enter on the cursor row plays it.
#[test]
fn enter_on_the_cursor_row_plays_it() {
    let content = emby_content(false, &movies(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_emby(state, content.clone(), LibraryPane::List, Some("m2"));

        route(state, facts, "enter", "Enter", false);
    });
    assert_eq!(facts, ["enter=Handled"]);
    assert_eq!(
        commands(&rx),
        [mbv::GuiLibraryCommand::EmbyPlay {
            lib_idx: 3,
            item_id: "m2".to_owned(),
        }]
    );
}

// A folder or boxset row plays through the core's folder-play rule, so Enter
// on it sends the same play command as any other row (row 8.1).
#[test]
fn enter_on_a_collection_row_plays_it() {
    let content = emby_content(true, &feed_rows(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_emby(state, content.clone(), LibraryPane::List, Some("f1"));

        route(state, facts, "enter", "Enter", false);
    });
    assert_eq!(facts, ["enter=Handled"]);
    assert_eq!(
        commands(&rx),
        [mbv::GuiLibraryCommand::EmbyPlay {
            lib_idx: 3,
            item_id: "f1".to_owned(),
        }]
    );
}

// `n` and `a` play the cursor row next and queue it.
#[test]
fn n_and_a_play_the_cursor_row_next_and_queue_it() {
    let content = emby_content(false, &movies(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_emby(state, content.clone(), LibraryPane::List, Some("m1"));

        route(state, facts, "n", "n", false);
        route(state, facts, "a", "a", false);
    });
    assert_eq!(facts, ["n=Handled", "a=Handled"]);
    assert_eq!(
        commands(&rx),
        [
            mbv::GuiLibraryCommand::EmbyPlayNext {
                lib_idx: 3,
                item_id: "m1".to_owned(),
            },
            mbv::GuiLibraryCommand::EmbyQueue {
                lib_idx: 3,
                item_id: "m1".to_owned(),
            },
        ]
    );
}

// A movement key moves the Emby cursor and sends no key.
#[test]
fn a_movement_key_moves_the_emby_cursor_and_sends_no_key() {
    let content = emby_content(false, &movies(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_emby(state, content.clone(), LibraryPane::List, Some("m1"));

        route(state, facts, "down", "ArrowDown", false);
        facts.lock().expect("facts").push(ui_facts(state));
    });
    assert_eq!(facts[0], "down=Handled");
    assert!(facts[1].contains("cursor=Some(\"m2\")"));
    assert_eq!(commands(&rx), []);
}

// Right on the feed-group pills focuses the first pill and sends no key.
#[test]
fn right_on_the_feed_group_pills_focuses_the_first_pill_and_sends_no_key() {
    let content = emby_content(true, &feed_rows(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_emby(state, content.clone(), LibraryPane::Pills, None);

        route(state, facts, "right", "ArrowRight", false);
        facts.lock().expect("facts").push(ui_facts(state));
    });
    assert_eq!(facts[0], "right=Handled");
    assert!(facts[1].contains("focused_pill=Some(\"all\")"));
    assert_eq!(commands(&rx), []);
}

// Space on a focused feed-group pill asks for its group and sends no key.
#[test]
fn space_on_a_focused_feed_group_pill_asks_for_its_group_and_sends_no_key() {
    let content = emby_content(true, &feed_rows(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_emby(state, content.clone(), LibraryPane::Pills, None);
        let mut emby_ui = state.emby_ui;
        emby_ui.with_mut(|ui| ui.focused_pill = Some("folder:folder-1".to_owned()));

        route(state, facts, "space", " ", false);
        facts.lock().expect("facts").push(ui_facts(state));
    });
    assert_eq!(facts[0], "space=Handled");
    assert!(facts[1].contains("pill_select=Some(Folder(\"folder-1\"))"));
    assert_eq!(commands(&rx), []);
}

// `/` opens the list's filter under Library focus (design D3, D7): the
// rail's keyboard path on the plain list, and the Queue filter never sees
// the key.
#[test]
fn slash_opens_the_emby_filter_and_sends_no_key() {
    let content = emby_content(false, &movies(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_emby(state, content.clone(), LibraryPane::List, Some("m1"));

        route(state, facts, "slash", "/", false);
        facts.lock().expect("facts").push(ui_facts(state));
    });
    assert_eq!(facts[0], "slash=Handled");
    assert!(facts[1].contains("filter_open=true"));
    assert_eq!(commands(&rx), []);
}

// Esc closes the open filter (design D3, step 2): its text goes with it,
// and the close runs the Emby list's own resolve, not another
// destination's.
#[test]
fn esc_closes_the_emby_filter_and_drops_its_text() {
    let content = emby_content(false, &movies(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_emby(state, content.clone(), LibraryPane::List, Some("m1"));
        let mut emby_ui = state.emby_ui;
        emby_ui.with_mut(|ui| ui.filter = "m1".to_owned());
        let mut ui = state.ui;
        ui.with_mut(|ui| ui.filter_open = true);

        route(state, facts, "esc", "Escape", false);
        facts.lock().expect("facts").push(ui_facts(state));
    });
    assert_eq!(facts[0], "esc=Handled");
    assert!(facts[1].contains("filter_open=false"));
    assert!(facts[1].contains("emby_filter=\"\""));
    assert_eq!(commands(&rx), []);
}

// The pane walk skips the panes the Emby frame does not paint (row 8.1): a
// plain library paints no pill row, so the list is the walk's first pane, and
// a frame without a Hero leaves the list the last one.
#[test]
fn shift_tab_before_the_plain_list_takes_queue_panel_focus() {
    let content = emby_content(false, &movies(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_emby(state, content.clone(), LibraryPane::List, None);

        route(state, facts, "shift_tab", "Tab", true);
    });
    assert_eq!(facts, ["shift_tab=Handled"]);
    assert_eq!(commands(&rx), [mbv::GuiLibraryCommand::TakeQueueFocus]);
}

#[test]
fn tab_past_the_plain_list_without_a_hero_takes_queue_panel_focus() {
    let content = emby_content(false, &movies(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_emby(state, content.clone(), LibraryPane::List, None);

        route(state, facts, "tab", "Tab", false);
    });
    assert_eq!(facts, ["tab=Handled"]);
    assert_eq!(commands(&rx), [mbv::GuiLibraryCommand::TakeQueueFocus]);
}

#[test]
fn tab_walks_from_the_plain_list_into_the_emby_hero() {
    let content = emby_content(false, &movies(), Some(emby_hero()));
    let (facts, rx) = run(move |state, facts| {
        carry_emby(state, content.clone(), LibraryPane::List, None);

        route(state, facts, "tab", "Tab", false);
        facts.lock().expect("facts").push(ui_facts(state));
    });
    assert_eq!(facts[0], "tab=Handled");
    assert!(facts[1].contains("pane=Some(Hero)"));
    assert_eq!(commands(&rx), []);
}

#[test]
fn tab_from_the_queue_panel_focuses_the_plain_list_not_an_absent_pill_row() {
    let content = emby_content(false, &movies(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_emby(state, content.clone(), LibraryPane::List, None);
        let mut focus = state.sections.focus;
        focus.write().panel = GuiPanel::Queue;

        route(state, facts, "tab", "Tab", false);
        facts.lock().expect("facts").push(ui_facts(state));
    });
    assert_eq!(facts[0], "tab=Handled");
    assert!(facts[1].contains("pane=Some(List)"));
    assert_eq!(commands(&rx), [mbv::GuiLibraryCommand::TakeFocus]);
}

// Review of row 7.1's lesson, applied to Emby from the start: no Emby pane
// leaks a Library key to the core's router — the feed-group pill row, the
// Hero, and a hero-less Hero pane included. A pane that does not act on a key
// still consumes it, so `a` and `n` never reach the core's playback chords,
// which are not gated on Panel focus.
#[test]
fn the_feed_group_pills_and_a_hero_less_hero_pane_consume_every_library_key() {
    let content = emby_content(true, &feed_rows(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_emby(state, content.clone(), LibraryPane::Pills, None);
        for value in LIBRARY_KEYS {
            route(state, facts, value, value, false);
        }
        let mut ui = state.ui;
        ui.with_mut(|ui| ui.pane = Some(LibraryPane::Hero));
        for value in LIBRARY_KEYS {
            route(state, facts, value, value, false);
        }
    });
    let taken: Vec<String> = LIBRARY_KEYS
        .iter()
        .chain(LIBRARY_KEYS.iter())
        .map(|value| format!("{value}=Handled"))
        .collect();
    assert_eq!(facts, taken);
    assert_eq!(commands(&rx), []);
}

// `a` on the pills pane queues the list cursor row and sends no key.
#[test]
fn a_on_the_pills_pane_queues_the_list_cursor_row_and_sends_no_key() {
    let content = emby_content(true, &feed_rows(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_emby(state, content.clone(), LibraryPane::Pills, Some("f2"));

        route(state, facts, "a", "a", false);
    });
    assert_eq!(facts, ["a=Handled"]);
    assert_eq!(
        commands(&rx),
        [mbv::GuiLibraryCommand::EmbyQueue {
            lib_idx: 3,
            item_id: "f2".to_owned(),
        }]
    );
}

// `n` on the Emby Hero pane plays the selected row next.
#[test]
fn n_on_the_emby_hero_pane_plays_the_selected_row_next() {
    let content = emby_content(false, &movies(), Some(emby_hero()));
    let (facts, rx) = run(move |state, facts| {
        carry_emby(state, content.clone(), LibraryPane::Hero, Some("m2"));

        route(state, facts, "n", "n", false);
    });
    assert_eq!(facts, ["n=Handled"]);
    assert_eq!(
        commands(&rx),
        [mbv::GuiLibraryCommand::EmbyPlayNext {
            lib_idx: 3,
            item_id: "m1".to_owned(),
        }]
    );
}

// A movement key on the Emby Hero pane does not reach the core.
#[test]
fn a_movement_key_on_the_emby_hero_pane_does_not_reach_the_core() {
    let content = emby_content(false, &movies(), Some(emby_hero()));
    let (facts, rx) = run(move |state, facts| {
        carry_emby(state, content.clone(), LibraryPane::Hero, None);

        route(state, facts, "down", "ArrowDown", false);
    });
    assert_eq!(facts, ["down=Handled"]);
    assert_eq!(commands(&rx), []);
}
