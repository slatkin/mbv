//! Headless tests of the Search field (D16): the contracts of `search.test.ts`
//! — one debounced query per settled typing over the injected clock, the
//! emptied field's clear at once, the close's cancel and command, and the
//! highlight's id movement. The harness builds a `DioxusDocument` whose root
//! creates the store over a recording clock and core link, so no test opens a
//! window or sleeps, and no IPC runs.

use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use blitz_dom::Document as _;
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use crate::bridge::frame::library::{GuiSearchFrame, GuiSearchRowFrame};
use crate::core_thread::CoreInput;
use crate::window::channel::CoreLink;
use crate::window::clock::{Clock, RecordingClock};
use crate::window::library::search::initial_library_search;
use crate::window::library::state::LibraryState;
use crate::window::sections::Sections;

/// The facts one scenario recorded, in its run order.
type Facts = Arc<Mutex<Vec<String>>>;

/// One scenario the harness runs against the built store and the test's clock.
type Scenario = std::rc::Rc<dyn Fn(&LibraryState, &RecordingClock, &Facts)>;

/// The root: creates the store over the recording clock once and runs the
/// scenario in one effect.
fn harness(
    (tx, facts, clock, scenario): (
        mpsc::Sender<CoreInput>,
        Facts,
        Arc<RecordingClock>,
        Scenario,
    ),
) -> Element {
    let state = use_hook(|| {
        // The playhead position the Search state reads: none — the tests hold
        // it at 0.
        let playhead_position = Memo::new(|| 0.0);
        LibraryState::new(
            Sections::new(),
            CoreLink::new(tx),
            playhead_position,
            Arc::clone(&clock) as Arc<dyn Clock>,
        )
    });
    use_effect(move || {
        scenario(&state, &clock, &facts);
    });
    rsx! {
        div { "search-harness" }
    }
}

/// Run one scenario against a fresh store and clock, and return the facts it
/// recorded, the commands the core link received, and the clock. Bounded by
/// construction: the effect runs once and the poll loop stops when the
/// document goes idle.
fn run(
    scenario: impl Fn(&LibraryState, &RecordingClock, &Facts) + 'static,
) -> (Vec<String>, mpsc::Receiver<CoreInput>, Arc<RecordingClock>) {
    let (tx, rx) = mpsc::channel::<CoreInput>();
    let facts: Facts = Arc::new(Mutex::new(Vec::new()));
    let clock = Arc::new(RecordingClock::new());
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(
            harness,
            (
                tx,
                Arc::clone(&facts),
                Arc::clone(&clock),
                std::rc::Rc::new(
                    move |state: &LibraryState, clock: &RecordingClock, facts: &Facts| {
                        scenario(state, clock, facts);
                    },
                ),
            ),
        ),
        DocumentConfig::default(),
    );
    doc.initial_build();
    poll_until_idle(&mut doc);
    let recorded = facts.lock().expect("facts").clone();
    (recorded, rx, clock)
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

/// A deadline far past every timer the tests arm, so [`Clock::fire_due`]
/// fires them all.
fn fired_by() -> Instant {
    Instant::now() + Duration::from_secs(3_600)
}

/// Fire every timer the clock holds past its deadline, as the timer thread
/// would, and route each passed timer's id to the state that armed it. The
/// fired count is the replace fact's proof: a cancelled timer never fires.
fn fire(state: &LibraryState, clock: &RecordingClock, facts: &Facts) {
    let fired = clock.fire_due(fired_by());
    note(facts, &format!("fired={}", fired.len()));
    for timer in fired {
        state.on_search_timer(timer);
    }
}

/// One fact the scenario recorded, in its run order.
fn note(facts: &Facts, fact: &str) {
    facts.lock().expect("facts").push(fact.to_owned());
}

/// One found row, as the frame carries it.
fn row(id: &str) -> GuiSearchRowFrame {
    GuiSearchRowFrame {
        id: id.to_owned(),
        item_type: "Artist".to_owned(),
        title: format!("Title {id}"),
        subtitle: "Artist".to_owned(),
    }
}

/// A results frame whose rows are fresh values on every call: two frames
/// built from the same ids share no row, so an id, not an identity, is all
/// the movement can match on.
fn results(ids: &[&str]) -> GuiSearchFrame {
    GuiSearchFrame {
        query: "q".to_owned(),
        loading: false,
        failed: false,
        results: ids.iter().map(|id| row(id)).collect(),
    }
}

/// The settled typing sends one debounced query, and a fresh keystroke
/// replaces the pending send, never stacks one: two keystrokes arm one send —
/// still one pending timer after the second — which answers the last query
/// and leaves no timer behind.
#[test]
fn the_settled_typing_sends_one_debounced_query() {
    let (facts, rx, clock) = run(|state, clock, facts| {
        state.set_search_query("ba".to_owned());
        note(facts, &format!("armed={}", clock.pending()));
        state.set_search_query("bat".to_owned());
        fire(state, clock, facts);
    });
    assert_eq!(facts, ["armed=1", "fired=1"]);
    assert_eq!(
        commands(&rx),
        [crate::gui_core::GuiLibraryCommand::SearchQuery {
            query: "bat".to_owned()
        }]
    );
    assert_eq!(clock.pending(), 0);
}

/// An emptied field clears the results at once, with no debounce: the empty
/// query sends now and arms no timer.
#[test]
fn an_emptied_field_clears_the_results_at_once_with_no_debounce() {
    let (facts, rx, clock) = run(|state, clock, facts| {
        state.set_search_query(String::new());
        note(facts, &format!("armed={}", clock.pending()));
    });
    assert_eq!(facts, ["armed=0"]);
    assert_eq!(
        commands(&rx),
        [crate::gui_core::GuiLibraryCommand::SearchQuery {
            query: String::new()
        }]
    );
    assert_eq!(clock.pending(), 0);
}

/// Esc closes the field, clears it, and sends the close command: the state
/// resets to the mounted one, behind the settled query's own send.
#[test]
fn esc_closes_the_field_clears_it_and_sends_the_close_command() {
    let (facts, rx, _clock) = run(|state, clock, facts| {
        state.focus_search_field();
        state.set_search_query("bat".to_owned());
        fire(state, clock, facts);
        state.close_library_search();
        note(facts, &format!("search={:?}", state.search.peek().clone()));
    });
    assert_eq!(
        commands(&rx),
        [
            crate::gui_core::GuiLibraryCommand::SearchQuery {
                query: "bat".to_owned()
            },
            crate::gui_core::GuiLibraryCommand::SearchClose,
        ]
    );
    assert_eq!(facts[1], format!("search={:?}", initial_library_search()));
}

/// A close cancels the pending send with it: the armed timer the close
/// cancelled never fires, and the close command is the only one the close
/// sends.
#[test]
fn a_close_cancels_the_pending_send_with_it() {
    let (facts, rx, _clock) = run(|state, clock, facts| {
        state.set_search_query("bat".to_owned());
        state.close_library_search();
        fire(state, clock, facts);
    });
    assert_eq!(facts, ["fired=0"]);
    assert_eq!(
        commands(&rx),
        [crate::gui_core::GuiLibraryCommand::SearchClose]
    );
}

/// The arrows move the highlight along held row ids.
#[test]
fn the_arrows_move_the_highlight_along_held_row_ids() {
    let (facts, _rx, _clock) = run(|state, _clock, facts| {
        let search = results(&["r1", "r2", "r3"]);
        state.move_search_highlight(1, Some(&search));
        note(
            facts,
            &format!("highlight={:?}", state.search.peek().highlight),
        );
        state.move_search_highlight(1, Some(&search));
        note(
            facts,
            &format!("highlight={:?}", state.search.peek().highlight),
        );
    });
    assert_eq!(facts, ["highlight=Some(\"r1\")", "highlight=Some(\"r2\")"]);
}

/// The highlight moves by id through a new frame whose rows share no object:
/// a fresh frame's rows are fresh values, so the movement finds the held id's
/// place by id and keeps walking from there.
#[test]
fn the_highlight_moves_by_id_through_a_new_frame() {
    let (facts, _rx, _clock) = run(|state, _clock, facts| {
        state.move_search_highlight(1, Some(&results(&["r1", "r2"])));
        state.move_search_highlight(1, Some(&results(&["r1", "r2"])));
        note(
            facts,
            &format!("highlight={:?}", state.search.peek().highlight),
        );
    });
    assert_eq!(facts, ["highlight=Some(\"r2\")"]);
}

/// An arrow with no results moves nothing and holds no id.
#[test]
fn an_arrow_with_no_results_moves_nothing_and_holds_no_id() {
    let (facts, _rx, _clock) = run(|state, _clock, facts| {
        state.move_search_highlight(1, Some(&results(&[])));
        note(
            facts,
            &format!("highlight={:?}", state.search.peek().highlight),
        );
    });
    assert_eq!(facts, ["highlight=None"]);
}

/// Activating a row sends its id and item type, never the close command.
#[test]
fn activating_a_row_sends_its_id_and_item_type_never_the_close_command() {
    let (_facts, rx, _clock) = run(|state, _clock, _facts| {
        state.activate_search_row("r7", "Artist");
    });
    assert_eq!(
        commands(&rx),
        [crate::gui_core::GuiLibraryCommand::SearchActivate {
            id: "r7".to_owned(),
            item_type: "Artist".to_owned(),
        }]
    );
}

/// A field blur retracts the keyboard without closing anything: the focus
/// fact goes, and the mount fact, the query text, and the highlight stay.
#[test]
fn a_field_blur_retracts_the_keyboard_without_closing_anything() {
    let (facts, rx, _clock) = run(|state, _clock, facts| {
        state.mount_search_field();
        state.focus_search_field();
        state.set_search_query("bat".to_owned());
        state.set_search_highlight(Some("r1".to_owned()));
        state.blur_search_field();
        let held = state.search.peek();
        note(
            facts,
            &format!(
                "focused={} mounted={} query={:?} highlight={:?}",
                held.focused, held.mounted, held.query, held.highlight
            ),
        );
    });
    assert_eq!(
        facts,
        ["focused=false mounted=true query=\"bat\" highlight=Some(\"r1\")"]
    );
    assert_eq!(commands(&rx), []);
}
