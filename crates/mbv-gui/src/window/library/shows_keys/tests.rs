//! Headless tests of the Shows keys (D16): the contracts of
//! `shows-keys.test.ts`, on the row the pointer marks
//! (`gui-pointer-target-interaction` D2) — the movement keys scroll the list
//! under the pointer, Enter selects a series row, `n` and `a` act on a
//! Workspace episode only, and `/` is taken. The router under test is the
//! panel's one routing site, [`super::super::keys::route_library_key`], so these
//! assertions cover the Shows dispatch it hands to
//! [`super::route_shows_panel_key`]. The harness builds a `DioxusDocument`
//! whose root creates the store and runs each scenario in one effect over a
//! recording core link, so no test opens a window or sleeps, and no IPC runs.

use std::sync::{Arc, Mutex, mpsc};

use blitz_dom::Document as _;
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use crate::bridge::frame::library::{
    GuiArtShapeFrame, GuiDestinationList, GuiHomeRowStateFrame, GuiLibraryDetail, GuiLibraryList,
    GuiLibraryTabFrame, GuiRowArtFrame, GuiRowMediaKindFrame, GuiShowsDetail,
    GuiShowsEpisodeRowFrame, GuiShowsHeroFrame, GuiShowsList, GuiShowsSeasonFrame,
    GuiShowsSeasonPillFrame, GuiShowsSeriesFrame, GuiShowsWorkspaceFrame,
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
fn commands(rx: &mpsc::Receiver<CoreInput>) -> Vec<crate::gui_core::GuiLibraryCommand> {
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

/// Route one keydown with the pointer over the Shows list, resting on the row
/// `row` (design D1, D2), against the window's own section stores.
fn route_on_row(state: &LibraryState, facts: &Facts, name: &str, value: &str, row: &str) {
    let pointer = PointerTarget {
        zone: Some(PointerZone::Library),
        list: Some(PointerList::Shows),
        row: Some(row.to_owned()),
    };
    let outcome = super::super::keys::route_library_key(
        &key(value, false),
        Some(&state.sections),
        state,
        None,
        GuiArrangement::Wide,
        &pointer,
    );
    facts
        .lock()
        .expect("facts")
        .push(format!("{name}={outcome:?}"));
}

/// Route one keydown on the Hero's Workspace, resting on the episode `row`
/// (design D1, D2).
fn route_on_workspace(state: &LibraryState, facts: &Facts, name: &str, value: &str, row: &str) {
    let pointer = PointerTarget {
        zone: Some(PointerZone::Library),
        list: Some(PointerList::Workspace),
        row: Some(row.to_owned()),
    };
    let outcome = super::super::keys::route_library_key(
        &key(value, false),
        Some(&state.sections),
        state,
        None,
        GuiArrangement::Wide,
        &pointer,
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

/// The Shows list's scroll facts the assertions read.
fn scroll_facts(state: &LibraryState) -> String {
    let scroll = *state.scrolls.shows.peek();
    format!(
        "top={} viewport={}",
        scroll.scroll_top, scroll.viewport_height
    )
}

/// The Workspace episode rows the frame carries, with a season selected and
/// its episodes.
fn workspace_episodes() -> Vec<GuiShowsEpisodeRowFrame> {
    vec![
        episode("ep1", "The Big Bang"),
        episode("ep2", "Cat in the Bag"),
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
            art: GuiRowArtFrame::NoSource,
            media: GuiRowMediaKindFrame::Video,
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
            art: GuiRowArtFrame::NoSource,
            media: GuiRowMediaKindFrame::Video,
            id: "s2".to_owned(),
            title: "The Bear".to_owned(),
            sort_letter: 'B',
            expandable: true,
            seasons: Vec::new(),
        },
    ]
}

/// The carried content while Shows is active (design D12, D8): no Music and
/// no Home content, the series list, and the Hero.
fn shows_content(
    series: Vec<GuiShowsSeriesFrame>,
    hero: Option<GuiShowsHeroFrame>,
) -> (GuiLibraryList, GuiLibraryDetail) {
    let selected = series.first().map(|node| node.id.clone());
    let list = GuiLibraryList {
        refreshing: false,
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
            sort: None,
            view: crate::gui_core::GuiShowsView::All,
            lib_idx: 0,
            loading: false,
            series,
            upcoming_failed: false,
            rows: Vec::new(),
        }),
    };
    let detail = GuiLibraryDetail::Shows(GuiShowsDetail {
        workspace_loading: false,
        selected,
        hero,
    });
    (list, detail)
}

/// Carry the Shows content and start the list's interaction state over.
fn carry_shows(state: &LibraryState, content: (GuiLibraryList, GuiLibraryDetail)) {
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
    let mut shows_ui = state.shows_ui;
    shows_ui.with_mut(|ui| ui.filter.clear());
}

/// The filter facts the assertions read.
fn ui_facts(state: &LibraryState) -> String {
    let ui = state.ui.peek().clone();
    let shows = state.shows_ui.peek().clone();
    format!(
        "filter_open={} shows_filter={:?}",
        ui.filter_open, shows.filter,
    )
}

// A series Hero whose Workspace carries a season and its episodes: the rows the
// Workspace keys act on (design D2).
fn series_hero() -> GuiShowsHeroFrame {
    GuiShowsHeroFrame {
        title: "Better Call Saul".to_owned(),
        parent: None,
        meta_rows: Vec::new(),
        duration_row: None,
        progress_row: None,
        overview: None,
        art_shape: GuiArtShapeFrame::Landscape,
        art: None,
        workspace: Some(GuiShowsWorkspaceFrame {
            seasons: vec![
                GuiShowsSeasonPillFrame {
                    season_id: "s1".to_owned(),
                    name: "Season 1".to_owned(),
                    selected: true,
                },
                GuiShowsSeasonPillFrame {
                    season_id: "s2".to_owned(),
                    name: "Season 2".to_owned(),
                    selected: false,
                },
            ],
            selected_season: Some("s1".to_owned()),
            episodes: workspace_episodes(),
        }),
    }
}

// Enter on a series row selects it for the Hero, and opens the node.
#[test]
fn enter_on_a_series_row_selects_it_for_the_hero() {
    let content = shows_content(shows_tree(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_shows(state, content.clone());

        route_on_row(state, facts, "enter", "Enter", "shows-series:s1");
    });
    assert_eq!(facts, ["enter=Handled"]);
    assert_eq!(
        commands(&rx),
        [crate::gui_core::GuiLibraryCommand::ShowsSelect {
            lib_idx: 0,
            row: "s1".to_owned(),
        }]
    );
}

// A series row is not playable (row 7.1): its `a` sends nothing, so the key
// never acts on a row the core refuses. `n` is not a Library key either.
#[test]
fn a_on_a_series_row_sends_nothing() {
    let content = shows_content(shows_tree(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_shows(state, content.clone());

        route_on_row(state, facts, "n", "n", "shows-series:s1");
        route_on_row(state, facts, "a", "a", "shows-series:s1");
    });
    assert_eq!(facts, ["n=Core(None)", "a=Handled"]);
    assert_eq!(commands(&rx), []);
}

// Down over the Shows list scrolls the list by the height of the row at the top
// edge (design D6) and changes no selection.
#[test]
fn down_over_the_shows_list_scrolls_it_by_one_row() {
    let content = shows_content(shows_tree(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_shows(state, content.clone());
        state.measure_shows_list(40.0);

        route_on_row(state, facts, "down", "ArrowDown", "shows-series:s1");
        facts.lock().expect("facts").push(scroll_facts(state));
    });
    assert_eq!(facts[0], "down=Handled");
    // The letter heading covers the list's top edge, so the step is its 24 px.
    assert_eq!(facts[1], "top=24 viewport=40");
    assert_eq!(commands(&rx), []);
}

// The movement keys over the Hero's Workspace are consumed and move no row
// (design D2, D6): the Workspace keeps no cursor.
#[test]
fn down_over_the_workspace_is_consumed_and_moves_nothing() {
    let content = shows_content(shows_tree(), Some(series_hero()));
    let (facts, rx) = run(move |state, facts| {
        carry_shows(state, content.clone());
        state.measure_shows_list(40.0);

        route_on_workspace(state, facts, "down", "ArrowDown", "ep2");
        facts.lock().expect("facts").push(scroll_facts(state));
    });
    assert_eq!(facts[0], "down=Handled");
    assert_eq!(facts[1], "top=0 viewport=40");
    assert_eq!(commands(&rx), []);
}

// Enter and `a` on the Workspace act on the episode the pointer marks
// (design D2): the Hero names no cursor, so the marker is the row. `n` is not a
// Library key, so it goes to the core.
#[test]
fn enter_n_and_a_on_the_workspace_name_the_hovered_episode() {
    let content = shows_content(shows_tree(), Some(series_hero()));
    let (facts, rx) = run(move |state, facts| {
        carry_shows(state, content.clone());

        route_on_workspace(state, facts, "enter", "Enter", "ep2");
        route_on_workspace(state, facts, "n", "n", "ep2");
        route_on_workspace(state, facts, "a", "a", "ep2");
    });
    assert_eq!(facts, ["enter=Handled", "n=Core(None)", "a=Handled"]);
    assert_eq!(
        commands(&rx),
        [
            crate::gui_core::GuiLibraryCommand::ShowsPlay {
                lib_idx: 0,
                item_id: "ep2".to_owned(),
            },
            crate::gui_core::GuiLibraryCommand::ShowsQueue {
                lib_idx: 0,
                item_id: "ep2".to_owned(),
            },
        ]
    );
}

// A marker that names no painted episode sends nothing.
#[test]
fn enter_on_the_workspace_with_no_hovered_episode_sends_nothing() {
    let content = shows_content(shows_tree(), Some(series_hero()));
    let (facts, rx) = run(move |state, facts| {
        carry_shows(state, content.clone());

        route_on_workspace(state, facts, "enter", "Enter", "gone");
    });
    assert_eq!(facts, ["enter=Handled"]);
    assert_eq!(commands(&rx), []);
}

// `/` opens the list's filter under Library focus (design D3, D7): the
// rail's keyboard path, and the Queue filter never sees the key.
#[test]
fn slash_opens_the_shows_filter_and_sends_no_key() {
    let content = shows_content(shows_tree(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_shows(state, content.clone());

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
    let content = shows_content(shows_tree(), None);
    let (facts, rx) = run(move |state, facts| {
        carry_shows(state, content.clone());
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
