//! Headless tests of the Library list (D16): the contracts of
//! `LibraryList.test.ts` — the canvas tree paints over native scroll and
//! only the rows in view, a press on an album picks it and leaves it closed,
//! a double-click on an album without its tracklist opens it and asks the core
//! for the tracks (gui-tree-double-click design D1, D5), a double-click closes
//! and reopens a node while the filter narrows the rows, a double-click on a
//! track plays it, a track press moves the cursor onto the row, a right-press
//! requests the row's item menu, and the header names the carried selection.
//! The IPC of
//! the earlier tests is the mock core link here; the asserted commands are
//! the same neutral shapes. A headless document cannot fire a DOM click,
//! input, or context-menu event, so the press, filter, and menu contracts
//! ride the state methods the component's own handlers call one line each
//! (the top bar's precedent). The row geometry is asserted on the model in
//! `window/library/list/tests.rs`. The core channel is a mock pair; no test
//! opens a window, sleeps, or reaches the core.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, mpsc};

use blitz_dom::{Document as _, local_name};
use blitz_traits::shell::{ColorScheme, Viewport};
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use super::LibraryList;
use crate::bridge::frame::library::{
    GuiArtistKey, GuiCollectionEntry, GuiCollections, GuiDestinationList, GuiLibraryDetail,
    GuiLibraryList, GuiLibraryTabFrame, GuiMusicAlbumFrame, GuiMusicArtistFrame,
    GuiMusicArtistPageFrame, GuiMusicDetail, GuiMusicHeroFrame, GuiMusicHeroSubjectFrame,
    GuiMusicList, GuiMusicTrackFrame, GuiRowArtFrame, GuiRowMediaKindFrame,
};
use crate::bridge::sections::{GuiSection, GuiSectionBatch};
use crate::core_thread::CoreInput;
use crate::window::channel::CoreLink;
use crate::window::clock::{Clock, RecordingClock};
use crate::window::layout::LayoutEpoch;
use crate::window::library::list::LibraryRow;
use crate::window::library::state::LibraryState;
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
    ".items-baseline { align-items: baseline; }",
    ".justify-between { justify-content: space-between; }",
    ".justify-end { justify-content: end; }",
    ".min-w-0 { min-width: 0; }",
    ".min-h-0 { min-height: 0; }",
    ".flex-1 { flex: 1 1 0%; }",
    ".flex-none { flex: none; }",
    ".w-full { width: 100%; }",
    ".block { display: block; }",
    ".relative { position: relative; }",
    ".overflow-hidden { overflow: hidden; }",
    ".truncate { width: 100%; }",
    "[data-library-list] { height: 120px; }",
);

/// The harness's props: the mock core sender and the handle the test reads
/// the store through. The carried content lands on the sections (the list
/// reads the store itself, as the panel's mount does).
#[derive(Clone)]
struct FixtureProps {
    sender: mpsc::Sender<CoreInput>,
    handle: Rc<RefCell<Option<FixtureHandles>>>,
}

/// The root: provides the Library store and the layout epoch, and paints the
/// list, as the panel composes it.
fn list_app(props: FixtureProps) -> Element {
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
        LibraryList {}
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

    /// Resolve the document's style and layout, as the renderer's frame
    /// would, and let the re-measures the epoch bump drives land.
    fn lay_out(&mut self) {
        self.doc.inner.borrow_mut().resolve(0.0);
        let mut epoch = self.epoch;
        self.doc.vdom.in_runtime(move || epoch.bump());
        self.poll_until_idle();
    }

    /// Carry the Music sections, as the core's batch would, so the row model
    /// reads the carried union at the batch's revision.
    fn carry(&mut self, list: &GuiMusicList, detail: &GuiMusicDetail) {
        let batch = GuiSectionBatch {
            library_list: Some(GuiSection {
                revision: 1,
                value: Some(GuiLibraryList {
                    tabs: vec![GuiLibraryTabFrame {
                        position: 1,
                        label: "Music".to_owned(),
                    }],
                    current_tab: Some(1),
                    destination: GuiDestinationList::Music(list.clone()),
                }),
            }),
            library_detail: Some(GuiSection {
                revision: 1,
                value: Some(GuiLibraryDetail::Music(detail.clone())),
            }),
            ..GuiSectionBatch::default()
        };
        let mut sections = self.state.sections.clone();
        self.in_runtime(move || sections.apply_batch(&batch));
        self.lay_out();
    }

    /// Whether the selector names a node in the painted tree.
    fn presents(&self, selector: &str) -> bool {
        matches!(self.doc.inner().query_selector(selector), Ok(Some(_)))
    }

    /// The attribute of the first node the selector names.
    fn attr_of(&self, selector: &str, attribute: blitz_dom::LocalName) -> Option<String> {
        let dom = self.doc.inner();
        let id = dom
            .query_selector(selector)
            .expect("the selector is valid")
            .unwrap_or_else(|| panic!("{selector} paints"));
        dom.get_node(id)
            .expect("the node is in the document")
            .attr(attribute)
            .map(str::to_owned)
    }

    /// The hover controls the host of the row `row_id` offers, in paint order
    /// (gui-row-hover-actions design D3): the actions are the row button's later
    /// sibling in the row's hover host, so the row's own node names them.
    fn hover_controls_of(&self, row_id: &str) -> Vec<String> {
        let dom = self.doc.inner();
        let row = dom
            .query_selector(&format!("[data-library-row-id=\"{row_id}\"]"))
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
                    .attr(blitz_dom::LocalName::from("data-row-action"))
                    .map(str::to_owned)
            })
            .collect()
    }

    /// The `RowsInView` reports the core link received, in send order.
    fn reported_rows_in_view(&mut self) -> Vec<crate::gui_core::GuiLibraryCommand> {
        self.receiver
            .try_iter()
            .filter_map(|input| match input {
                CoreInput::Library(
                    command @ crate::gui_core::GuiLibraryCommand::RowsInView { .. },
                ) => Some(command),
                _ => None,
            })
            .collect()
    }

    /// The Library commands the core link received, in send order.
    fn library_commands(&mut self) -> Vec<crate::gui_core::GuiLibraryCommand> {
        self.receiver
            .try_iter()
            .filter_map(|input| match input {
                CoreInput::Library(crate::gui_core::GuiLibraryCommand::RowsInView { .. }) => None,
                CoreInput::Library(command) => Some(command),
                _ => None,
            })
            .collect()
    }
}

/// Build the document over the carried content, carry the sections, and let
/// the list's first measurement land.
fn built(list: &GuiMusicList, detail: &GuiMusicDetail) -> Fixture {
    let (sender, receiver) = mpsc::channel::<CoreInput>();
    let handle = Rc::new(RefCell::new(None));
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(
            list_app,
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

/// The Beastie Boys artist: one album with a cached tracklist, one without.
fn beastie() -> GuiMusicArtistFrame {
    GuiMusicArtistFrame {
        art: GuiRowArtFrame::Loading,
        media: GuiRowMediaKindFrame::Audio,
        artist_key: GuiArtistKey::Service("a1".to_owned()),
        name: "Beastie Boys".to_owned(),
        sort_letter: 'B',
        albums: vec![
            GuiMusicAlbumFrame {
                album_id: "al1".to_owned(),
                name: "Licensed To Ill".to_owned(),
                year: "1986".to_owned(),
                tracks: Some(vec![GuiMusicTrackFrame {
                    track_id: "t1".to_owned(),
                    name: "Rhymin & Stealin".to_owned(),
                    number: 1,
                    duration: "3:27".to_owned(),
                }]),
                total: None,
            },
            GuiMusicAlbumFrame {
                album_id: "al2".to_owned(),
                name: "Paul’s Boutique".to_owned(),
                year: "1989".to_owned(),
                tracks: None,
                total: None,
            },
        ],
    }
}

/// The Brand Nubian artist: one album, closed under the artist.
fn brand() -> GuiMusicArtistFrame {
    GuiMusicArtistFrame {
        art: GuiRowArtFrame::NoSource,
        media: GuiRowMediaKindFrame::Audio,
        artist_key: GuiArtistKey::Service("a2".to_owned()),
        name: "Brand Nubian".to_owned(),
        sort_letter: 'B',
        albums: vec![GuiMusicAlbumFrame {
            album_id: "al3".to_owned(),
            name: "One For All".to_owned(),
            year: "1990".to_owned(),
            tracks: None,
            total: None,
        }],
    }
}

/// The carried Music content: the two artists, and the checked collections
/// whose one derivation `selected_names` names the header's label (design D6).
fn music(collection_names: &[&str]) -> GuiMusicList {
    GuiMusicList {
        lib_idx: 0,
        collections: GuiCollections {
            target: crate::gui_core::GuiCollectionTarget::Music(0),
            all: collection_names.is_empty(),
            entries: collection_names
                .iter()
                .map(|name| GuiCollectionEntry {
                    key: format!("g-{}", name.to_lowercase()),
                    name: (*name).to_owned(),
                    checked: true,
                })
                .collect(),
            noun: crate::gui_core::GuiCollectionNoun::Groups,
        },
        loading: false,
        failure: None,
        artists: vec![beastie(), brand()],
    }
}

/// The Music detail part with no album picked: the list paints no Hero.
fn no_hero() -> GuiMusicDetail {
    GuiMusicDetail {
        hero: None,
        art: None,
        playing_track_id: None,
    }
}

/// One album of the refresh scenario, carrying its tracklist so opening its
/// node sends no command.
fn loaded_album(album_id: &str, name: &str, year: &str, track_id: &str) -> GuiMusicAlbumFrame {
    GuiMusicAlbumFrame {
        album_id: album_id.to_owned(),
        name: name.to_owned(),
        year: year.to_owned(),
        tracks: Some(vec![GuiMusicTrackFrame {
            track_id: track_id.to_owned(),
            name: "A track".to_owned(),
            number: 1,
            duration: "3:00".to_owned(),
        }]),
        total: None,
    }
}

/// The carried content of the refresh scenario (gui-list-header-buttons design
/// D7): one artist holding three albums whose tracklists are all cached, so a
/// press that opens one sends no command.
fn three_albums() -> GuiMusicList {
    GuiMusicList {
        artists: vec![GuiMusicArtistFrame {
            art: GuiRowArtFrame::NoSource,
            media: GuiRowMediaKindFrame::Audio,
            artist_key: GuiArtistKey::Service("a1".to_owned()),
            name: "Beastie Boys".to_owned(),
            sort_letter: 'B',
            albums: vec![
                loaded_album("al1", "Licensed To Ill", "1986", "t1"),
                loaded_album("al2", "Paul’s Boutique", "1989", "t2"),
                loaded_album("al3", "Check Your Head", "1992", "t3"),
            ],
        }],
        ..music(&["Hip-Hop"])
    }
}

/// Open the Beastie Boys node, so its albums paint.
fn open_beastie(fixture: &mut Fixture) {
    toggle_row(fixture, "artist:service:a1");
}

/// Open the Licensed To Ill album node, so its cached tracks paint.
fn open_licensed_to_ill(fixture: &mut Fixture) {
    toggle_row(fixture, "album:al1");
}

/// Report a `viewport_height` px viewport, so the rows in view are the rows
/// a measured list of that height paints.
fn measure(fixture: &mut Fixture, viewport_height: f32) {
    let state = fixture.state.clone();
    fixture.in_runtime(move || state.measure_library_list(viewport_height));
}

/// The painted row of `id`, as the press and menu handlers take it.
fn row_of(fixture: &mut Fixture, id: &str) -> LibraryRow {
    let id = id.to_owned();
    fixture.compute(move |state| {
        let ui = state.ui.peek().clone();
        state
            .rows_of(&ui)
            .iter()
            .find(|row| row.id() == id.as_str())
            .cloned()
            .unwrap_or_else(|| panic!("{id} paints"))
    })
}

/// Press the row of `id`, as the row's press handler does.
fn press_row(fixture: &mut Fixture, id: &str) {
    let row = row_of(fixture, id);
    let state = fixture.state.clone();
    fixture.in_runtime(move || state.press_library_row(&row));
}

/// Toggle the node of `id`, as a tree parent's double-click handler does.
fn toggle_row(fixture: &mut Fixture, id: &str) {
    let row = row_of(fixture, id);
    let state = fixture.state.clone();
    fixture.in_runtime(move || state.toggle_library_node(&row));
}

/// Contract (gui-tree-double-click design D1, D5): a double-click on an album
/// without its tracklist opens it and asks for the tracks.
#[test]
fn a_double_click_on_an_album_without_its_tracklist_opens_it_and_asks_for_the_tracks() {
    let mut fixture = built(&music(&["Hip-Hop"]), &no_hero());
    open_beastie(&mut fixture);
    measure(&mut fixture, 240.0);

    toggle_row(&mut fixture, "album:al2");

    assert_eq!(
        fixture.library_commands(),
        [crate::gui_core::GuiLibraryCommand::OpenAlbum {
            album_id: "al2".to_owned(),
        }]
    );
    assert_eq!(
        fixture.attr_of(
            "[data-library-row-id=\"album:al2\"]",
            local_name!("aria-expanded"),
        ),
        Some("true".to_owned()),
        "the double-clicked album paints open"
    );
}

/// Contract (gui-tree-double-click design D1, D5): a press on an album is the
/// pick's half only — the Hero takes its detail, and the node stays closed.
#[test]
fn a_press_on_an_album_picks_it_and_leaves_it_closed() {
    let mut fixture = built(&music(&["Hip-Hop"]), &no_hero());
    open_beastie(&mut fixture);
    measure(&mut fixture, 240.0);

    press_row(&mut fixture, "album:al2");

    assert_eq!(
        fixture.library_commands(),
        [crate::gui_core::GuiLibraryCommand::ShowDetail {
            item_id: "al2".to_owned(),
        }]
    );
    assert_eq!(
        fixture.attr_of(
            "[data-library-row-id=\"album:al2\"]",
            local_name!("aria-expanded"),
        ),
        Some("false".to_owned()),
        "the pressed album stays closed"
    );
}

/// Contract (gui-row-hover-actions design D3, D4; gui-tree-double-click design
/// D1, D5): a Music album row is a collection that names its own item, so its
/// hover offers all four controls — Play, Add, Shuffle, and the node's Expand.
#[test]
fn an_album_row_offers_every_hover_control() {
    let mut fixture = built(&music(&["Hip-Hop"]), &no_hero());
    open_beastie(&mut fixture);
    measure(&mut fixture, 240.0);

    assert_eq!(
        fixture.hover_controls_of("album:al1"),
        ["play", "queue", "shuffle", "toggle"],
        "the album row offers Play, Add, Shuffle, and Expand"
    );
}

/// Contract (gui-row-hover-actions design D3, D4): a Music track row is a leaf
/// and no collection, so its hover offers Play and Add alone.
#[test]
fn a_track_row_offers_play_and_add() {
    let mut fixture = built(&music(&["Hip-Hop"]), &no_hero());
    open_beastie(&mut fixture);
    open_licensed_to_ill(&mut fixture);
    measure(&mut fixture, 240.0);

    assert_eq!(
        fixture.hover_controls_of("track:t1"),
        ["play", "queue"],
        "the track row offers Play and Add and no Shuffle"
    );
}

/// Contract (`LibraryList.test.ts`): a right-press on a row requests the
/// item's menu for its id — the artist's item id, an album's own id, a
/// track's id.
#[test]
fn a_right_press_on_a_row_requests_the_items_menu_for_its_id() {
    let mut fixture = built(&music(&["Hip-Hop"]), &no_hero());
    open_beastie(&mut fixture);
    open_licensed_to_ill(&mut fixture);
    measure(&mut fixture, 240.0);

    let row = row_of(&mut fixture, "album:al1");
    let state = fixture.state.clone();
    fixture.in_runtime(move || {
        state.open_row_menu_at(
            LibraryState::action_item_id(&row).map(LibraryState::row_menu_item),
            10.0,
            12.0,
        );
    });

    assert_eq!(
        fixture.library_commands(),
        [crate::gui_core::GuiLibraryCommand::RowMenu {
            row: crate::gui_core::GuiLibraryMenuRow::Item("al1".to_owned()),
        }]
    );
}

/// Contract (`LibraryList.test.ts`): the list paints only the rows in view.
#[test]
fn the_list_paints_only_the_rows_in_view() {
    let mut fixture = built(&music(&["Hip-Hop"]), &no_hero());
    measure(&mut fixture, 40.0);

    assert!(
        fixture.presents("[data-library-row-id=\"artist:service:a1\"]"),
        "the first artist paints in the 40 px window"
    );
    assert!(
        !fixture.presents("[data-library-row-id=\"artist:service:a2\"]"),
        "the second artist sits past the window and paints nothing"
    );
}

/// Contract (`LibraryList.test.ts`): the mounted list reports its measured
/// viewport height, so the rows in view follow the layout (design D9).
#[test]
fn the_mounted_list_reports_its_measured_viewport() {
    let fixture = built(&music(&["Hip-Hop"]), &no_hero());

    assert_eq!(fixture.state.music_scroll.peek().viewport_height, 120.0);
}

/// Contract (`LibraryList.test.ts`): the filter narrows the painted rows to
/// the names that contain its text.
#[test]
fn the_filter_narrows_the_painted_rows_to_the_names_that_contain_its_text() {
    let mut fixture = built(&music(&["Hip-Hop"]), &no_hero());
    measure(&mut fixture, 240.0);

    let state = fixture.state.clone();
    fixture.in_runtime(move || state.set_library_filter("nubian".to_owned()));

    assert!(
        !fixture.presents("[data-library-row-id=\"artist:service:a1\"]"),
        "the artist whose name holds no match paints nothing"
    );
    assert!(
        fixture.presents("[data-library-row-id=\"artist:service:a2\"]"),
        "the artist whose name matches paints"
    );
}

/// Review of row 4.5, finding P2 (`LibraryList.test.ts`): the filter opens a
/// node that holds a match, so a double-click under the filter must still close
/// it — the close half of the design's "a double-click on an artist or album
/// row opens or closes it".
#[test]
fn a_double_click_closes_and_reopens_a_node_while_the_filter_narrows_the_rows() {
    let mut fixture = built(&music(&["Hip-Hop"]), &no_hero());
    measure(&mut fixture, 240.0);
    let state = fixture.state.clone();
    fixture.in_runtime(move || state.set_library_filter("rhymin".to_owned()));

    let album = "[data-library-row-id=\"album:al1\"]";
    assert_eq!(
        fixture.attr_of(album, local_name!("aria-expanded")),
        Some("true".to_owned()),
        "the filter opens the node that holds the match"
    );
    toggle_row(&mut fixture, "album:al1");
    assert_eq!(
        fixture.attr_of(album, local_name!("aria-expanded")),
        Some("false".to_owned()),
        "the double-click closes the node the filter opened"
    );
    toggle_row(&mut fixture, "album:al1");
    assert_eq!(
        fixture.attr_of(album, local_name!("aria-expanded")),
        Some("true".to_owned()),
        "the second double-click reopens the node"
    );
}

/// Press the header's Refresh button, as its callback does.
fn refresh_music(fixture: &mut Fixture) {
    let state = fixture.state.clone();
    fixture.in_runtime(move || state.refresh_music_list());
}

/// Contract (gui-list-header-buttons design D7, spec "A refresh of an open
/// album in view" and "An open album out of view"): the Music refresh names the
/// album ids of the open album rows the painted window holds, so neither the
/// closed album in view nor the open album scrolled out of it names an id.
#[test]
fn a_refresh_of_music_names_only_the_open_album_rows_in_view() {
    let mut fixture = built(&three_albums(), &no_hero());
    open_beastie(&mut fixture);
    toggle_row(&mut fixture, "album:al1");
    toggle_row(&mut fixture, "album:al3");
    // 150 px holds the letter, the artist, al1 with its track, and the closed
    // al2; al3's row starts past the window.
    measure(&mut fixture, 150.0);

    refresh_music(&mut fixture);

    assert_eq!(
        fixture.library_commands(),
        [crate::gui_core::GuiLibraryCommand::Refresh {
            album_ids: vec!["al1".to_owned()],
        }]
    );
}

/// Contract (`LibraryList.test.ts`): a double-click on a track row sends
/// that track play, through the same play helper the Hero's tracklist rows
/// use.
#[test]
fn a_double_click_on_a_track_row_sends_that_track_play() {
    let mut fixture = built(&music(&["Hip-Hop"]), &no_hero());
    open_beastie(&mut fixture);
    open_licensed_to_ill(&mut fixture);
    measure(&mut fixture, 240.0);

    let state = fixture.state.clone();
    fixture.in_runtime(move || state.play_library_track("t1"));

    assert_eq!(
        fixture.library_commands(),
        [crate::gui_core::GuiLibraryCommand::Play {
            item_id: "t1".to_owned(),
        }]
    );
}

/// Contract (gui-delineated-rows design D7): the list reports the artist rows it
/// paints with the core's own artist ids, under the Music scope, so the core
/// queues their row art without a coordinate.
#[test]
fn the_list_reports_the_artist_rows_in_view_under_the_music_scope() {
    let mut fixture = built(&music(&["Hip-Hop"]), &no_hero());
    measure(&mut fixture, 240.0);

    assert_eq!(
        fixture.reported_rows_in_view().last(),
        Some(&crate::gui_core::GuiLibraryCommand::RowsInView {
            scope: crate::gui_core::GuiRowArtScope::Music,
            ids: vec!["artist:a1".to_owned(), "artist:a2".to_owned()],
        }),
        "the report names the painted artist rows under the Music scope"
    );
}

/// Contract (`LibraryList.test.ts`, design D6 of
/// `gui-pointer-target-interaction`): a track press picks the track's album and
/// marks no row — the list keeps no cursor.
#[test]
fn a_press_on_a_track_row_picks_its_album_and_marks_no_row() {
    let mut fixture = built(&music(&["Hip-Hop"]), &no_hero());
    open_beastie(&mut fixture);
    open_licensed_to_ill(&mut fixture);
    measure(&mut fixture, 240.0);

    press_row(&mut fixture, "track:t1");

    assert_eq!(
        fixture.library_commands(),
        [crate::gui_core::GuiLibraryCommand::ShowDetail {
            item_id: "al1".to_owned(),
        }]
    );
    assert!(
        !fixture.presents("[data-library-row-id=\"track:t1\"] [data-library-cursor]"),
        "no row paints a cursor mark"
    );
}

mod artist_row;
