//! Headless tests of the Audiobooks keys (D16): the contracts of
//! `audiobooks-keys.test.ts` on the row the pointer marks
//! (`gui-pointer-target-interaction` D2) — the movement keys scroll the list
//! under the pointer, Enter plays the hovered book and `a` queues it, the
//! Hero's Enter plays the hovered chapter, and `n` is consumed and does
//! nothing (books have no play-next). The 7.1 review's lesson holds: every
//! Library key on the list returns handled and none reaches the core. The
//! router under test is the panel's one routing site,
//! [`super::super::keys::route_library_key`], so these assertions cover the
//! Audiobooks dispatch it hands to [`super::route_audiobooks_panel_key`]. The
//! harness builds a `DioxusDocument` whose root creates the store and runs
//! each scenario in one effect over a recording core link, so no test opens a
//! window or sleeps, and no IPC runs.

use std::sync::{Arc, Mutex, mpsc};

use blitz_dom::Document as _;
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use crate::bridge::frame::library::{
    GuiArtShapeFrame, GuiAudiobooksChapterRowFrame, GuiAudiobooksDetail, GuiAudiobooksHeroFrame,
    GuiAudiobooksList, GuiAudiobooksRowFrame, GuiAudiobooksWorkspaceFrame, GuiDestinationList,
    GuiLibraryDetail, GuiLibraryList, GuiLibraryTabFrame, GuiMetaKindFrame, GuiMetaSegmentFrame,
    GuiRowArtFrame, GuiRowMediaKindFrame,
};
use crate::bridge::keys::{DomKey, DomModifiers};
use crate::bridge::sections::{GuiSection, GuiSectionBatch};
use crate::core_thread::CoreInput;
use crate::window::arrangement::GuiArrangement;
use crate::window::channel::CoreLink;
use crate::window::clock::{Clock, RecordingClock};
use crate::window::library::state::LibraryState;
use crate::window::pointer_target::{PointerList, PointerTarget, PointerZone};
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

/// The pointer target a list key routes under: the pointer over the Library
/// column, not on a list or a Hero control.
fn library_pointer() -> PointerTarget {
    PointerTarget {
        zone: Some(PointerZone::Library),
        list: None,
        row: None,
    }
}

/// The pointer target over `list`, resting on the row `row` when one is named.
fn pointer_on(list: PointerList, row: Option<&str>) -> PointerTarget {
    PointerTarget {
        zone: Some(PointerZone::Library),
        list: Some(list),
        row: row.map(str::to_owned),
    }
}

/// Route one keydown under one pointer target and paint the outcome.
fn route_pointer(
    state: &LibraryState,
    facts: &Facts,
    name: &str,
    value: &str,
    pointer: &PointerTarget,
) {
    let outcome = super::super::keys::route_library_key(
        &key(value, false),
        Some(&state.sections),
        state,
        None,
        GuiArrangement::Wide,
        pointer,
    );
    facts
        .lock()
        .expect("facts")
        .push(format!("{name}={outcome:?}"));
}

/// Route one keydown with the pointer over the book list, resting on `row`.
fn route_row(state: &LibraryState, facts: &Facts, name: &str, value: &str, row: &str) {
    route_pointer(
        state,
        facts,
        name,
        value,
        &pointer_on(PointerList::Audiobooks, Some(row)),
    );
}

/// Route one keydown with the pointer over the Hero's chapter Workspace,
/// resting on `row` when one is named.
fn route_hero(state: &LibraryState, facts: &Facts, name: &str, value: &str, row: Option<&str>) {
    route_pointer(
        state,
        facts,
        name,
        value,
        &pointer_on(PointerList::Workspace, row),
    );
}

/// Route one keydown with the pointer over the Library column, on no list.
fn route(state: &LibraryState, facts: &Facts, name: &str, value: &str, shift: bool) {
    let outcome = super::super::keys::route_library_key(
        &key(value, shift),
        Some(&state.sections),
        state,
        None,
        GuiArrangement::Wide,
        &library_pointer(),
    );
    facts
        .lock()
        .expect("facts")
        .push(format!("{name}={outcome:?}"));
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
        art: GuiRowArtFrame::NoSource,
        media: GuiRowMediaKindFrame::Audio,
        id: id.to_owned(),
        title: title.to_owned(),
        subtitle: Some(subtitle.to_owned()),
        duration: duration.to_owned(),
        sort_letter: Some(sort_letter),
    }
}

/// The carried Audiobooks list part: the catalog, its load facts.
fn audiobooks_list() -> GuiAudiobooksList {
    GuiAudiobooksList {
        sort: None,
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
            rows: vec![
                GuiAudiobooksChapterRowFrame {
                    chapter: 1,
                    title: "Act one".to_owned(),
                    duration: "2:16".to_owned(),
                    start_ticks: 0,
                    end_ticks: 136 * 1_000_000,
                },
                GuiAudiobooksChapterRowFrame {
                    chapter: 2,
                    title: "Act two".to_owned(),
                    duration: "2:16".to_owned(),
                    start_ticks: 136 * 1_000_000,
                    end_ticks: 272 * 1_000_000,
                },
            ],
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

/// Carry the content without a Hero.
fn carry(state: &LibraryState) {
    carry_content(state, content(None));
}

/// Carry the content with the Hero: its chapter Workspace is the control the
/// pointer marks.
fn carry_hero(state: &LibraryState) {
    carry_content(state, content(Some(audiobooks_hero())));
}

/// Carry the given content.
fn carry_content(state: &LibraryState, carried: (GuiLibraryList, GuiLibraryDetail)) {
    let (list, detail) = carried;
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
    let mut audiobooks_ui = state.audiobooks_ui;
    audiobooks_ui.with_mut(|ui| ui.filter.clear());
}

/// The list's scroll facts the assertions read.
fn scroll_facts(state: &LibraryState) -> String {
    let scroll = *state.scrolls.audiobooks.peek();
    format!(
        "top={} viewport={}",
        scroll.scroll_top, scroll.viewport_height
    )
}

/// The filter facts the assertions read.
fn ui_facts(state: &LibraryState) -> String {
    let ui = state.ui.peek().clone();
    let audiobooks = state.audiobooks_ui.peek().clone();
    format!(
        "filter_open={} filter={:?}",
        ui.filter_open, audiobooks.filter,
    )
}

// Enter on the hovered book plays it from its saved position.
#[test]
fn enter_on_the_hovered_book_plays_it_from_its_saved_position() {
    let (facts, rx) = run(|state, facts| {
        carry(state);

        route_row(state, facts, "enter", "Enter", "b2");
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

// `a` on the hovered book queues it.
#[test]
fn a_on_the_hovered_book_queues_it() {
    let (facts, rx) = run(|state, facts| {
        carry(state);

        route_row(state, facts, "a", "a", "b3");
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
fn n_on_the_hovered_book_does_nothing_and_is_handled() {
    let (facts, rx) = run(|state, facts| {
        carry(state);

        route_row(state, facts, "n", "n", "b2");
    });
    assert_eq!(facts, ["n=Handled"]);
    assert_eq!(commands(&rx), []);
}

// Enter, `n`, and `a` with the pointer over no book row do nothing (design
// D2, step 3): the key is the window's, and it sends no command.
#[test]
fn enter_n_and_a_with_no_hovered_row_send_nothing() {
    let (facts, rx) = run(|state, facts| {
        carry(state);

        route(state, facts, "enter", "Enter", false);
        route(state, facts, "n", "n", false);
        route(state, facts, "a", "a", false);
    });
    assert_eq!(facts, ["enter=Handled", "n=Handled", "a=Handled"]);
    assert_eq!(commands(&rx), []);
}

// Down over the book list scrolls it by the height of the row at the top edge
// (design D6) and plays nothing.
#[test]
fn down_over_the_book_list_scrolls_it_by_one_row() {
    let (facts, rx) = run(|state, facts| {
        carry(state);
        state.measure_audiobooks_list(40.0);

        route_row(state, facts, "down", "ArrowDown", "b1");
        facts.lock().expect("facts").push(scroll_facts(state));
    });
    assert_eq!(facts[0], "down=Handled");
    // The letter heading covers the list's top edge, so the step is its 24 px.
    assert_eq!(facts[1], "top=24 viewport=40");
    assert_eq!(commands(&rx), []);
}

// The movement keys are the core's while the pointer rests off the book list
// (design D2, step 4): nothing scrolls.
#[test]
fn down_off_the_book_list_is_the_cores_key() {
    let (facts, rx) = run(|state, facts| {
        carry(state);
        state.measure_audiobooks_list(40.0);

        route(state, facts, "down", "ArrowDown", false);
        facts.lock().expect("facts").push(scroll_facts(state));
    });
    assert_eq!(facts[0], "down=Core(None)");
    assert_eq!(facts[1], "top=0 viewport=40");
    assert_eq!(commands(&rx), []);
}

// `/` opens the list's filter under Library focus (design D3, D7): the
// rail's keyboard path on the plain list, and the Queue filter never sees
// the key.
#[test]
fn slash_opens_the_audiobooks_filter_and_sends_no_key() {
    let (facts, rx) = run(|state, facts| {
        carry(state);

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
        carry(state);
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

// Enter on the hovered chapter plays or seeks from it (design D1, D2): the
// chapter row's marker is the chapter number, and the Enter path parses it
// back.
#[test]
fn enter_on_the_hovered_chapter_plays_it() {
    let (facts, rx) = run(|state, facts| {
        carry_hero(state);

        route_hero(state, facts, "enter", "Enter", Some("2"));
    });
    assert_eq!(facts, ["enter=Handled"]);
    assert_eq!(
        commands(&rx),
        [mbv::GuiLibraryCommand::AbsPlayChapter {
            lib_idx: 4,
            book_id: "b1".to_owned(),
            chapter: 2,
        }]
    );
}

// Enter on a marker that names no painted chapter sends nothing, and the key
// stays the window's.
#[test]
fn enter_on_no_painted_chapter_sends_nothing() {
    let (facts, rx) = run(|state, facts| {
        carry_hero(state);

        route_hero(state, facts, "enter", "Enter", Some("7"));
        route_hero(state, facts, "enter", "Enter", None);
    });
    assert_eq!(facts, ["enter=Handled", "enter=Handled"]);
    assert_eq!(commands(&rx), []);
}

// The movement keys over the Workspace move nothing and send no key: the
// Workspace keeps no cursor (design D6).
#[test]
fn a_movement_key_on_the_workspace_moves_no_row() {
    let (facts, rx) = run(|state, facts| {
        carry_hero(state);

        route_hero(state, facts, "down", "ArrowDown", Some("1"));
    });
    assert_eq!(facts, ["down=Handled"]);
    assert_eq!(commands(&rx), []);
}

// The Hero's Workspace queues the selected book on `a`.
#[test]
fn the_hero_workspace_queues_the_selected_book_on_a() {
    let (facts, rx) = run(|state, facts| {
        carry_hero(state);

        route_hero(state, facts, "a", "a", Some("1"));
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

// The Hero's Workspace consumes `n` without a call.
#[test]
fn the_hero_workspace_consumes_n_without_a_call() {
    let (facts, rx) = run(|state, facts| {
        carry_hero(state);

        route_hero(state, facts, "n", "n", Some("1"));
    });
    assert_eq!(facts, ["n=Handled"]);
    assert_eq!(commands(&rx), []);
}

// Review of row 7.1's lesson, applied to Audiobooks from the start: no list
// leaks a Library key to the core's router. A control that does not act on a
// key still consumes it, so `a` and `n` never reach the core's playback
// chords.
#[test]
fn the_list_consumes_every_library_key() {
    let (facts, _rx) = run(|state, facts| {
        carry(state);
        for value in LIBRARY_KEYS {
            route_row(state, facts, value, value, "b1");
        }
    });
    let taken: Vec<String> = LIBRARY_KEYS
        .iter()
        .map(|value| format!("{value}=Handled"))
        .collect();
    assert_eq!(facts, taken);
}
