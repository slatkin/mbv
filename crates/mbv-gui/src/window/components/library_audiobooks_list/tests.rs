//! Headless tests of the Audiobooks book list (D16): the contracts of
//! `LibraryAudiobooksList.test.ts` — the catalog paints its letter headings
//! with the A–Z rail a press scrolls with, a rail letter no row carries is
//! disabled, a press selects the book the Hero shows while a double-click plays
//! it from its saved position, the core's selected book paints its fill, and the
//! load and failure facts paint their lines. The IPC of the earlier tests is the
//! mock core link here; a headless document cannot fire a DOM click, so the
//! press, play, and jump contracts ride the state methods the component's own
//! handlers call one line each. The core channel is a mock pair; no test opens a
//! window, sleeps, or reaches the core.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, mpsc};

use blitz_dom::{Document as _, LocalName};
use blitz_traits::shell::{ColorScheme, Viewport};
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use super::LibraryAudiobooksList;
use crate::bridge::frame::library::{
    GuiAudiobooksDetail, GuiAudiobooksList, GuiAudiobooksRowFrame, GuiDestinationList,
    GuiLibraryDetail, GuiLibraryList, GuiRowArtFrame, GuiRowMediaKindFrame,
};
use crate::core_thread::CoreInput;
use crate::window::channel::CoreLink;
use crate::window::clock::{Clock, RecordingClock};
use crate::window::layout::LayoutEpoch;
use crate::window::library::state::LibraryState;
use crate::window::sections::SectionRef;
use crate::window::sections::Sections;

/// The context handles the root hands the test: the Library store and the
/// layout epoch the re-measures read.
type FixtureHandles = (LibraryState, LayoutEpoch);

/// The layout rules the fixture resolves with, in place of the app
/// stylesheet a real window loads (the same rules the panel's harness uses),
/// and the fixed 120 px scroller box the list's first measurement reports.
const FIXTURE_CSS: &str = concat!(
    ".flex { display: flex; }",
    ".flex-col { flex-direction: column; }",
    ".items-center { align-items: center; }",
    ".items-end { align-items: end; }",
    ".justify-center { justify-content: center; }",
    ".min-w-0 { min-width: 0; }",
    ".min-h-0 { min-height: 0; }",
    ".flex-1 { flex: 1 1 0%; }",
    ".flex-none { flex: none; }",
    ".w-full { width: 100%; }",
    ".block { display: block; }",
    ".relative { position: relative; }",
    ".overflow-hidden { overflow: hidden; }",
    ".truncate { width: 100%; }",
    "[data-library-abs-list] { height: 120px; }",
);

/// The harness's props: the mock core sender and the handle the test reads
/// the store through.
#[derive(Clone)]
struct FixtureProps {
    sender: mpsc::Sender<CoreInput>,
    handle: Rc<RefCell<Option<FixtureHandles>>>,
}

/// The root: provides the Library store and the layout epoch, and paints the
/// book list, as the panel composes it.
fn audiobooks_list_app(props: FixtureProps) -> Element {
    let FixtureProps { sender, handle } = props;
    let clock: Arc<dyn Clock> = Arc::new(RecordingClock::new());
    let state = use_hook(move || {
        // The playhead position the interaction state reads: none — the
        // tests hold it at 0.
        let playhead_position = Memo::new(|| 0.0);
        LibraryState::new(
            Sections::new(),
            CoreLink::new(sender),
            playhead_position,
            Arc::clone(&clock),
        )
    });
    provide_context(state.clone());
    let epoch = use_hook(LayoutEpoch::new);
    provide_context(epoch);
    handle.borrow_mut().replace((state, epoch));
    rsx! {
        LibraryAudiobooksList {}
    }
}

struct Fixture {
    doc: DioxusDocument,
    state: LibraryState,
    epoch: LayoutEpoch,
    receiver: mpsc::Receiver<CoreInput>,
}

impl Fixture {
    /// Poll the document until the effects have run and their re-renders
    /// landed (a fixed poll count with a tripwire: never an unbounded wait).
    fn poll_until_idle(&mut self) {
        for _ in 0..8 {
            if !self.doc.poll(None) {
                return;
            }
        }
        panic!("document did not become idle");
    }

    /// Run `f` inside the virtual dom's runtime, so the signal writes mark
    /// their subscribers, and let the re-renders land.
    fn in_runtime(&mut self, f: impl FnOnce() + 'static) {
        self.doc.vdom.in_runtime(f);
        self.poll_until_idle();
    }

    /// Resolve the document's style and layout, as the renderer's frame
    /// would, and let the re-measures the epoch bump drives land.
    fn lay_out(&mut self) {
        self.doc.inner.borrow_mut().resolve(0.0);
        let mut epoch = self.epoch;
        self.doc.vdom.in_runtime(move || epoch.bump());
        self.poll_until_idle();
    }

    /// Carry the Audiobooks sections, as the core's batch would, so the row
    /// model reads the carried frame at the batch's revision.
    fn carry(&mut self, list: GuiAudiobooksList, detail: GuiAudiobooksDetail) {
        let mut sections = self.state.sections.clone();
        self.in_runtime(move || {
            sections
                .library_list
                .set(Some(SectionRef::new(GuiLibraryList {
                    tabs: Vec::new(),
                    current_tab: Some(4),
                    destination: GuiDestinationList::Audiobooks(list),
                })));
            sections
                .library_detail
                .set(Some(SectionRef::new(GuiLibraryDetail::Audiobooks(detail))));
        });
        self.lay_out();
    }

    /// Report a `viewport_height` px viewport, so the rows in view are the
    /// rows a measured list of that height paints: an unmeasured viewport
    /// paints nothing, as the Music list does.
    fn measure(&mut self, viewport_height: f32) {
        let state = self.state.clone();
        self.in_runtime(move || state.measure_audiobooks_list(viewport_height));
    }

    /// Whether the selector names a node in the painted tree.
    fn presents(&self, selector: &str) -> bool {
        matches!(self.doc.inner().query_selector(selector), Ok(Some(_)))
    }

    /// The attribute of the first node the selector names.
    fn attr_of(&self, selector: &str, attribute: &str) -> Option<String> {
        let dom = self.doc.inner();
        let id = dom
            .query_selector(selector)
            .expect("the selector is valid")
            .unwrap_or_else(|| panic!("{selector} paints"));
        dom.get_node(id)
            .expect("the node is in the document")
            .attr(LocalName::from(attribute))
            .map(str::to_owned)
    }

    /// The attribute of every node the selector names that carries it, in
    /// paint order.
    fn all_attrs(&self, selector: &str, attribute: &str) -> Vec<String> {
        let dom = self.doc.inner();
        dom.query_selector_all(selector)
            .expect("the selector is valid")
            .into_iter()
            .filter_map(|id| {
                dom.get_node(id)
                    .expect("the node is in the document")
                    .attr(LocalName::from(attribute))
                    .map(str::to_owned)
            })
            .collect()
    }

    /// Run `f` with the store inside the virtual dom's runtime and return
    /// the value it computed, then let the re-renders land.
    fn compute<R: 'static>(&mut self, f: impl FnOnce(&LibraryState) -> R + 'static) -> R {
        let slot = Rc::new(RefCell::new(None::<R>));
        let state = self.state.clone();
        let inner = Rc::clone(&slot);
        self.in_runtime(move || {
            inner.borrow_mut().replace(f(&state));
        });
        slot.borrow_mut()
            .take()
            .expect("the scenario returned a value")
    }

    /// The `RowsInView` reports the core link received, in send order.
    fn reported_rows_in_view(&mut self) -> Vec<mbv::GuiLibraryCommand> {
        self.receiver
            .try_iter()
            .filter_map(|input| match input {
                CoreInput::Library(command @ mbv::GuiLibraryCommand::RowsInView { .. }) => {
                    Some(command)
                }
                _ => None,
            })
            .collect()
    }

    /// The Library commands the core link received, in send order.
    fn library_commands(&mut self) -> Vec<mbv::GuiLibraryCommand> {
        self.receiver
            .try_iter()
            .filter_map(|input| match input {
                CoreInput::Library(mbv::GuiLibraryCommand::RowsInView { .. }) => None,
                CoreInput::Library(command) => Some(command),
                _ => None,
            })
            .collect()
    }
}

/// Build the document over the carried content, carry the sections, and let
/// the list's first measurement land.
fn built(list: GuiAudiobooksList, detail: GuiAudiobooksDetail) -> Fixture {
    let (sender, receiver) = mpsc::channel::<CoreInput>();
    let handle = Rc::new(RefCell::new(None));
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(
            audiobooks_list_app,
            FixtureProps {
                sender,
                handle: Rc::clone(&handle),
            },
        ),
        DocumentConfig {
            viewport: Some(Viewport::new(1200, 900, 1.0, ColorScheme::Light)),
            ua_stylesheets: vec![
                blitz_dom::UserAgentStylesheet::default_sheet(),
                blitz_dom::UserAgentStylesheet::parse(FIXTURE_CSS),
            ],
            ..DocumentConfig::default()
        },
    );
    doc.initial_build();
    let (state, epoch) = handle
        .borrow_mut()
        .take()
        .expect("the root handed the store over");
    let mut fixture = Fixture {
        doc,
        state,
        epoch,
        receiver,
    };
    fixture.poll_until_idle();
    fixture.carry(list, detail);
    fixture
}

/// The catalog's book rows: two under `A` and one under `B`.
fn catalog() -> Vec<GuiAudiobooksRowFrame> {
    vec![
        GuiAudiobooksRowFrame {
            art: GuiRowArtFrame::Loading,
            media: GuiRowMediaKindFrame::Audio,
            id: "b1".to_owned(),
            title: "Aftermath".to_owned(),
            subtitle: Some("Bill Wyman".to_owned()),
            duration: "4:32".to_owned(),
            sort_letter: Some('A'),
        },
        GuiAudiobooksRowFrame {
            art: GuiRowArtFrame::NoSource,
            media: GuiRowMediaKindFrame::Audio,
            id: "b2".to_owned(),
            title: "Audition".to_owned(),
            subtitle: Some("Ryu Murakami".to_owned()),
            duration: "2:14".to_owned(),
            sort_letter: Some('A'),
        },
        GuiAudiobooksRowFrame {
            art: GuiRowArtFrame::NoSource,
            media: GuiRowMediaKindFrame::Audio,
            id: "b3".to_owned(),
            title: "Blindsight".to_owned(),
            subtitle: Some("Peter Watts".to_owned()),
            duration: "10:02".to_owned(),
            sort_letter: Some('B'),
        },
    ]
}

/// The carried Audiobooks list part over `rows` (row 9.1).
fn list_of(rows: Vec<GuiAudiobooksRowFrame>) -> GuiAudiobooksList {
    GuiAudiobooksList {
        sort: None,
        lib_idx: 4,
        loading: false,
        failed: false,
        rows,
    }
}

/// The Audiobooks detail part with `selected` picked and no Hero (row 9.1).
fn detail_of(selected: Option<&str>) -> GuiAudiobooksDetail {
    GuiAudiobooksDetail {
        workspace_loading: false,
        selected: selected.map(str::to_owned),
        playing_book_id: None,
        hero: None,
    }
}

/// The painted rows' ids, in paint order.
fn painted_rows(fixture: &Fixture) -> Vec<String> {
    fixture.all_attrs("[data-library-abs-row-id]", "data-library-abs-row-id")
}

/// The painted row of `id`, as the press handler takes it.
fn row_of(
    fixture: &mut Fixture,
    id: &str,
) -> crate::window::library::audiobooks_list::AudiobooksListRow {
    let id = id.to_owned();
    fixture.compute(move |state| {
        let filter = state.audiobooks_ui.peek().filter.clone();
        state
            .audiobooks_rows(&filter)
            .iter()
            .find(|row| row.id() == id)
            .cloned()
            .expect("the row paints")
    })
}

/// Press the row of `id`, as the row's press handler does.
fn press_row(fixture: &mut Fixture, id: &str) {
    let row = row_of(fixture, id);
    let state = fixture.state.clone();
    fixture.in_runtime(move || state.press_audiobooks_row(&row));
}

/// Contract (`LibraryAudiobooksList.test.ts`): the list paints the letter
/// headings, its rows, and the rail beside them. The rail press rides the jump
/// handler (a headless document cannot fire a DOM click).
#[test]
fn the_list_paints_letter_headings_its_rows_and_the_rail_beside_them() {
    let mut fixture = built(list_of(catalog()), detail_of(Some("b1")));
    fixture.measure(300.0);

    let letters = fixture.all_attrs("[data-library-abs-letter]", "data-library-abs-letter");
    assert_eq!(
        letters,
        ["A".to_owned(), "B".to_owned()],
        "the headings paint"
    );
    assert_eq!(
        painted_rows(&fixture),
        ["b1".to_owned(), "b2".to_owned(), "b3".to_owned()]
    );
    // The pointer markers (`gui-pointer-target-interaction` 3.3): the scroller
    // names the Audiobooks list — not the `abs` hook prefix its library
    // attributes carry — and a row names its own id.
    assert_eq!(
        fixture.attr_of("[data-pointer-list]", "data-pointer-list"),
        Some("audiobooks".to_owned())
    );
    assert_eq!(
        fixture.attr_of("[data-pointer-row]", "data-pointer-row"),
        Some("b1".to_owned())
    );

    assert_eq!(
        fixture.attr_of("[data-library-rail-letter=\"B\"]", "disabled"),
        None,
        "the rail's B is enabled by the book under it"
    );
    assert_eq!(
        fixture.all_attrs(
            "[data-library-abs-row-id=\"b1\"] [data-library-abs-thumb]",
            "data-library-abs-thumb"
        ),
        ["loading".to_owned()],
        "the book row paints the shared part's thumb with its art state"
    );
    assert_eq!(
        fixture.all_attrs(
            "[data-library-abs-row-id=\"b2\"] [data-library-abs-thumb]",
            "data-library-abs-thumb"
        ),
        ["none".to_owned()],
        "a row with no art source paints the no-source thumb"
    );
}

/// Contract (gui-delineated-rows design D7): the list reports the rows it
/// paints under its own scope, so the core queues their row art without a
/// coordinate.
#[test]
fn the_list_reports_the_rows_in_view_under_the_audiobooks_scope() {
    let mut fixture = built(list_of(catalog()), detail_of(Some("b1")));
    fixture.measure(300.0);

    assert_eq!(
        fixture.reported_rows_in_view().last(),
        Some(&mbv::GuiLibraryCommand::RowsInView {
            scope: mbv::GuiRowArtScope::Audiobooks,
            ids: vec!["b1".to_owned(), "b2".to_owned(), "b3".to_owned()],
        }),
        "the report names the painted rows under the Audiobooks scope"
    );
}

/// Contract (`LibraryAudiobooksList.test.ts`): a rail letter no row carries
/// is disabled, and its press scrolls nothing.
#[test]
fn a_rail_letter_no_row_carries_is_disabled() {
    let mut fixture = built(list_of(catalog()), detail_of(Some("b1")));
    fixture.measure(300.0);

    assert_eq!(
        fixture.attr_of("[data-library-rail-letter=\"C\"]", "disabled",),
        Some("true".to_owned()),
        "the letterless rail letter paints disabled"
    );

    let state = fixture.state.clone();
    fixture.in_runtime(move || state.jump_audiobooks_to_letter('C'));

    assert_eq!(
        fixture.state.scrolls.audiobooks.peek().scroll_top,
        0.0,
        "the disabled letter's press scrolls nothing"
    );
}

/// Contract (`LibraryAudiobooksList.test.ts`): a press selects the book and
/// sends the select command. The press rides the handler function (a
/// headless document cannot fire a DOM click).
#[test]
fn a_press_selects_the_book_and_sends_the_select_command() {
    let mut fixture = built(list_of(catalog()), detail_of(Some("b1")));
    fixture.measure(300.0);

    press_row(&mut fixture, "b2");

    assert_eq!(
        fixture.library_commands(),
        [mbv::GuiLibraryCommand::AbsSelect {
            lib_idx: 4,
            row: "b2".to_owned(),
        }],
        "the select command names the book id"
    );
}

/// Contract (`LibraryAudiobooksList.test.ts`): a double-click plays the book
/// from its saved position. The double-click rides the handler function (a
/// headless document cannot fire a DOM click).
#[test]
fn a_double_click_plays_the_book_from_its_saved_position() {
    let mut fixture = built(list_of(catalog()), detail_of(Some("b1")));

    let state = fixture.state.clone();
    fixture.in_runtime(move || state.play_audiobooks_row("b3"));

    assert_eq!(
        fixture.library_commands(),
        [mbv::GuiLibraryCommand::AbsPlay {
            lib_idx: 4,
            item_id: "b3".to_owned(),
        }],
        "the play command names the book id"
    );
}

/// Contract (`LibraryAudiobooksList.test.ts`): the core's selected book paints
/// its mark and its fill, and no row paints a cursor — the list keeps none
/// (`gui-pointer-target-interaction` D6).
#[test]
fn the_selected_book_paints_its_fill_and_no_row_paints_a_cursor() {
    let mut fixture = built(list_of(catalog()), detail_of(Some("b3")));
    fixture.measure(300.0);

    assert_eq!(
        fixture.attr_of(
            "[data-library-abs-row-id=\"b3\"]",
            "data-library-abs-selected",
        ),
        Some(String::new()),
        "the selected book paints its mark"
    );
    assert!(
        fixture
            .attr_of("[data-library-abs-row-id=\"b3\"]", "style")
            .is_some_and(|style| style.contains("background")),
        "the selected book paints its fill"
    );
    assert!(
        !fixture.presents("[data-library-abs-cursor]"),
        "no row paints a cursor"
    );
}

/// Contract (`LibraryAudiobooksList.test.ts`): the load and failure facts
/// paint their placeholder lines.
#[test]
fn loading_and_failed_paint_their_placeholder_lines() {
    let mut list = list_of(Vec::new());
    list.loading = true;
    list.failed = true;
    let fixture = built(list, detail_of(None));

    assert!(
        fixture.presents("[data-library-abs-loading]"),
        "the loading line paints"
    );
    assert!(
        fixture.presents("[data-library-abs-failed]"),
        "the failure line paints"
    );
}

/// Contract (`LibraryAudiobooksList.test.ts`, design D15): the Audiobooks
/// column carries the Music list's header.
#[test]
fn the_list_paints_the_audiobooks_header() {
    let mut fixture = built(list_of(catalog()), detail_of(Some("b1")));
    fixture.measure(300.0);

    assert_eq!(
        fixture.attr_of("[data-library-list-mark]", "data-library-list-mark"),
        Some("audiobookshelf".to_owned()),
        "the header marks the destination's Service"
    );
    assert!(
        !fixture.presents("[data-library-selection-label]"),
        "the whole catalog is the default scope, so no label names it"
    );
}
