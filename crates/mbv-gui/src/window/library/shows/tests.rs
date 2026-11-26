//! Headless tests of the Shows interaction state (D16, gui-list-header-buttons
//! design D4): Expand all opens the painted series rows and no season, and
//! Collapse all closes every open series and season. The harness builds a
//! `DioxusDocument` whose root creates the store and runs the scenario in one
//! effect, so no test opens a window or sleeps, and the carried content is mock
//! data: no IPC runs.

use std::sync::{Arc, Mutex, mpsc};

use blitz_dom::Document as _;
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use crate::bridge::frame::library::{
    GuiDestinationList, GuiLibraryList, GuiRowArtFrame, GuiRowMediaKindFrame, GuiShowsList,
    GuiShowsSeasonFrame, GuiShowsSeriesFrame,
};
use crate::bridge::sections::{GuiSection, GuiSectionBatch};
use crate::core_thread::CoreInput;
use crate::window::channel::CoreLink;
use crate::window::clock::{Clock, RecordingClock};
use crate::window::library::shows_list::{ShowsListRow, season_row_id, series_row_id};
use crate::window::library::state::LibraryState;
use crate::window::sections::Sections;

/// The facts one scenario recorded, in its run order.
type Facts = Arc<Mutex<Vec<String>>>;

/// One scenario the harness runs against the built store.
type Scenario = std::rc::Rc<dyn Fn(&LibraryState, &Facts)>;

/// The root: creates the store once and runs the scenario in one effect.
fn harness((tx, facts, scenario): (mpsc::Sender<CoreInput>, Facts, Scenario)) -> Element {
    let state = use_hook(|| {
        // The playhead position the Audiobooks Workspace reads: the tests hold
        // it at 0.
        let playhead_position = Memo::new(|| 0.0);
        let clock: Arc<dyn Clock> = Arc::new(RecordingClock::new());
        LibraryState::new(Sections::new(), CoreLink::new(tx), playhead_position, clock)
    });
    use_effect(move || {
        scenario(&state, &facts);
    });
    rsx! {
        div { "shows-harness" }
    }
}

/// Run one scenario against a fresh store and return the facts it recorded and
/// the Library commands it sent. Bounded by construction: the effect runs once
/// and the poll loop stops when the document goes idle.
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

/// The Shows list section value: the given series under the `Shows` tab.
fn shows_list(series: Vec<GuiShowsSeriesFrame>) -> GuiLibraryList {
    GuiLibraryList {
        refreshing: false,
        tabs: Vec::new(),
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
    }
}

/// One series node under the `A` heading, with its seasons.
fn series(id: &str, title: &str, seasons: Vec<GuiShowsSeasonFrame>) -> GuiShowsSeriesFrame {
    GuiShowsSeriesFrame {
        art: GuiRowArtFrame::NoSource,
        media: GuiRowMediaKindFrame::Video,
        id: id.to_owned(),
        title: title.to_owned(),
        sort_letter: 'A',
        expandable: true,
        seasons,
    }
}

/// One season node whose episodes have not landed.
fn season(id: &str, name: &str) -> GuiShowsSeasonFrame {
    GuiShowsSeasonFrame {
        id: id.to_owned(),
        name: name.to_owned(),
        episodes: None,
    }
}

/// The series fixture: one series holding two seasons, then one holding none.
fn tree() -> Vec<GuiShowsSeriesFrame> {
    vec![
        series(
            "s1",
            "Atlanta",
            vec![season("se1", "Season 1"), season("se2", "Season 2")],
        ),
        series("s2", "Better Call Saul", Vec::new()),
    ]
}

/// Carry the given Shows content into the store's sections.
fn carry(state: &LibraryState, list: GuiLibraryList) {
    let batch = GuiSectionBatch {
        library_list: Some(GuiSection {
            revision: 1,
            value: Some(list),
        }),
        ..GuiSectionBatch::default()
    };
    let mut sections = state.sections.clone();
    sections.apply_batch(&batch);
}

/// The open mark of every painted series and season row, in paint order, as
/// `id=true` or `id=false`: the Shows tree's state in one fact.
fn node_marks(state: &LibraryState) -> String {
    let marks: Vec<String> = state
        .shows_rows()
        .iter()
        .filter_map(|row| match row {
            ShowsListRow::Series(series) => Some(format!("{}={}", series.id, series.open)),
            ShowsListRow::Season(season) => Some(format!("{}={}", season.id, season.open)),
            ShowsListRow::Heading(_) | ShowsListRow::Episode(_) | ShowsListRow::Flat(_) => None,
        })
        .collect();
    marks.join(" ")
}

/// gui-list-header-buttons design D4, spec "Expand all opens every top-level
/// parent row the list shows": Expand all opens every painted series row, opens
/// no season, and sends no command.
#[test]
fn expand_all_shows_opens_every_painted_series_and_no_season() {
    let expected = [
        format!("{}=true", series_row_id("s1")),
        format!("{}=false", season_row_id("s1", "se1")),
        format!("{}=false", season_row_id("s1", "se2")),
        format!("{}=true", series_row_id("s2")),
    ]
    .join(" ");
    let list = shows_list(tree());
    let (facts, rx) = run(move |state, facts| {
        carry(state, list.clone());
        state.expand_all_shows();
        facts.lock().expect("facts").push(node_marks(state));
    });
    assert_eq!(facts, [expected]);
    assert_eq!(commands(&rx), []);
}

/// gui-list-header-buttons design D4, spec "Collapse all closes every open
/// parent row": Collapse all closes the open series and season, so only the
/// series rows paint and no node stays open.
#[test]
fn collapse_all_shows_closes_every_open_series_and_season() {
    let expected = [
        format!("{}=false", series_row_id("s1")),
        format!("{}=false", series_row_id("s2")),
    ]
    .join(" ");
    let list = shows_list(tree());
    let (facts, _rx) = run(move |state, facts| {
        carry(state, list.clone());
        let mut shows_ui = state.shows_ui;
        shows_ui.with_mut(|ui| {
            ui.open.push(series_row_id("s1"));
            ui.open.push(season_row_id("s1", "se1"));
        });
        state.collapse_all_shows();
        let (open_empty, closed_empty) = {
            let ui = state.shows_ui.peek();
            (ui.open.is_empty(), ui.closed.is_empty())
        };
        facts.lock().expect("facts").push(format!(
            "{} open_empty={open_empty} closed_empty={closed_empty}",
            node_marks(state)
        ));
    });
    assert_eq!(
        facts,
        [format!("{expected} open_empty=true closed_empty=true")]
    );
}
