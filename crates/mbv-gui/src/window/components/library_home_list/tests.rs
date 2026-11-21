//! Headless tests of the Home list (D16): the contracts of
//! `LibraryHomeList.test.ts` — the flat list paints the two sections'
//! headings in order, the item rows with their date gutter, and only the
//! rows in view; a press sends the select command with the row's opaque
//! identity, a double-click the play command, and a right-press the row-menu
//! command; the rows without art paint no art box, the active row paints its
//! resume percent, and the load and empty facts paint their lines. The
//! IPC of the earlier tests is the mock core link here; the asserted commands
//! are the same neutral shapes. A headless document cannot fire a DOM click,
//! so the press, play, and menu contracts ride the state methods the
//! component's own handlers call one line each. The core channel is a mock
//! pair; no test opens a window, sleeps, or reaches the core.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, mpsc};

use blitz_dom::{Document as _, LocalName};
use blitz_traits::shell::{ColorScheme, Viewport};
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};
use mbv_queue::QueueItemContentId;

use super::LibraryHomeList;
use crate::bridge::frame::library::{
    GuiDestinationList, GuiHomeDetail, GuiHomeItemRowFrame, GuiHomeList, GuiHomeRowFrame,
    GuiHomeRowIdFrame, GuiHomeRowKindFrame, GuiHomeRowStateFrame, GuiLibraryDetail, GuiLibraryList,
    GuiRowArtFrame, GuiRowMediaKindFrame,
};
use crate::core_thread::CoreInput;
use crate::window::channel::CoreLink;
use crate::window::clock::{Clock, FiredTimers, RecordingClock};
use crate::window::components::art_image::{ArtEvents, ScaleFactor};
use crate::window::layout::LayoutEpoch;
use crate::window::library::home_list::{HOME_ITEM_ROW_HEIGHT, HomeListRow, home_row_key};
use crate::window::library::state::LibraryState;
use crate::window::sections::SectionRef;
use crate::window::sections::Sections;

/// The context handles the root hands the test: the Library store and the
/// layout epoch the re-measures read.
type FixtureHandles = (LibraryState, LayoutEpoch);

/// The layout rules the fixture resolves with, in place of the app
/// stylesheet a real window loads: the handful of utilities the list tree
/// sizes its boxes with (the same rules the panel's harness uses), and the
/// fixed 120 px scroller box the list's first measurement reports.
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
    ".absolute { position: absolute; }",
    ".overflow-hidden { overflow: hidden; }",
    ".truncate { width: 100%; }",
    "[data-library-home-list] { height: 120px; }",
);

/// The harness's props: the mock core sender and the handle the test reads
/// the store through.
#[derive(Clone)]
struct FixtureProps {
    sender: mpsc::Sender<CoreInput>,
    handle: Rc<RefCell<Option<FixtureHandles>>>,
}

/// The root: provides the Library store and the layout epoch, and paints the
/// Home list, as the panel composes it.
fn home_list_app(props: FixtureProps) -> Element {
    let FixtureProps { sender, handle } = props;
    // The art painter's contexts (the art painter consumes them), over the
    // clock the store holds. The stores hook once, as the art painter's own
    // harness holds them.
    let events = use_hook(ArtEvents::new);
    let fired = use_hook(FiredTimers::new);
    provide_context(events);
    provide_context(fired);
    provide_context(ScaleFactor(Signal::new(1.0)));
    let clock: Arc<dyn Clock> = Arc::new(RecordingClock::new());
    provide_context(Arc::clone(&clock));
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
        LibraryHomeList {}
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

    /// Carry the Home sections, as the core's batch would, so the row model
    /// reads the carried frame at the batch's revision.
    fn carry(&mut self, list: GuiHomeList, detail: GuiHomeDetail) {
        let mut sections = self.state.sections.clone();
        self.in_runtime(move || {
            sections
                .library_list
                .set(Some(SectionRef::new(GuiLibraryList {
                    tabs: Vec::new(),
                    current_tab: None,
                    destination: GuiDestinationList::Home(list),
                })));
            sections
                .library_detail
                .set(Some(SectionRef::new(GuiLibraryDetail::Home(detail))));
        });
        self.lay_out();
    }

    /// Report a `viewport_height` px viewport, so the rows in view are the
    /// rows a measured list of that height paints: an unmeasured viewport
    /// paints nothing, as the Music list does.
    fn measure(&mut self, viewport_height: f32) {
        let state = self.state.clone();
        self.in_runtime(move || state.measure_home_list(viewport_height));
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

    /// The Library commands the core link received, in send order. The row
    /// art report the list sends (gui-delineated-rows design D7) rides its own
    /// test, so a caller that asserts the command its press sends reads it
    /// alone.
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
fn built(list: GuiHomeList, detail: GuiHomeDetail) -> Fixture {
    let (sender, receiver) = mpsc::channel::<CoreInput>();
    let handle = Rc::new(RefCell::new(None));
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(
            home_list_app,
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

/// The `Continue watching` row with art and a resume percent.
fn continued_with_art() -> GuiHomeItemRowFrame {
    GuiHomeItemRowFrame {
        id: GuiHomeRowIdFrame::Continue("e1".to_owned()),
        title: "Better Call Saul".to_owned(),
        subtitle: Some("S3E4 · Sabrosito".to_owned()),
        trailing: None,
        kind: GuiHomeRowKindFrame::Media,
        state: GuiHomeRowStateFrame::Active { progress: Some(42) },
        art: GuiRowArtFrame::Loading,
        media: GuiRowMediaKindFrame::Video,
    }
}

/// The played `Continue watching` row without art.
fn played() -> GuiHomeItemRowFrame {
    GuiHomeItemRowFrame {
        id: GuiHomeRowIdFrame::Continue("e2".to_owned()),
        title: "The Wire".to_owned(),
        subtitle: None,
        trailing: None,
        kind: GuiHomeRowKindFrame::Media,
        state: GuiHomeRowStateFrame::Played,
        art: GuiRowArtFrame::NoSource,
        media: GuiRowMediaKindFrame::Video,
    }
}

/// The `Recently added` row with its gutter date.
fn added() -> GuiHomeItemRowFrame {
    GuiHomeItemRowFrame {
        id: GuiHomeRowIdFrame::Recent(QueueItemContentId::Feed("f1".to_owned())),
        title: "A Feed Entry".to_owned(),
        subtitle: Some("The Feed".to_owned()),
        trailing: Some("12 May".to_owned()),
        kind: GuiHomeRowKindFrame::Media,
        state: GuiHomeRowStateFrame::Ordinary,
        art: GuiRowArtFrame::NoSource,
        media: GuiRowMediaKindFrame::Video,
    }
}

/// The two sections' rows: the heading, the two item rows, the spacer, the
/// second heading, and the third item row.
fn home_frame() -> GuiHomeList {
    GuiHomeList {
        loading: false,
        rows: vec![
            GuiHomeRowFrame::Heading {
                text: "Continue watching".to_owned(),
            },
            GuiHomeRowFrame::Item(continued_with_art()),
            GuiHomeRowFrame::Item(played()),
            GuiHomeRowFrame::Spacer,
            GuiHomeRowFrame::Heading {
                text: "Recently added".to_owned(),
            },
            GuiHomeRowFrame::Item(added()),
        ],
    }
}

/// The Home detail part: the first item row selected, and no Hero painted
/// here.
fn home_detail() -> GuiHomeDetail {
    GuiHomeDetail {
        selected: Some(GuiHomeRowIdFrame::Continue("e1".to_owned())),
        hero: None,
    }
}

/// The painted rows' ids, in paint order.
fn painted_rows(fixture: &Fixture) -> Vec<String> {
    fixture.all_attrs("[data-library-home-row-id]", "data-library-home-row-id")
}

/// The painted row of `id`, as the press handler takes it.
fn row_of(fixture: &mut Fixture, id: &GuiHomeRowIdFrame) -> HomeListRow {
    let key = home_row_key(id);
    fixture.compute(move |state| {
        let ui = state.home_ui.peek().clone();
        state
            .home_rows(&ui.filter)
            .iter()
            .find(|row| row.id() == key)
            .cloned()
            .expect("the row paints")
    })
}

/// Contract (`LibraryHomeList.test.ts`): the sections paint their headings
/// and the date gutter in order, and the spacer paints no marker.
#[test]
fn the_sections_paint_their_headings_and_the_date_gutter_in_order() {
    let mut fixture = built(home_frame(), home_detail());
    fixture.measure(300.0);

    let headings = fixture.all_attrs("[data-library-home-heading]", "data-library-home-heading");
    assert_eq!(
        headings,
        ["Continue watching".to_owned(), "Recently added".to_owned()],
        "the headings paint in order"
    );
    assert_eq!(
        fixture
            .text_of("[data-library-home-row-id=\"home-recent:Feed:f1\"] [data-library-home-date]"),
        "12 May",
        "the recent row paints its gutter date"
    );
    assert!(
        !fixture.presents("[data-library-home-row=\"spacer\"]"),
        "the spacer paints no marker"
    );
    // The pointer markers (`gui-pointer-target-interaction` 3.3): only the item
    // rows name a row. A heading and a spacer name none, so a key over one
    // resolves no item.
    assert_eq!(
        fixture.all_attrs("[data-pointer-row]", "data-pointer-row"),
        painted_rows(&fixture),
        "the pointer rows are the item rows"
    );
}

/// Contract (`LibraryHomeList.test.ts`): the headings and the spacer paint
/// no control — the cursor and a press reach the item rows only.
#[test]
fn the_headings_and_the_spacer_paint_no_control() {
    let mut fixture = built(home_frame(), home_detail());
    fixture.measure(300.0);

    let buttons = fixture.all_attrs("button", "data-library-home-row");
    assert!(
        buttons.len() > 2,
        "the item rows paint as buttons: {buttons:?}"
    );
    assert!(
        buttons.iter().all(|kind| kind == "item"),
        "every control is an item row: {buttons:?}"
    );
}

/// Contract (`LibraryHomeList.test.ts`): the list paints only the rows in
/// view.
#[test]
fn the_list_paints_only_the_rows_in_view() {
    let mut fixture = built(home_frame(), home_detail());
    fixture.measure(HOME_ITEM_ROW_HEIGHT);

    let painted = painted_rows(&fixture);
    assert!(
        painted.contains(&"home-continue:e1".to_owned()),
        "the first item row paints in view: {painted:?}"
    );
    assert!(
        !painted.contains(&"home-continue:e2".to_owned()),
        "the second item row paints only in view: {painted:?}"
    );
}

/// The four sections' rows in paint order (gui-collection-dropdown design
/// D9): `Continue watching`, `Recently added`, `Latest episodes`, then
/// Contract (`LibraryHomeList.test.ts`): a press sends the select command
/// with the row's opaque id. The press that sends it is the handler function
/// the component's own handler calls one line each (a headless document
/// cannot fire a DOM click).
#[test]
fn a_press_sends_the_select_command_with_the_rows_opaque_id() {
    let mut fixture = built(home_frame(), home_detail());
    fixture.measure(300.0);
    let row = row_of(
        &mut fixture,
        &GuiHomeRowIdFrame::Recent(QueueItemContentId::Feed("f1".to_owned())),
    );

    let state = fixture.state.clone();
    fixture.in_runtime(move || state.press_home_row(&row));

    assert_eq!(
        fixture.library_commands(),
        [mbv::GuiLibraryCommand::HomeSelect {
            row: mbv::GuiHomeRowId::Recent(QueueItemContentId::Feed("f1".to_owned())),
        }],
        "the select command names the row's opaque id"
    );
}

/// Contract (`LibraryHomeList.test.ts`): a double-click sends the play
/// command with the row's opaque id. The double-click contract rides the
/// handler function (a headless document cannot fire a DOM click).
#[test]
fn a_double_click_sends_the_play_command_with_the_rows_opaque_id() {
    let mut fixture = built(home_frame(), home_detail());

    let state = fixture.state.clone();
    fixture.in_runtime(move || state.play_home_row(GuiHomeRowIdFrame::Continue("e1".to_owned())));

    assert_eq!(
        fixture.library_commands(),
        [mbv::GuiLibraryCommand::HomePlay {
            row: mbv::GuiHomeRowId::Continue("e1".to_owned()),
        }],
        "the play command names the row's opaque id"
    );
}

/// Contract (`LibraryHomeList.test.ts`): each row paints its thumb, and the
/// thumb names the art state it holds (gui-delineated-rows design D1).
#[test]
fn the_rows_paint_their_thumb_with_its_art_state() {
    let mut fixture = built(home_frame(), home_detail());
    fixture.measure(300.0);

    assert_eq!(
        fixture.all_attrs(
            "[data-library-home-row-id=\"home-continue:e1\"] [data-library-home-thumb]",
            "data-library-home-thumb"
        ),
        ["loading".to_owned()],
        "the row paints its thumb and names the state it holds"
    );
    assert_eq!(
        fixture.all_attrs(
            "[data-library-home-row-id=\"home-recent:Feed:f1\"] [data-library-home-thumb]",
            "data-library-home-thumb"
        ),
        ["none".to_owned()],
        "a kind with no art source paints the no-source state"
    );
}

/// Contract (`LibraryHomeList.test.ts`): the active row paints its resume
/// percent and the played row none.
#[test]
fn the_active_row_paints_its_resume_percent_and_the_played_row_none() {
    let mut fixture = built(home_frame(), home_detail());
    fixture.measure(300.0);

    assert!(
        fixture.presents(
            "[data-library-home-row-id=\"home-continue:e1\"] [data-library-home-thumb-progress]"
        ),
        "the active row paints its resume percent"
    );
    assert!(
        !fixture.presents(
            "[data-library-home-row-id=\"home-continue:e2\"] [data-library-home-thumb-progress]"
        ),
        "the played row paints no resume percent"
    );
}

/// Contract (gui-delineated-rows design D7): the list reports the rows it
/// paints with the core's Home ids, so the core queues their row art without a
/// coordinate.
#[test]
fn the_list_reports_the_rows_in_view_with_the_cores_ids() {
    let mut fixture = built(home_frame(), home_detail());
    fixture.measure(300.0);

    let reported: Vec<mbv::GuiLibraryCommand> = fixture
        .receiver
        .try_iter()
        .filter_map(|input| match input {
            CoreInput::Library(command @ mbv::GuiLibraryCommand::RowsInView { .. }) => {
                Some(command)
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        reported.last(),
        Some(&mbv::GuiLibraryCommand::RowsInView {
            scope: mbv::GuiRowArtScope::Home,
            ids: vec![
                "continue:e1".to_owned(),
                "continue:e2".to_owned(),
                "recent:feed:f1".to_owned(),
            ],
        }),
        "the report names the core's own row ids"
    );
}

/// Contract (`LibraryHomeList.test.ts`, design D15): the Home column carries
/// the Music list's header.
#[test]
fn the_list_paints_the_home_header() {
    let mut fixture = built(home_frame(), home_detail());
    fixture.measure(300.0);

    assert_eq!(
        fixture.text_of("[data-library-list-title]"),
        "Home",
        "the header names the list"
    );
    assert!(
        !fixture.presents("[data-library-selection-label]"),
        "the whole destination is the default scope, so no label names it"
    );
}

/// Contract (`LibraryHomeList.test.ts`, design D12): a loading Home paints
/// the loading line.
#[test]
fn a_loading_home_paints_the_loading_line() {
    let list = GuiHomeList {
        loading: true,
        rows: Vec::new(),
    };
    let fixture = built(list, home_detail());

    assert!(
        fixture.presents("[data-library-home-loading]"),
        "the loading line paints"
    );
}

/// Contract (`LibraryHomeList.test.ts`, design D12): an empty Home paints
/// the empty line.
#[test]
fn an_empty_home_paints_the_empty_line() {
    let list = GuiHomeList {
        loading: false,
        rows: Vec::new(),
    };
    let fixture = built(list, home_detail());

    assert!(
        fixture
            .text_of("[data-library-home-empty]")
            .contains("Nothing to play"),
        "the empty line says so"
    );
}

/// Contract (`LibraryHomeList.test.ts`, design D8): a right-click opens the
/// item menu for the row's stable id, which the core resolves and the panel
/// paints (D15). The right-press contract rides the handler function (a
/// headless document cannot fire a context-menu event).
#[test]
fn a_right_click_on_a_row_sends_the_row_menu_command_with_its_id() {
    let mut fixture = built(home_frame(), home_detail());

    let state = fixture.state.clone();
    fixture.in_runtime(move || {
        state.open_row_menu_at(
            Some(mbv::GuiLibraryMenuRow::Home(mbv::GuiHomeRowId::Continue(
                "e1".to_owned(),
            ))),
            5.0,
            6.0,
        );
    });

    assert_eq!(
        fixture.library_commands(),
        [mbv::GuiLibraryCommand::RowMenu {
            row: mbv::GuiLibraryMenuRow::Home(mbv::GuiHomeRowId::Continue("e1".to_owned())),
        }],
        "the row-menu command names the row's item id"
    );
}
