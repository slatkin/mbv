//! Headless tests of the Feeds list (D16): the contracts of
//! `LibraryFeedsList.test.ts` — the flat entry list paints its date gutter
//! with no letter heading and no rail, a press sends the select command with
//! the row id while a double-click plays the entry, the core's selected entry
//! carries its fill and no row paints a cursor, a played entry dims its
//! title while an active one paints its resume percent, the loading fact
//! paints its own line, and the header names the active pill. The IPC
//! of the earlier tests is the mock core link here; a headless document
//! cannot fire a DOM click, so the press and play contracts ride the state
//! methods the component's own handlers call one line each. The core channel
//! is a mock pair; no test opens a window, sleeps, or reaches the core.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, mpsc};

use blitz_dom::{Document as _, LocalName};
use blitz_traits::shell::{ColorScheme, Viewport};
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use super::LibraryFeedsList;
use crate::bridge::frame::library::{
    GuiCollections, GuiDestinationList, GuiEntryRowFrame, GuiFeedsDetail, GuiFeedsList,
    GuiHomeRowStateFrame, GuiLibraryDetail, GuiLibraryList, GuiRowArtFrame, GuiRowMediaKindFrame,
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
    ".min-w-0 { min-width: 0; }",
    ".min-h-0 { min-height: 0; }",
    ".flex-1 { flex: 1 1 0%; }",
    ".flex-none { flex: none; }",
    ".w-full { width: 100%; }",
    ".block { display: block; }",
    ".relative { position: relative; }",
    ".absolute { position: absolute; }",
    ".overflow-hidden { overflow: hidden; }",
    ".truncate { width: 100%; }",
    "[data-library-feeds-list] { height: 120px; }",
);

/// The harness's props: the mock core sender and the handle the test reads
/// the store through.
#[derive(Clone)]
struct FixtureProps {
    sender: mpsc::Sender<CoreInput>,
    handle: Rc<RefCell<Option<FixtureHandles>>>,
}

/// The root: provides the Library store and the layout epoch, and paints the
/// Feeds list, as the panel composes it.
fn feeds_list_app(props: FixtureProps) -> Element {
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
        LibraryFeedsList {}
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

    /// Carry the Feeds sections, as the core's batch would, so the row model
    /// reads the carried frame at the batch's revision.
    fn carry(&mut self, list: GuiFeedsList, detail: GuiFeedsDetail) {
        let mut sections = self.state.sections.clone();
        self.in_runtime(move || {
            sections
                .library_list
                .set(Some(SectionRef::new(GuiLibraryList {
                    tabs: Vec::new(),
                    current_tab: Some(2),
                    destination: GuiDestinationList::Feeds(list),
                })));
            sections
                .library_detail
                .set(Some(SectionRef::new(GuiLibraryDetail::Feeds(detail))));
        });
        self.lay_out();
    }

    /// Report a `viewport_height` px viewport, so the rows in view are the
    /// rows a measured list of that height paints: an unmeasured viewport
    /// paints nothing, as the Music list does.
    fn measure(&mut self, viewport_height: f32) {
        let state = self.state.clone();
        self.in_runtime(move || state.measure_feeds_list(viewport_height));
    }

    /// Whether the selector names a node in the painted tree.
    fn presents(&self, selector: &str) -> bool {
        matches!(self.doc.inner().query_selector(selector), Ok(Some(_)))
    }

    /// The trimmed text of the first node the selector names.
    fn text_of(&self, selector: &str) -> String {
        let dom = self.doc.inner();
        let id = dom
            .query_selector(selector)
            .expect("the selector is valid")
            .unwrap_or_else(|| panic!("{selector} paints"));
        dom.get_node(id)
            .expect("the node is in the document")
            .text_content()
            .trim()
            .to_owned()
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
fn built(list: GuiFeedsList, detail: GuiFeedsDetail) -> Fixture {
    let (sender, receiver) = mpsc::channel::<CoreInput>();
    let handle = Rc::new(RefCell::new(None));
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(
            feeds_list_app,
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

/// The Latest pill's merged rows: two dated entries, one played, and one
/// partly played with its resume percent.
fn entries() -> Vec<GuiEntryRowFrame> {
    vec![
        GuiEntryRowFrame {
            art: GuiRowArtFrame::Loading,
            media: GuiRowMediaKindFrame::Video,
            id: "s1::e1".to_owned(),
            title: "Entry A".to_owned(),
            subtitle: Some("Sub One".to_owned()),
            date: Some("12 May".to_owned()),
            state: GuiHomeRowStateFrame::Ordinary,
        },
        GuiEntryRowFrame {
            art: GuiRowArtFrame::NoSource,
            media: GuiRowMediaKindFrame::Video,
            id: "s2::e2".to_owned(),
            title: "Entry B".to_owned(),
            subtitle: Some("Sub Two".to_owned()),
            date: Some("9 May".to_owned()),
            state: GuiHomeRowStateFrame::Played,
        },
        GuiEntryRowFrame {
            art: GuiRowArtFrame::NoSource,
            media: GuiRowMediaKindFrame::Video,
            id: "s1::e3".to_owned(),
            title: "Entry C".to_owned(),
            subtitle: Some("Sub One".to_owned()),
            date: Some("4 May".to_owned()),
            state: GuiHomeRowStateFrame::Active { progress: Some(40) },
        },
    ]
}

/// The carried Feeds list part over `rows`, with its load fact as the caller
/// sets it (design D12).
fn frame(rows: Vec<GuiEntryRowFrame>, loading: bool) -> GuiFeedsList {
    GuiFeedsList {
        collections: GuiCollections {
            target: mbv::GuiCollectionTarget::Feeds,
            all: true,
            entries: Vec::new(),
            noun: mbv::GuiCollectionNoun::Subscriptions,
            order: Some(mbv::GuiDateOrder::Newest),
        },
        loading,
        failed: false,
        rows,
    }
}

/// The Feeds detail part: `selected` picked, and no Hero.
fn detail(selected: Option<&str>) -> GuiFeedsDetail {
    GuiFeedsDetail {
        selected: selected.map(str::to_owned),
        hero: None,
    }
}

/// The painted rows' ids, in paint order.
fn painted_rows(fixture: &Fixture) -> Vec<String> {
    fixture.all_attrs("[data-library-feeds-row-id]", "data-library-feeds-row-id")
}

/// The painted row of `id`, as the press handler takes it.
fn row_of(fixture: &mut Fixture, id: &str) -> crate::window::library::feeds_list::FeedsItemRow {
    let id = id.to_owned();
    fixture.compute(move |state| {
        state
            .feeds_rows()
            .iter()
            .find(|row| row.id == id)
            .cloned()
            .expect("the row paints")
    })
}

/// Press the row of `id`, as the row's press handler does.
fn press_row(fixture: &mut Fixture, id: &str) {
    let row = row_of(fixture, id);
    let state = fixture.state.clone();
    fixture.in_runtime(move || state.press_feeds_row(&row));
}

/// Contract (`LibraryFeedsList.test.ts`): the date-ordered list paints the
/// date gutter and no letter heading and no rail, the entry title over its
/// subscription.
#[test]
fn the_flat_list_paints_its_date_gutter_with_no_heading_and_no_rail() {
    let mut fixture = built(frame(entries(), false), detail(Some("s1::e1")));
    fixture.measure(300.0);

    assert_eq!(
        painted_rows(&fixture),
        [
            "s1::e1".to_owned(),
            "s2::e2".to_owned(),
            "s1::e3".to_owned()
        ]
    );
    assert!(
        !fixture.presents("[data-library-region=\"rail\"]"),
        "the flat list paints no rail"
    );
    assert!(
        !fixture.presents("[data-library-feeds-letter]"),
        "the flat list paints no heading"
    );
    assert_eq!(
        fixture.text_of("[data-library-feeds-row-id=\"s2::e2\"] [data-library-feeds-date]"),
        "9 May",
        "the date gutter paints"
    );
    assert_eq!(
        fixture.text_of("[data-library-feeds-row-id=\"s1::e1\"] [data-library-feeds-subtitle]"),
        "Sub One",
        "the entry title paints over its subscription"
    );
    assert_eq!(
        fixture.all_attrs(
            "[data-library-feeds-row-id=\"s1::e1\"] [data-library-feeds-thumb]",
            "data-library-feeds-thumb"
        ),
        ["loading".to_owned()],
        "the entry row paints the shared part's thumb with its art state"
    );
    assert_eq!(
        fixture.all_attrs(
            "[data-library-feeds-row-id=\"s2::e2\"] [data-library-feeds-thumb]",
            "data-library-feeds-thumb"
        ),
        ["none".to_owned()],
        "a row with no art source paints the no-source thumb"
    );
}

/// Contract (gui-delineated-rows design D7): the list reports the rows it
/// paints under its own scope, so the core queues their row art without a
/// coordinate.
#[test]
fn the_list_reports_the_rows_in_view_under_the_feeds_scope() {
    let mut fixture = built(frame(entries(), false), detail(None));
    fixture.measure(300.0);

    assert_eq!(
        fixture.reported_rows_in_view().last(),
        Some(&mbv::GuiLibraryCommand::RowsInView {
            scope: mbv::GuiRowArtScope::Feeds,
            ids: vec![
                "s1::e1".to_owned(),
                "s2::e2".to_owned(),
                "s1::e3".to_owned()
            ],
        }),
        "the report names the painted rows under the Feeds scope"
    );
}

/// Contract (`LibraryFeedsList.test.ts`): a press sends the select command
/// with the row id. The press rides the handler function (a headless
/// document cannot fire a DOM click).
#[test]
fn a_press_sends_the_select_command_with_the_row_id() {
    let mut fixture = built(frame(entries(), false), detail(Some("s1::e1")));
    fixture.measure(300.0);

    press_row(&mut fixture, "s2::e2");

    assert_eq!(
        fixture.library_commands(),
        [mbv::GuiLibraryCommand::FeedsSelect {
            row: "s2::e2".to_owned(),
        }],
        "the select command names the row id"
    );
}

/// Contract (`LibraryFeedsList.test.ts`): a double-click plays the entry.
/// The double-click rides the handler function (a headless document cannot
/// fire a DOM click).
#[test]
fn a_double_click_plays_the_entry() {
    let mut fixture = built(frame(entries(), false), detail(Some("s1::e1")));

    let state = fixture.state.clone();
    fixture.in_runtime(move || state.play_feeds_row("s2::e2"));

    assert_eq!(
        fixture.library_commands(),
        [mbv::GuiLibraryCommand::FeedsPlay {
            item_id: "s2::e2".to_owned(),
        }],
        "the play command names the entry id"
    );
}

/// Contract (`LibraryFeedsList.test.ts`): the core's selected entry carries
/// its mark and its fill, and no row paints a cursor — the list keeps none
/// (`gui-pointer-target-interaction` D6).
#[test]
fn the_cores_selected_entry_carries_its_fill_and_no_row_paints_a_cursor() {
    let mut fixture = built(frame(entries(), false), detail(Some("s1::e1")));
    fixture.measure(300.0);

    assert_eq!(
        fixture.attr_of(
            "[data-library-feeds-row-id=\"s1::e1\"]",
            "data-library-feeds-selected",
        ),
        Some(String::new()),
        "the selected entry paints its mark"
    );
    assert!(
        fixture
            .attr_of("[data-library-feeds-row-id=\"s1::e1\"]", "style")
            .is_some_and(|style| style.contains("background")),
        "the selected entry paints its fill"
    );
    assert!(
        !fixture.presents("[data-library-feeds-cursor]"),
        "no row paints a cursor"
    );
}

/// Contract (`LibraryFeedsList.test.ts`): a played entry dims its title and
/// an active one paints its resume percent.
#[test]
fn a_played_entry_dims_its_title_and_an_active_one_paints_its_resume_percent() {
    let mut fixture = built(frame(entries(), false), detail(None));
    fixture.measure(300.0);

    assert!(
        fixture
            .attr_of(
                "[data-library-feeds-row-id=\"s2::e2\"] [data-library-feeds-title]",
                "style",
            )
            .is_some_and(|style| style.contains("var(--canvas-muted)")),
        "the played entry's title dims"
    );
    assert!(
        fixture.presents(
            "[data-library-feeds-row-id=\"s1::e3\"] [data-library-feeds-thumb-progress]",
        ),
        "the active entry paints its resume percent"
    );
}

/// Contract (`LibraryFeedsList.test.ts`): a loading tab paints its line and
/// no row.
#[test]
fn a_loading_tab_paints_its_line_and_no_row() {
    let fixture = built(frame(Vec::new(), true), detail(None));

    assert!(
        fixture.presents("[data-library-feeds-loading]"),
        "the loading line paints"
    );
    assert!(
        painted_rows(&fixture).is_empty(),
        "no row paints under the loading line"
    );
}

/// Contract (`LibraryFeedsList.test.ts`, design D15, D6a): the Feeds column
/// carries the Music list's header, with the collection dropdown's trigger
/// naming the selection on the title's line.
#[test]
fn the_list_paints_the_feeds_header_with_the_collections_trigger() {
    let mut fixture = built(frame(Vec::new(), false), detail(None));
    fixture.measure(300.0);

    assert_eq!(
        fixture.text_of("[data-library-list-title]"),
        "Feeds",
        "the header names the list"
    );
    assert_eq!(
        fixture.text_of("[data-library-collections-trigger]"),
        "All",
        "the trigger names All while the selection is All"
    );
}

/// Contract (`LibraryFeedsList.test.ts`): a landed view with no entry paints
/// the empty line.
#[test]
fn a_landed_view_with_no_entry_paints_the_empty_line() {
    let fixture = built(frame(Vec::new(), false), detail(None));

    assert!(
        fixture
            .text_of("[data-library-feeds-empty]")
            .contains("No entries"),
        "the empty line says so"
    );
}
