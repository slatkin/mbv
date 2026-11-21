//! Headless tests of the lettered list (D16), one suite over the four
//! destinations (`gui-tech-debt-simplification` design D10): the plain Emby
//! library, Audiobooks, Podcasts, and Feeds paint the same body. Each common
//! contract runs once over the destinations that carry it, and the facts a
//! destination alone owns (Emby's feed-group mode and row menu, Audiobooks'
//! disabled rail letters and pointer markers) stay beside it in their own
//! modules.
//!
//! The IPC of the earlier tests is the mock core link here; a headless
//! document cannot fire a DOM click, so the press, play, jump, and menu
//! contracts ride the state methods the component's own handlers call one line
//! each. The core channel is a mock pair; no test opens a window, sleeps, or
//! reaches the core.

use std::cell::RefCell;
use std::marker::PhantomData;
use std::rc::Rc;
use std::sync::{Arc, mpsc};

use blitz_dom::{Document as _, LocalName};
use blitz_traits::shell::{ColorScheme, Viewport};
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use crate::bridge::frame::library::{
    GuiAudiobooksDetail, GuiAudiobooksList, GuiAudiobooksRowFrame, GuiCollections,
    GuiDestinationList, GuiEmbyDetail, GuiEmbyList, GuiEmbyRowFrame, GuiEmbyRowKindFrame,
    GuiEntryRowFrame, GuiFeedsDetail, GuiFeedsList, GuiHomeRowStateFrame, GuiLibraryDetail,
    GuiLibraryList, GuiPodcastsDetail, GuiPodcastsList, GuiRowArtFrame, GuiRowMediaKindFrame,
};
use crate::core_thread::CoreInput;
use crate::window::channel::CoreLink;
use crate::window::clock::{Clock, RecordingClock};
use crate::window::components::library_lettered_list::LibraryLetteredList;
use crate::window::layout::LayoutEpoch;
use crate::window::library::lettered::{
    AudiobooksList, EmbyList, FeedsList, LetteredDestination, LetteredItemRow, PodcastsList,
};
use crate::window::library::list::LetteredRow;
use crate::window::library::state::LibraryState;
use crate::window::sections::SectionRef;
use crate::window::sections::Sections;

mod audiobooks;
mod emby;

/// The context handles the root hands the test: the Library store and the
/// layout epoch the re-measures read.
type FixtureHandles = (LibraryState, LayoutEpoch);

/// The layout rules the fixture resolves with, in place of the app
/// stylesheet a real window loads (the same rules the panel's harness uses),
/// and the fixed 120 px scroller box each destination's list reports.
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
    "[data-library-emby-list] { height: 120px; }",
    "[data-library-abs-list] { height: 120px; }",
    "[data-library-podcasts-list] { height: 120px; }",
    "[data-library-feeds-list] { height: 120px; }",
);

/// The destination the root paints, because each list is a typed component.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Which {
    Emby,
    Audiobooks,
    Podcasts,
    Feeds,
}

/// The harness's props: the destination to paint, the mock core sender, and
/// the handle the test reads the store through.
#[derive(Clone)]
struct FixtureProps {
    which: Which,
    sender: mpsc::Sender<CoreInput>,
    handle: Rc<RefCell<Option<FixtureHandles>>>,
}

/// The root: provides the Library store and the layout epoch, and paints the
/// destination's list, as the panel composes it.
fn list_app(props: FixtureProps) -> Element {
    let FixtureProps {
        which,
        sender,
        handle,
    } = props;
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
    match which {
        Which::Emby => rsx! {
            LibraryLetteredList::<EmbyList> {}
        },
        Which::Audiobooks => rsx! {
            LibraryLetteredList::<AudiobooksList> {}
        },
        Which::Podcasts => rsx! {
            LibraryLetteredList::<PodcastsList> {}
        },
        Which::Feeds => rsx! {
            LibraryLetteredList::<FeedsList> {}
        },
    }
}

/// The facts one destination's suite reads from the shared harness: the
/// section wrappers and tab the generic list does not name, and the three
/// sample rows' ids, texts, and commands.
trait Host: LetteredDestination {
    const WHICH: Which;
    const TAB: usize;
    const MARK: &'static str;
    const IDS: [&'static str; 3];
    const SELECTED: &'static str;
    const FIRST_SUBTITLE: &'static str;
    const SECOND_DATE: &'static str;

    fn wrap_list(list: Self::List) -> GuiDestinationList;
    fn wrap_detail(detail: Self::Detail) -> GuiLibraryDetail;
    fn rows() -> Vec<Self::Row>;
    fn frame(rows: Vec<Self::Row>, loading: bool, failed: bool) -> Self::List;
    fn detail_of(selected: Option<&str>) -> Self::Detail;
    fn select_command(id: &str) -> mbv::GuiLibraryCommand;
    fn play_command(id: &str) -> mbv::GuiLibraryCommand;
}

struct Fixture<D: Host> {
    doc: DioxusDocument,
    state: LibraryState,
    epoch: LayoutEpoch,
    receiver: mpsc::Receiver<CoreInput>,
    host: PhantomData<D>,
}

impl<D: Host> Fixture<D> {
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

    /// Carry the destination's sections, as the core's batch would, so the
    /// row model reads the carried frame at the batch's revision.
    fn carry(&mut self, list: D::List, detail: D::Detail) {
        let mut sections = self.state.sections.clone();
        self.in_runtime(move || {
            sections
                .library_list
                .set(Some(SectionRef::new(GuiLibraryList {
                    tabs: Vec::new(),
                    current_tab: Some(D::TAB),
                    destination: D::wrap_list(list),
                })));
            sections
                .library_detail
                .set(Some(SectionRef::new(D::wrap_detail(detail))));
        });
        self.lay_out();
    }

    /// Report a `viewport_height` px viewport, so the rows in view are the
    /// rows a measured list of that height paints: an unmeasured viewport
    /// paints nothing, as the Music list does.
    fn measure(&mut self, viewport_height: f32) {
        let state = self.state.clone();
        self.in_runtime(move || state.measure_lettered_list::<D>(viewport_height));
    }

    /// Jump to a rail letter, as the rail's press handler does.
    fn jump(&mut self, letter: char) {
        let state = self.state.clone();
        self.in_runtime(move || state.jump_lettered_to_letter::<D>(letter));
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

    /// The painted rows' ids, in paint order.
    fn painted_rows(&self) -> Vec<String> {
        let selector = format!("[data-library-{}-row-id]", D::ATTR);
        self.all_attrs(&selector, &format!("data-library-{}-row-id", D::ATTR))
    }

    /// The painted row of `id`, as the press handler takes it.
    fn row_of(&mut self, id: &str) -> LetteredRow<LetteredItemRow<D::Row>> {
        let id = id.to_owned();
        self.compute(move |state| {
            D::painted_rows(state)
                .iter()
                .find(|row| matches!(row, LetteredRow::Item(item) if item.id == id))
                .cloned()
                .expect("the row paints")
        })
    }

    /// Press the row of `id`, as the row's press handler does.
    fn press_row(&mut self, id: &str) {
        let row = self.row_of(id);
        let state = self.state.clone();
        self.in_runtime(move || D::select(&state, &row));
    }

    /// Play the item `id` names, as the double-click handler does.
    fn play_row(&mut self, id: &str) {
        let state = self.state.clone();
        let id = id.to_owned();
        self.in_runtime(move || D::play(&state, &id));
    }
}

/// The selector one painted row carries.
fn row_selector<D: Host>(id: &str) -> String {
    format!("[data-library-{}-row-id=\"{id}\"]", D::ATTR)
}

/// The attribute selector one row's part paints under.
fn part_selector<D: Host>(id: &str, part: &str) -> String {
    format!(
        "{} [data-library-{}-{part}]",
        row_selector::<D>(id),
        D::ATTR
    )
}

/// Build the document over the destination, carry the sections, and let the
/// list's first measurement land.
fn built<D: Host>(list: D::List, detail: D::Detail) -> Fixture<D> {
    let (sender, receiver) = mpsc::channel::<CoreInput>();
    let handle = Rc::new(RefCell::new(None));
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(
            list_app,
            FixtureProps {
                which: D::WHICH,
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
        host: PhantomData,
    };
    fixture.poll_until_idle();
    fixture.carry(list, detail);
    fixture
}

/// The Emby plain mode's sample rows: two `A` movies and one `B` movie.
fn emby_movies() -> Vec<GuiEmbyRowFrame> {
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

/// The Emby list part in one mode over `rows` (row 8.1).
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

/// The Emby detail part: `selected` picked, and no Hero painted here.
fn emby_detail(selected: Option<&str>) -> GuiEmbyDetail {
    GuiEmbyDetail {
        selected: selected.map(str::to_owned),
        hero: None,
    }
}

/// The book catalog's rows: two books under `A` and one under `B`.
fn audiobooks_catalog() -> Vec<GuiAudiobooksRowFrame> {
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

/// The Audiobooks list part over `rows` (row 9.1).
fn audiobooks_frame(rows: Vec<GuiAudiobooksRowFrame>) -> GuiAudiobooksList {
    GuiAudiobooksList {
        sort: None,
        lib_idx: 4,
        loading: false,
        failed: false,
        rows,
    }
}

/// The Audiobooks detail part with `selected` picked and no Hero (row 9.1).
fn audiobooks_detail(selected: Option<&str>) -> GuiAudiobooksDetail {
    GuiAudiobooksDetail {
        workspace_loading: false,
        selected: selected.map(str::to_owned),
        playing_book_id: None,
        hero: None,
    }
}

/// The flat destinations' sample rows: two dated entries, one played, and one
/// partly played with its resume percent.
fn entry_rows(media: GuiRowMediaKindFrame, subtitles: [&str; 3]) -> Vec<GuiEntryRowFrame> {
    vec![
        GuiEntryRowFrame {
            sort_letter: None,
            art: GuiRowArtFrame::Loading,
            media,
            id: "e1:ep1".to_owned(),
            title: "Episode A".to_owned(),
            subtitle: Some(subtitles[0].to_owned()),
            date: Some("12 May".to_owned()),
            state: GuiHomeRowStateFrame::Ordinary,
        },
        GuiEntryRowFrame {
            sort_letter: None,
            art: GuiRowArtFrame::NoSource,
            media,
            id: "e2:ep2".to_owned(),
            title: "Episode B".to_owned(),
            subtitle: Some(subtitles[1].to_owned()),
            date: Some("9 May".to_owned()),
            state: GuiHomeRowStateFrame::Played,
        },
        GuiEntryRowFrame {
            sort_letter: None,
            art: GuiRowArtFrame::NoSource,
            media,
            id: "e3:ep3".to_owned(),
            title: "Episode C".to_owned(),
            subtitle: Some(subtitles[2].to_owned()),
            date: Some("4 May".to_owned()),
            state: GuiHomeRowStateFrame::Active { progress: Some(40) },
        },
    ]
}

/// The Podcasts list part over `rows`, with its load facts as the caller sets
/// them (design D12).
fn podcasts_frame(rows: Vec<GuiEntryRowFrame>, loading: bool, failed: bool) -> GuiPodcastsList {
    GuiPodcastsList {
        sort: None,
        lib_idx: 0,
        collections: GuiCollections {
            target: mbv::GuiCollectionTarget::Podcasts(0),
            all: true,
            entries: Vec::new(),
            noun: mbv::GuiCollectionNoun::Shows,
        },
        loading,
        failed,
        rows,
    }
}

/// The Podcasts detail part: `selected` picked, and no Hero.
fn podcasts_detail(selected: Option<&str>) -> GuiPodcastsDetail {
    GuiPodcastsDetail {
        selected: selected.map(str::to_owned),
        hero: None,
    }
}

/// The Feeds list part over `rows`, with its load facts as the caller sets
/// them (design D12).
fn feeds_frame(rows: Vec<GuiEntryRowFrame>, loading: bool, failed: bool) -> GuiFeedsList {
    GuiFeedsList {
        sort: None,
        collections: GuiCollections {
            target: mbv::GuiCollectionTarget::Feeds,
            all: true,
            entries: Vec::new(),
            noun: mbv::GuiCollectionNoun::Subscriptions,
        },
        loading,
        failed,
        platform: None,
        rows,
    }
}

/// The Feeds detail part: `selected` picked, and no Hero.
fn feeds_detail(selected: Option<&str>) -> GuiFeedsDetail {
    GuiFeedsDetail {
        selected: selected.map(str::to_owned),
        hero: None,
    }
}

impl Host for EmbyList {
    const WHICH: Which = Which::Emby;
    // The plain Emby library's harness data (design D4).
    const TAB: usize = 3;
    const MARK: &'static str = "emby";
    const IDS: [&'static str; 3] = ["m1", "m2", "m3"];
    const SELECTED: &'static str = "m1";
    const FIRST_SUBTITLE: &'static str = "1979";
    const SECOND_DATE: &'static str = "1998";

    fn wrap_list(list: Self::List) -> GuiDestinationList {
        GuiDestinationList::Emby(list)
    }

    fn wrap_detail(detail: Self::Detail) -> GuiLibraryDetail {
        GuiLibraryDetail::Emby(detail)
    }

    fn rows() -> Vec<Self::Row> {
        emby_movies()
    }

    fn frame(rows: Vec<Self::Row>, loading: bool, failed: bool) -> Self::List {
        GuiEmbyList {
            loading,
            failed,
            ..emby_frame(false, rows)
        }
    }

    fn detail_of(selected: Option<&str>) -> Self::Detail {
        emby_detail(selected)
    }

    fn select_command(id: &str) -> mbv::GuiLibraryCommand {
        mbv::GuiLibraryCommand::EmbySelect {
            lib_idx: 3,
            row: id.to_owned(),
        }
    }

    fn play_command(id: &str) -> mbv::GuiLibraryCommand {
        mbv::GuiLibraryCommand::EmbyPlay {
            lib_idx: 3,
            item_id: id.to_owned(),
        }
    }
}

impl Host for AudiobooksList {
    const WHICH: Which = Which::Audiobooks;
    const TAB: usize = 4;
    const MARK: &'static str = "audiobookshelf";
    const IDS: [&'static str; 3] = ["b1", "b2", "b3"];
    const SELECTED: &'static str = "b3";
    const FIRST_SUBTITLE: &'static str = "Bill Wyman";
    const SECOND_DATE: &'static str = "9 May";

    fn wrap_list(list: Self::List) -> GuiDestinationList {
        GuiDestinationList::Audiobooks(list)
    }

    fn wrap_detail(detail: Self::Detail) -> GuiLibraryDetail {
        GuiLibraryDetail::Audiobooks(detail)
    }

    fn rows() -> Vec<Self::Row> {
        audiobooks_catalog()
    }

    fn frame(rows: Vec<Self::Row>, loading: bool, failed: bool) -> Self::List {
        GuiAudiobooksList {
            loading,
            failed,
            ..audiobooks_frame(rows)
        }
    }

    fn detail_of(selected: Option<&str>) -> Self::Detail {
        audiobooks_detail(selected)
    }

    fn select_command(id: &str) -> mbv::GuiLibraryCommand {
        mbv::GuiLibraryCommand::AbsSelect {
            lib_idx: 4,
            row: id.to_owned(),
        }
    }

    fn play_command(id: &str) -> mbv::GuiLibraryCommand {
        mbv::GuiLibraryCommand::AbsPlay {
            lib_idx: 4,
            item_id: id.to_owned(),
        }
    }
}

impl Host for PodcastsList {
    const WHICH: Which = Which::Podcasts;
    const TAB: usize = 2;
    const MARK: &'static str = "audiobookshelf";
    const IDS: [&'static str; 3] = ["e1:ep1", "e2:ep2", "e3:ep3"];
    const SELECTED: &'static str = "e1:ep1";
    const FIRST_SUBTITLE: &'static str = "Show One";
    const SECOND_DATE: &'static str = "9 May";

    fn wrap_list(list: Self::List) -> GuiDestinationList {
        GuiDestinationList::Podcasts(list)
    }

    fn wrap_detail(detail: Self::Detail) -> GuiLibraryDetail {
        GuiLibraryDetail::Podcasts(detail)
    }

    fn rows() -> Vec<Self::Row> {
        entry_rows(
            GuiRowMediaKindFrame::Audio,
            ["Show One", "Show Two", "Show One"],
        )
    }

    fn frame(rows: Vec<Self::Row>, loading: bool, failed: bool) -> Self::List {
        podcasts_frame(rows, loading, failed)
    }

    fn detail_of(selected: Option<&str>) -> Self::Detail {
        podcasts_detail(selected)
    }

    fn select_command(id: &str) -> mbv::GuiLibraryCommand {
        mbv::GuiLibraryCommand::PodcastsSelect {
            lib_idx: 0,
            row: id.to_owned(),
        }
    }

    fn play_command(id: &str) -> mbv::GuiLibraryCommand {
        mbv::GuiLibraryCommand::PodcastsPlay {
            lib_idx: 0,
            item_id: id.to_owned(),
        }
    }
}

impl Host for FeedsList {
    const WHICH: Which = Which::Feeds;
    const TAB: usize = 2;
    const MARK: &'static str = "rss";
    const IDS: [&'static str; 3] = ["s1::e1", "s2::e2", "s1::e3"];
    const SELECTED: &'static str = "s1::e1";
    const FIRST_SUBTITLE: &'static str = "Sub One";
    const SECOND_DATE: &'static str = "9 May";

    fn wrap_list(list: Self::List) -> GuiDestinationList {
        GuiDestinationList::Feeds(list)
    }

    fn wrap_detail(detail: Self::Detail) -> GuiLibraryDetail {
        GuiLibraryDetail::Feeds(detail)
    }

    fn rows() -> Vec<Self::Row> {
        let mut rows = entry_rows(
            GuiRowMediaKindFrame::Video,
            ["Sub One", "Sub Two", "Sub One"],
        );
        rows[0].id = "s1::e1".to_owned();
        rows[1].id = "s2::e2".to_owned();
        rows[2].id = "s1::e3".to_owned();
        rows
    }

    fn frame(rows: Vec<Self::Row>, loading: bool, failed: bool) -> Self::List {
        feeds_frame(rows, loading, failed)
    }

    fn detail_of(selected: Option<&str>) -> Self::Detail {
        feeds_detail(selected)
    }

    fn select_command(id: &str) -> mbv::GuiLibraryCommand {
        mbv::GuiLibraryCommand::FeedsSelect { row: id.to_owned() }
    }

    fn play_command(id: &str) -> mbv::GuiLibraryCommand {
        mbv::GuiLibraryCommand::FeedsPlay {
            item_id: id.to_owned(),
        }
    }
}

mod common;
