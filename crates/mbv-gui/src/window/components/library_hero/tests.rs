//! Headless tests of the Music Hero (D16): the contracts of
//! `LibraryHero.test.ts` — the album's Play, Add to queue, and
//! Shuffle artist sends name the item's own stable id, the More actions menu
//! paints the entries the frame carries under their shared-builder indexes
//! and a chosen entry sends its index, an album whose menu remainder is empty
//! paints no More actions control, the tracklist paints the cursor on its
//! first track before any key moves it, a track's play gestures send that
//! track's id, the playing row marks the play glyph and keeps its own fill,
//! and the album's block pills type their count and their time at the repeated
//! tracklist durations. The track row's Add-to-queue send rides
//! the tracklist's action closure, which a headless document cannot fire; the
//! command it builds is the same shape the album's
//! Add-to-queue test asserts, and the track id it carries is the row's own
//! data the press contract (`play_library_track`) asserts. The IPC of
//! the earlier tests is the mock core link here; the asserted commands are the
//! same neutral shapes. A headless document cannot fire a DOM click, so the
//! press contracts ride the handler functions the component's own handlers
//! call one line each. The core channel is a mock pair; no test opens a
//! window, sleeps, or reaches the core.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, mpsc};

use blitz_dom::{Document as _, LocalName};
use blitz_traits::shell::{ColorScheme, Viewport};
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use super::LibraryHero;
use super::{
    add_album_to_queue, add_artist_to_queue, play_album, play_artist, press_menu_entry,
    run_menu_entry, shuffle_artist,
};
use crate::bridge::frame::library::{
    GuiArtistKey, GuiCollections, GuiDestinationList, GuiLibraryDetail, GuiLibraryList,
    GuiMusicAlbumFrame, GuiMusicArtistFrame, GuiMusicArtistPageFrame, GuiMusicDetail,
    GuiMusicHeroFrame, GuiMusicHeroSubjectFrame, GuiMusicList, GuiMusicMenuEntryFrame,
    GuiMusicTrackFrame, GuiMusicTrackTotalFrame, GuiRowArtFrame, GuiRowMediaKindFrame,
};
use crate::core_thread::CoreInput;
use crate::window::channel::CoreLink;
use crate::window::clock::FiredTimers;
use crate::window::clock::{Clock, RecordingClock};
use crate::window::components::art_image::ArtEvents;
use crate::window::layout::LayoutEpoch;
use crate::window::library::state::LibraryState;
use crate::window::sections::{SectionRef, Sections};

/// The layout rules the fixture resolves with, in place of the app
/// stylesheet a real window loads: the handful of utilities the Hero tree
/// and its menu card size their boxes with (the same rules the panel's
/// harness uses), so the menu's `Truncate` measures real widths.
const FIXTURE_CSS: &str = concat!(
    "[data-slot] { width: 100%; }",
    ".flex { display: flex; }",
    ".flex-col { flex-direction: column; }",
    ".items-center { align-items: center; }",
    ".items-start { align-items: start; }",
    ".items-baseline { align-items: baseline; }",
    ".justify-between { justify-content: space-between; }",
    ".justify-center { justify-content: center; }",
    ".text-left { text-align: left; }",
    ".min-w-0 { min-width: 0; }",
    ".flex-1 { flex: 1 1 0%; }",
    ".flex-none { flex: none; }",
    ".shrink-0 { flex-shrink: 0; }",
    ".w-full { width: 100%; }",
    ".h-full { height: 100%; }",
    ".block { display: block; }",
    ".relative { position: relative; }",
    ".absolute { position: absolute; }",
    ".fixed { position: fixed; }",
    ".overflow-hidden { overflow: hidden; }",
    ".truncate { width: 100%; }",
);

/// The harness's props: the carried Music content, the mock core sender, and
/// the handle the test reads the store through.
#[derive(Clone)]
struct FixtureProps {
    sender: mpsc::Sender<CoreInput>,
    artists: Vec<GuiMusicArtistFrame>,
    detail: SectionRef<GuiLibraryDetail>,
    handle: Rc<RefCell<Option<FixtureHandles>>>,
}

/// The root: provides the Library store and paints the Hero, as the panel
/// composes it.
fn hero_app(props: FixtureProps) -> Element {
    let FixtureProps {
        sender,
        artists,
        detail,
        handle,
    } = props;
    // The art painter's contexts (the art painter consumes them), over the
    // clock the store holds. The stores hook once, as the art painter's own
    // harness holds them.
    let events = use_hook(ArtEvents::new);
    let fired = use_hook(FiredTimers::new);
    let clock: Arc<dyn Clock> = Arc::new(RecordingClock::new());
    provide_context(events);
    provide_context(fired);
    let harness_clock = Arc::clone(&clock);
    let state = use_hook(move || {
        // The playhead position the interaction state reads: none — the
        // tests hold it at 0.
        let playhead_position = Memo::new(|| 0.0);
        LibraryState::new(
            Sections::new(),
            CoreLink::new(sender),
            playhead_position,
            harness_clock,
        )
    });
    provide_context(clock);
    let epoch = use_hook(LayoutEpoch::new);
    provide_context(epoch);
    // The carried Music content, as the core's batch would: the union the
    // shuffle reads, under the Music destination's own list.
    let mut sections = state.sections.clone();
    sections
        .library_list
        .set(Some(SectionRef::new(GuiLibraryList {
            refreshing: false,
            tabs: Vec::new(),
            current_tab: None,
            destination: GuiDestinationList::Music(GuiMusicList {
                lib_idx: 0,
                collections: GuiCollections {
                    target: crate::gui_core::GuiCollectionTarget::Music(0),
                    all: false,
                    entries: vec![crate::gui_core::GuiCollectionEntry {
                        key: "g1".to_owned(),
                        name: "Hip-Hop".to_owned(),
                        checked: true,
                    }],
                    noun: crate::gui_core::GuiCollectionNoun::Groups,
                },
                loading: false,
                failure: None,
                artists,
            }),
        })));
    // The carried detail, as the core's batch would: `play_library_track`
    // reads the subject from the section store, not from the Hero's props.
    sections.library_detail.set(Some(detail.clone()));
    provide_context(state.clone());
    handle.borrow_mut().replace((state, epoch));
    rsx! {
        LibraryHero { detail }
    }
}

/// The picked album's Hero facts (the former client `hero()` fixture): two union
/// albums behind it, a cached tracklist, and the item menu the core built for
/// the album (design D8).
fn hero() -> GuiMusicHeroFrame {
    GuiMusicHeroFrame {
        artist: "Beastie Boys".to_owned(),
        collections: vec!["Hip-Hop".to_owned()],
        album_count: 2,
        artist_tracks: Some(GuiMusicTrackTotalFrame {
            count: 2,
            duration: "7:02".to_owned(),
        }),
        subject: GuiMusicHeroSubjectFrame::Album(GuiMusicAlbumFrame {
            album_id: "al1".to_owned(),
            name: "Licensed To Ill".to_owned(),
            year: "1986".to_owned(),
            tracks: Some(vec![
                GuiMusicTrackFrame {
                    track_id: "t1".to_owned(),
                    name: "Rhymin & Stealin".to_owned(),
                    number: 1,
                    duration: "4:08".to_owned(),
                },
                GuiMusicTrackFrame {
                    track_id: "t2".to_owned(),
                    name: "The New Style".to_owned(),
                    number: 2,
                    duration: "4:35".to_owned(),
                },
            ]),
            total: Some(GuiMusicTrackTotalFrame {
                count: 2,
                duration: "8:43".to_owned(),
            }),
        }),
        menu: vec![
            GuiMusicMenuEntryFrame {
                label: "Play All".to_owned(),
                key_hint: Some("Enter".to_owned()),
                index: 0,
            },
            GuiMusicMenuEntryFrame {
                label: "Shuffle".to_owned(),
                key_hint: None,
                index: 1,
            },
            GuiMusicMenuEntryFrame {
                label: "Add to Queue".to_owned(),
                key_hint: None,
                index: 2,
            },
        ],
    }
}

/// The carried Music content (the former client `music(facts)` fixture): the artist
/// that owns the picked album, whose union albums Shuffle artist names.
fn artists(facts: &GuiMusicHeroFrame) -> Vec<GuiMusicArtistFrame> {
    vec![GuiMusicArtistFrame {
        art: GuiRowArtFrame::NoSource,
        media: GuiRowMediaKindFrame::Audio,
        artist_key: GuiArtistKey::Service("a1".to_owned()),
        name: "Beastie Boys".to_owned(),
        sort_letter: 'B',
        albums: vec![
            facts
                .album()
                .expect("the fixture's subject is an album")
                .clone(),
            pauls_boutique(),
        ],
    }]
}

/// The union's second album.
fn pauls_boutique() -> GuiMusicAlbumFrame {
    GuiMusicAlbumFrame {
        album_id: "al2".to_owned(),
        name: "Paul’s Boutique".to_owned(),
        year: "1989".to_owned(),
        tracks: None,
        total: None,
    }
}

/// The Music detail part: the picked album's Hero, and the track playing at
/// the playhead (the former client `musicDetail(facts, playingTrackId)` fixture).
fn detail(facts: GuiMusicHeroFrame, playing_track_id: Option<String>) -> GuiMusicDetail {
    GuiMusicDetail {
        hero: Some(facts),
        art: None,
        playing_track_id,
    }
}

/// The context handles the root hands the test: the Library store and the
/// layout epoch the re-measures read.
type FixtureHandles = (LibraryState, LayoutEpoch);

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

    /// Whether the selector names a node in the painted tree.
    fn presents(&self, selector: &str) -> bool {
        matches!(self.doc.inner().query_selector(selector), Ok(Some(_)))
    }

    /// The count of nodes the selector names, in paint order.
    fn count_of(&self, selector: &str) -> usize {
        self.doc
            .inner()
            .query_selector_all(selector)
            .expect("the selector is valid")
            .len()
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

    /// The style attribute of the first node the selector names.
    fn style_of(&self, selector: &str) -> String {
        self.attr_of(selector, "style").unwrap_or_default()
    }

    /// The `data-menu-entry` value of every painted menu entry, in paint
    /// order.
    fn menu_entries(&self) -> Vec<(String, String)> {
        let dom = self.doc.inner();
        dom.query_selector_all("[data-row-menu] [data-menu-entry]")
            .expect("the selector is valid")
            .into_iter()
            .filter_map(|id| {
                let node = dom.get_node(id).expect("the node is in the document");
                let label = node.text_content().trim().to_owned();
                let index = node.attr(LocalName::from("data-menu-entry"))?.to_owned();
                Some((label, index))
            })
            .collect()
    }

    /// The Library commands the core link received, in send order.
    fn library_commands(&mut self) -> Vec<crate::gui_core::GuiLibraryCommand> {
        self.receiver
            .try_iter()
            .filter_map(|input| match input {
                CoreInput::Library(command) => Some(command),
                _ => None,
            })
            .collect()
    }
}

/// Build the document over the carried content and let the first render and
/// effects land.
fn built(artists: Vec<GuiMusicArtistFrame>, detail: GuiMusicDetail) -> Fixture {
    let (sender, receiver) = mpsc::channel::<CoreInput>();
    let handle = Rc::new(RefCell::new(None));
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(
            hero_app,
            FixtureProps {
                sender,
                artists,
                detail: SectionRef::new(GuiLibraryDetail::Music(detail)),
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
    fixture.lay_out();
    fixture
}

/// The Hero mounted for the picked album.
fn built_hero() -> Fixture {
    let facts = hero();
    built(artists(&facts), detail(facts, None))
}

/// Contract (user direction 2026-11-26, spec "Every Hero is one frame with
/// the largest unit as its title"): the album's track list paints bare under
/// the frame, so no list section header and no label rule paints above it.
#[test]
fn the_album_hero_paints_its_track_list_without_a_section_header() {
    let fixture = built_hero();

    assert!(
        fixture.presents("[data-library-tracklist]"),
        "the album's track list paints"
    );
    assert!(
        !fixture.presents("[data-library-hero-list]"),
        "the track list paints no section header"
    );
}

/// Contract (`LibraryHero.test.ts`): Play sends the picked album's id.
#[test]
fn play_sends_the_picked_album_s_id() {
    let mut fixture = built_hero();

    let album_id = "al1".to_owned();
    let state = fixture.state.clone();
    fixture.in_runtime(move || play_album(&state, &album_id));

    assert_eq!(
        fixture.library_commands(),
        vec![crate::gui_core::GuiLibraryCommand::Play {
            item_id: "al1".to_owned(),
        }],
        "the play command names the picked album"
    );
}

/// Contract (`LibraryHero.test.ts`): Add to queue sends the picked album's
/// id.
#[test]
fn add_to_queue_sends_the_picked_album_s_id() {
    let mut fixture = built_hero();

    let album_id = "al1".to_owned();
    let state = fixture.state.clone();
    fixture.in_runtime(move || add_album_to_queue(&state, &album_id));

    assert_eq!(
        fixture.library_commands(),
        vec![crate::gui_core::GuiLibraryCommand::AddToQueue {
            item_id: "al1".to_owned(),
        }],
        "the add-to-queue command names the picked album"
    );
}

/// Contract (`LibraryHero.test.ts`): Shuffle artist sends the artist's albums
/// in the union.
#[test]
fn shuffle_artist_sends_the_union_s_albums() {
    let mut fixture = built_hero();
    let facts = hero();

    let union = artists(&facts);
    let state = fixture.state.clone();
    fixture.in_runtime(move || shuffle_artist(&state, &union, &facts));

    assert_eq!(
        fixture.library_commands(),
        vec![crate::gui_core::GuiLibraryCommand::ShuffleArtist {
            album_ids: vec!["al1".to_owned(), "al2".to_owned()],
        }],
        "the shuffle command names the union's albums"
    );
}

/// Contract (`LibraryHero.test.ts`): a menu entry sends its index for the
/// picked item.
#[test]
fn a_menu_entry_sends_its_index_for_the_picked_item() {
    let mut fixture = built_hero();

    let item_id = "al1".to_owned();
    let state = fixture.state.clone();
    fixture.in_runtime(move || run_menu_entry(&state, &item_id, 1));

    assert_eq!(
        fixture.library_commands(),
        vec![crate::gui_core::GuiLibraryCommand::MenuEntry {
            row: crate::gui_core::GuiLibraryMenuRow::Item("al1".to_owned()),
            index: 1,
        }],
        "the menu-entry command names the picked item and the entry's index"
    );
}

/// Contract (the review-of-7.7 finding P1): the press handler the `on_select`
/// closure calls one line each forwards the received index verbatim — past
/// the filtered menu's end too, so an entry the dropped-duplicates rule
/// re-numbered still runs. The removed lookup indexed the filtered list and
/// dropped the press (or ran the wrong entry); the handler pins the
/// pass-through at the seam a headless document can reach.
#[test]
fn a_menu_press_sends_the_shared_builder_index_past_the_filtered_lists_end() {
    let mut fixture = built_hero();

    let item_id = "al1".to_owned();
    let state = fixture.state.clone();
    fixture.in_runtime(move || press_menu_entry(&state, &item_id, 3));

    assert_eq!(
        fixture.library_commands(),
        vec![crate::gui_core::GuiLibraryCommand::MenuEntry {
            row: crate::gui_core::GuiLibraryMenuRow::Item("al1".to_owned()),
            index: 3,
        }],
        "the index passes through as received, past the filtered list's end"
    );
}

/// Contract: the artwork box's placeholder names the
/// missing art while the frame carries none.
#[test]
fn the_artwork_box_names_its_placeholder_while_the_frame_carries_no_art() {
    let fixture = built_hero();

    assert_eq!(
        fixture.text_of("[data-library-hero-art]"),
        "[Album art · Emby Primary]",
        "the artwork box names the missing art"
    );
}

/// Contract (`LibraryHero.test.ts`): More actions paints the item menu's
/// entries from the frame. The Hero's own buttons already send Play All and
/// Add to Queue, so the dropped-duplicates rule leaves Shuffle the only
/// painted entry, under its shared-builder index. The press that opens the
/// menu is the interaction state's own open, driven here (a headless
/// document cannot fire a DOM click).
#[test]
fn more_actions_paints_the_item_menu_s_entries_from_the_frame() {
    let mut fixture = built_hero();

    let state = fixture.state.clone();
    fixture.in_runtime(move || state.open_library_item_menu());
    // The menu card's laid-out widths: the entry's `Truncate` measures
    // against them.
    fixture.lay_out();

    assert_eq!(
        fixture.menu_entries(),
        vec![("Shuffle".to_owned(), "1".to_owned())],
        "the dropped-duplicates rule leaves Shuffle under its shared-builder index"
    );
}

/// Contract (`LibraryHero.test.ts`): the frame carries no entries when every
/// entry duplicates a Hero button (the dropped-duplicates rule) — the menu
/// would add nothing, so no More actions control paints at all.
#[test]
fn an_empty_hero_menu_paints_no_more_actions_control() {
    let facts = GuiMusicHeroFrame {
        menu: Vec::new(),
        ..hero()
    };
    let fixture = built(artists(&facts), detail(facts, None));

    assert!(
        !fixture.presents("[data-library-hero-more]"),
        "an album whose menu left empty paints no control at all"
    );
}

/// Contract (`LibraryHero.test.ts`): a double-click on a track name sends
/// that track play, and so does the row's play control — the two gestures
/// ride the same play path the tracklist's handlers call
/// (`LibraryState::play_library_track`, one line each). A headless document
/// fires neither gesture.
#[test]
fn a_track_s_play_gesture_sends_that_track_play() {
    let mut fixture = built_hero();

    let track_id = "t2".to_owned();
    let state = fixture.state.clone();
    fixture.in_runtime(move || state.play_library_track(&track_id));

    assert_eq!(
        fixture.library_commands(),
        vec![crate::gui_core::GuiLibraryCommand::Play {
            item_id: "t2".to_owned(),
        }],
        "the play command names the played track"
    );
}

/// Contract (`LibraryHero.test.ts`): the tracklist marks the frame playing
/// track with the bars mark in its number cell, not a fill — a highlight marks
/// focus only, so the playing row keeps its own fill.
#[test]
fn the_playing_track_marks_the_bars_mark_and_keeps_its_own_fill() {
    let facts = hero();
    let fixture = built(artists(&facts), detail(facts, Some("t1".to_owned())));

    assert_eq!(
        fixture.attr_of("[data-library-track=\"t1\"]", "data-library-track-playing"),
        Some(String::new()),
        "the playing row carries the playing mark"
    );
    assert!(
        fixture.presents("[data-library-track=\"t1\"] [data-library-track-number] svg"),
        "the playing row paints the bars mark in its number cell"
    );
    assert!(
        fixture
            .attr_of("[data-library-track=\"t2\"]", "data-library-track-playing")
            .is_none(),
        "no other row carries it"
    );
    let row_style = fixture.style_of("[data-library-track=\"t1\"]");
    assert!(
        !row_style.contains("var(--canvas-accent)"),
        "the playing row keeps its own fill, painted: {row_style}"
    );
}

/// Contract (spec "The Hero paints a picked album"): the album's header
/// carries no artist facts, so it paints no fact pills.
#[test]
fn the_album_header_carries_no_fact_pills() {
    let fixture = built_hero();

    assert!(
        !fixture.presents("[data-library-hero-pills]"),
        "the album header paints no fact pills"
    );
}

/// Contract (spec "The Hero paints a picked album", add-gui-settings-screen
/// design D14): the album block shows its track count and its total time as two
/// fact pills, and each pill's value paints its own type's token.
#[test]
fn the_album_block_pills_type_their_count_and_their_time() {
    let fixture = built_hero();

    assert_eq!(
        fixture.count_of("[data-library-hero-block-pills] [data-library-hero-pill]"),
        2,
        "the block paints two fact pills"
    );
    let count = fixture.attr_of(
        "[data-library-hero-block-pills] [data-meta-kind=\"count\"]",
        "style",
    );
    assert!(
        count
            .as_ref()
            .is_some_and(|style| style.contains("var(--canvas-type-count)")),
        "the count paints its own token, painted: {count:?}"
    );
    assert_eq!(
        fixture.text_of("[data-library-hero-block-pills] [data-meta-kind=\"count\"]"),
        "2",
        "the count's text is the track count"
    );
    let time = fixture.attr_of(
        "[data-library-hero-block-pills] [data-meta-kind=\"time\"]",
        "style",
    );
    assert!(
        time.as_deref()
            .is_some_and(|style| style.contains("var(--canvas-type-time)")),
        "the time paints its own token, painted: {time:?}"
    );
}

/// Contract (`LibraryHero.test.ts`, design D14): a duration repeats down the
/// tracklist, so it paints the time token at the repeat opacity.
#[test]
fn a_tracklist_duration_carries_the_repeated_time_token() {
    let fixture = built_hero();

    let repeated = fixture.attr_of(
        "[data-library-track=\"t2\"] [data-library-track-duration] [data-meta-kind=\"time\"]",
        "style",
    );
    assert!(
        repeated
            .as_ref()
            .is_some_and(|style| style.contains("var(--canvas-type-time)")),
        "the repeated duration paints the time token, painted: {repeated:?}"
    );
    assert!(
        repeated
            .as_ref()
            .is_some_and(|style| style.contains("var(--canvas-type-repeat-opacity)")),
        "the repeated duration paints at the repeat opacity, painted: {repeated:?}"
    );
}

mod artist_page;
