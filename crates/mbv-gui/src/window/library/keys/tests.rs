//! Headless tests of the Library key router (D16): the Music-routing, gating,
//! and window-key contracts of `keys.test.ts` — the router runs before the
//! forwarder and decides whether the core ever sees a keydown. The Home
//! routing contracts of `keys.test.ts` route with the Home keys' port (task
//! 6.4). The harness builds a `DioxusDocument` whose root creates the store
//! and runs each scenario in one effect over a recording core link, so no
//! test opens a window or sleeps, and no IPC runs.

use std::sync::{Arc, Mutex, mpsc};

use blitz_dom::Document as _;
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use super::route_library_key;
use crate::bridge::frame::library::{
    GuiCollectionEntry, GuiCollections, GuiDestinationList, GuiLibraryDetail, GuiLibraryList,
    GuiLibraryTabFrame, GuiMusicAlbumFrame, GuiMusicArtistFrame, GuiMusicDetail, GuiMusicHeroFrame,
    GuiMusicHeroSubjectFrame, GuiMusicList, GuiMusicTrackFrame, GuiRowArtFrame,
    GuiRowMediaKindFrame, GuiShowsDetail, GuiShowsList,
};
use crate::bridge::frame::overlay::GuiOverlayFrame;
use crate::bridge::keys::{DomKey, DomModifiers};
use crate::bridge::library::GuiLibraryCommand;
use crate::bridge::sections::GuiSectionBatch;
use crate::core_thread::CoreInput;
use crate::window::arrangement::GuiArrangement;
use crate::window::channel::CoreLink;
use crate::window::clock::{Clock, RecordingClock};
use crate::window::library::state::LibraryState;
use crate::window::pointer_target::{PointerList, PointerTarget, PointerZone};
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
        div { "library-keys-harness" }
    }
}

/// Run one scenario against a fresh store and return the facts it recorded
/// and the commands the core link received. Bounded by construction: the
/// effect runs once and the poll loop stops when the document goes idle.
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

/// One keydown as the DOM names it.
fn key(value: &str, shift: bool) -> DomKey {
    DomKey {
        key: value.to_owned(),
        code: String::new(),
        meta_key: false,
        repeat: false,
        modifiers: DomModifiers {
            shift_key: shift,
            ctrl_key: false,
            alt_key: false,
        },
    }
}

/// The pointer target a Library key routes under (design D2 of
/// `gui-pointer-target-interaction`): the pointer over the Library column and
/// no Hero list, so the destination's list keys act.
fn library_pointer() -> PointerTarget {
    PointerTarget {
        zone: Some(PointerZone::Library),
        list: None,
        row: None,
    }
}

/// Arm the Status bar's prefix (design D3, step 1).
fn arm_prefix(state: &LibraryState) {
    let mut status = state.sections.status;
    status.write().prefix_armed = true;
}

/// Open a core overlay (design D3, step 1).
fn open_core_overlay(state: &LibraryState) {
    let mut overlay = state.sections.overlay;
    overlay.write().replace(GuiOverlayFrame::RowMenu {
        anchor_slot: 0,
        cursor: 0,
        entries: Vec::new(),
    });
}

/// Route one keydown against the given section stores and paint the outcome.
fn route_on(
    state: &LibraryState,
    facts: &Facts,
    name: &str,
    value: &str,
    shift: bool,
    sections: &Sections,
    arrangement: GuiArrangement,
) {
    route_with(
        state,
        facts,
        name,
        value,
        shift,
        sections,
        arrangement,
        &library_pointer(),
    );
}

/// Route one keydown under a named pointer target and paint the outcome.
fn route_with(
    state: &LibraryState,
    facts: &Facts,
    name: &str,
    value: &str,
    shift: bool,
    sections: &Sections,
    arrangement: GuiArrangement,
    pointer: &PointerTarget,
) {
    let outcome = route_library_key(
        &key(value, shift),
        Some(sections),
        state,
        None,
        arrangement,
        pointer,
    );
    facts
        .lock()
        .expect("facts")
        .push(format!("{name}={outcome:?}"));
}

/// Route one keydown with the pointer over `zone`, against the window's own
/// section stores, and paint the outcome: the pointer target decides the panel
/// a core answer names (design D2 of `gui-pointer-target-interaction`).
fn route_under(state: &LibraryState, facts: &Facts, name: &str, value: &str, zone: PointerZone) {
    let pointer = PointerTarget {
        zone: Some(zone),
        list: None,
        row: None,
    };
    route_with(
        state,
        facts,
        name,
        value,
        false,
        &state.sections,
        GuiArrangement::Wide,
        &pointer,
    );
}

/// Route one keydown with the pointer over the Hero's tracklist, resting on the
/// track `row`.
fn route_hero(state: &LibraryState, facts: &Facts, name: &str, value: &str, row: &str) {
    let pointer = PointerTarget {
        zone: Some(PointerZone::Library),
        list: Some(PointerList::Tracklist),
        row: Some(row.to_owned()),
    };
    route_with(
        state,
        facts,
        name,
        value,
        false,
        &state.sections,
        GuiArrangement::Wide,
        &pointer,
    );
}

/// Route one keydown with the pointer over a Library list, resting on the row
/// `row` of that list (design D1, D2).
fn route_on_row(
    state: &LibraryState,
    facts: &Facts,
    name: &str,
    value: &str,
    list: PointerList,
    row: &str,
) {
    let pointer = PointerTarget {
        zone: Some(PointerZone::Library),
        list: Some(list),
        row: Some(row.to_owned()),
    };
    route_with(
        state,
        facts,
        name,
        value,
        false,
        &state.sections,
        GuiArrangement::Wide,
        &pointer,
    );
}

/// Route one keydown against the window's own section stores and paint the
/// outcome, in the arrangement a Wide Library column paints: no Hero overlay is
/// up unless a scenario routes in it.
fn route(state: &LibraryState, facts: &Facts, name: &str, value: &str, shift: bool) {
    route_on(
        state,
        facts,
        name,
        value,
        shift,
        &state.sections,
        GuiArrangement::Wide,
    );
}

/// Route one keydown in the given arrangement, against the window's own section
/// stores (design D4): the keys close the Narrow Hero overlay only while the
/// panel paints one.
fn route_in(
    state: &LibraryState,
    facts: &Facts,
    name: &str,
    value: &str,
    shift: bool,
    arrangement: GuiArrangement,
) {
    route_on(
        state,
        facts,
        name,
        value,
        shift,
        &state.sections,
        arrangement,
    );
}

/// The Music fixture's collection selector: the one Rock group checked
/// (design D6).
fn collections() -> GuiCollections {
    GuiCollections {
        target: crate::gui_core::GuiCollectionTarget::Music(0),
        all: false,
        entries: vec![
            GuiCollectionEntry {
                key: "g1".to_owned(),
                name: "Rock".to_owned(),
                checked: true,
            },
            GuiCollectionEntry {
                key: "g2".to_owned(),
                name: "Jazz".to_owned(),
                checked: false,
            },
        ],
        noun: crate::gui_core::GuiCollectionNoun::Groups,
    }
}

/// The Beastie Boys artist: one album with a cached tracklist, one without.
fn beastie() -> GuiMusicArtistFrame {
    GuiMusicArtistFrame {
        art: GuiRowArtFrame::NoSource,
        media: GuiRowMediaKindFrame::Audio,
        artist_key: crate::bridge::frame::library::GuiArtistKey::Service("a1".to_owned()),
        name: "Beastie Boys".to_owned(),
        sort_letter: 'B',
        albums: vec![
            GuiMusicAlbumFrame {
                album_id: "al1".to_owned(),
                name: "Licensed To Ill".to_owned(),
                year: "1986".to_owned(),
                tracks: Some(vec![track("t1", "Rhymin & Stealin", 1, "3:27")]),
                total: None,
            },
            GuiMusicAlbumFrame {
                album_id: "al2".to_owned(),
                name: "Paul's Boutique".to_owned(),
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
        artist_key: crate::bridge::frame::library::GuiArtistKey::Service("a2".to_owned()),
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

/// One tracklist row.
fn track(id: &str, name: &str, number: i64, duration: &str) -> GuiMusicTrackFrame {
    GuiMusicTrackFrame {
        track_id: id.to_owned(),
        name: name.to_owned(),
        number,
        duration: duration.to_owned(),
    }
}

/// The Hero fixture: the picked album's tracklist the Hero pane's keys move
/// through (design D3, D8).
fn hero() -> GuiMusicHeroFrame {
    GuiMusicHeroFrame {
        artist: "Beastie Boys".to_owned(),
        collections: vec!["Rock".to_owned()],
        album_count: 1,
        artist_tracks: None,
        subject: GuiMusicHeroSubjectFrame::Album(GuiMusicAlbumFrame {
            album_id: "al1".to_owned(),
            name: "Licensed To Ill".to_owned(),
            year: "1986".to_owned(),
            tracks: Some(vec![
                track("t1", "Rhymin & Stealin", 1, "4:08"),
                track("t2", "The New Style", 2, "4:35"),
            ]),
            total: None,
        }),
        menu: Vec::new(),
    }
}

/// The carried Music content: the collection selector with one checked group,
/// the union the list keys move through, and the Hero's facts while an album
/// is picked.
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
            collections: collections(),
            loading: false,
            failure: None,
            artists,
            favourites: None,
        }),
    }
}

/// The carried Shows content: the Shows destination's series list, whose view
/// dropdown has its own open fact and mounts no filter field (design D8, D15).
fn shows_list() -> GuiLibraryList {
    GuiLibraryList {
        refreshing: false,
        tabs: Vec::new(),
        current_tab: Some(2),
        destination: GuiDestinationList::Shows(GuiShowsList {
            sort: None,
            lib_idx: 0,
            view: crate::gui_core::GuiLibraryView::All,
            loading: false,
            series: Vec::new(),
            upcoming_failed: false,
            rows: Vec::new(),
            favourites: None,
        }),
    }
}

/// The Shows detail section value: no Hero.
fn shows_detail() -> GuiLibraryDetail {
    GuiLibraryDetail::Shows(GuiShowsDetail {
        playing_episode_id: None,
        workspace_loading: false,
        selected: None,
        hero: None,
    })
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

/// Carry the given content into the store's sections.
fn carry(state: &LibraryState, list: GuiLibraryList, detail: GuiLibraryDetail) {
    let batch = GuiSectionBatch {
        library_list: Some(crate::bridge::sections::GuiSection {
            revision: 1,
            value: Some(list),
        }),
        library_detail: Some(crate::bridge::sections::GuiSection {
            revision: 1,
            value: Some(detail),
        }),
        ..GuiSectionBatch::default()
    };
    let mut sections = state.sections.clone();
    sections.apply_batch(&batch);
}

/// The filter and open facts the assertions read.
fn ui_facts(state: &LibraryState) -> String {
    let ui = state.ui.peek().clone();
    format!(
        "filter_open={} filter={:?} collections_open={} collections_filter={:?} open={:?} dismissed={}",
        ui.filter_open,
        ui.filter,
        ui.collections.is_open(),
        ui.collections.filter().to_owned(),
        ui.open_nodes,
        ui.hero_dismissed,
    )
}

/// Carry the union the list keys move through, the list measured at 40 px.
fn carry_list(state: &LibraryState, open: &[&str]) {
    carry(
        state,
        music_list(vec![beastie(), brand()]),
        music_detail(None),
    );
    let mut ui = state.ui;
    ui.with_mut(|ui| {
        ui.open_nodes = open.iter().map(|id| (*id).to_owned()).collect();
    });
    state.measure_library_list(40.0);
}

// A cursor move below the rows in view scrolls its row into view.

mod collections;
mod gating;
mod home;
mod list;
mod search_keys;
mod window_keys;
