//! Headless tests of the Feeds keys (D16): the contracts of
//! `feeds-keys.test.ts` — the list's cursor moves, Enter plays the entry it
//! rests on, `a` queues it, `n` is consumed and does nothing, the pill row
//! walks its views, and the pane walk skips the Hero a frame without one does
//! not paint. The router under test is the panel's one routing site,
//! [`super::super::keys::route_library_key`], so these assertions cover the
//! Feeds dispatch it hands to [`super::route_feeds_panel_key`]. The harness
//! builds a `DioxusDocument` whose root creates the store and runs each
//! scenario in one effect over a recording core link, so no test opens a
//! window or sleeps, and no IPC runs.

use std::sync::{Arc, Mutex, mpsc};

use blitz_dom::Document as _;
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use crate::bridge::frame::focus::GuiPanel;
use crate::bridge::frame::library::{
    GuiDestinationList, GuiEntryRowFrame, GuiFeedsDetail, GuiFeedsHeroFrame, GuiFeedsList,
    GuiHomeRowStateFrame, GuiLibraryDetail, GuiLibraryList, GuiLibraryTabFrame, GuiMetaKindFrame,
    GuiMetaSegmentFrame, GuiPillFrame,
};
use crate::bridge::keys::{DomKey, DomModifiers};
use crate::bridge::library::GuiFeedPill;
use crate::bridge::sections::{GuiSection, GuiSectionBatch};
use crate::core_thread::CoreInput;
use crate::window::channel::CoreLink;
use crate::window::clock::{Clock, RecordingClock};
use crate::window::library::feeds::initial_feeds_ui;
use crate::window::library::state::{LibraryPane, LibraryState};
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

/// The entry Hero a selected entry paints (design D12, row 11.1).
fn hero() -> GuiFeedsHeroFrame {
    GuiFeedsHeroFrame {
        title: "Entry A".to_owned(),
        parent: Some("Sub One".to_owned()),
        meta_rows: vec![
            vec![GuiMetaSegmentFrame {
                text: "Sub One".to_owned(),
                kind: GuiMetaKindFrame::Plain,
            }],
            vec![GuiMetaSegmentFrame {
                text: "12:30".to_owned(),
                kind: GuiMetaKindFrame::Time,
            }],
        ],
        duration_row: Some(1),
        progress_row: None,
        art_shape: crate::bridge::frame::library::GuiArtShapeFrame::Square,
    }
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

/// Apply one carried Feeds frame without touching the interaction state: a
/// re-carried frame is a fresh deserialization, and the pill focus the walk
/// keeps must survive it (row 10.1's P2).
fn carry_feeds_batch(
    state: &LibraryState,
    revision: u64,
    content: (GuiLibraryList, GuiLibraryDetail),
) {
    let (list, detail) = content;
    let batch = GuiSectionBatch {
        library_list: Some(GuiSection {
            revision,
            value: Some(list),
        }),
        library_detail: Some(GuiSection {
            revision,
            value: Some(detail),
        }),
        ..GuiSectionBatch::default()
    };
    let mut sections = state.sections.clone();
    sections.apply_batch(&batch);
    let mut focus = sections.focus;
    focus.write().panel = GuiPanel::Library;
}

/// Carry the Feeds content and focus the pane the key acts on.
fn carry_feeds(
    state: &LibraryState,
    content: (GuiLibraryList, GuiLibraryDetail),
    pane: LibraryPane,
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
    let mut focus = sections.focus;
    focus.write().panel = GuiPanel::Library;
    let mut ui = state.ui;
    ui.with_mut(|ui| {
        ui.hero_dismissed = true;
        ui.item_menu_open = true;
    });
    let mut ui = state.ui;
    ui.with_mut(|ui| ui.pane = Some(pane));
    let mut feeds_ui = state.feeds_ui;
    feeds_ui.with_mut(|ui| {
        *ui = initial_feeds_ui();
        ui.cursor = cursor.map(str::to_owned);
    });
}

/// The pane focus and pill facts the assertions read.
fn ui_facts(state: &LibraryState) -> String {
    let ui = state.ui.peek().clone();
    let feeds = state.feeds_ui.peek().clone();
    let focused = feeds.focused_pill.clone();
    format!(
        "pane={:?} filter_open={} dismissed={} item_menu_open={} cursor={:?} filter={:?} focused_pill={focused:?} pill_select={:?}",
        ui.pane,
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
        carry_feeds(state, content.clone(), LibraryPane::List, Some("s2::e2"));

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
        carry_feeds(state, content.clone(), LibraryPane::List, Some("s1::e1"));

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
        carry_feeds(state, content.clone(), LibraryPane::List, Some("s1::e1"));

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
        carry_feeds(state, content.clone(), LibraryPane::List, Some("s1::e1"));

        route(state, facts, "n", "n", false);
    });
    assert_eq!(facts, ["n=Handled"]);
    assert_eq!(commands(&rx), []);
}

// Right on the pill row focuses the first pill and sends no key.
#[test]
fn right_on_the_pill_row_focuses_the_first_pill_and_sends_no_key() {
    let content = feeds_content(PillSelection::Latest, &entries(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_feeds(state, content.clone(), LibraryPane::Pills, None);

        route(state, facts, "right", "ArrowRight", false);
        facts.lock().expect("facts").push(ui_facts(state));
    });
    assert_eq!(facts[0], "right=Handled");
    assert!(
        facts[1].contains("focused_pill=Some(Latest)"),
        "facts={:?}",
        facts
    );
    assert_eq!(commands(&rx), []);
}

// Space on a focused subscription pill asks for its selection.
#[test]
fn space_on_a_focused_subscription_pill_asks_for_its_selection() {
    let content = feeds_content(PillSelection::Latest, &entries(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_feeds(state, content.clone(), LibraryPane::Pills, None);
        let mut feeds_ui = state.feeds_ui;
        feeds_ui.with_mut(|ui| {
            ui.focused_pill = Some(GuiFeedPill::Subscription("https://sub.one".to_owned()))
        });

        route(state, facts, "space", " ", false);
        facts.lock().expect("facts").push(ui_facts(state));
    });
    assert_eq!(facts[0], "space=Handled");
    assert!(facts[1].contains("pill_select=Some(Subscription(\"https://sub.one\"))"));
    assert_eq!(commands(&rx), []);
}

// Review of row 10.1, finding P2: the pill walk finds the focused pill by
// its key, not its object, because every carried frame is a fresh
// deserialization. An object-identity find would lose the focus across
// frames and the walk would restart from a row end.
#[test]
fn the_pill_walk_keeps_its_place_when_the_frame_re_carries_equal_pills() {
    let first = feeds_content(PillSelection::Latest, &entries(), None);
    let second = feeds_content(PillSelection::Subscription, &entries(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_feeds(state, first.clone(), LibraryPane::Pills, None);
        let mut feeds_ui = state.feeds_ui;
        feeds_ui.with_mut(|ui| {
            ui.focused_pill = Some(GuiFeedPill::Subscription("https://sub.one".to_owned()))
        });

        // A new frame: structurally equal pills, all fresh objects.
        // A new revision: the section store writes a frame it has not held.
        carry_feeds_batch(state, 2, second.clone());

        route(state, facts, "right", "ArrowRight", false);
        facts.lock().expect("facts").push(ui_facts(state));
    });
    assert_eq!(facts[0], "right=Handled");
    assert!(
        facts[1].contains("focused_pill=Some(Subscription(\"https://sub.two\"))"),
        "facts={:?}",
        facts
    );
    assert_eq!(commands(&rx), []);
}

// Enter on a focused pill asks for its selection.
#[test]
fn enter_on_a_focused_pill_asks_for_its_selection() {
    let content = feeds_content(PillSelection::Subscription, &entries(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_feeds(state, content.clone(), LibraryPane::Pills, None);

        route(state, facts, "right", "ArrowRight", false);
        route(state, facts, "enter", "Enter", false);
        facts.lock().expect("facts").push(ui_facts(state));
    });
    assert_eq!(&facts[0..2], ["right=Handled", "enter=Handled"]);
    assert!(facts[2].contains("pill_select=Some(Latest)"));
    assert_eq!(commands(&rx), []);
}

// `/` opens the list's filter under Library focus (design D3, D7): the
// rail's keyboard path on the plain list, and the Queue filter never sees
// the key.
#[test]
fn slash_opens_the_feeds_filter_and_sends_no_key() {
    let content = feeds_content(PillSelection::Latest, &entries(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_feeds(state, content.clone(), LibraryPane::List, Some("s1::e1"));

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
        carry_feeds(state, content.clone(), LibraryPane::List, Some("s1::e1"));
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

// `a` in the Hero pane queues the frame's selected entry.
#[test]
fn a_in_the_hero_pane_queues_the_frames_selected_entry() {
    let content = feeds_content(PillSelection::Latest, &entries(), Some(hero()));
    let (facts, rx) = run(move |state, facts| {
        carry_feeds(state, content.clone(), LibraryPane::Hero, None);

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

// A movement key in the Hero pane changes nothing and sends no key.
#[test]
fn a_movement_key_in_the_hero_pane_changes_nothing_and_sends_no_key() {
    let content = feeds_content(PillSelection::Latest, &entries(), Some(hero()));
    let (facts, rx) = run(move |state, facts| {
        carry_feeds(state, content.clone(), LibraryPane::Hero, None);

        route(state, facts, "down", "ArrowDown", false);
    });
    assert_eq!(facts, ["down=Handled"]);
    assert_eq!(commands(&rx), []);
}

// Review of row 7.1's P1 lesson, row 11.1: the Feeds panes keep the
// Library's keys from the core's router (design D3, step 4). A pane that does
// not act on a key still consumes it, so `n` and `a` never reach the core's
// playback chords, which are not gated on Panel focus.
#[test]
fn a_movement_key_on_the_pills_pane_does_not_reach_the_core() {
    let content = feeds_content(PillSelection::Latest, &entries(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_feeds(state, content.clone(), LibraryPane::Pills, None);

        route(state, facts, "up", "ArrowUp", false);
    });
    assert_eq!(facts, ["up=Handled"]);
    assert_eq!(commands(&rx), []);
}

// `a` on the pills pane queues the list cursor entry and sends no key.
#[test]
fn a_on_the_pills_pane_queues_the_list_cursor_entry_and_sends_no_key() {
    let content = feeds_content(PillSelection::Latest, &entries(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_feeds(state, content.clone(), LibraryPane::Pills, Some("s2::e2"));

        route(state, facts, "a", "a", false);
    });
    assert_eq!(facts, ["a=Handled"]);
    assert_eq!(
        commands(&rx),
        [mbv::GuiLibraryCommand::FeedsQueue {
            item_id: "s2::e2".to_owned(),
        }]
    );
}

// `n` on the pills pane is consumed and sends no key.
#[test]
fn n_on_the_pills_pane_is_consumed_and_sends_no_key() {
    let content = feeds_content(PillSelection::Latest, &entries(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_feeds(state, content.clone(), LibraryPane::Pills, Some("s2::e2"));

        route(state, facts, "n", "n", false);
    });
    assert_eq!(facts, ["n=Handled"]);
    assert_eq!(commands(&rx), []);
}

// `n` on the hero-less Hero pane is consumed and sends no key.
#[test]
fn n_on_the_hero_less_hero_pane_is_consumed_and_sends_no_key() {
    let content = feeds_content(PillSelection::Latest, &entries(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_feeds(state, content.clone(), LibraryPane::Hero, Some("s1::e1"));

        route(state, facts, "n", "n", false);
    });
    assert_eq!(facts, ["n=Handled"]);
    assert_eq!(commands(&rx), []);
}

// Space with no focused pill keeps its core effect.
#[test]
fn space_with_no_focused_pill_keeps_its_core_effect() {
    let content = feeds_content(PillSelection::Latest, &entries(), None);
    let (_facts, _rx) = run(move |state, facts| {
        carry_feeds(state, content.clone(), LibraryPane::List, Some("s1::e1"));

        route(state, facts, "space", " ", false);
    });
    // The core's own router takes the key: the routing outcome is the fact,
    // and the harness forwards no core key for the forwarder to record.
}

// The pane walk skips the Hero a frame without one does not paint (design
// D12, row 11.1): the list is the walk's last pane there, so Tab past it
// takes Queue Panel focus.
#[test]
fn tab_past_the_hero_less_list_takes_queue_panel_focus() {
    let content = feeds_content(PillSelection::Latest, &entries(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_feeds(state, content.clone(), LibraryPane::List, None);

        route(state, facts, "tab", "Tab", false);
    });
    assert_eq!(facts, ["tab=Handled"]);
    assert_eq!(commands(&rx), [mbv::GuiLibraryCommand::TakeQueueFocus]);
}

// Tab walks from the list into the Hero a frame carries.
#[test]
fn tab_walks_from_the_list_into_the_hero_a_frame_carries() {
    let content = feeds_content(PillSelection::Latest, &entries(), Some(hero()));
    let (facts, rx) = run(move |state, facts| {
        carry_feeds(state, content.clone(), LibraryPane::List, None);

        route(state, facts, "tab", "Tab", false);
        facts.lock().expect("facts").push(ui_facts(state));
    });
    assert_eq!(facts[0], "tab=Handled");
    assert!(facts[1].contains("pane=Some(Hero)"));
    assert_eq!(commands(&rx), []);
}
