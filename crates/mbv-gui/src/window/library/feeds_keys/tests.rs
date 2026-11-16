//! Headless tests of the Feeds keys (D16): the contracts of
//! `feeds-keys.test.ts` — the list's cursor moves, Enter plays the entry it
//! rests on, `a` queues it, `n` is consumed and does nothing, and Space stays
//! the core's key. The router under test is the panel's one routing site,
//! [`super::super::keys::route_library_key`], so these assertions cover the
//! Feeds dispatch it hands to [`super::route_feeds_panel_key`]. The harness
//! builds a `DioxusDocument` whose root creates the store and runs each
//! scenario in one effect over a recording core link, so no test opens a
//! window or sleeps, and no IPC runs.

use std::sync::{Arc, Mutex, mpsc};

use blitz_dom::Document as _;
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use crate::bridge::frame::library::{
    GuiDestinationList, GuiEntryRowFrame, GuiFeedsDetail, GuiFeedsHeroFrame, GuiFeedsList,
    GuiHomeRowStateFrame, GuiLibraryDetail, GuiLibraryList, GuiLibraryTabFrame, GuiPillFrame,
};
use crate::bridge::keys::{DomKey, DomModifiers};
use crate::bridge::library::GuiFeedPill;
use crate::bridge::sections::{GuiSection, GuiSectionBatch};
use crate::core_thread::CoreInput;
use crate::window::arrangement::GuiArrangement;
use crate::window::channel::CoreLink;
use crate::window::clock::{Clock, RecordingClock};
use crate::window::library::feeds::initial_feeds_ui;
use crate::window::library::state::LibraryState;
use crate::window::pointer_target::{PointerTarget, PointerZone};
use crate::window::sections::Sections;

/// The facts one scenario recorded, in its run order.
type Facts = Arc<Mutex<Vec<String>>>;

/// One scenario the harness runs against the built store.
type Scenario = std::rc::Rc<dyn Fn(&LibraryState, &Facts)>;

/// The root: creates the store once and runs the scenario in one effect.
fn harness((tx, facts, scenario): (mpsc::Sender<CoreInput>, Facts, Scenario)) -> Element {
    let state = use_hook(|| {
        // The playhead position the Feeds list reads: none — the tests hold
        // it at 0.
        let playhead_position = Memo::new(|| 0.0);
        let clock: Arc<dyn Clock> = Arc::new(RecordingClock::new());
        LibraryState::new(Sections::new(), CoreLink::new(tx), playhead_position, clock)
    });
    use_effect(move || {
        scenario(&state, &facts);
    });
    rsx! {
        div { "feeds-keys-harness" }
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
        &library_pointer(),
    );
    facts
        .lock()
        .expect("facts")
        .push(format!("{name}={outcome:?}"));
}

/// The pointer target a list key routes under: the pointer over the Library
/// column, not on a Hero list.
fn library_pointer() -> PointerTarget {
    PointerTarget {
        zone: Some(PointerZone::Library),
        list: None,
        row: None,
    }
}

/// Route one keydown against the window's own section stores and paint the
/// outcome.
fn route(state: &LibraryState, facts: &Facts, name: &str, value: &str, shift: bool) {
    route_on(state, facts, name, value, shift, &state.sections);
}

/// The entry rows the Latest pill merges, newest first.
fn entries() -> Vec<GuiEntryRowFrame> {
    vec![
        entry(
            "s1::e1",
            "Entry A",
            "Sub One",
            "12 May",
            GuiHomeRowStateFrame::Ordinary,
        ),
        entry(
            "s2::e2",
            "Entry B",
            "Sub Two",
            "9 May",
            GuiHomeRowStateFrame::Played,
        ),
    ]
}

/// One entry row, as the frame carries it.
fn entry(
    id: &str,
    title: &str,
    subtitle: &str,
    date: &str,
    state: GuiHomeRowStateFrame,
) -> GuiEntryRowFrame {
    GuiEntryRowFrame {
        id: id.to_owned(),
        title: title.to_owned(),
        subtitle: Some(subtitle.to_owned()),
        date: Some(date.to_owned()),
        state,
    }
}

/// The Latest pill and two subscription pills, with the frame carrying
/// `selected` (design D12).
fn pills(selected: PillSelection) -> Vec<GuiPillFrame<GuiFeedPill>> {
    vec![
        GuiPillFrame {
            pill: GuiFeedPill::Latest,
            name: "Latest".to_owned(),
            selected: selected == PillSelection::Latest,
            collection: false,
        },
        GuiPillFrame {
            pill: GuiFeedPill::Subscription("https://sub.one".to_owned()),
            name: "Sub One".to_owned(),
            selected: selected == PillSelection::Subscription,
            collection: false,
        },
        GuiPillFrame {
            pill: GuiFeedPill::Subscription("https://sub.two".to_owned()),
            name: "Sub Two".to_owned(),
            selected: false,
            collection: false,
        },
    ]
}

/// The pill selection the fixtures name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PillSelection {
    Latest,
    Subscription,
}

/// The carried content while the Feeds destination is active (design D12):
/// no other destination's content, the pill row, and `hero` the caller
/// picks. The frame carries `selected` — the first row's, as the TS default
/// does.
fn feeds_content(
    pill_selection: PillSelection,
    rows: &[GuiEntryRowFrame],
    hero: Option<GuiFeedsHeroFrame>,
) -> (GuiLibraryList, GuiLibraryDetail) {
    let selected = rows.first().map(|row| row.id.clone());
    let list = GuiLibraryList {
        tabs: vec![
            GuiLibraryTabFrame {
                position: 1,
                label: "Music".to_owned(),
            },
            GuiLibraryTabFrame {
                position: 2,
                label: "Feeds".to_owned(),
            },
        ],
        current_tab: Some(2),
        destination: GuiDestinationList::Feeds(GuiFeedsList {
            pills: pills(pill_selection),
            loading: false,
            failed: false,
            rows: rows.to_owned(),
        }),
    };
    let detail = GuiLibraryDetail::Feeds(GuiFeedsDetail { selected, hero });
    (list, detail)
}

/// Carry the Feeds content and paint the list's cursor.
fn carry_feeds(
    state: &LibraryState,
    content: (GuiLibraryList, GuiLibraryDetail),
    cursor: Option<&str>,
) {
    let (list, detail) = content;
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
    let mut feeds_ui = state.feeds_ui;
    feeds_ui.with_mut(|ui| {
        *ui = initial_feeds_ui();
        ui.cursor = cursor.map(str::to_owned);
    });
}

/// The filter and cursor facts the assertions read.
fn ui_facts(state: &LibraryState) -> String {
    let ui = state.ui.peek().clone();
    let feeds = state.feeds_ui.peek().clone();
    format!(
        "filter_open={} dismissed={} item_menu_open={} cursor={:?} filter={:?} pill_select={:?}",
        ui.filter_open,
        ui.hero_dismissed,
        ui.item_menu_open,
        feeds.cursor,
        feeds.filter,
        feeds.pill_select,
    )
}

// Enter on the cursor row plays the entry.
#[test]
fn enter_on_the_cursor_row_plays_the_entry() {
    let content = feeds_content(PillSelection::Latest, &entries(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_feeds(state, content.clone(), Some("s2::e2"));

        route(state, facts, "enter", "Enter", false);
    });
    assert_eq!(facts, ["enter=Handled"]);
    assert_eq!(
        commands(&rx),
        [mbv::GuiLibraryCommand::FeedsPlay {
            item_id: "s2::e2".to_owned(),
        }]
    );
}

// A movement key moves the cursor and sends no key.
#[test]
fn a_movement_key_moves_the_cursor_and_sends_no_key() {
    let content = feeds_content(PillSelection::Latest, &entries(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_feeds(state, content.clone(), Some("s1::e1"));

        route(state, facts, "down", "ArrowDown", false);
        facts.lock().expect("facts").push(ui_facts(state));
    });
    assert_eq!(facts[0], "down=Handled");
    assert!(facts[1].contains("cursor=Some(\"s2::e2\")"));
    assert_eq!(commands(&rx), []);
}

// `a` on the cursor row queues the entry.
#[test]
fn a_on_the_cursor_row_queues_the_entry() {
    let content = feeds_content(PillSelection::Latest, &entries(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_feeds(state, content.clone(), Some("s1::e1"));

        route(state, facts, "a", "a", false);
    });
    assert_eq!(facts, ["a=Handled"]);
    assert_eq!(
        commands(&rx),
        [mbv::GuiLibraryCommand::FeedsQueue {
            item_id: "s1::e1".to_owned(),
        }]
    );
}

// A feed entry has no play-next (row 11.1): `n` is consumed and sends
// nothing, so it neither queues nor reaches the core's playback chords.
#[test]
fn n_on_the_cursor_row_sends_nothing_and_is_handled() {
    let content = feeds_content(PillSelection::Latest, &entries(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_feeds(state, content.clone(), Some("s1::e1"));

        route(state, facts, "n", "n", false);
    });
    assert_eq!(facts, ["n=Handled"]);
    assert_eq!(commands(&rx), []);
}

// `/` opens the list's filter under Library focus (design D3, D7): the
// rail's keyboard path on the plain list, and the Queue filter never sees
// the key.
#[test]
fn slash_opens_the_feeds_filter_and_sends_no_key() {
    let content = feeds_content(PillSelection::Latest, &entries(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_feeds(state, content.clone(), Some("s1::e1"));

        route(state, facts, "slash", "/", false);
        facts.lock().expect("facts").push(ui_facts(state));
    });
    assert_eq!(facts[0], "slash=Handled");
    assert!(facts[1].contains("filter_open=true"));
    assert_eq!(commands(&rx), []);
}

// Esc closes the open filter (design D3, step 2): its text goes with it,
// and the close runs the Feeds list's own resolve, not another
// destination's.
#[test]
fn esc_closes_the_feeds_filter_and_drops_its_text() {
    let content = feeds_content(PillSelection::Latest, &entries(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_feeds(state, content.clone(), Some("s1::e1"));
        let mut feeds_ui = state.feeds_ui;
        feeds_ui.with_mut(|ui| ui.filter = "s1".to_owned());
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
