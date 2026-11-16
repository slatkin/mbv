//! Headless tests of the Shows keys (D16): the contracts of
//! `shows-keys.test.ts` — the list's cursor moves, Enter selects a series row
//! and plays a flat row, `n` and `a` act on a flat row only, the pill row
//! walks its modes, `/` is taken, and the pane walk skips the Hero the
//! Upcoming mode does not paint. The router under test is the panel's one
//! routing site, [`super::super::keys::route_library_key`], so these
//! assertions cover the Shows dispatch it hands to
//! [`super::route_shows_panel_key`]. The harness builds a `DioxusDocument`
//! whose root creates the store and runs each scenario in one effect over a
//! recording core link, so no test opens a window or sleeps, and no IPC runs.

use std::sync::{Arc, Mutex, mpsc};

use blitz_dom::Document as _;
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use crate::bridge::frame::focus::GuiPanel;
use crate::bridge::frame::library::GuiShowsModeFrame as Mode;
use crate::bridge::frame::library::{
    GuiArtShapeFrame, GuiDestinationList, GuiHomeRowStateFrame, GuiLibraryDetail, GuiLibraryList,
    GuiLibraryTabFrame, GuiMetaKindFrame, GuiMetaSegmentFrame, GuiShowsDetail,
    GuiShowsEpisodeRowFrame, GuiShowsHeroFrame, GuiShowsList, GuiShowsPillFrame, GuiShowsRowFrame,
    GuiShowsSeasonFrame, GuiShowsSeasonPillFrame, GuiShowsSeriesFrame, GuiShowsWorkspaceFrame,
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
        div { "shows-keys-harness" }
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

/// The series rows the All series mode's list moves through, as the flat rows
/// the frame carries beside the tree.
fn series_rows() -> Vec<GuiShowsRowFrame> {
    vec![
        GuiShowsRowFrame {
            id: "s1".to_owned(),
            title: "Atlanta".to_owned(),
            subtitle: None,
            date: None,
            sort_letter: Some('A'),
        },
        GuiShowsRowFrame {
            id: "s2".to_owned(),
            title: "The Bear".to_owned(),
            subtitle: None,
            date: None,
            sort_letter: Some('B'),
        },
    ]
}

/// One flat newest-episode row the Latest mode paints.
fn flat_row(id: &str, title: &str, date: &str) -> GuiShowsRowFrame {
    GuiShowsRowFrame {
        id: id.to_owned(),
        title: title.to_owned(),
        subtitle: Some("S3E4 · Sabrosito".to_owned()),
        date: Some(date.to_owned()),
        sort_letter: None,
    }
}

/// The flat newest-episode rows the Latest mode paints.
fn episode_rows() -> Vec<GuiShowsRowFrame> {
    vec![
        flat_row("e1", "Atlanta", "12 May"),
        flat_row("e2", "The Bear", "9 May"),
    ]
}

/// One episode row of a season, as the frame carries it.
fn episode(id: &str, name: &str) -> GuiShowsEpisodeRowFrame {
    GuiShowsEpisodeRowFrame {
        episode_id: id.to_owned(),
        number: 1,
        name: name.to_owned(),
        duration: "24:00".to_owned(),
        state: GuiHomeRowStateFrame::Ordinary,
    }
}

/// The All series mode's tree (design D14): one series with a season whose
/// episodes have landed, and one with none.
fn shows_tree() -> Vec<GuiShowsSeriesFrame> {
    vec![
        GuiShowsSeriesFrame {
            id: "s1".to_owned(),
            title: "Atlanta".to_owned(),
            sort_letter: 'A',
            expandable: true,
            seasons: vec![GuiShowsSeasonFrame {
                id: "se1".to_owned(),
                name: "Season 1".to_owned(),
                episodes: Some(vec![episode("ep1", "The Big Bang")]),
            }],
        },
        GuiShowsSeriesFrame {
            id: "s2".to_owned(),
            title: "The Bear".to_owned(),
            sort_letter: 'B',
            expandable: true,
            seasons: Vec::new(),
        },
    ]
}

/// The three view modes' pills, with `mode` selected (design D12).
fn shows_pills(mode: Mode) -> Vec<GuiShowsPillFrame> {
    [
        (Mode::Latest, "Latest"),
        (Mode::Upcoming, "Upcoming"),
        (Mode::All, "All series"),
    ]
    .into_iter()
    .map(|(pill_mode, name)| GuiShowsPillFrame {
        mode: pill_mode,
        name: name.to_owned(),
        selected: pill_mode == mode,
        collection: false,
    })
    .collect()
}

/// The All series mode's Hero: a series whose Workspace the Hero pane walks.
fn shows_hero() -> GuiShowsHeroFrame {
    GuiShowsHeroFrame {
        title: "Atlanta".to_owned(),
        parent: None,
        meta_rows: vec![vec![GuiMetaSegmentFrame {
            text: "4 seasons".to_owned(),
            kind: GuiMetaKindFrame::Count,
        }]],
        duration_row: None,
        progress_row: None,
        overview: None,
        art_shape: GuiArtShapeFrame::Landscape,
        art: None,
        workspace: Some(GuiShowsWorkspaceFrame {
            seasons: vec![GuiShowsSeasonPillFrame {
                season_id: "se1".to_owned(),
                name: "Season 1".to_owned(),
                selected: true,
            }],
            selected_season: Some("se1".to_owned()),
            episodes: vec![
                episode("ep1", "The Big Bang"),
                episode("ep2", "Streets on Fire"),
            ],
        }),
    }
}

/// The carried content while Shows is active (design D12): no Music and no
/// Home content, the mode's rows, and the Hero the mode paints.
fn shows_content(
    mode: Mode,
    rows: Vec<GuiShowsRowFrame>,
    hero: Option<GuiShowsHeroFrame>,
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
                label: "Shows".to_owned(),
            },
        ],
        current_tab: Some(2),
        destination: GuiDestinationList::Shows(GuiShowsList {
            lib_idx: 0,
            pills: shows_pills(mode),
            collection_names: vec!["All series".to_owned()],
            loading: false,
            series: if mode == Mode::All {
                shows_tree()
            } else {
                Vec::new()
            },
            upcoming_failed: false,
            rows,
        }),
    };
    let detail = GuiLibraryDetail::Shows(GuiShowsDetail {
        workspace_loading: false,
        selected,
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

/// Carry the Shows content and focus the pane the key acts on.
fn carry_shows(
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
    carry_panel_focus(&sections);
    let mut ui = state.ui;
    ui.with_mut(|ui| ui.pane = Some(pane));
    let mut shows_ui = state.shows_ui;
    shows_ui.with_mut(|ui| {
        ui.cursor = cursor.map(str::to_owned);
        ui.focused_pill = None;
        ui.pill_select = None;
        ui.filter.clear();
    });
}

/// The pane focus and filter facts the assertions read.
fn ui_facts(state: &LibraryState) -> String {
    let ui = state.ui.peek().clone();
    let shows = state.shows_ui.peek().clone();
    format!(
        "pane={:?} filter_open={} shows_filter={:?} focused_pill={:?} pill_select={:?} cursor={:?}",
        ui.pane, ui.filter_open, shows.filter, shows.focused_pill, shows.pill_select, shows.cursor,
    )
}

// Enter on a series row selects it for the Hero.
#[test]
fn enter_on_a_series_row_selects_it_for_the_hero() {
    let content = shows_content(Mode::All, series_rows(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_shows(
            state,
            content.clone(),
            LibraryPane::List,
            Some("shows-series:s1"),
        );

        route(state, facts, "enter", "Enter", false);
    });
    assert_eq!(facts, ["enter=Handled"]);
    assert_eq!(
        commands(&rx),
        [mbv::GuiLibraryCommand::ShowsSelect {
            lib_idx: 0,
            row: "s1".to_owned(),
        }]
    );
}

// Enter on a flat row plays the episode.
#[test]
fn enter_on_a_flat_row_plays_the_episode() {
    let content = shows_content(Mode::Latest, episode_rows(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_shows(state, content.clone(), LibraryPane::List, Some("e2"));

        route(state, facts, "enter", "Enter", false);
    });
    assert_eq!(facts, ["enter=Handled"]);
    assert_eq!(
        commands(&rx),
        [mbv::GuiLibraryCommand::ShowsPlay {
            lib_idx: 0,
            item_id: "e2".to_owned(),
        }]
    );
}

// A series row is not playable (row 7.1): its `n` and `a` send nothing, so
// the keys never act on a row the core refuses.
#[test]
fn n_and_a_on_a_series_row_send_nothing() {
    let content = shows_content(Mode::All, series_rows(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_shows(
            state,
            content.clone(),
            LibraryPane::List,
            Some("shows-series:s1"),
        );

        route(state, facts, "n", "n", false);
        route(state, facts, "a", "a", false);
    });
    assert_eq!(facts, ["n=Handled", "a=Handled"]);
    assert_eq!(commands(&rx), []);
}

// `n` and `a` on a flat row play the episode next and queue it.
#[test]
fn n_and_a_on_a_flat_row_play_the_episode_next_and_queue_it() {
    let content = shows_content(Mode::Latest, episode_rows(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_shows(state, content.clone(), LibraryPane::List, Some("e1"));

        route(state, facts, "n", "n", false);
        route(state, facts, "a", "a", false);
    });
    assert_eq!(facts, ["n=Handled", "a=Handled"]);
    assert_eq!(
        commands(&rx),
        [
            mbv::GuiLibraryCommand::ShowsPlayNext {
                lib_idx: 0,
                item_id: "e1".to_owned(),
            },
            mbv::GuiLibraryCommand::ShowsQueue {
                lib_idx: 0,
                item_id: "e1".to_owned(),
            },
        ]
    );
}

// A movement key moves the Shows cursor and sends no key.
#[test]
fn a_movement_key_moves_the_shows_cursor_and_sends_no_key() {
    let content = shows_content(Mode::Latest, episode_rows(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_shows(state, content.clone(), LibraryPane::List, Some("e1"));

        route(state, facts, "down", "ArrowDown", false);
        facts.lock().expect("facts").push(ui_facts(state));
    });
    assert_eq!(facts[0], "down=Handled");
    assert!(facts[1].contains("cursor=Some(\"e2\")"));
    assert_eq!(commands(&rx), []);
}

// Right on the Shows pill row focuses the first mode pill and sends no key.
#[test]
fn right_on_the_shows_pill_row_focuses_the_first_mode_pill_and_sends_no_key() {
    let content = shows_content(Mode::Latest, episode_rows(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_shows(state, content.clone(), LibraryPane::Pills, None);

        route(state, facts, "right", "ArrowRight", false);
        facts.lock().expect("facts").push(ui_facts(state));
    });
    assert_eq!(facts[0], "right=Handled");
    assert!(facts[1].contains("focused_pill=Some(Latest)"));
    assert_eq!(commands(&rx), []);
}

// Space on a focused Shows pill asks for its mode and sends no key.
#[test]
fn space_on_a_focused_shows_pill_asks_for_its_mode_and_sends_no_key() {
    let content = shows_content(Mode::Latest, episode_rows(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_shows(state, content.clone(), LibraryPane::Pills, None);
        let mut shows_ui = state.shows_ui;
        shows_ui.with_mut(|ui| ui.focused_pill = Some(Mode::Upcoming));

        route(state, facts, "space", " ", false);
        facts.lock().expect("facts").push(ui_facts(state));
    });
    assert_eq!(facts[0], "space=Handled");
    assert!(facts[1].contains("pill_select=Some(Upcoming)"));
    assert_eq!(commands(&rx), []);
}

// `/` opens the list's filter under Library focus (design D3, D7): the
// rail's keyboard path, and the Queue filter never sees the key.
#[test]
fn slash_opens_the_shows_filter_and_sends_no_key() {
    let content = shows_content(Mode::Latest, episode_rows(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_shows(state, content.clone(), LibraryPane::List, Some("e1"));

        route(state, facts, "slash", "/", false);
        facts.lock().expect("facts").push(ui_facts(state));
    });
    assert_eq!(facts[0], "slash=Handled");
    assert!(facts[1].contains("filter_open=true"));
    assert_eq!(commands(&rx), []);
}

// Esc closes the open filter (design D3, step 2): its text goes with it,
// and the close runs the Shows list's own resolve, not another
// destination's.
#[test]
fn esc_closes_the_shows_filter_and_drops_its_text() {
    let content = shows_content(Mode::Latest, episode_rows(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_shows(state, content.clone(), LibraryPane::List, Some("e1"));
        let mut shows_ui = state.shows_ui;
        shows_ui.with_mut(|ui| ui.filter = "e1".to_owned());
        let mut ui = state.ui;
        ui.with_mut(|ui| ui.filter_open = true);

        route(state, facts, "esc", "Escape", false);
        facts.lock().expect("facts").push(ui_facts(state));
    });
    assert_eq!(facts[0], "esc=Handled");
    assert!(facts[1].contains("filter_open=false"));
    assert!(facts[1].contains("shows_filter=\"\""));
    assert_eq!(commands(&rx), []);
}

// The pane walk skips the Hero the Upcoming mode does not paint (design
// D12): the list is the walk's last pane there, so Tab past it takes Queue
// Panel focus.
#[test]
fn tab_past_the_upcoming_list_takes_queue_panel_focus() {
    let content = shows_content(Mode::Upcoming, episode_rows(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_shows(state, content.clone(), LibraryPane::List, None);

        route(state, facts, "tab", "Tab", false);
    });
    assert_eq!(facts, ["tab=Handled"]);
    assert_eq!(commands(&rx), [mbv::GuiLibraryCommand::TakeQueueFocus]);
}

// Tab walks from the All series list into the series Hero.
#[test]
fn tab_walks_from_the_all_series_list_into_the_series_hero() {
    let content = shows_content(Mode::All, series_rows(), Some(shows_hero()));
    let (facts, rx) = run(move |state, facts| {
        carry_shows(state, content.clone(), LibraryPane::List, None);

        route(state, facts, "tab", "Tab", false);
        facts.lock().expect("facts").push(ui_facts(state));
    });
    assert_eq!(facts[0], "tab=Handled");
    assert!(facts[1].contains("pane=Some(Hero)"));
    assert_eq!(commands(&rx), []);
}

// Tab from the Queue panel focuses the Shows pill row.
#[test]
fn tab_from_the_queue_panel_focuses_the_shows_pill_row() {
    let content = shows_content(Mode::Latest, episode_rows(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_shows(state, content.clone(), LibraryPane::List, None);
        let mut focus = state.sections.focus;
        focus.write().panel = GuiPanel::Queue;

        route(state, facts, "tab", "Tab", false);
        facts.lock().expect("facts").push(ui_facts(state));
    });
    assert_eq!(facts[0], "tab=Handled");
    assert!(facts[1].contains("pane=Some(Pills)"));
    assert_eq!(commands(&rx), [mbv::GuiLibraryCommand::TakeFocus]);
}

// Review of row 7.1, finding P1: the Shows panes keep the Library's keys
// from the core's router (design D3, step 4). A pane that does not act on a
// key still consumes it, so `a` and `n` never reach the core's playback
// chords, which are not gated on Panel focus.
#[test]
fn a_movement_key_on_the_pills_pane_does_not_reach_the_core() {
    let content = shows_content(Mode::Latest, episode_rows(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_shows(state, content.clone(), LibraryPane::Pills, None);

        route(state, facts, "down", "ArrowDown", false);
    });
    assert_eq!(facts, ["down=Handled"]);
    assert_eq!(commands(&rx), []);
}

// `a` on the pills pane queues the list cursor's flat row and sends no key.
#[test]
fn a_on_the_pills_pane_queues_the_list_cursor_flat_row_and_sends_no_key() {
    let content = shows_content(Mode::Latest, episode_rows(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_shows(state, content.clone(), LibraryPane::Pills, Some("e1"));

        route(state, facts, "a", "a", false);
    });
    assert_eq!(facts, ["a=Handled"]);
    assert_eq!(
        commands(&rx),
        [mbv::GuiLibraryCommand::ShowsQueue {
            lib_idx: 0,
            item_id: "e1".to_owned(),
        }]
    );
}

// `n` on the hero-less Hero pane never reaches the core's playback chords
// (design D3, step 4): the pane does not act on the key, but the Library's
// remaining keys still take it, so `n` plays the list cursor's flat row next.
#[test]
fn n_on_the_hero_less_hero_pane_does_not_reach_the_core() {
    let content = shows_content(Mode::Upcoming, episode_rows(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_shows(state, content.clone(), LibraryPane::Hero, Some("e2"));

        route(state, facts, "n", "n", false);
    });
    assert_eq!(facts, ["n=Handled"]);
    assert_eq!(
        commands(&rx),
        [mbv::GuiLibraryCommand::ShowsPlayNext {
            lib_idx: 0,
            item_id: "e2".to_owned(),
        }]
    );
}
