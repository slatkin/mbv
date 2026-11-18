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
    GuiCollections, GuiDestinationList, GuiLibraryDetail, GuiLibraryList, GuiLibraryTabFrame,
    GuiMusicAlbumFrame, GuiMusicArtistFrame, GuiMusicDetail, GuiMusicHeroFrame, GuiMusicList,
    GuiMusicTrackFrame,
};
use crate::bridge::sections::GuiSectionBatch;
use crate::core_thread::CoreInput;
use crate::window::channel::CoreLink;
use crate::window::clock::{Clock, RecordingClock};
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
fn commands(rx: &mpsc::Receiver<CoreInput>) -> Vec<mbv::GuiLibraryCommand> {
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
        tabs: vec![GuiLibraryTabFrame {
            position: 1,
            label: "Music".to_owned(),
        }],
        current_tab: Some(1),
        destination: GuiDestinationList::Music(GuiMusicList {
            lib_idx: 0,
            collections: GuiCollections {
                target: mbv::GuiCollectionTarget::Music(0),
                all: false,
                entries: vec![mbv::GuiCollectionEntry {
                    key: "g1".to_owned(),
                    name: "Rock".to_owned(),
                    checked: true,
                }],
                noun: mbv::GuiCollectionNoun::Groups,
                order: None,
            },
            loading: false,
            failure: None,
            artists,
        }),
    }
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
        artist_key: crate::bridge::frame::library::GuiArtistKey::Service(format!("{id}-{index}")),
        name: format!("Artist {index}"),
        sort_letter: 'A',
        albums: Vec::new(),
    }
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
        album: album("al3", "One For All", "1990", Some(Vec::new())),
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
        state.reveal_picked_album(music, detail);
        state.measure_library_list(40.0);
        facts.lock().expect("facts").push(music_scroll(state));
    });
    // The album's row starts below the 40 px viewport (twenty closed artists
    // under the `A` heading sit above it), so the reveal scrolls it into
    // view: the album's bottom edge at the viewport's top.
    assert_eq!(facts, ["top=668 viewport=40"]);
    // The album's tracks are cached, so the reveal sends no open-album.
    assert_eq!(commands(&rx), []);
}
