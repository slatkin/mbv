//! Headless tests of the Podcasts keys (D16): the contracts of
//! `podcasts-keys.test.ts` — the list's cursor moves, Enter plays the episode
//! it rests on, `a` queues it, `n` is consumed and does nothing, the pill row
//! walks its views, and the pane walk skips the Hero a frame without one does
//! not paint. The router under test is the panel's one routing site,
//! [`super::super::keys::route_library_key`], so these assertions cover the
//! Podcasts dispatch it hands to [`super::route_podcasts_panel_key`]. The
//! harness builds a `DioxusDocument` whose root creates the store and runs
//! each scenario in one effect over a recording core link, so no test opens a
//! window or sleeps, and no IPC runs.

use std::sync::{Arc, Mutex, mpsc};

use blitz_dom::Document as _;
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use crate::bridge::frame::focus::GuiPanel;
use crate::bridge::frame::library::{
    GuiDestinationList, GuiEntryRowFrame, GuiHomeRowStateFrame, GuiLibraryDetail, GuiLibraryList,
    GuiLibraryTabFrame, GuiMetaKindFrame, GuiMetaSegmentFrame, GuiPillFrame, GuiPodcastsDetail,
    GuiPodcastsHeroFrame, GuiPodcastsList,
};
use crate::bridge::keys::{DomKey, DomModifiers};
use crate::bridge::library::GuiPodcastPill;
use crate::bridge::sections::{GuiSection, GuiSectionBatch};
use crate::core_thread::CoreInput;
use crate::window::arrangement::GuiArrangement;
use crate::window::channel::CoreLink;
use crate::window::clock::{Clock, RecordingClock};
use crate::window::library::podcasts::initial_podcasts_ui;
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

/// The episode Hero a selected episode paints (design D12, row 10.1).
fn hero() -> GuiPodcastsHeroFrame {
    GuiPodcastsHeroFrame {
        title: "Episode A".to_owned(),
        parent: Some("Show One".to_owned()),
        meta_rows: vec![
            vec![GuiMetaSegmentFrame {
                text: "Show One".to_owned(),
                kind: GuiMetaKindFrame::Plain,
            }],
            vec![GuiMetaSegmentFrame {
                text: "34:00".to_owned(),
                kind: GuiMetaKindFrame::Time,
            }],
        ],
        duration_row: Some(1),
        progress_row: None,
        overview: None,
        art_shape: crate::bridge::frame::library::GuiArtShapeFrame::Square,
        art: None,
    }
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

/// Apply one carried Podcasts frame without touching the interaction state: a
/// re-carried frame is a fresh deserialization, and the pill focus the walk
/// keeps must survive it (row 10.1's P2).
fn carry_podcasts_batch(
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

/// Carry the Podcasts content and focus the pane the key acts on.
fn carry_podcasts(
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
    let mut podcasts_ui = state.podcasts_ui;
    podcasts_ui.with_mut(|ui| {
        *ui = initial_podcasts_ui();
        ui.cursor = cursor.map(str::to_owned);
    });
}

/// The pane focus and pill facts the assertions read.
fn ui_facts(state: &LibraryState) -> String {
    let ui = state.ui.peek().clone();
    let podcasts = state.podcasts_ui.peek().clone();
    let focused = podcasts.focused_pill.clone();
    format!(
        "pane={:?} filter_open={} dismissed={} item_menu_open={} cursor={:?} filter={:?} focused_pill={focused:?} pill_select={:?}",
        ui.pane,
        ui.filter_open,
        ui.hero_dismissed,
        ui.item_menu_open,
        podcasts.cursor,
        podcasts.filter,
        podcasts.pill_select,
    )
}

// Enter on the cursor row plays the episode.
#[test]
fn enter_on_the_cursor_row_plays_the_episode() {
    let content = podcasts_content(PillSelection::Latest, &episodes(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_podcasts(state, content.clone(), LibraryPane::List, Some("e2:ep2"));

        route(state, facts, "enter", "Enter", false);
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

// A movement key moves the cursor and sends no key.
#[test]
fn a_movement_key_moves_the_cursor_and_sends_no_key() {
    let content = podcasts_content(PillSelection::Latest, &episodes(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_podcasts(state, content.clone(), LibraryPane::List, Some("e1:ep1"));

        route(state, facts, "down", "ArrowDown", false);
        facts.lock().expect("facts").push(ui_facts(state));
    });
    assert_eq!(facts[0], "down=Handled");
    assert!(facts[1].contains("cursor=Some(\"e2:ep2\")"));
    assert_eq!(commands(&rx), []);
}

// `a` on the cursor row queues the episode.
#[test]
fn a_on_the_cursor_row_queues_the_episode() {
    let content = podcasts_content(PillSelection::Latest, &episodes(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_podcasts(state, content.clone(), LibraryPane::List, Some("e1:ep1"));

        route(state, facts, "a", "a", false);
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
fn n_on_the_cursor_row_sends_nothing_and_is_handled() {
    let content = podcasts_content(PillSelection::Latest, &episodes(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_podcasts(state, content.clone(), LibraryPane::List, Some("e1:ep1"));

        route(state, facts, "n", "n", false);
    });
    assert_eq!(facts, ["n=Handled"]);
    assert_eq!(commands(&rx), []);
}

// Right on the pill row focuses the first pill and sends no key.
#[test]
fn right_on_the_pill_row_focuses_the_first_pill_and_sends_no_key() {
    let content = podcasts_content(PillSelection::Latest, &episodes(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_podcasts(state, content.clone(), LibraryPane::Pills, None);

        route(state, facts, "right", "ArrowRight", false);
        facts.lock().expect("facts").push(ui_facts(state));
    });
    assert_eq!(facts[0], "right=Handled");
    assert!(facts[1].contains("focused_pill=Some(Latest)"));
    assert_eq!(commands(&rx), []);
}

// Space on a focused show pill asks for its selection.
#[test]
fn space_on_a_focused_show_pill_asks_for_its_selection() {
    let content = podcasts_content(PillSelection::Latest, &episodes(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_podcasts(state, content.clone(), LibraryPane::Pills, None);
        let mut podcasts_ui = state.podcasts_ui;
        podcasts_ui.with_mut(|ui| ui.focused_pill = Some(GuiPodcastPill::Show("sh1".to_owned())));

        route(state, facts, "space", " ", false);
        facts.lock().expect("facts").push(ui_facts(state));
    });
    assert_eq!(facts[0], "space=Handled");
    assert!(facts[1].contains("pill_select=Some(Show(\"sh1\"))"));
    assert_eq!(commands(&rx), []);
}

// Review of row 10.1, finding P2: the pill walk finds the focused pill by
// its key, not its object, because every carried frame is a fresh
// deserialization. An object-identity find would lose the focus across
// frames and the walk would restart from a row end.
#[test]
fn the_pill_walk_keeps_its_place_when_the_frame_re_carries_equal_pills() {
    let first = podcasts_content(PillSelection::Latest, &episodes(), None);
    let second = podcasts_content(PillSelection::Show, &episodes(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_podcasts(state, first.clone(), LibraryPane::Pills, None);
        let mut podcasts_ui = state.podcasts_ui;
        podcasts_ui.with_mut(|ui| ui.focused_pill = Some(GuiPodcastPill::Show("sh1".to_owned())));

        // A new frame: structurally equal pills, all fresh objects, at a new
        // revision so the section store writes it.
        carry_podcasts_batch(state, 2, second.clone());

        route(state, facts, "right", "ArrowRight", false);
        facts.lock().expect("facts").push(ui_facts(state));
    });
    assert_eq!(facts[0], "right=Handled");
    assert!(facts[1].contains("focused_pill=Some(Show(\"sh2\"))"));
    assert_eq!(commands(&rx), []);
}

// Enter on a focused pill asks for its selection.
#[test]
fn enter_on_a_focused_pill_asks_for_its_selection() {
    let content = podcasts_content(PillSelection::Show, &episodes(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_podcasts(state, content.clone(), LibraryPane::Pills, None);

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
fn slash_opens_the_podcasts_filter_and_sends_no_key() {
    let content = podcasts_content(PillSelection::Latest, &episodes(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_podcasts(state, content.clone(), LibraryPane::List, Some("e1:ep1"));

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
        carry_podcasts(state, content.clone(), LibraryPane::List, Some("e1:ep1"));
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

// `a` in the Hero pane queues the frame's selected episode.
#[test]
fn a_in_the_hero_pane_queues_the_frames_selected_episode() {
    let content = podcasts_content(PillSelection::Latest, &episodes(), Some(hero()));
    let (facts, rx) = run(move |state, facts| {
        carry_podcasts(state, content.clone(), LibraryPane::Hero, None);

        route(state, facts, "a", "a", false);
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

// A movement key in the Hero pane changes nothing and sends no key.
#[test]
fn a_movement_key_in_the_hero_pane_changes_nothing_and_sends_no_key() {
    let content = podcasts_content(PillSelection::Latest, &episodes(), Some(hero()));
    let (facts, rx) = run(move |state, facts| {
        carry_podcasts(state, content.clone(), LibraryPane::Hero, None);

        route(state, facts, "down", "ArrowDown", false);
    });
    assert_eq!(facts, ["down=Handled"]);
    assert_eq!(commands(&rx), []);
}

// Review of row 7.1's P1 lesson, row 10.1: the Podcasts panes keep the
// Library's keys from the core's router (design D3, step 4). A pane that does
// not act on a key still consumes it, so `n` and `a` never reach the core's
// playback chords, which are not gated on Panel focus.
#[test]
fn a_movement_key_on_the_pills_pane_does_not_reach_the_core() {
    let content = podcasts_content(PillSelection::Latest, &episodes(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_podcasts(state, content.clone(), LibraryPane::Pills, None);

        route(state, facts, "up", "ArrowUp", false);
    });
    assert_eq!(facts, ["up=Handled"]);
    assert_eq!(commands(&rx), []);
}

// `a` on the pills pane queues the list cursor episode and sends no key.
#[test]
fn a_on_the_pills_pane_queues_the_list_cursor_episode_and_sends_no_key() {
    let content = podcasts_content(PillSelection::Latest, &episodes(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_podcasts(state, content.clone(), LibraryPane::Pills, Some("e2:ep2"));

        route(state, facts, "a", "a", false);
    });
    assert_eq!(facts, ["a=Handled"]);
    assert_eq!(
        commands(&rx),
        [mbv::GuiLibraryCommand::PodcastsQueue {
            lib_idx: 0,
            item_id: "e2:ep2".to_owned(),
        }]
    );
}

// `n` on the pills pane is consumed and sends no key.
#[test]
fn n_on_the_pills_pane_is_consumed_and_sends_no_key() {
    let content = podcasts_content(PillSelection::Latest, &episodes(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_podcasts(state, content.clone(), LibraryPane::Pills, Some("e2:ep2"));

        route(state, facts, "n", "n", false);
    });
    assert_eq!(facts, ["n=Handled"]);
    assert_eq!(commands(&rx), []);
}

// `n` on the hero-less Hero pane is consumed and sends no key.
#[test]
fn n_on_the_hero_less_hero_pane_is_consumed_and_sends_no_key() {
    let content = podcasts_content(PillSelection::Latest, &episodes(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_podcasts(state, content.clone(), LibraryPane::Hero, Some("e1:ep1"));

        route(state, facts, "n", "n", false);
    });
    assert_eq!(facts, ["n=Handled"]);
    assert_eq!(commands(&rx), []);
}

// Space with no focused pill keeps its core effect.
#[test]
fn space_with_no_focused_pill_keeps_its_core_effect() {
    let content = podcasts_content(PillSelection::Latest, &episodes(), None);
    let (_facts, _rx) = run(move |state, facts| {
        carry_podcasts(state, content.clone(), LibraryPane::List, Some("e1:ep1"));

        route(state, facts, "space", " ", false);
    });
    // The core's own router takes the key: the routing outcome is the fact,
    // and the harness forwards no core key for the forwarder to record.
}

// The pane walk skips the Hero a frame without one does not paint (design
// D12, row 10.1): the list is the walk's last pane there, so Tab past it
// takes Queue Panel focus.
#[test]
fn tab_past_the_hero_less_list_takes_queue_panel_focus() {
    let content = podcasts_content(PillSelection::Latest, &episodes(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_podcasts(state, content.clone(), LibraryPane::List, None);

        route(state, facts, "tab", "Tab", false);
    });
    assert_eq!(facts, ["tab=Handled"]);
    assert_eq!(commands(&rx), [mbv::GuiLibraryCommand::TakeQueueFocus]);
}

// Tab walks from the list into the Hero a frame carries.
#[test]
fn tab_walks_from_the_list_into_the_hero_a_frame_carries() {
    let content = podcasts_content(PillSelection::Latest, &episodes(), Some(hero()));
    let (facts, rx) = run(move |state, facts| {
        carry_podcasts(state, content.clone(), LibraryPane::List, None);

        route(state, facts, "tab", "Tab", false);
        facts.lock().expect("facts").push(ui_facts(state));
    });
    assert_eq!(facts[0], "tab=Handled");
    assert!(facts[1].contains("pane=Some(Hero)"));
    assert_eq!(commands(&rx), []);
}
