//! Headless tests of the Library interaction state (D16): the contracts of
//! `scroll.test.ts` and `reveal.test.ts` — a scroll step moves the Music
//! list's scroll store and leaves the rows the list derives in place, and a
//! restored pick reveals its album row in the list once the list is measured.
//! The harness builds a `DioxusDocument` whose root creates the store and
//! runs the scenario in one effect, so no test opens a window or sleeps, and
//! the carried content is mock data: no IPC runs.

use std::sync::{Arc, Mutex, mpsc};

use blitz_dom::Document as _;
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use crate::bridge::frame::GuiPanelToggles;
use crate::bridge::frame::library::{
    GuiCollections, GuiDestinationList, GuiEmbyRowFrame, GuiEmbyRowKindFrame, GuiFavouritesList,
    GuiHomeRowStateFrame, GuiLibraryDetail, GuiLibraryList, GuiLibraryTabFrame, GuiMusicAlbumFrame,
    GuiMusicArtistFrame, GuiMusicDetail, GuiMusicHeroFrame, GuiMusicHeroSubjectFrame, GuiMusicList,
    GuiMusicTrackFrame, GuiRowArtFrame, GuiRowMediaKindFrame,
};
use crate::bridge::sections::GuiSectionBatch;
use crate::core_thread::CoreInput;
use crate::window::channel::CoreLink;
use crate::window::clock::{Clock, RecordingClock};
use crate::window::library::list::{LibraryRow, album_row_id, artist_row_id};
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
        div { "library-harness" }
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

/// The Music list section value: the given artists under the `Music` tab.
fn music_list(artists: Vec<GuiMusicArtistFrame>) -> GuiLibraryList {
    GuiLibraryList {
        refreshing: false,
        tabs: vec![GuiLibraryTabFrame {
            position: 1,
            label: "Music".to_owned(),
        }],
        current_tab: Some(1),
        destination: GuiDestinationList::Music(GuiMusicList {
            view: crate::gui_core::GuiLibraryView::All,
            lib_idx: 0,
            collections: GuiCollections {
                target: crate::gui_core::GuiCollectionTarget::Music(0),
                all: false,
                entries: vec![crate::gui_core::GuiCollectionEntry {
                    key: "g1".to_owned(),
                    name: "Rock".to_owned(),
                    checked: true,
                }],
                noun: crate::gui_core::GuiCollectionNoun::Groups,
            },
            loading: false,
            failure: None,
            artists,
            favourites: None,
        }),
    }
}

/// The Music list section value with the destination's view Favourites
/// (emby-favourites design D6): the favourites rows ride the list part and the
/// artist tree is empty, as the core sends it while that view paints.
fn music_favourites_list(rows: Vec<GuiEmbyRowFrame>) -> GuiLibraryList {
    GuiLibraryList {
        refreshing: false,
        tabs: vec![GuiLibraryTabFrame {
            position: 1,
            label: "Music".to_owned(),
        }],
        current_tab: Some(1),
        destination: GuiDestinationList::Music(GuiMusicList {
            view: crate::gui_core::GuiLibraryView::Favourites,
            lib_idx: 0,
            collections: GuiCollections {
                target: crate::gui_core::GuiCollectionTarget::Music(0),
                all: true,
                entries: Vec::new(),
                noun: crate::gui_core::GuiCollectionNoun::Groups,
            },
            loading: false,
            failure: None,
            artists: Vec::new(),
            favourites: Some(GuiFavouritesList {
                loading: false,
                rows,
                selected: None,
                hero: None,
            }),
        }),
    }
}

/// One carried favourites row with no art and no played state.
fn favourite_row(id: &str, title: &str) -> GuiEmbyRowFrame {
    GuiEmbyRowFrame {
        id: id.to_owned(),
        title: title.to_owned(),
        subtitle: None,
        date: None,
        sort_letter: None,
        kind: GuiEmbyRowKindFrame::Media,
        state: GuiHomeRowStateFrame::Ordinary,
        art: GuiRowArtFrame::NoSource,
        media: GuiRowMediaKindFrame::Video,
    }
}

/// The Favourites view's own scroll facts of one library, as the tests read
/// them.
fn favourites_scroll(state: &LibraryState, lib_idx: usize) -> String {
    let scroll = state.favourites_view(lib_idx).scroll;
    format!(
        "top={} viewport={}",
        scroll.scroll_top, scroll.viewport_height
    )
}

/// The Music detail section value: the given Hero, `None` while none is
/// picked.
fn music_detail(hero: Option<GuiMusicHeroFrame>) -> GuiLibraryDetail {
    GuiLibraryDetail::Music(GuiMusicDetail {
        hero,
        art: None,
        playing_track_id: None,
    })
}

/// One artist with no albums.
fn artist(id: &str, index: usize) -> GuiMusicArtistFrame {
    GuiMusicArtistFrame {
        art: GuiRowArtFrame::NoSource,
        media: GuiRowMediaKindFrame::Audio,
        artist_key: crate::bridge::frame::library::GuiArtistKey::Service(format!("{id}-{index}")),
        name: format!("Artist {index}"),
        sort_letter: 'A',
        albums: Vec::new(),
    }
}

/// The artist helper's row over `albums`, so a tree test's artist paints its
/// albums.
fn artist_holding(id: &str, index: usize, albums: Vec<GuiMusicAlbumFrame>) -> GuiMusicArtistFrame {
    GuiMusicArtistFrame {
        albums,
        ..artist(id, index)
    }
}

/// The open mark of every painted artist and album row, in paint order, as
/// `id=true` or `id=false`: the Music tree's state in one fact.
fn node_marks(state: &LibraryState) -> String {
    let ui = state.ui.peek().clone();
    let rows = state.rows_of(&ui);
    let marks: Vec<String> = rows
        .iter()
        .filter_map(|row| match row {
            LibraryRow::Artist { base, open, .. } | LibraryRow::Album { base, open, .. } => {
                Some(format!("{}={open}", base.id))
            }
            LibraryRow::Letter(_) | LibraryRow::Track { .. } => None,
        })
        .collect();
    marks.join(" ")
}

/// One album with its cached tracklist.
fn album(
    id: &str,
    name: &str,
    year: &str,
    tracks: Option<Vec<GuiMusicTrackFrame>>,
) -> GuiMusicAlbumFrame {
    GuiMusicAlbumFrame {
        album_id: id.to_owned(),
        name: name.to_owned(),
        year: year.to_owned(),
        tracks,
        total: None,
    }
}

/// The `panel` section value the batch carries: the expanded panel at the
/// first reset epoch.
fn panel() -> GuiPanelToggles {
    GuiPanelToggles {
        collapsed: false,
        queue_folded: false,
        ui_reset_epoch: 0,
    }
}

/// A batch that carries the given Library sections.
fn library_batch(
    list: Option<GuiLibraryList>,
    detail: Option<GuiLibraryDetail>,
) -> GuiSectionBatch {
    let mut batch = GuiSectionBatch::default();
    if let Some(list) = list {
        batch.library_list = Some(crate::bridge::sections::GuiSection {
            revision: 1,
            value: Some(list),
        });
    }
    if let Some(detail) = detail {
        batch.library_detail = Some(crate::bridge::sections::GuiSection {
            revision: 1,
            value: Some(detail),
        });
    }
    batch.panel = Some(crate::bridge::sections::GuiSection {
        revision: 1,
        value: panel(),
    });
    batch
}

/// Carry the given Library content into the store's sections.
fn carry(state: &LibraryState, list: Option<GuiLibraryList>, detail: Option<GuiLibraryDetail>) {
    let mut sections = state.sections.clone();
    sections.apply_batch(&library_batch(list, detail));
}

/// The Music scroll facts, as the tests read them.
fn music_scroll(state: &LibraryState) -> String {
    let scroll = *state.music_scroll.peek();
    format!(
        "top={} viewport={}",
        scroll.scroll_top, scroll.viewport_height
    )
}

/// gui-change-driven-sections design D9, the contract of `scroll.test.ts`'s
/// "a scroll step moves the scroll store and keeps the derived rows": a
/// scroll step moves the Music list's scroll store and leaves the rows the
/// list derives in place.
#[test]
fn a_scroll_step_moves_the_scroll_store_and_keeps_the_derived_rows() {
    // Twenty artists under one letter, each closed, so the list is taller
    // than a short viewport.
    let artists: Vec<GuiMusicArtistFrame> = (0..20).map(|index| artist("artist", index)).collect();
    let list = music_list(artists.clone());
    let (facts, rx) = run(move |state, facts| {
        carry(state, Some(list.clone()), Some(music_detail(None)));
        state.measure_library_list(40.0);
        let ui = state.ui.peek().clone();
        let before = Arc::as_ptr(&state.rows_of(&ui));
        state.scroll_library_list(20.0);
        let after = Arc::as_ptr(&state.rows_of(&ui));
        facts.lock().expect("facts").push(format!(
            "same_rows={} scroll={}",
            std::ptr::addr_eq(before, after),
            music_scroll(state)
        ));
    });
    assert_eq!(facts, ["same_rows=true scroll=top=20 viewport=40"]);
    // A scroll sends no command.
    assert_eq!(commands(&rx), []);
}

/// `add-gui-library-panel`, the contract of `reveal.test.ts`'s "a reveal
/// scrolls the picked album row into view once the list is measured": a
/// restored pick reveals its album row in the list, waiting for the list's
/// first measurement.
#[test]
fn a_reveal_scrolls_the_picked_album_row_into_view_once_the_list_is_measured() {
    // Twenty artists with no albums above the target, so its row starts
    // below a short viewport. The target artist: one album with its tracks
    // cached, so no fetch runs.
    let mut artists: Vec<GuiMusicArtistFrame> =
        (0..20).map(|index| artist("filler", index)).collect();
    artists.push(GuiMusicArtistFrame {
        art: GuiRowArtFrame::NoSource,
        media: GuiRowMediaKindFrame::Audio,
        artist_key: crate::bridge::frame::library::GuiArtistKey::Service("a2".to_owned()),
        name: "Brand Nubian".to_owned(),
        sort_letter: 'B',
        albums: vec![album("al3", "One For All", "1990", Some(Vec::new()))],
    });
    let hero = GuiMusicHeroFrame {
        artist: "Brand Nubian".to_owned(),
        collections: vec!["Rock".to_owned()],
        album_count: 1,
        artist_tracks: None,
        subject: GuiMusicHeroSubjectFrame::Album(album(
            "al3",
            "One For All",
            "1990",
            Some(Vec::new()),
        )),
        menu: Vec::new(),
    };
    let list = music_list(artists.clone());
    let detail = music_detail(Some(hero));
    let (facts, rx) = run(move |state, facts| {
        carry(state, Some(list.clone()), Some(detail.clone()));
        let list_part = state.sections.library_list.peek().clone();
        let detail_part = state.sections.library_detail.peek().clone();
        let GuiDestinationList::Music(music) = &list_part.expect("list").destination else {
            panic!("music fixture");
        };
        let GuiLibraryDetail::Music(detail) = &*detail_part.expect("detail") else {
            panic!("detail fixture");
        };
        state.reveal_picked_row(music, detail);
        state.measure_library_list(40.0);
        facts.lock().expect("facts").push(music_scroll(state));
    });
    // The album's row starts below the 40 px viewport (twenty closed artists
    // under the `A` heading sit above it), so the reveal scrolls it into
    // view: the album's bottom edge at the viewport's top.
    assert_eq!(facts, ["top=1214 viewport=40"]);
    // The album's tracks are cached, so the reveal sends no open-album.
    assert_eq!(commands(&rx), []);
}

/// gui-music-artist-page design D6: a reveal whose subject is the artist page
/// scrolls the artist row into view once the list is measured, and opens
/// nothing — the artist row stays closed and no command sends.
#[test]
fn a_reveal_of_the_artist_page_scrolls_the_artist_row_into_view_and_opens_nothing() {
    // Twenty artists with no albums above the target, so its row starts
    // below a short viewport.
    let mut artists: Vec<GuiMusicArtistFrame> =
        (0..20).map(|index| artist("filler", index)).collect();
    artists.push(GuiMusicArtistFrame {
        art: GuiRowArtFrame::NoSource,
        media: GuiRowMediaKindFrame::Audio,
        artist_key: crate::bridge::frame::library::GuiArtistKey::Service("a2".to_owned()),
        name: "Brand Nubian".to_owned(),
        sort_letter: 'B',
        albums: vec![album("al3", "One For All", "1990", Some(Vec::new()))],
    });
    let hero = GuiMusicHeroFrame {
        artist: "Brand Nubian".to_owned(),
        collections: vec!["Rock".to_owned()],
        album_count: 1,
        artist_tracks: None,
        subject: GuiMusicHeroSubjectFrame::Artist(
            crate::bridge::frame::library::GuiMusicArtistPageFrame {
                artist_key: crate::bridge::frame::library::GuiArtistKey::Service("a2".to_owned()),
                albums: vec![album("al3", "One For All", "1990", Some(Vec::new()))],
                years: Some(("1990".to_owned(), "1990".to_owned())),
            },
        ),
        menu: Vec::new(),
    };
    let list = music_list(artists.clone());
    let detail = music_detail(Some(hero));
    let (facts, rx) = run(move |state, facts| {
        carry(state, Some(list.clone()), Some(detail.clone()));
        let list_part = state.sections.library_list.peek().clone();
        let detail_part = state.sections.library_detail.peek().clone();
        let GuiDestinationList::Music(music) = &list_part.expect("list").destination else {
            panic!("music fixture");
        };
        let GuiLibraryDetail::Music(detail) = &*detail_part.expect("detail") else {
            panic!("detail fixture");
        };
        state.reveal_picked_row(music, detail);
        state.measure_library_list(40.0);
        facts.lock().expect("facts").push(music_scroll(state));
        facts
            .lock()
            .expect("facts")
            .push(format!("open={}", state.ui.peek().open_nodes.len()));
    });
    // The artist row's bottom edge at the viewport's top: the row scrolled
    // into view, and no node opened.
    assert_eq!(facts, ["top=1184 viewport=40", "open=0"]);
    assert_eq!(commands(&rx), []);
}

/// gui-list-header-buttons design D4, spec "Expand all in Music": Expand all
/// opens every painted artist row, leaves every album's open state as it was,
/// and sends no command.
#[test]
fn expand_all_opens_every_painted_artist_and_leaves_the_albums_as_they_were() {
    let artists = vec![
        artist_holding(
            "a0",
            0,
            vec![
                album("al0", "One For All", "1990", Some(Vec::new())),
                album("al1", "Blues", "1991", Some(Vec::new())),
            ],
        ),
        artist_holding(
            "a1",
            1,
            vec![album("al2", "Dah Shinin'", "1995", Some(Vec::new()))],
        ),
        artist("a2", 2),
    ];
    let expected = [
        format!("{}=true", artist_row_id(&artists[0])),
        format!("{}=true", album_row_id("al0")),
        format!("{}=false", album_row_id("al1")),
        format!("{}=true", artist_row_id(&artists[1])),
        format!("{}=false", album_row_id("al2")),
        format!("{}=true", artist_row_id(&artists[2])),
    ]
    .join(" ");
    let list = music_list(artists);
    let (facts, rx) = run(move |state, facts| {
        carry(state, Some(list.clone()), Some(music_detail(None)));
        // The fixture's one open album, so the press shows it staying open.
        let mut ui = state.ui;
        ui.with_mut(|ui| ui.open_nodes.push(album_row_id("al0")));
        state.expand_all_music();
        facts.lock().expect("facts").push(node_marks(state));
    });
    assert_eq!(facts, [expected]);
    assert_eq!(commands(&rx), []);
}

/// gui-list-header-buttons design D4, spec "Expand all under a filter": the
/// artist the filter kept opens and leaves the filter's closed set, so it
/// still paints open once the filter clears.
#[test]
fn expand_all_under_a_filter_opens_the_closed_artist_the_filter_kept() {
    let artists = vec![
        artist_holding(
            "a0",
            0,
            vec![album("al0", "One For All", "1990", Some(Vec::new()))],
        ),
        artist_holding(
            "a1",
            1,
            vec![album("al1", "Blues", "1991", Some(Vec::new()))],
        ),
    ];
    let a0 = artist_row_id(&artists[0]);
    let cleared = [
        format!("{a0}=true"),
        format!("{}=false", album_row_id("al0")),
        format!("{}=false", artist_row_id(&artists[1])),
    ]
    .join(" ");
    let list = music_list(artists);
    let closed_artist = a0.clone();
    let (facts, _rx) = run(move |state, facts| {
        carry(state, Some(list.clone()), Some(music_detail(None)));
        state.set_library_filter("One".to_owned());
        // The user closes the artist the filter opened.
        let mut ui = state.ui;
        ui.with_mut(|ui| ui.filter_closed.push(closed_artist.clone()));
        state.expand_all_music();
        let (open, closed) = {
            let ui = state.ui.peek();
            (
                ui.open_nodes.contains(&closed_artist),
                ui.filter_closed.contains(&closed_artist),
            )
        };
        facts
            .lock()
            .expect("facts")
            .push(format!("open={open} closed={closed}"));
        state.set_library_filter(String::new());
        facts.lock().expect("facts").push(node_marks(state));
    });
    assert_eq!(facts, [String::from("open=true closed=false"), cleared]);
}

/// gui-list-header-buttons design D4, spec "Collapse all under a filter": the
/// press closes the artist the filter opened, clears the open nodes, and leaves
/// the filter text in place.
#[test]
fn collapse_all_under_a_filter_closes_the_filter_opened_artist_and_clears_the_open_nodes() {
    let artists = vec![
        artist_holding(
            "a0",
            0,
            vec![album("al0", "One For All", "1990", Some(Vec::new()))],
        ),
        artist_holding(
            "a1",
            1,
            vec![album("al1", "Blues", "1991", Some(Vec::new()))],
        ),
    ];
    let a0 = artist_row_id(&artists[0]);
    let list = music_list(artists);
    let closed_artist = a0.clone();
    let (facts, _rx) = run(move |state, facts| {
        carry(state, Some(list.clone()), Some(music_detail(None)));
        let mut ui = state.ui;
        ui.with_mut(|ui| ui.open_nodes.push(album_row_id("al0")));
        state.set_library_filter("One".to_owned());
        state.collapse_all_music();
        let marks = node_marks(state);
        let (open_empty, in_closed, filter) = {
            let ui = state.ui.peek();
            (
                ui.open_nodes.is_empty(),
                ui.filter_closed.contains(&closed_artist),
                ui.filter.clone(),
            )
        };
        facts.lock().expect("facts").push(format!(
            "{marks} open_empty={open_empty} closed={in_closed} filter={filter}"
        ));
    });
    assert_eq!(
        facts,
        [format!("{a0}=false open_empty=true closed=true filter=One")]
    );
}

/// Issue 1028: one close point. A popover that joins `close_popovers` is
/// closed by the root press and by a destination change, so none can stick.
#[test]
fn close_popovers_closes_every_dropdown_and_the_search_panel_and_clears_filters() {
    let (facts, _rx) = run(|state, facts| {
        state.set_search_collapsed(true);
        state.toggle_collections_dropdown();
        state.set_collections_filter("rock".to_owned());
        state.toggle_library_view_dropdown();
        state.toggle_shows_seasons_dropdown();
        state.set_shows_seasons_filter("s1".to_owned());
        state.open_search_panel();
        state.close_popovers();
        let ui = state.ui.peek();
        let mut facts = facts.lock().expect("facts");
        facts.push(format!("collections={:?}", ui.collections));
        facts.push(format!("view={:?}", ui.library_view));
        facts.push(format!("seasons={:?}", ui.shows_seasons));
        facts.push(format!("search_panel={}", ui.search_control.panel_open));
    });
    assert_eq!(
        facts,
        [
            "collections=DropdownState(None)",
            "view=DropdownState(None)",
            "seasons=DropdownState(None)",
            "search_panel=false",
        ]
    );
}

/// emby-favourites design D6, D7: the Favourites list keeps its own scroll and
/// filter text per library, so scrolling and filtering it leave the All list's
/// scroll store and filter text as they were and another library's Favourites
/// view untouched.
#[test]
fn the_favourites_scroll_and_filter_are_their_own_per_library() {
    // Twenty closed artists under one letter: the All list is taller than its
    // short viewport, so its scroll holds a real offset.
    let artists: Vec<GuiMusicArtistFrame> = (0..20).map(|index| artist("artist", index)).collect();
    let all = music_list(artists);
    let rows = vec![favourite_row("m1", "Alien"), favourite_row("m2", "Brazil")];
    let favourites = music_favourites_list(rows.clone());
    let (facts, _rx) = run(move |state, facts| {
        carry(state, Some(all.clone()), Some(music_detail(None)));
        state.measure_library_list(40.0);
        state.scroll_library_list(120.0);
        facts
            .lock()
            .expect("facts")
            .push(format!("all {}", music_scroll(state)));
        // The view moves to Favourites: the core sends the favourites part and
        // no artist tree.
        carry(state, Some(favourites.clone()), Some(music_detail(None)));
        let part = GuiFavouritesList {
            loading: false,
            rows: rows.clone(),
            selected: None,
            hero: None,
        };
        let painted = crate::window::library::favourites::favourites_rows(&part, "");
        state.measure_favourites_list(0, 40.0, None, &painted);
        state.scroll_favourites_list(0, 56.0, &painted);
        let mut recorded = facts.lock().expect("facts");
        recorded.push(format!("favourites0 {}", favourites_scroll(state, 0)));
        recorded.push(format!("favourites1 {}", favourites_scroll(state, 1)));
        drop(recorded);
        state.set_favourites_filter(0, "Ali".to_owned());
        let mut recorded = facts.lock().expect("facts");
        recorded.push(format!("all after {}", music_scroll(state)));
        recorded.push(format!("filter {:?}", state.ui.peek().filter));
    });
    assert_eq!(
        facts,
        [
            "all top=120 viewport=40",
            "favourites0 top=56 viewport=40",
            "favourites1 top=0 viewport=0",
            "all after top=120 viewport=40",
            "filter \"\"",
        ]
    );
}

/// A late filter write must not reopen a closed popover (review of #1028).
#[test]
fn a_filter_write_leaves_a_closed_dropdown_closed() {
    let (facts, _rx) = run(|state, facts| {
        state.set_collections_filter("late".to_owned());
        facts
            .lock()
            .expect("facts")
            .push(format!("open={}", state.collections_open()));
    });
    assert_eq!(facts, ["open=false"]);
}
