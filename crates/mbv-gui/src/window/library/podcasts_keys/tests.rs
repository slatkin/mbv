//! Headless tests of the Podcasts keys (D16): the contracts of
//! `podcasts-keys.test.ts` on the row the pointer marks
//! (`gui-pointer-target-interaction` D2) — the movement keys scroll the list
//! under the pointer, Enter plays the hovered episode, `a` queues it, `n` is
//! consumed and does nothing, and Space stays the core's key. The router under
//! test is the panel's one routing site,
//! [`super::super::keys::route_library_key`], so these assertions cover the
//! Podcasts dispatch it hands to [`super::route_podcasts_panel_key`]. The
//! harness builds a `DioxusDocument` whose root creates the store and runs
//! each scenario in one effect over a recording core link, so no test opens a
//! window or sleeps, and no IPC runs.

use std::sync::{Arc, Mutex, mpsc};

use blitz_dom::Document as _;
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use crate::bridge::frame::library::{
    GuiDestinationList, GuiEntryRowFrame, GuiHomeRowStateFrame, GuiLibraryDetail, GuiLibraryList,
    GuiLibraryTabFrame, GuiPillFrame, GuiPodcastsDetail, GuiPodcastsHeroFrame, GuiPodcastsList,
};
use crate::bridge::keys::{DomKey, DomModifiers};
use crate::bridge::library::GuiPodcastPill;
use crate::bridge::sections::{GuiSection, GuiSectionBatch};
use crate::core_thread::CoreInput;
use crate::window::arrangement::GuiArrangement;
use crate::window::channel::CoreLink;
use crate::window::clock::{Clock, RecordingClock};
use crate::window::library::podcasts::initial_podcasts_ui;
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
        // The playhead position the Podcasts list reads: none — the tests
        // hold it at 0.
        let playhead_position = Memo::new(|| 0.0);
        let clock: Arc<dyn Clock> = Arc::new(RecordingClock::new());
        LibraryState::new(Sections::new(), CoreLink::new(tx), playhead_position, clock)
    });
    use_effect(move || {
        scenario(&state, &facts);
    });
    rsx! {
        div { "podcasts-keys-harness" }
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

/// Route one keydown with the pointer over the episode list, resting on `row`.
fn route_row(state: &LibraryState, facts: &Facts, name: &str, value: &str, row: &str) {
    route_pointer(
        state,
        facts,
        name,
        value,
        &pointer_on(PointerList::Podcasts, Some(row)),
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

/// The episode rows the Latest pill merges, newest first.
fn episodes() -> Vec<GuiEntryRowFrame> {
    vec![
        episode(
            "e1:ep1",
            "Episode A",
            "Show One",
            "12 May",
            GuiHomeRowStateFrame::Ordinary,
        ),
        episode(
            "e2:ep2",
            "Episode B",
            "Show Two",
            "9 May",
            GuiHomeRowStateFrame::Played,
        ),
    ]
}

/// One episode row, as the frame carries it.
fn episode(
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

/// The Latest pill and two show pills, with the frame carrying `selected`
/// (design D12).
fn pills(selected: PillSelection) -> Vec<GuiPillFrame<GuiPodcastPill>> {
    vec![
        GuiPillFrame {
            pill: GuiPodcastPill::Latest,
            name: "Latest".to_owned(),
            selected: selected == PillSelection::Latest,
            collection: false,
        },
        GuiPillFrame {
            pill: GuiPodcastPill::Show("sh1".to_owned()),
            name: "Show One".to_owned(),
            selected: selected == PillSelection::Show,
            collection: false,
        },
        GuiPillFrame {
            pill: GuiPodcastPill::Show("sh2".to_owned()),
            name: "Show Two".to_owned(),
            selected: false,
            collection: false,
        },
    ]
}

/// The pill selection the fixtures name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PillSelection {
    Latest,
    Show,
}

/// The carried content while Podcasts is active (design D12): no other
/// destination's content, the pill row, and `hero` the caller picks. The
/// frame carries `selected` — the first row's, as the TS default does.
fn podcasts_content(
    pill_selection: PillSelection,
    rows: &[GuiEntryRowFrame],
    hero: Option<GuiPodcastsHeroFrame>,
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
                label: "Podcasts".to_owned(),
            },
        ],
        current_tab: Some(2),
        destination: GuiDestinationList::Podcasts(GuiPodcastsList {
            lib_idx: 0,
            pills: pills(pill_selection),
            loading: false,
            failed: false,
            rows: rows.to_owned(),
        }),
    };
    let detail = GuiLibraryDetail::Podcasts(GuiPodcastsDetail { selected, hero });
    (list, detail)
}

/// Carry the Podcasts content and start the list's interaction state over.
fn carry_podcasts(state: &LibraryState, content: (GuiLibraryList, GuiLibraryDetail)) {
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
    let mut podcasts_ui = state.podcasts_ui;
    podcasts_ui.with_mut(|ui| *ui = initial_podcasts_ui());
}

/// The list's scroll facts the assertions read.
fn scroll_facts(state: &LibraryState) -> String {
    let scroll = *state.scrolls.podcasts.peek();
    format!(
        "top={} viewport={}",
        scroll.scroll_top, scroll.viewport_height
    )
}

/// The filter facts the assertions read.
fn ui_facts(state: &LibraryState) -> String {
    let ui = state.ui.peek().clone();
    let podcasts = state.podcasts_ui.peek().clone();
    format!(
        "filter_open={} dismissed={} item_menu_open={} filter={:?} pill_select={:?}",
        ui.filter_open, ui.hero_dismissed, ui.item_menu_open, podcasts.filter, podcasts.pill_select,
    )
}

// Enter on the hovered row plays the episode.
#[test]
fn enter_on_the_hovered_row_plays_the_episode() {
    let content = podcasts_content(PillSelection::Latest, &episodes(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_podcasts(state, content.clone());

        route_row(state, facts, "enter", "Enter", "e2:ep2");
    });
    assert_eq!(facts, ["enter=Handled"]);
    assert_eq!(
        commands(&rx),
        [mbv::GuiLibraryCommand::PodcastsPlay {
            lib_idx: 0,
            item_id: "e2:ep2".to_owned(),
        }]
    );
}

// `a` on the hovered row queues the episode.
#[test]
fn a_on_the_hovered_row_queues_the_episode() {
    let content = podcasts_content(PillSelection::Latest, &episodes(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_podcasts(state, content.clone());

        route_row(state, facts, "a", "a", "e1:ep1");
    });
    assert_eq!(facts, ["a=Handled"]);
    assert_eq!(
        commands(&rx),
        [mbv::GuiLibraryCommand::PodcastsQueue {
            lib_idx: 0,
            item_id: "e1:ep1".to_owned(),
        }]
    );
}

// A podcast episode has no play-next (row 10.1): `n` is consumed and sends
// nothing, so it neither queues nor reaches the core's playback chords.
#[test]
fn n_on_the_hovered_row_sends_nothing_and_is_handled() {
    let content = podcasts_content(PillSelection::Latest, &episodes(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_podcasts(state, content.clone());

        route_row(state, facts, "n", "n", "e1:ep1");
    });
    assert_eq!(facts, ["n=Handled"]);
    assert_eq!(commands(&rx), []);
}

// Enter, `n`, and `a` with the pointer over no episode row do nothing (design
// D2, step 3): the key is the window's, and it sends no command.
#[test]
fn enter_n_and_a_with_no_hovered_row_send_nothing() {
    let content = podcasts_content(PillSelection::Latest, &episodes(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_podcasts(state, content.clone());

        route(state, facts, "enter", "Enter", false);
        route(state, facts, "n", "n", false);
        route(state, facts, "a", "a", false);
    });
    assert_eq!(facts, ["enter=Handled", "n=Handled", "a=Handled"]);
    assert_eq!(commands(&rx), []);
}

// Down over the list scrolls it by the height of the row at the top edge
// (design D6) and plays nothing.
#[test]
fn down_over_the_podcasts_list_scrolls_it_by_one_row() {
    let content = podcasts_content(PillSelection::Latest, &episodes(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_podcasts(state, content.clone());
        state.measure_podcasts_list(40.0);

        route_row(state, facts, "down", "ArrowDown", "e1:ep1");
        facts.lock().expect("facts").push(scroll_facts(state));
    });
    assert_eq!(facts[0], "down=Handled");
    // The first row is 48 px, and the 96 px of rows clamp the step at the
    // list's end.
    assert_eq!(facts[1], "top=48 viewport=40");
    assert_eq!(commands(&rx), []);
}

// The movement keys are the core's while the pointer rests off the list
// (design D2, step 4): nothing scrolls.
#[test]
fn down_off_the_podcasts_list_is_the_cores_key() {
    let content = podcasts_content(PillSelection::Latest, &episodes(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_podcasts(state, content.clone());
        state.measure_podcasts_list(40.0);

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
fn slash_opens_the_podcasts_filter_and_sends_no_key() {
    let content = podcasts_content(PillSelection::Latest, &episodes(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_podcasts(state, content.clone());

        route(state, facts, "slash", "/", false);
        facts.lock().expect("facts").push(ui_facts(state));
    });
    assert_eq!(facts[0], "slash=Handled");
    assert!(facts[1].contains("filter_open=true"));
    assert_eq!(commands(&rx), []);
}

// Esc closes the open filter (design D3, step 2): its text goes with it,
// and the close runs the Podcasts list's own resolve, not another
// destination's.
#[test]
fn esc_closes_the_podcasts_filter_and_drops_its_text() {
    let content = podcasts_content(PillSelection::Latest, &episodes(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_podcasts(state, content.clone());
        let mut podcasts_ui = state.podcasts_ui;
        podcasts_ui.with_mut(|ui| ui.filter = "e1".to_owned());
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
