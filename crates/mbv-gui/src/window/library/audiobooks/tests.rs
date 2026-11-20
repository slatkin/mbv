//! Headless tests of the book list's interaction state (D16): the contracts
//! of `audiobooks.test.ts` — a press selects the book the Hero shows, the
//! play path and a queue send their commands with the book's stable
//! `library_item_id`, and a rail jump scrolls to the letter's first book. The harness builds a `DioxusDocument` whose root creates the store
//! and runs each scenario in one effect over a recording core link, so no
//! test opens a window or sleeps, and no IPC runs.

use std::sync::{Arc, Mutex, mpsc};

use blitz_dom::Document as _;
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use crate::bridge::frame::library::{
    GuiAudiobooksDetail, GuiAudiobooksList, GuiAudiobooksRowFrame, GuiDestinationList,
    GuiLibraryDetail, GuiLibraryList, GuiLibraryTabFrame, GuiRowArtFrame, GuiRowMediaKindFrame,
};
use crate::bridge::sections::{GuiSection, GuiSectionBatch};
use crate::core_thread::CoreInput;
use crate::window::channel::CoreLink;
use crate::window::clock::{Clock, RecordingClock};
use crate::window::library::audiobooks_list::AudiobooksListRow;
use crate::window::library::state::LibraryState;
use crate::window::sections::Sections;

/// The facts one scenario recorded, in its run order.
type Facts = Arc<Mutex<Vec<String>>>;

/// One scenario the harness runs against the built store.
type Scenario = std::rc::Rc<dyn Fn(&LibraryState, &Facts)>;

/// The root: creates the store once and runs the scenario in one effect.
fn harness((tx, facts, scenario): (mpsc::Sender<CoreInput>, Facts, Scenario)) -> Element {
    let state = use_hook(|| {
        // The playhead position the Audiobooks Workspace reads: the tests
        // hold it at 0, which marks no chapter here.
        let playhead_position = Memo::new(|| 0.0);
        let clock: Arc<dyn Clock> = Arc::new(RecordingClock::new());
        LibraryState::new(Sections::new(), CoreLink::new(tx), playhead_position, clock)
    });
    use_effect(move || {
        scenario(&state, &facts);
    });
    rsx! {
        div { "audiobooks-harness" }
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
        sort_letter,
    }
}

/// The carried Audiobooks content with `selected` picked (row 9.1).
fn audiobooks_content(selected: Option<&str>) -> (GuiLibraryList, GuiLibraryDetail) {
    let list = GuiLibraryList {
        tabs: vec![GuiLibraryTabFrame {
            position: 4,
            label: "Audiobooks".to_owned(),
        }],
        current_tab: Some(4),
        destination: GuiDestinationList::Audiobooks(GuiAudiobooksList {
            lib_idx: 4,
            loading: false,
            failed: false,
            rows: catalog(),
        }),
    };
    let detail = GuiLibraryDetail::Audiobooks(GuiAudiobooksDetail {
        workspace_loading: false,
        selected: selected.map(str::to_owned),
        playing_book_id: None,
        hero: None,
    });
    (list, detail)
}

/// Carry the content with `selected` picked.
fn carry(state: &LibraryState, selected: Option<&str>) {
    let (list, detail) = audiobooks_content(selected);
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
    let mut ui = state.ui;
    ui.with_mut(|ui| {
        ui.hero_dismissed = true;
        ui.item_menu_open = true;
    });
    let mut audiobooks_ui = state.audiobooks_ui;
    audiobooks_ui.with_mut(|ui| ui.filter.clear());
}

/// The interaction facts the assertions read.
fn ui_facts(state: &LibraryState) -> String {
    let ui = state.ui.peek().clone();
    let audiobooks = state.audiobooks_ui.peek().clone();
    format!(
        "dismissed={} item_menu_open={} filter={:?}",
        ui.hero_dismissed, ui.item_menu_open, audiobooks.filter,
    )
}

/// The list's scroll facts the assertions read.
fn scroll_facts(state: &LibraryState) -> String {
    let scroll = *state.scrolls.audiobooks.peek();
    format!(
        "top={} viewport={}",
        scroll.scroll_top, scroll.viewport_height
    )
}

/// The painted rows of the carried catalog.
fn painted_rows(state: &LibraryState) -> Arc<[AudiobooksListRow]> {
    let filter = state.audiobooks_ui.peek().filter.clone();
    state.audiobooks_rows(&filter)
}

// A press selects the book.
#[test]
fn a_press_selects_the_book() {
    let (_facts, rx) = run(|state, _facts| {
        carry(state, Some("b1"));
        let row = painted_rows(state)
            .iter()
            .find(|row| matches!(row, AudiobooksListRow::Item(item) if item.row.id == "b3"))
            .expect("the fixture paints b3")
            .clone();

        state.press_audiobooks_row(&row);
    });
    assert_eq!(
        commands(&rx),
        [mbv::GuiLibraryCommand::AbsSelect {
            lib_idx: 4,
            row: "b3".to_owned(),
        }]
    );
}

// A press on a heading selects nothing.
#[test]
fn a_press_on_a_heading_selects_nothing() {
    let (_facts, rx) = run(|state, _facts| {
        carry(state, Some("b1"));
        let heading = painted_rows(state)[0].clone();

        state.press_audiobooks_row(&heading);
    });
    assert_eq!(commands(&rx), []);
}

// A press never opens the Hero overlay (only the row menu's View Detail does)
// and closes an open item menu.
#[test]
fn a_press_keeps_the_hero_overlay_dismissed_and_closes_an_open_item_menu() {
    let (facts, rx) = run(|state, facts| {
        carry(state, Some("b1"));
        let row = painted_rows(state)[1].clone();

        state.press_audiobooks_row(&row);
        facts.lock().expect("facts").push(ui_facts(state));
    });
    assert!(facts[0].contains("dismissed=true"));
    assert!(facts[0].contains("item_menu_open=false"));
    assert_eq!(
        commands(&rx),
        [mbv::GuiLibraryCommand::AbsSelect {
            lib_idx: 4,
            row: "b1".to_owned(),
        }]
    );
}

// Playing a book sends absPlay with its id.
#[test]
fn playing_a_book_sends_abs_play_with_its_id() {
    let (_facts, rx) = run(|state, _facts| {
        carry(state, Some("b1"));

        state.play_audiobooks_row("b2");
    });
    assert_eq!(
        commands(&rx),
        [mbv::GuiLibraryCommand::AbsPlay {
            lib_idx: 4,
            item_id: "b2".to_owned(),
        }]
    );
}

// Queueing a book sends absQueue with its id.
#[test]
fn queueing_a_book_sends_abs_queue_with_its_id() {
    let (_facts, rx) = run(|state, _facts| {
        carry(state, Some("b1"));

        state.queue_audiobooks_row("b3");
    });
    assert_eq!(
        commands(&rx),
        [mbv::GuiLibraryCommand::AbsQueue {
            lib_idx: 4,
            item_id: "b3".to_owned(),
        }]
    );
}

// A rail jump scrolls the list to the letter's first book (design D7).
#[test]
fn a_rail_jump_scrolls_to_the_letter_first_book() {
    let (facts, rx) = run(|state, facts| {
        carry(state, Some("b1"));
        state.measure_audiobooks_list(40.0);

        state.jump_audiobooks_to_letter('B');
        facts.lock().expect("facts").push(scroll_facts(state));
    });
    // `B`'s heading and the `A` heading with its two books cover 160 px
    // before `B`'s first book, and the 56 px book's bottom edge sits at the
    // viewport's bottom.
    assert_eq!(facts[0], "top=176 viewport=40");
    assert_eq!(commands(&rx), []);
}
