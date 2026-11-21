//! Headless tests of the plain Emby-library list (D16): the contracts of
//! `LibraryEmbyList.test.ts` — the plain mode paints its name-sorted rows
//! under their letter headings with the A–Z rail beside them
//! with, the feed-group mode paints its flat date-ordered rows with their
//! date gutter and no rail; a press sends the select command with the row's
//! stable id and a double-click the play command, a collection row included;
//! and the load, failure, and empty facts paint their lines. The IPC
//! of the earlier tests is the mock core link here; a headless document
//! cannot fire a DOM click, so the press, play, jump, and menu contracts
//! ride the state methods the component's own handlers call one line each.
//! The core channel is a mock pair; no test opens a window, sleeps, or
//! reaches the core.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, mpsc};

use blitz_dom::{Document as _, LocalName};
use blitz_traits::shell::{ColorScheme, Viewport};
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use super::LibraryEmbyList;
use crate::bridge::frame::library::{
    GuiDestinationList, GuiEmbyDetail, GuiEmbyList, GuiEmbyRowFrame, GuiEmbyRowKindFrame,
    GuiLibraryDetail, GuiLibraryList, GuiRowArtFrame, GuiRowMediaKindFrame,
};
use crate::core_thread::CoreInput;
use crate::window::channel::CoreLink;
use crate::window::clock::{Clock, RecordingClock};
use crate::window::layout::LayoutEpoch;
use crate::window::library::lettered::LetteredItemRow;
use crate::window::library::list::GeometryRow;
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
    ".min-w-0 { min-width: 0; }",
    ".min-h-0 { min-height: 0; }",
    ".flex-1 { flex: 1 1 0%; }",
    ".flex-none { flex: none; }",
    ".w-full { width: 100%; }",
    ".block { display: block; }",
    ".relative { position: relative; }",
    ".overflow-hidden { overflow: hidden; }",
    ".truncate { width: 100%; }",
    "[data-library-emby-list] { height: 120px; }",
);

/// The harness's props: the mock core sender and the handle the test reads
/// the store through.
#[derive(Clone)]
struct FixtureProps {
    sender: mpsc::Sender<CoreInput>,
    handle: Rc<RefCell<Option<FixtureHandles>>>,
}

/// The root: provides the Library store and the layout epoch, and paints the
/// Emby list, as the panel composes it.
fn emby_list_app(props: FixtureProps) -> Element {
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
        LibraryEmbyList {}
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

    /// Carry the Emby sections, as the core's batch would, so the row model
    /// reads the carried frame at the batch's revision.
    fn carry(&mut self, list: GuiEmbyList, detail: GuiEmbyDetail) {
        let mut sections = self.state.sections.clone();
        self.in_runtime(move || {
            sections
                .library_list
                .set(Some(SectionRef::new(GuiLibraryList {
                    tabs: Vec::new(),
                    current_tab: Some(3),
                    destination: GuiDestinationList::Emby(list),
                })));
            sections
                .library_detail
                .set(Some(SectionRef::new(GuiLibraryDetail::Emby(detail))));
        });
        self.lay_out();
    }

    /// Report a `viewport_height` px viewport, so the rows in view are the
    /// rows a measured list of that height paints: an unmeasured viewport
    /// paints nothing, as the Music list does.
    fn measure(&mut self, viewport_height: f32) {
        let state = self.state.clone();
        self.in_runtime(move || state.measure_emby_list(viewport_height));
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
fn built(list: GuiEmbyList, detail: GuiEmbyDetail) -> Fixture {
    let (sender, receiver) = mpsc::channel::<CoreInput>();
    let handle = Rc::new(RefCell::new(None));
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(
            emby_list_app,
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

/// The plain mode's name-sorted rows: two `A` movies and one `B` movie.
fn movies() -> Vec<GuiEmbyRowFrame> {
    vec![
        GuiEmbyRowFrame {
            art: GuiRowArtFrame::Loading,
            media: GuiRowMediaKindFrame::Video,
            id: "m1".to_owned(),
            title: "Alien".to_owned(),
            subtitle: Some("1979".to_owned()),
            date: None,
            sort_letter: Some('A'),
            kind: GuiEmbyRowKindFrame::Media,
        },
        GuiEmbyRowFrame {
            art: GuiRowArtFrame::NoSource,
            media: GuiRowMediaKindFrame::Video,
            id: "m2".to_owned(),
            title: "Amélie".to_owned(),
            subtitle: Some("2001".to_owned()),
            date: None,
            sort_letter: Some('A'),
            kind: GuiEmbyRowKindFrame::Media,
        },
        GuiEmbyRowFrame {
            art: GuiRowArtFrame::NoSource,
            media: GuiRowMediaKindFrame::Video,
            id: "m3".to_owned(),
            title: "Brazil".to_owned(),
            subtitle: Some("1985".to_owned()),
            date: None,
            sort_letter: Some('B'),
            kind: GuiEmbyRowKindFrame::Media,
        },
    ]
}

/// The feed-group mode's date-ordered rows, with a boxset collection row.
fn feed() -> Vec<GuiEmbyRowFrame> {
    vec![
        GuiEmbyRowFrame {
            art: GuiRowArtFrame::NoSource,
            media: GuiRowMediaKindFrame::Video,
            id: "f1".to_owned(),
            title: "Family".to_owned(),
            subtitle: None,
            date: Some("1998".to_owned()),
            sort_letter: None,
            kind: GuiEmbyRowKindFrame::Collection,
        },
        GuiEmbyRowFrame {
            art: GuiRowArtFrame::NoSource,
            media: GuiRowMediaKindFrame::Video,
            id: "f2".to_owned(),
            title: "Fargo".to_owned(),
            subtitle: None,
            date: Some("1996".to_owned()),
            sort_letter: None,
            kind: GuiEmbyRowKindFrame::Media,
        },
    ]
}

/// The carried Emby list part in one mode over `rows` (row 8.1).
fn emby_frame(feed_group: bool, rows: Vec<GuiEmbyRowFrame>) -> GuiEmbyList {
    GuiEmbyList {
        sort: None,
        lib_idx: 3,
        library_name: "Movies".to_owned(),
        collections: None,
        collection_names: Vec::new(),
        feed_group,
        loading: false,
        failed: false,
        rows,
    }
}

/// The Emby detail part: the first row selected, and no Hero painted here.
fn emby_detail(rows: &[GuiEmbyRowFrame]) -> GuiEmbyDetail {
    GuiEmbyDetail {
        selected: rows.first().map(|row| row.id.clone()),
        hero: None,
    }
}

/// The painted rows' ids, in paint order.
fn painted_rows(fixture: &Fixture) -> Vec<String> {
    fixture.all_attrs("[data-library-emby-row-id]", "data-library-emby-row-id")
}

/// The painted item row of `id`, as the press handler takes it.
fn row_of(
    fixture: &mut Fixture,
    id: &str,
) -> crate::window::library::list::LetteredRow<LetteredItemRow<GuiEmbyRowFrame>> {
    let id = id.to_owned();
    fixture.compute(move |state| {
        state
            .emby_rows()
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
    fixture.in_runtime(move || state.press_emby_row(&row));
}

/// Contract (`LibraryEmbyList.test.ts`): the plain mode paints the letter
/// headings with the rail beside the list, and the rail's B is enabled because
/// a painted row carries it.
#[test]
fn the_plain_mode_paints_letter_headings_its_rows_and_the_rail_beside_them() {
    let mut fixture = built(emby_frame(false, movies()), emby_detail(&movies()));
    fixture.measure(300.0);

    let letters = fixture.all_attrs("[data-library-emby-letter]", "data-library-emby-letter");
    assert_eq!(
        letters,
        ["A".to_owned(), "B".to_owned()],
        "the headings paint"
    );
    assert_eq!(
        painted_rows(&fixture),
        ["m1".to_owned(), "m2".to_owned(), "m3".to_owned()]
    );
    assert_eq!(
        fixture.attr_of("[data-library-rail-letter=\"B\"]", "disabled"),
        None,
        "the rail's B is enabled by the row under it"
    );
    assert_eq!(
        fixture.all_attrs(
            "[data-library-emby-row-id=\"m1\"] [data-library-emby-thumb]",
            "data-library-emby-thumb"
        ),
        ["loading".to_owned()],
        "the item row paints the shared part's thumb with its art state"
    );
    assert_eq!(
        fixture.all_attrs(
            "[data-library-emby-row-id=\"m2\"] [data-library-emby-thumb]",
            "data-library-emby-thumb"
        ),
        ["none".to_owned()],
        "a row with no art source paints the no-source thumb"
    );
}

/// Contract (gui-delineated-rows design D7): the list reports the rows it
/// paints under its own scope, so the core queues their row art without a
/// coordinate.
#[test]
fn the_list_reports_the_rows_in_view_under_the_emby_scope() {
    let mut fixture = built(emby_frame(false, movies()), emby_detail(&movies()));
    fixture.measure(300.0);

    assert_eq!(
        fixture.reported_rows_in_view().last(),
        Some(&mbv::GuiLibraryCommand::RowsInView {
            scope: mbv::GuiRowArtScope::Emby,
            ids: vec!["m1".to_owned(), "m2".to_owned(), "m3".to_owned()],
        }),
        "the report names the painted rows under the Emby scope"
    );
}

/// Contract (`LibraryEmbyList.test.ts`): the plain mode paints the row's
/// year as its secondary label and no date gutter.
#[test]
fn the_plain_mode_paints_the_rows_year_as_its_secondary_label() {
    let mut fixture = built(emby_frame(false, movies()), emby_detail(&movies()));
    fixture.measure(300.0);

    assert_eq!(
        fixture.text_of("[data-library-emby-row-id=\"m1\"] [data-library-emby-subtitle]"),
        "1979",
        "the year paints as the secondary label"
    );
    assert!(
        !fixture.presents("[data-library-emby-date]"),
        "the plain mode paints no date gutter"
    );
}

/// Contract (`LibraryEmbyList.test.ts`): the feed-group mode paints the flat
/// date-ordered rows with their gutter and no rail and no heading.
#[test]
fn the_feed_group_mode_paints_the_date_gutter_and_no_rail() {
    let mut fixture = built(emby_frame(true, feed()), emby_detail(&feed()));
    fixture.measure(300.0);

    assert_eq!(painted_rows(&fixture), ["f1".to_owned(), "f2".to_owned()]);
    assert!(
        !fixture.presents("[data-library-emby-letter]"),
        "the feed-group mode paints no heading"
    );
    assert!(
        !fixture.presents("[data-library-region=\"rail\"]"),
        "the feed-group mode paints no rail"
    );
    assert_eq!(
        fixture.text_of("[data-library-emby-row-id=\"f1\"] [data-library-emby-date]"),
        "1998",
        "the date gutter paints"
    );
}

/// Contract (`LibraryEmbyList.test.ts`): a press sends the select command
/// with the row id. The press rides the handler function (a headless
/// document cannot fire a DOM click).
#[test]
fn a_press_sends_the_select_command_with_the_row_id() {
    let mut fixture = built(emby_frame(false, movies()), emby_detail(&movies()));
    fixture.measure(300.0);

    press_row(&mut fixture, "m2");

    assert_eq!(
        fixture.library_commands(),
        [mbv::GuiLibraryCommand::EmbySelect {
            lib_idx: 3,
            row: "m2".to_owned(),
        }],
        "the select command names the row id"
    );
}

/// Contract (`LibraryEmbyList.test.ts`): a double-click on a row plays it.
/// The double-click rides the handler function (a headless document cannot
/// fire a DOM click).
#[test]
fn a_double_click_on_a_row_plays_it() {
    let mut fixture = built(emby_frame(false, movies()), emby_detail(&movies()));

    let state = fixture.state.clone();
    fixture.in_runtime(move || state.play_emby_row("m3"));

    assert_eq!(
        fixture.library_commands(),
        [mbv::GuiLibraryCommand::EmbyPlay {
            lib_idx: 3,
            item_id: "m3".to_owned(),
        }],
        "the play command names the row id"
    );
}

/// Contract (`LibraryEmbyList.test.ts`): a folder row plays through the
/// core's folder-play rule with no drill-in, so the double-click sends the
/// same play command as any other row.
#[test]
fn a_double_click_on_a_collection_row_plays_it() {
    let mut fixture = built(emby_frame(true, feed()), emby_detail(&feed()));

    let state = fixture.state.clone();
    fixture.in_runtime(move || state.play_emby_row("f1"));

    assert_eq!(
        fixture.library_commands(),
        [mbv::GuiLibraryCommand::EmbyPlay {
            lib_idx: 3,
            item_id: "f1".to_owned(),
        }],
        "the collection row plays with no drill-in"
    );
}

/// Contract (`LibraryEmbyList.test.ts`): a list that has not landed paints
/// the loading line.
#[test]
fn a_list_that_has_not_landed_paints_the_loading_line() {
    let mut list = emby_frame(false, Vec::new());
    list.loading = true;
    let fixture = built(list, emby_detail(&[]));

    assert!(
        fixture.presents("[data-library-emby-loading]"),
        "the loading line paints"
    );
}

/// Contract (`LibraryEmbyList.test.ts`): a failed plain-mode fetch paints
/// the failure line.
#[test]
fn a_failed_plain_mode_fetch_paints_the_failure_line() {
    let mut list = emby_frame(false, Vec::new());
    list.failed = true;
    let fixture = built(list, emby_detail(&[]));

    assert!(
        fixture.presents("[data-library-emby-failed]"),
        "the failure line paints"
    );
}

/// Contract (`LibraryEmbyList.test.ts`, design D15, user override): the list
/// column carries the Music list's header — the library's name and the `/`
/// field — and paints no selection label: the plain list is the whole library,
/// and the UI is not verbose.
#[test]
fn the_list_paints_the_library_header_without_the_selection_label() {
    let mut fixture = built(emby_frame(false, movies()), emby_detail(&movies()));
    fixture.measure(300.0);

    assert_eq!(
        fixture.attr_of("[data-library-list-mark]", "data-library-list-mark"),
        Some("emby".to_owned()),
        "the header marks the library's Service"
    );
    assert!(
        !fixture.presents("[data-library-selection-label]"),
        "a plain library paints no selection label"
    );
}

/// Contract (`LibraryEmbyList.test.ts`): a landed library with no row paints
/// the empty line.
#[test]
fn a_landed_library_with_no_row_paints_the_empty_line() {
    let fixture = built(emby_frame(false, Vec::new()), emby_detail(&[]));

    assert!(
        fixture
            .text_of("[data-library-emby-empty]")
            .contains("No items"),
        "the empty line says so"
    );
}

/// Contract (`LibraryEmbyList.test.ts`, design D8): a right-click opens the
/// item menu for the row's stable id, which the panel paints (D15). The
/// right-press rides the handler function (a headless document cannot fire a
/// context-menu event).
#[test]
fn a_right_click_on_a_row_sends_the_row_menu_command_with_its_id() {
    let mut fixture = built(emby_frame(false, movies()), emby_detail(&movies()));

    let state = fixture.state.clone();
    fixture.in_runtime(move || {
        state.open_row_menu_at(
            Some(LibraryState::row_menu_item("m2".to_owned())),
            12.0,
            34.0,
        );
    });

    assert_eq!(
        fixture.library_commands(),
        [mbv::GuiLibraryCommand::RowMenu {
            row: mbv::GuiLibraryMenuRow::Item("m2".to_owned()),
        }],
        "the row-menu command names the row's item id"
    );
}
