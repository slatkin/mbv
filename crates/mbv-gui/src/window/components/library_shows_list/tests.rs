//! Headless tests of the Shows list (D16): the contracts of
//! `LibraryShowsList.test.ts` — the series list paints its letter headings and
//! series tree with the A–Z rail beside them, a press on a series node selects
//! it and leaves it closed, a toggle opens it, and a press sends the select
//! command with the row's stable id. The IPC of the earlier tests is the mock core
//! link here; a headless document cannot fire a DOM click, so the press,
//! play, and jump contracts ride the state methods the component's own
//! handlers call one line each. The core channel is a mock pair; no test
//! opens a window, sleeps, or reaches the core.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, mpsc};

use blitz_dom::{Document as _, LocalName};
use blitz_traits::shell::{ColorScheme, Viewport};
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use super::LibraryShowsList;
use crate::bridge::frame::library::{
    GuiDestinationList, GuiHomeRowStateFrame, GuiLibraryDetail, GuiLibraryList, GuiRowArtFrame,
    GuiRowMediaKindFrame, GuiShowsDetail, GuiShowsEpisodeRowFrame, GuiShowsList, GuiShowsRowFrame,
    GuiShowsSeasonFrame, GuiShowsSeriesFrame,
};
use crate::core_thread::CoreInput;
use crate::window::channel::CoreLink;
use crate::window::clock::{Clock, RecordingClock};
use crate::window::layout::LayoutEpoch;
use crate::window::library::state::LibraryState;
use crate::window::sections::SectionRef;
use crate::window::sections::Sections;

mod reports;

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
    "[data-library-shows-list] { height: 120px; }",
);

/// The harness's props: the mock core sender and the handle the test reads
/// the store through.
#[derive(Clone)]
struct FixtureProps {
    sender: mpsc::Sender<CoreInput>,
    handle: Rc<RefCell<Option<FixtureHandles>>>,
}

/// The root: provides the Library store and the layout epoch, and paints the
/// Shows list, as the panel composes it.
fn shows_list_app(props: FixtureProps) -> Element {
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
        LibraryShowsList {}
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

    /// Carry the Shows sections, as the core's batch would, so the row model
    /// reads the carried frame at the batch's revision.
    fn carry(&mut self, list: GuiShowsList, detail: GuiShowsDetail) {
        let mut sections = self.state.sections.clone();
        self.in_runtime(move || {
            sections
                .library_list
                .set(Some(SectionRef::new(GuiLibraryList {
                    refreshing: false,
                    tabs: Vec::new(),
                    current_tab: Some(2),
                    destination: GuiDestinationList::Shows(list),
                })));
            sections
                .library_detail
                .set(Some(SectionRef::new(GuiLibraryDetail::Shows(detail))));
        });
        self.lay_out();
    }

    /// Report a `viewport_height` px viewport, so the rows in view are the
    /// rows a measured list of that height paints: an unmeasured viewport
    /// paints nothing, as the Music list does.
    fn measure(&mut self, viewport_height: f32) {
        let state = self.state.clone();
        self.in_runtime(move || state.measure_shows_list(viewport_height));
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

    /// The hover controls the host of the row `row_id` offers, in paint order
    /// (gui-row-hover-actions design D3): the actions are the row button's later
    /// sibling in the row's hover host, so the row's own node names them.
    fn hover_controls_of(&self, row_id: &str) -> Vec<String> {
        let dom = self.doc.inner();
        let row = dom
            .query_selector(&format!("[data-library-shows-row-id=\"{row_id}\"]"))
            .expect("the selector is valid")
            .unwrap_or_else(|| panic!("{row_id} paints"));
        let host = dom
            .get_node(row)
            .expect("the row is in the document")
            .parent
            .expect("the row sits in its hover host");
        dom.query_selector_all_in(host, "[data-row-action]")
            .expect("the selector is valid")
            .into_iter()
            .filter_map(|action| {
                dom.get_node(action)
                    .expect("the control is in the document")
                    .attr(LocalName::from("data-row-action"))
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

    /// The Library commands the core link received, in send order, without the
    /// two reports the window sends on its own: the row art set and the open
    /// Shows nodes.
    fn library_commands(&mut self) -> Vec<crate::gui_core::GuiLibraryCommand> {
        self.receiver
            .try_iter()
            .filter_map(|input| match input {
                CoreInput::Library(
                    crate::gui_core::GuiLibraryCommand::RowsInView { .. }
                    | crate::gui_core::GuiLibraryCommand::ShowsOpenInView { .. },
                ) => None,
                CoreInput::Library(command) => Some(command),
                _ => None,
            })
            .collect()
    }
}

/// Build the document over the carried content, carry the sections, and let
/// the list's first measurement land.
fn built(list: GuiShowsList, detail: GuiShowsDetail) -> Fixture {
    let (sender, receiver) = mpsc::channel::<CoreInput>();
    let handle = Rc::new(RefCell::new(None));
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(
            shows_list_app,
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

/// The carried Shows content over `tree` (design D14, D8): the series list,
/// and no collection selector.
fn shows_frame(tree: Vec<GuiShowsSeriesFrame>) -> GuiShowsList {
    GuiShowsList {
        sort: None,
        view: crate::gui_core::GuiShowsView::All,
        lib_idx: 0,
        loading: false,
        series: tree,
        upcoming_failed: false,
        rows: Vec::new(),
    }
}

/// The Shows detail part over `selected`, with no Hero painted here.
fn shows_detail(selected: Option<&str>) -> GuiShowsDetail {
    GuiShowsDetail {
        playing_episode_id: None,
        workspace_loading: false,
        selected: selected.map(str::to_owned),
        hero: None,
    }
}

/// The series list's tree (design D14): Atlanta with one season, then
/// Better Call Saul under a second heading. Atlanta's season has no episodes
/// yet, so its open asks the core for them.
fn tree() -> Vec<GuiShowsSeriesFrame> {
    vec![
        GuiShowsSeriesFrame {
            art: GuiRowArtFrame::Loading,
            media: GuiRowMediaKindFrame::Video,
            id: "s1".to_owned(),
            title: "Atlanta".to_owned(),
            sort_letter: 'A',
            expandable: true,
            seasons: vec![GuiShowsSeasonFrame {
                id: "se1".to_owned(),
                name: "Season 1".to_owned(),
                episodes: None,
            }],
        },
        GuiShowsSeriesFrame {
            art: GuiRowArtFrame::NoSource,
            media: GuiRowMediaKindFrame::Video,
            id: "s3".to_owned(),
            title: "Better Call Saul".to_owned(),
            sort_letter: 'B',
            expandable: true,
            seasons: Vec::new(),
        },
    ]
}

/// The series list's tree with a landed season: Atlanta with Season 1 and one
/// episode, so the episode leaf paints beside its nodes.
fn tree_with_episodes() -> Vec<GuiShowsSeriesFrame> {
    vec![GuiShowsSeriesFrame {
        art: GuiRowArtFrame::Loading,
        media: GuiRowMediaKindFrame::Video,
        id: "s1".to_owned(),
        title: "Atlanta".to_owned(),
        sort_letter: 'A',
        expandable: true,
        seasons: vec![GuiShowsSeasonFrame {
            id: "se1".to_owned(),
            name: "Season 1".to_owned(),
            episodes: Some(vec![GuiShowsEpisodeRowFrame {
                episode_id: "ep1".to_owned(),
                number: 1,
                name: "The Big Bang".to_owned(),
                duration: "24:00".to_owned(),
                state: GuiHomeRowStateFrame::Ordinary,
            }]),
        }],
    }]
}

/// The painted rows' ids, in paint order.
fn painted_rows(fixture: &Fixture) -> Vec<String> {
    fixture.all_attrs("[data-library-shows-row-id]", "data-library-shows-row-id")
}

/// The painted row of `id`, as the press handler takes it.
fn row_of(fixture: &mut Fixture, id: &str) -> crate::window::library::shows_list::ShowsListRow {
    let id = id.to_owned();
    fixture.compute(move |state| {
        state
            .shows_rows()
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
    fixture.in_runtime(move || state.press_shows_row(&row));
}

/// Toggle the node of `id`, as the row's double-click handler does.
fn toggle_row(fixture: &mut Fixture, id: &str) {
    let row = row_of(fixture, id);
    let state = fixture.state.clone();
    fixture.in_runtime(move || {
        if let Some(node_id) = row.as_node() {
            state.toggle_shows_node(node_id);
        }
    });
}

/// Contract (`LibraryShowsList.test.ts`): the All series mode paints the
/// letter headings and the A–Z rail beside the list, and the rail's B is
/// enabled because a painted series row carries it.
#[test]
fn the_all_series_mode_paints_letter_headings_its_tree_and_the_rail_beside_them() {
    let mut fixture = built(shows_frame(tree()), shows_detail(None));
    fixture.measure(300.0);

    let letters = fixture.all_attrs("[data-library-shows-letter]", "data-library-shows-letter");
    assert_eq!(
        letters,
        ["A".to_owned(), "B".to_owned()],
        "the headings paint"
    );
    assert_eq!(
        painted_rows(&fixture),
        ["shows-series:s1".to_owned(), "shows-series:s3".to_owned()]
    );
    assert_eq!(
        fixture.attr_of("[data-library-rail-letter=\"B\"]", "disabled"),
        None,
        "the rail's B is enabled by the series row under it"
    );
    assert_eq!(
        fixture.all_attrs(
            "[data-library-shows-row-id=\"shows-series:s1\"] [data-library-shows-thumb]",
            "data-library-shows-thumb"
        ),
        ["loading".to_owned()],
        "the series row paints the shared part's thumb with its art state"
    );
    assert_eq!(
        fixture.all_attrs(
            "[data-library-shows-row-id=\"shows-series:s3\"] [data-library-shows-thumb]",
            "data-library-shows-thumb"
        ),
        ["none".to_owned()],
        "a series row with no art source paints the no-source thumb"
    );
}

/// Contract (gui-delineated-rows design D1; gui-unified-list-rows design D3):
/// a series row is a top-level row and paints the shared part's thumb, while
/// the season row nested under it paints through the nested row part and
/// paints none.
#[test]
fn the_series_row_paints_a_thumb_and_its_nested_season_row_paints_none() {
    let mut fixture = built(shows_frame(tree()), shows_detail(None));
    fixture.measure(300.0);

    toggle_row(&mut fixture, "shows-series:s1");

    assert!(
        fixture
            .presents("[data-library-shows-row-id=\"shows-series:s1\"] [data-library-shows-thumb]"),
        "the top-level series row paints its thumb"
    );
    assert!(
        !fixture.presents(
            "[data-library-shows-row-id=\"shows-season:s1:se1\"] [data-library-shows-thumb]"
        ),
        "the nested season row paints no thumb"
    );
}

/// The New view's flat rows (design D8): one episode row, date-sorted, carrying
/// no rail letter.
fn new_view_rows() -> Vec<GuiShowsRowFrame> {
    vec![GuiShowsRowFrame {
        art: GuiRowArtFrame::Loading,
        media: GuiRowMediaKindFrame::Video,
        id: "new:e1".to_owned(),
        title: "Pilot".to_owned(),
        subtitle: Some("Atlanta".to_owned()),
        date: Some("2026-11-18".to_owned()),
        sort_letter: None,
        display_only: false,
    }]
}

/// Contract (spec "The letter rail jumps a name-sorted list"): a list sorted
/// by date shows no rail, so the New view's flat rows paint without one.
#[test]
fn the_date_sorted_view_paints_no_rail() {
    let frame = GuiShowsList {
        sort: None,
        view: crate::gui_core::GuiShowsView::New,
        rows: new_view_rows(),
        ..shows_frame(Vec::new())
    };
    let mut fixture = built(frame, shows_detail(None));
    fixture.measure(300.0);

    assert_eq!(painted_rows(&fixture), ["new:e1".to_owned()]);
    assert!(
        !fixture.presents("[data-library-region=\"rail\"]"),
        "the date-sorted view paints no rail"
    );
    assert_eq!(
        fixture.all_attrs(
            "[data-library-shows-row-id=\"new:e1\"] [data-library-shows-thumb]",
            "data-library-shows-thumb"
        ),
        ["loading".to_owned()],
        "the flat New row paints the shared part's thumb with its art state"
    );
}

/// Contract (design D14, user override): the Shows header paints no selection
/// label — the UI is not verbose.
#[test]
fn the_shows_header_paints_no_selection_label() {
    let mut fixture = built(shows_frame(tree()), shows_detail(None));
    fixture.measure(300.0);

    assert!(
        !fixture.presents("[data-library-selection-label]"),
        "the Shows header paints no selection label"
    );
}

/// Contract (gui-row-hover-actions design D3, D4): a series and a season name a
/// collection, so their hover offers Play, Add, Shuffle, and the node's Expand,
/// while an episode leaf offers Play and Add alone.
#[test]
fn the_tree_rows_offer_their_hover_controls() {
    let mut fixture = built(shows_frame(tree_with_episodes()), shows_detail(None));
    fixture.measure(300.0);
    toggle_row(&mut fixture, "shows-series:s1");
    toggle_row(&mut fixture, "shows-season:s1:se1");

    assert_eq!(
        fixture.hover_controls_of("shows-series:s1"),
        ["play", "queue", "shuffle", "toggle"],
        "the series row offers Play, Add, Shuffle, and Expand"
    );
    assert_eq!(
        fixture.hover_controls_of("shows-season:s1:se1"),
        ["play", "queue", "shuffle", "toggle"],
        "the season row offers Play, Add, Shuffle, and Expand"
    );
    assert_eq!(
        fixture.hover_controls_of("shows-episode:ep1"),
        ["play", "queue"],
        "the episode row offers Play and Add and no Shuffle"
    );
}

/// Contract (gui-row-hover-actions design D3, D4): a New row is a playable
/// episode, so its hover offers Play and Add and no Shuffle.
#[test]
fn a_new_row_offers_play_and_add() {
    let frame = GuiShowsList {
        sort: None,
        view: crate::gui_core::GuiShowsView::New,
        rows: new_view_rows(),
        ..shows_frame(Vec::new())
    };
    let mut fixture = built(frame, shows_detail(None));
    fixture.measure(300.0);

    assert_eq!(
        fixture.hover_controls_of("new:e1"),
        ["play", "queue"],
        "the New row offers Play and Add"
    );
}

/// Contract (gui-row-hover-actions design D3): an Upcoming row is display-only,
/// so it paints no hover control.
#[test]
fn an_upcoming_row_offers_no_hover_control() {
    let mut row = new_view_rows().remove(0);
    row.display_only = true;
    let frame = GuiShowsList {
        sort: None,
        view: crate::gui_core::GuiShowsView::Upcoming,
        rows: vec![row],
        ..shows_frame(Vec::new())
    };
    let mut fixture = built(frame, shows_detail(None));
    fixture.measure(300.0);

    assert!(
        fixture.hover_controls_of("new:e1").is_empty(),
        "the display-only row paints no hover control"
    );
}

/// Contract: an expandable node paints its open
/// state as `aria-expanded` — `false` while closed, `true` once opened.
#[test]
fn an_expandable_node_paints_its_open_state_as_aria_expanded() {
    let mut fixture = built(shows_frame(tree()), shows_detail(None));
    fixture.measure(300.0);

    assert_eq!(
        fixture.attr_of(
            "[data-library-shows-row-id=\"shows-series:s1\"]",
            "aria-expanded",
        ),
        Some("false".to_owned()),
        "the closed series paints aria-expanded false"
    );

    toggle_row(&mut fixture, "shows-series:s1");

    assert_eq!(
        fixture.attr_of(
            "[data-library-shows-row-id=\"shows-series:s1\"]",
            "aria-expanded",
        ),
        Some("true".to_owned()),
        "the opened series paints aria-expanded true"
    );
}

/// Contract: a node that does not open paints no
/// `aria-expanded` attribute at all.
#[test]
fn a_node_that_does_not_open_paints_no_aria_expanded() {
    let series = GuiShowsSeriesFrame {
        art: GuiRowArtFrame::NoSource,
        media: GuiRowMediaKindFrame::Video,
        id: "s9".to_owned(),
        title: "Fixed".to_owned(),
        sort_letter: 'A',
        expandable: false,
        seasons: Vec::new(),
    };
    let mut fixture = built(shows_frame(vec![series]), shows_detail(None));
    fixture.measure(300.0);

    assert_eq!(
        fixture.attr_of(
            "[data-library-shows-row-id=\"shows-series:s9\"]",
            "aria-expanded",
        ),
        None,
        "the non-expandable node paints no aria-expanded"
    );
}

/// Contract (`LibraryShowsList.test.ts`, gui-tree-double-click design D1): a
/// press on a series node selects it for the Hero and leaves the node closed.
#[test]
fn a_press_on_a_series_node_selects_it_and_leaves_it_closed() {
    let mut fixture = built(shows_frame(tree()), shows_detail(None));
    fixture.measure(300.0);

    press_row(&mut fixture, "shows-series:s1");

    assert_eq!(
        painted_rows(&fixture),
        ["shows-series:s1".to_owned(), "shows-series:s3".to_owned()],
        "the pressed series stays closed and paints no season"
    );
    assert_eq!(
        fixture.library_commands(),
        [crate::gui_core::GuiLibraryCommand::ShowsSelect {
            lib_idx: 0,
            row: "s1".to_owned(),
        }],
        "the press selects the series"
    );
}

/// Contract (gui-tree-double-click design D1): a double-click on a series node
/// opens it, so its seasons paint under it.
#[test]
fn a_toggle_on_a_series_node_opens_it() {
    let mut fixture = built(shows_frame(tree()), shows_detail(None));
    fixture.measure(300.0);

    toggle_row(&mut fixture, "shows-series:s1");

    assert_eq!(
        painted_rows(&fixture),
        [
            "shows-series:s1".to_owned(),
            "shows-season:s1:se1".to_owned(),
            "shows-series:s3".to_owned(),
        ],
        "the toggled series paints open with its season under it"
    );
}

/// Contract (`LibraryShowsList.test.ts`): a press sends the select command
/// with the row id. The press rides the handler function (a headless
/// document cannot fire a DOM click).
#[test]
fn a_press_sends_the_select_command_with_the_row_id() {
    let mut fixture = built(shows_frame(tree()), shows_detail(None));
    fixture.measure(300.0);

    press_row(&mut fixture, "shows-series:s3");

    assert_eq!(
        fixture.library_commands(),
        [crate::gui_core::GuiLibraryCommand::ShowsSelect {
            lib_idx: 0,
            row: "s3".to_owned(),
        }],
        "the select command names the row id"
    );
}

/// Contract (gui-list-header-buttons design D1, spec "Shows New shows no tree
/// buttons"; the requirement is which controls show): the All view's header
/// paints Expand all and Collapse all, and the New view's header paints
/// neither, because its list is flat.
#[test]
fn the_all_view_paints_the_tree_buttons_and_the_new_view_paints_neither() {
    let mut all = built(shows_frame(tree()), shows_detail(None));
    all.measure(300.0);
    assert!(
        all.presents("[data-library-expand-all]"),
        "the All view's header paints Expand all"
    );
    assert!(
        all.presents("[data-library-collapse-all]"),
        "the All view's header paints Collapse all"
    );

    let new = GuiShowsList {
        view: crate::gui_core::GuiShowsView::New,
        rows: new_view_rows(),
        ..shows_frame(Vec::new())
    };
    let mut new = built(new, shows_detail(None));
    new.measure(300.0);
    assert!(
        !new.presents("[data-library-expand-all]"),
        "the New view's header paints no Expand all"
    );
    assert!(
        !new.presents("[data-library-collapse-all]"),
        "the New view's header paints no Collapse all"
    );
}
