//! Headless tests of the Audiobooks keys (D16): the contracts of
//! `audiobooks-keys.test.ts` — the list's cursor moves, Enter plays the
//! cursor book and `a` queues it, `n` is consumed and does nothing (books
//! have no play-next), and the Hero pane's keys act through the chapter
//! Workspace's own contract functions. The 7.1 review's lesson holds: from
//! the list pane and from a hero-less Hero pane, every Library key returns
//! handled and none reaches the core. The router under test is the panel's
//! one routing site, [`super::super::keys::route_library_key`], so these
//! assertions cover the Audiobooks dispatch it hands to
//! [`super::route_audiobooks_panel_key`]. The harness builds a
//! `DioxusDocument` whose root creates the store and runs each scenario in
//! one effect over a recording core link, so no test opens a window or
//! sleeps, and no IPC runs.

use std::sync::{Arc, Mutex, mpsc};

use blitz_dom::Document as _;
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use crate::bridge::frame::focus::GuiPanel;
use crate::bridge::frame::library::{
    GuiArtShapeFrame, GuiAudiobooksChapterRowFrame, GuiAudiobooksDetail, GuiAudiobooksHeroFrame,
    GuiAudiobooksList, GuiAudiobooksRowFrame, GuiAudiobooksWorkspaceFrame, GuiDestinationList,
    GuiLibraryDetail, GuiLibraryList, GuiLibraryTabFrame, GuiMetaKindFrame, GuiMetaSegmentFrame,
};
use crate::bridge::keys::{DomKey, DomModifiers};
use crate::bridge::sections::{GuiSection, GuiSectionBatch};
use crate::core_thread::CoreInput;
use crate::window::arrangement::GuiArrangement;
use crate::window::channel::CoreLink;
use crate::window::clock::{Clock, RecordingClock};
use crate::window::library::state::{LibraryPane, LibraryState};
use crate::window::pointer_target::PointerTarget;
use crate::window::sections::Sections;

/// The facts one scenario recorded, in its run order.
type Facts = Arc<Mutex<Vec<String>>>;

/// One scenario the harness runs against the built store.
type Scenario = std::rc::Rc<dyn Fn(&LibraryState, &Facts)>;

/// The root: creates the store once and runs the scenario in one effect.
fn harness((tx, facts, scenario): (mpsc::Sender<CoreInput>, Facts, Scenario)) -> Element {
    let state = use_hook(|| {
        let playhead_position = Memo::new(|| 0.0);
        let clock: Arc<dyn Clock> = Arc::new(RecordingClock::new());
        LibraryState::new(Sections::new(), CoreLink::new(tx), playhead_position, clock)
    });
    use_effect(move || {
        scenario(&state, &facts);
    });
    rsx! {
        div { "audiobooks-keys-harness" }
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
    let outcome = super::super::keys::route_library_key(
        &key(value, shift),
        Some(sections),
        state,
        None,
        GuiArrangement::Wide,
        &PointerTarget::default(),
    );
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

/// The catalog's book rows: two books under `A` and one under `B`.
fn catalog() -> Vec<GuiAudiobooksRowFrame> {
    vec![
        book("b1", "Aftermath", "Bill Wyman", "4:32", 'A'),
        book("b2", "Audition", "Ryu Murakami", "2:14", 'A'),
        book("b3", "Blindsight", "Peter Watts", "10:02", 'B'),
    ]
}

/// One book row, as the frame carries it.
fn book(
    id: &str,
    title: &str,
    subtitle: &str,
    duration: &str,
    sort_letter: char,
) -> GuiAudiobooksRowFrame {
    GuiAudiobooksRowFrame {
        id: id.to_owned(),
        title: title.to_owned(),
        subtitle: Some(subtitle.to_owned()),
        duration: duration.to_owned(),
        sort_letter,
    }
}

/// The carried Audiobooks list part: the catalog, its load facts.
fn audiobooks_list() -> GuiAudiobooksList {
    GuiAudiobooksList {
        lib_idx: 4,
        loading: false,
        failed: false,
        rows: catalog(),
    }
}

/// The carried Audiobooks detail part: the catalog's first book selected.
fn audiobooks_detail(hero: Option<GuiAudiobooksHeroFrame>) -> GuiAudiobooksDetail {
    GuiAudiobooksDetail {
        workspace_loading: false,
        selected: Some("b1".to_owned()),
        playing_book_id: None,
        hero,
    }
}

/// The selected book's Hero facts with its Workspace's chapter rows.
fn audiobooks_hero() -> GuiAudiobooksHeroFrame {
    GuiAudiobooksHeroFrame {
        title: "Aftermath".to_owned(),
        meta_rows: vec![
            vec![GuiMetaSegmentFrame {
                text: "Bill Wyman".to_owned(),
                kind: GuiMetaKindFrame::Plain,
            }],
            vec![GuiMetaSegmentFrame {
                text: "4:32".to_owned(),
                kind: GuiMetaKindFrame::Time,
            }],
        ],
        duration_row: Some(1),
        progress_row: None,
        overview: None,
        art_shape: GuiArtShapeFrame::Portrait,
        art: None,
        workspace: Some(GuiAudiobooksWorkspaceFrame {
            rows: vec![GuiAudiobooksChapterRowFrame {
                chapter: 1,
                title: "Act one".to_owned(),
                duration: "2:16".to_owned(),
                start_ticks: 0,
                end_ticks: 136 * 1_000_000,
            }],
        }),
    }
}

/// The carried Audiobooks content with a Hero on `hero`, and no Hero on
/// `None`.
fn content(hero: Option<GuiAudiobooksHeroFrame>) -> (GuiLibraryList, GuiLibraryDetail) {
    let list = GuiLibraryList {
        tabs: vec![
            GuiLibraryTabFrame {
                position: 1,
                label: "Music".to_owned(),
            },
            GuiLibraryTabFrame {
                position: 4,
                label: "Audiobooks".to_owned(),
            },
        ],
        current_tab: Some(4),
        destination: GuiDestinationList::Audiobooks(audiobooks_list()),
    };
    (list, GuiLibraryDetail::Audiobooks(audiobooks_detail(hero)))
}

/// A hero-less pane still carries no Hero: the detail without one.
fn pane_hero(pane: LibraryPane) -> Option<GuiAudiobooksHeroFrame> {
    (pane == LibraryPane::Hero).then(audiobooks_hero)
}

/// Carry the content, focus the pane, and paint the list's cursor.
fn carry(state: &LibraryState, pane: LibraryPane, cursor: Option<&str>) {
    let (list, detail) = content(pane_hero(pane));
    let batch = GuiSectionBatch {
        library_list: Some(GuiSection {
            revision: 1,
            value: Some(list),
        }),
        library_detail: Some(GuiSection {
            revision: 1,
            value: Some(detail),
        }),
        ..GuiSectionBatch::default()
    };
    let mut sections = state.sections.clone();
    sections.apply_batch(&batch);
    let mut focus = sections.focus;
    focus.write().panel = GuiPanel::Library;
    let mut ui = state.ui;
    ui.with_mut(|ui| ui.pane = Some(pane));
    let mut audiobooks_ui = state.audiobooks_ui;
    audiobooks_ui.with_mut(|ui| {
        ui.cursor = cursor.map(str::to_owned);
        ui.filter.clear();
    });
}

/// Carry `detail`'s facts again, at a new revision: a second batch the window
/// holds, as a later frame's carry does.
fn recarry(state: &LibraryState, hero: Option<GuiAudiobooksHeroFrame>) {
    let (list, detail) = content(hero);
    let batch = GuiSectionBatch {
        library_list: Some(GuiSection {
            revision: 2,
            value: Some(list),
        }),
        library_detail: Some(GuiSection {
            revision: 2,
            value: Some(detail),
        }),
        ..GuiSectionBatch::default()
    };
    let mut sections = state.sections.clone();
    sections.apply_batch(&batch);
}

/// The pane focus and filter facts the assertions read.
fn ui_facts(state: &LibraryState) -> String {
    let ui = state.ui.peek().clone();
    let audiobooks = state.audiobooks_ui.peek().clone();
    let workspace = state.audiobooks_workspace.peek().clone();
    format!(
        "pane={:?} filter_open={} filter={:?} cursor={:?} chapter_cursor={:?}",
        ui.pane, ui.filter_open, audiobooks.filter, audiobooks.cursor, workspace.cursor,
    )
}

// Enter on the cursor book plays it from its saved position.
#[test]
fn enter_on_the_cursor_book_plays_it_from_its_saved_position() {
    let (facts, rx) = run(|state, facts| {
        carry(state, LibraryPane::List, Some("b2"));

        route(state, facts, "enter", "Enter", false);
    });
    assert_eq!(facts, ["enter=Handled"]);
    assert_eq!(
        commands(&rx),
        [mbv::GuiLibraryCommand::AbsPlay {
            lib_idx: 4,
            item_id: "b2".to_owned(),
        }]
    );
}

// A movement key moves the book cursor and sends no key.
#[test]
fn a_movement_key_moves_the_book_cursor_and_sends_no_key() {
    let (facts, rx) = run(|state, facts| {
        carry(state, LibraryPane::List, Some("b1"));

        route(state, facts, "down", "ArrowDown", false);
        facts.lock().expect("facts").push(ui_facts(state));
    });
    assert_eq!(facts[0], "down=Handled");
    assert!(facts[1].contains("cursor=Some(\"b2\")"));
    assert_eq!(commands(&rx), []);
}

// `a` on the list pane queues the cursor book.
#[test]
fn a_on_the_list_pane_queues_the_cursor_book() {
    let (facts, rx) = run(|state, facts| {
        carry(state, LibraryPane::List, Some("b3"));

        route(state, facts, "a", "a", false);
    });
    assert_eq!(facts, ["a=Handled"]);
    assert_eq!(
        commands(&rx),
        [mbv::GuiLibraryCommand::AbsQueue {
            lib_idx: 4,
            item_id: "b3".to_owned(),
        }]
    );
}

// Books have no play-next: `n` is consumed, so no command leaves and no key
// reaches the core (design D12, row 9.1).
#[test]
fn n_on_the_list_pane_does_nothing_and_is_handled() {
    let (facts, rx) = run(|state, facts| {
        carry(state, LibraryPane::List, Some("b2"));

        route(state, facts, "n", "n", false);
    });
    assert_eq!(facts, ["n=Handled"]);
    assert_eq!(commands(&rx), []);
}

// `/` opens the list's filter under Library focus (design D3, D7): the
// rail's keyboard path on the plain list, and the Queue filter never sees
// the key.
#[test]
fn slash_opens_the_audiobooks_filter_and_sends_no_key() {
    let (facts, rx) = run(|state, facts| {
        carry(state, LibraryPane::List, Some("b1"));

        route(state, facts, "slash", "/", false);
        facts.lock().expect("facts").push(ui_facts(state));
    });
    assert_eq!(facts[0], "slash=Handled");
    assert!(facts[1].contains("filter_open=true"));
    assert_eq!(commands(&rx), []);
}

// Esc closes the open filter (design D3, step 2): its text goes with it,
// and the close runs the Audiobooks list's own resolve, not another
// destination's.
#[test]
fn esc_closes_the_audiobooks_filter_and_drops_its_text() {
    let (facts, rx) = run(|state, facts| {
        carry(state, LibraryPane::List, None);
        let mut audiobooks_ui = state.audiobooks_ui;
        audiobooks_ui.with_mut(|ui| ui.filter = "b1".to_owned());
        let mut ui = state.ui;
        ui.with_mut(|ui| ui.filter_open = true);

        route(state, facts, "esc", "Escape", false);
        facts.lock().expect("facts").push(ui_facts(state));
    });
    assert_eq!(facts[0], "esc=Handled");
    assert!(facts[1].contains("filter_open=false"));
    assert!(facts[1].contains("filter=\"\""));
    assert_eq!(commands(&rx), []);
}

// The Hero pane's keys act through the Workspace's own contract functions
// (design D12): the movements move the chapter cursor, Enter plays the
// chapter, and `a` queues the selected book.
#[test]
fn the_hero_pane_moves_the_chapter_cursor_through_the_workspace() {
    let (facts, rx) = run(|state, facts| {
        carry(state, LibraryPane::Hero, Some("b1"));

        route(state, facts, "down", "ArrowDown", false);
        facts.lock().expect("facts").push(ui_facts(state));
    });
    assert_eq!(facts[0], "down=Handled");
    // The Workspace's single chapter row: the move clamps at its end.
    assert!(facts[1].contains("chapter_cursor=Some(1)"));
    assert_eq!(commands(&rx), []);
}

// The Hero pane plays the cursor chapter on Enter.
#[test]
fn the_hero_pane_plays_the_cursor_chapter_on_enter() {
    let (facts, rx) = run(|state, facts| {
        carry(state, LibraryPane::Hero, Some("b1"));

        route(state, facts, "enter", "Enter", false);
    });
    assert_eq!(facts, ["enter=Handled"]);
    assert_eq!(
        commands(&rx),
        [mbv::GuiLibraryCommand::AbsPlayChapter {
            lib_idx: 4,
            book_id: "b1".to_owned(),
            chapter: 1,
        }]
    );
}

// The Hero pane queues the selected book on `a`.
#[test]
fn the_hero_pane_queues_the_selected_book_on_a() {
    let (facts, rx) = run(|state, facts| {
        carry(state, LibraryPane::Hero, Some("b1"));

        route(state, facts, "a", "a", false);
    });
    assert_eq!(facts, ["a=Handled"]);
    assert_eq!(
        commands(&rx),
        [mbv::GuiLibraryCommand::AbsQueue {
            lib_idx: 4,
            item_id: "b1".to_owned(),
        }]
    );
}

// The Hero pane consumes `n` without a Workspace call.
#[test]
fn the_hero_pane_consumes_n_without_a_workspace_call() {
    let (facts, rx) = run(|state, facts| {
        carry(state, LibraryPane::Hero, Some("b1"));

        route(state, facts, "n", "n", false);
    });
    assert_eq!(facts, ["n=Handled"]);
    assert_eq!(commands(&rx), []);
}

// Review of row 7.1's lesson, applied to Audiobooks from the start: no pane
// leaks a Library key to the core's router — the list pane and a hero-less
// Hero pane included. A pane that does not act on a key still consumes it,
// so `a` and `n` never reach the core's playback chords, which are not gated
// on Panel focus.
#[test]
fn the_list_pane_and_a_hero_less_hero_pane_consume_every_library_key() {
    let (facts, _rx) = run(|state, facts| {
        carry(state, LibraryPane::List, None);
        for value in LIBRARY_KEYS {
            route(state, facts, value, value, false);
        }
        let mut ui = state.ui;
        ui.with_mut(|ui| ui.pane = Some(LibraryPane::Hero));
        recarry(state, None);
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
}

// The pane walk skips the panes the frame does not paint (row 9.1): the
// destination paints no pill row, so the list is the walk's first pane, and a
// frame without a Hero leaves the list the last one.
#[test]
fn shift_tab_before_the_list_takes_queue_panel_focus() {
    let (facts, rx) = run(|state, facts| {
        carry(state, LibraryPane::List, None);

        route(state, facts, "shift_tab", "Tab", true);
    });
    assert_eq!(facts, ["shift_tab=Handled"]);
    assert_eq!(commands(&rx), [mbv::GuiLibraryCommand::TakeQueueFocus]);
}

#[test]
fn tab_past_the_list_without_a_hero_takes_queue_panel_focus() {
    let (facts, rx) = run(|state, facts| {
        carry(state, LibraryPane::List, None);

        route(state, facts, "tab", "Tab", false);
    });
    assert_eq!(facts, ["tab=Handled"]);
    assert_eq!(commands(&rx), [mbv::GuiLibraryCommand::TakeQueueFocus]);
}

// Tab walks from the list into the Hero.
#[test]
fn tab_walks_from_the_list_into_the_hero() {
    let (facts, rx) = run(|state, facts| {
        carry(state, LibraryPane::List, None);
        // The Hero lands after the carry: a second batch whose detail carries
        // it, so the walk sees the Hero pane.
        recarry(state, Some(audiobooks_hero()));

        route(state, facts, "tab", "Tab", false);
        facts.lock().expect("facts").push(ui_facts(state));
    });
    assert_eq!(facts[0], "tab=Handled");
    assert!(facts[1].contains("pane=Some(Hero)"));
    assert_eq!(commands(&rx), []);
}

// Tab from the Queue panel focuses the list, not an absent pill row.
#[test]
fn tab_from_the_queue_panel_focuses_the_list_not_an_absent_pill_row() {
    let (facts, rx) = run(|state, facts| {
        carry(state, LibraryPane::List, None);
        let mut focus = state.sections.focus;
        focus.write().panel = GuiPanel::Queue;

        route(state, facts, "tab", "Tab", false);
        facts.lock().expect("facts").push(ui_facts(state));
    });
    assert_eq!(facts[0], "tab=Handled");
    assert!(facts[1].contains("pane=Some(List)"));
    assert_eq!(commands(&rx), [mbv::GuiLibraryCommand::TakeFocus]);
}
