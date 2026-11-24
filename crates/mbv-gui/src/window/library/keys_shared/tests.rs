//! The harness the four lettered keys suites share (`gui-tech-debt-simplification`
//! design D10, task 4.7): each suite builds the store, runs its scenario in one
//! effect over a recording core link, routes a keydown, and reads the facts it
//! records. The scenarios and their carried content stay in each destination's
//! suite.
//!
//! The harness builds a `DioxusDocument` whose root creates the store and runs
//! each scenario in one effect over a recording core link, so no test opens a
//! window or sleeps, and no IPC runs.

use std::sync::{Arc, Mutex, mpsc};

use blitz_dom::Document as _;
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use crate::bridge::keys::{DomKey, DomModifiers};
use crate::core_thread::CoreInput;
use crate::window::arrangement::GuiArrangement;
use crate::window::channel::CoreLink;
use crate::window::clock::{Clock, RecordingClock};
use crate::window::library::list_state::ListUiState;
use crate::window::library::scroll::ListScroll;
use crate::window::library::state::LibraryState;
use crate::window::pointer_target::{PointerList, PointerTarget, PointerZone};
use crate::window::sections::Sections;

/// The facts one scenario recorded, in its run order.
pub(crate) type Facts = Arc<Mutex<Vec<String>>>;

/// One scenario the harness runs against the built store.
pub(crate) type Scenario = std::rc::Rc<dyn Fn(&LibraryState, &Facts)>;

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
        div { "library-keys-harness" }
    }
}

/// Run one scenario against a fresh store and return the facts it recorded
/// and the commands the core link received. Bounded by construction: the
/// effect runs once and the poll loop stops when the document goes idle.
pub(crate) fn run(
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
pub(crate) fn commands(rx: &mpsc::Receiver<CoreInput>) -> Vec<crate::gui_core::GuiLibraryCommand> {
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

/// Route one keydown with the pointer over the Library column and paint the
/// outcome.
pub(crate) fn route(state: &LibraryState, facts: &Facts, name: &str, value: &str, shift: bool) {
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

/// Route one keydown with the pointer over the destination's list, resting on
/// `row` (design D1, D2).
pub(crate) fn route_row(
    state: &LibraryState,
    facts: &Facts,
    name: &str,
    value: &str,
    list: PointerList,
    row: &str,
) {
    route_pointer(state, facts, name, value, &pointer_on(list, Some(row)));
}

/// Route one keydown with the pointer over the Hero's Workspace, resting on
/// `row` when one is named.
pub(crate) fn route_hero(
    state: &LibraryState,
    facts: &Facts,
    name: &str,
    value: &str,
    row: Option<&str>,
) {
    route_pointer(
        state,
        facts,
        name,
        value,
        &pointer_on(PointerList::Workspace, row),
    );
}

/// The list's scroll facts the assertions read.
pub(crate) fn scroll_facts(scroll: Signal<ListScroll>) -> String {
    let scroll = *scroll.peek();
    format!(
        "top={} viewport={}",
        scroll.scroll_top, scroll.viewport_height
    )
}

/// The filter facts the assertions read.
pub(crate) fn ui_facts(state: &LibraryState, ui: Signal<ListUiState>) -> String {
    let ui_state = state.ui.peek().clone();
    let destination = ui.peek().clone();
    format!(
        "filter_open={} filter={:?}",
        ui_state.filter_open, destination.filter,
    )
}
