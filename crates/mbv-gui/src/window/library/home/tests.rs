//! Headless tests of the Home list's interaction state (D16): the contracts
//! of `home.test.ts` — a press selects the row it rests on through the select
//! command with the row's opaque identity, the keys scroll the list, and the
//! scroll and measure reports keep the offset inside the list's range. The
//! harness
//! builds a `DioxusDocument` whose root creates the store and runs the
//! scenario in one effect over a recording core link, so no test opens a
//! window or sleeps, and the carried content is mock data: no IPC runs.

use std::sync::{Arc, Mutex, mpsc};

use blitz_dom::Document as _;
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use mbv_queue::QueueItemContentId;

use crate::bridge::frame::library::{
    GuiHomeItemRowFrame, GuiHomeList, GuiHomeRowFrame, GuiHomeRowIdFrame, GuiHomeRowKindFrame,
    GuiHomeRowStateFrame, GuiRowArtFrame, GuiRowMediaKindFrame,
};

use crate::bridge::sections::GuiSectionBatch;
use crate::core_thread::CoreInput;
use crate::window::channel::CoreLink;
use crate::window::clock::{Clock, RecordingClock};
use crate::window::library::home_list::{HOME_ITEM_ROW_HEIGHT, build_home_rows};
use crate::window::library::list::{GeometryRow, ListMove};
use crate::window::library::state::LibraryState;
use crate::window::sections::Sections;

/// The facts one scenario recorded, in its run order.
type Facts = Arc<Mutex<Vec<String>>>;

/// One scenario the harness runs against the built store.
type Scenario = std::rc::Rc<dyn Fn(&LibraryState, &Facts)>;

/// The root: creates the store once and runs the scenario in one effect.
fn harness((tx, facts, scenario): (mpsc::Sender<CoreInput>, Facts, Scenario)) -> Element {
    let state = use_hook(|| {
        // The playhead position the Audiobooks Workspace reads: the tests
        // hold it at 0, which marks no chapter but feeds the resolution.
        let playhead_position = Memo::new(|| 0.0);
        let clock: Arc<dyn Clock> = Arc::new(RecordingClock::new());
        LibraryState::new(Sections::new(), CoreLink::new(tx), playhead_position, clock)
    });
    use_effect(move || {
        scenario(&state, &facts);
    });
    rsx! {
        div { "home-harness" }
    }
}

/// Run one scenario against a fresh store and return the facts it recorded
/// and the Library commands it sent. Bounded by construction: the effect runs
/// once and the poll loop stops when the document goes idle.
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

/// The `Continue watching` row the fixture carries.
fn continued() -> GuiHomeItemRowFrame {
    GuiHomeItemRowFrame {
        id: GuiHomeRowIdFrame::Continue("e1".to_owned()),
        title: "Better Call Saul".to_owned(),
        subtitle: None,
        trailing: None,
        kind: GuiHomeRowKindFrame::Media,
        state: GuiHomeRowStateFrame::Ordinary,
        art: GuiRowArtFrame::NoSource,
        media: GuiRowMediaKindFrame::Video,
    }
}

/// The `Recently added` row the cursor steps to.
fn added() -> GuiHomeItemRowFrame {
    GuiHomeItemRowFrame {
        id: GuiHomeRowIdFrame::Recent(QueueItemContentId::Feed("f1".to_owned())),
        title: "A Feed Entry".to_owned(),
        subtitle: None,
        trailing: Some("12 May".to_owned()),
        kind: GuiHomeRowKindFrame::Media,
        state: GuiHomeRowStateFrame::Ordinary,
        art: GuiRowArtFrame::NoSource,
        media: GuiRowMediaKindFrame::Video,
    }
}

/// The carried content while Home is active (design D12): the two sections'
/// rows, and the selection the frame paints.
fn carry_home(state: &LibraryState) {
    let list = crate::bridge::frame::library::GuiLibraryList {
        refreshing: false,
        tabs: Vec::new(),
        current_tab: None,
        destination: crate::bridge::frame::library::GuiDestinationList::Home(GuiHomeList {
            loading: false,
            rows: vec![
                GuiHomeRowFrame::Heading {
                    text: "Continue watching".to_owned(),
                },
                GuiHomeRowFrame::Item(continued()),
                GuiHomeRowFrame::Spacer,
                GuiHomeRowFrame::Heading {
                    text: "Recently added".to_owned(),
                },
                GuiHomeRowFrame::Item(added()),
            ],
        }),
    };
    let detail = crate::bridge::frame::library::GuiLibraryDetail::Home(
        crate::bridge::frame::library::GuiHomeDetail {
            selected: Some(GuiHomeRowIdFrame::Continue("e1".to_owned())),
            hero: None,
        },
    );
    let batch = GuiSectionBatch {
        library_list: Some(crate::bridge::sections::GuiSection {
            revision: 1,
            value: Some(list),
        }),
        library_detail: Some(crate::bridge::sections::GuiSection {
            revision: 1,
            value: Some(detail),
        }),
        ..GuiSectionBatch::default()
    };
    let mut sections = state.sections.clone();
    sections.apply_batch(&batch);
}

/// The Home interaction state's facts, as the assertions read them.
fn home_facts(state: &LibraryState) -> String {
    let scroll = *state.scrolls.home.peek();
    format!("top={}", scroll.scroll_top)
}

// The movement keys scroll the list by one row (design D6): the step is the
// height of the row at the top edge, and no row is selected.
#[test]
fn down_scrolls_the_list_by_the_height_of_the_top_row() {
    let (facts, rx) = run(|state, facts| {
        carry_home(state);
        // The viewport leaves the list room to scroll, so the step is the top
        // row's height and not the range's clamp.
        state.measure_home_list(HOME_ITEM_ROW_HEIGHT * 2.0);

        state.step_home_scroll(ListMove::Down);

        facts.lock().expect("facts").push(home_facts(state));
    });
    assert_eq!(facts, ["top=28"]);
    assert_eq!(commands(&rx), []);
}

// End scrolls to the list's last screenful (design D6), inside the range.
#[test]
fn end_scrolls_to_the_lists_last_screenful() {
    let (facts, rx) = run(|state, facts| {
        carry_home(state);
        state.measure_home_list(HOME_ITEM_ROW_HEIGHT * 3.0);

        state.step_home_scroll(ListMove::End);

        facts.lock().expect("facts").push(home_facts(state));
    });
    let painted = build_home_rows(
        &[
            GuiHomeRowFrame::Heading {
                text: "Continue watching".to_owned(),
            },
            GuiHomeRowFrame::Item(continued()),
            GuiHomeRowFrame::Spacer,
            GuiHomeRowFrame::Heading {
                text: "Recently added".to_owned(),
            },
            GuiHomeRowFrame::Item(added()),
        ],
        "",
    );
    let bottom = painted
        .iter()
        .map(|row| GeometryRow::top(row) + GeometryRow::height(row))
        .fold(0.0_f32, f32::max);
    assert_eq!(
        facts,
        [format!("top={}", bottom - HOME_ITEM_ROW_HEIGHT * 3.0)]
    );
    assert_eq!(commands(&rx), []);
}

// The press is the selection's half only (design D12): it selects, and Enter
// or the double-click is the play's. A heading or spacer names no item, so
// its press changes nothing.
#[test]
fn a_press_on_a_heading_selects_nothing() {
    let (facts, rx) = run(|state, facts| {
        carry_home(state);
        let ui = state.home_ui.peek().clone();
        let rows = state.home_rows(&ui.filter);
        let heading = rows.first().expect("a heading opens the list").clone();

        state.press_home_row(&heading);

        facts.lock().expect("facts").push(home_facts(state));
    });
    assert_eq!(facts, ["top=0"]);
    assert_eq!(commands(&rx), []);
}
