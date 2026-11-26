//! Headless tests of the Shows Hero's Workspace (D16): the contracts of
//! `shows-workspace.test.ts` — the pointer's episode resolving against the
//! frame, the season step that sends the select-season command and stops at the
//! pills' ends, and the episode the Hero's play and queue commands name. The
//! harness builds a `DioxusDocument` whose root creates the store and runs each
//! scenario in one effect over a recording core link, so no test opens a window
//! or sleeps, and no IPC runs.

use std::sync::{Arc, Mutex, mpsc};

use blitz_dom::Document as _;
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use crate::bridge::frame::library::{
    GuiArtShapeFrame, GuiHomeRowStateFrame, GuiShowsDetail, GuiShowsEpisodeRowFrame,
    GuiShowsHeroFrame, GuiShowsSeasonPillFrame, GuiShowsWorkspaceFrame,
};
use crate::core_thread::CoreInput;
use crate::window::channel::CoreLink;
use crate::window::clock::{Clock, RecordingClock};
use crate::window::sections::Sections;

use super::{play_shows_episode, queue_shows_episode, shows_episode_at, step_shows_season};
use crate::window::library::state::LibraryState;

/// The facts one scenario recorded, in its run order.
type Facts = Arc<Mutex<Vec<String>>>;

/// One scenario the harness runs against the built store.
type Scenario = std::rc::Rc<dyn Fn(&LibraryState, &Facts)>;

/// The root: creates the store once and runs the scenario in one effect.
fn harness((tx, facts, scenario): (mpsc::Sender<CoreInput>, Facts, Scenario)) -> Element {
    let state = use_hook(|| {
        // The playhead position the Audiobooks Workspace reads; the Shows
        // Workspace reads none of it. The tests hold it at 0.
        let playhead_position = Memo::new(|| 0.0);
        let clock: Arc<dyn Clock> = Arc::new(RecordingClock::new());
        LibraryState::new(Sections::new(), CoreLink::new(tx), playhead_position, clock)
    });
    use_effect(move || {
        scenario(&state, &facts);
    });
    rsx! {
        div { "shows-workspace-harness" }
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

/// One episode row, as the frame carries it.
fn episode(id: &str, name: &str, duration: &str) -> GuiShowsEpisodeRowFrame {
    GuiShowsEpisodeRowFrame {
        episode_id: id.to_owned(),
        number: 1,
        name: name.to_owned(),
        duration: duration.to_owned(),
        state: GuiHomeRowStateFrame::Ordinary,
    }
}

/// A season's episodes.
fn episodes() -> Vec<GuiShowsEpisodeRowFrame> {
    vec![
        episode("ep1", "Pilot", "58:00"),
        episode("ep2", "Cat in the Bag", "47:00"),
    ]
}

/// The series Hero's Workspace over `episodes`, with `selected_season`
/// pressed.
fn workspace(
    episodes: Vec<GuiShowsEpisodeRowFrame>,
    selected_season: &str,
) -> GuiShowsWorkspaceFrame {
    GuiShowsWorkspaceFrame {
        seasons: vec![
            GuiShowsSeasonPillFrame {
                season_id: "s1".to_owned(),
                name: "Season 1".to_owned(),
                selected: selected_season == "s1",
            },
            GuiShowsSeasonPillFrame {
                season_id: "s2".to_owned(),
                name: "Season 2".to_owned(),
                selected: selected_season == "s2",
            },
        ],
        selected_season: Some(selected_season.to_owned()),
        episodes,
    }
}

/// The Shows library index the episode commands name.
const LIB_IDX: usize = 2;

/// The selected series' detail part, with a series Hero.
fn shows_detail(episodes: Vec<GuiShowsEpisodeRowFrame>, selected_season: &str) -> GuiShowsDetail {
    let hero = GuiShowsHeroFrame {
        title: "Better Call Saul".to_owned(),
        parent: None,
        facts: crate::bridge::frame::library::GuiHeroFactsFrame {
            pills: Vec::new(),
            rows: Vec::new(),
        },
        overview: None,
        art_shape: GuiArtShapeFrame::Landscape,
        art: None,
        workspace: Some(workspace(episodes, selected_season)),
    };
    GuiShowsDetail {
        playing_episode_id: None,
        workspace_loading: false,
        selected: Some("series1".to_owned()),
        hero: Some(hero),
    }
}

// The pointer's marker resolves against the frame's episodes (design D1): a
// painted episode's id names it, and any other id names none.
#[test]
fn the_pointer_marker_resolves_against_the_painted_episodes() {
    let detail = shows_detail(episodes(), "s1");

    assert_eq!(
        shows_episode_at(&detail, "ep2").map(|episode| episode.episode_id.as_str()),
        Some("ep2")
    );
    assert!(shows_episode_at(&detail, "gone").is_none());
}

// A season step sends the neighbouring season.
#[test]
fn a_season_step_sends_the_neighbouring_season() {
    let (_facts, rx) = run(|state, _facts| {
        step_shows_season(state, 1, LIB_IDX, &shows_detail(episodes(), "s1"));
    });

    assert_eq!(
        commands(&rx),
        [crate::gui_core::GuiLibraryCommand::ShowsSelectSeason {
            lib_idx: 2,
            season_id: "s2".to_owned(),
        }]
    );
}

// A season step past the end sends nothing.
#[test]
fn a_season_step_past_the_end_sends_nothing() {
    let (_facts, rx) = run(|state, _facts| {
        step_shows_season(state, -1, LIB_IDX, &shows_detail(episodes(), "s1"));
        step_shows_season(state, 1, LIB_IDX, &shows_detail(episodes(), "s2"));
    });

    assert_eq!(commands(&rx), []);
}

// The play command names the episode it is given.
#[test]
fn the_play_command_names_the_episode_it_is_given() {
    let (_facts, rx) = run(|state, _facts| {
        play_shows_episode(state, LIB_IDX, "ep2");
    });

    assert_eq!(
        commands(&rx),
        [crate::gui_core::GuiLibraryCommand::ShowsPlay {
            lib_idx: 2,
            item_id: "ep2".to_owned(),
        }]
    );
}

// The queue command names the episode it is given.
#[test]
fn the_queue_command_names_the_episode_it_is_given() {
    let (_facts, rx) = run(|state, _facts| {
        queue_shows_episode(state, LIB_IDX, "ep1");
    });

    assert_eq!(
        commands(&rx),
        [crate::gui_core::GuiLibraryCommand::ShowsQueue {
            lib_idx: 2,
            item_id: "ep1".to_owned(),
        }]
    );
}
