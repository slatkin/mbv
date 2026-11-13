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

use super::{LibraryKeyFrame, route_library_key};
use crate::bridge::frame::chrome::GuiStatusBar;
use crate::bridge::frame::focus::{GuiFocus, GuiPanel};
use crate::bridge::frame::library::{
    GuiDestinationList, GuiLibraryDetail, GuiLibraryList, GuiLibraryTabFrame, GuiMusicAlbumFrame,
    GuiMusicArtistFrame, GuiMusicDetail, GuiMusicHeroFrame, GuiMusicList, GuiMusicPillFrame,
    GuiMusicTrackFrame,
};
use crate::bridge::frame::overlay::GuiOverlayFrame;
use crate::bridge::keys::{DomKey, DomModifiers};
use crate::bridge::library::GuiLibraryCommand;
use crate::bridge::sections::GuiSectionBatch;
use crate::core_thread::CoreInput;
use crate::window::arrangement::GuiArrangement;
use crate::window::channel::CoreLink;
use crate::window::library::state::{LibraryPane, LibraryState};
use crate::window::sections::Sections;

/// The facts one scenario recorded, in its run order.
type Facts = Arc<Mutex<Vec<String>>>;

/// One scenario the harness runs against the built store.
type Scenario = std::rc::Rc<dyn Fn(&LibraryState, &Facts)>;

/// The root: creates the store once and runs the scenario in one effect.
fn harness((tx, facts, scenario): (mpsc::Sender<CoreInput>, Facts, Scenario)) -> Element {
    let state = use_hook(|| LibraryState::new(Sections::new(), CoreLink::new(tx)));
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
fn commands(rx: &mpsc::Receiver<CoreInput>) -> Vec<mbv::GuiLibraryCommand> {
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

/// A slice frame's routing facts (design D3) with the Library panel shown
/// under Library Panel focus.
fn frame(library_list: Option<&GuiLibraryList>) -> LibraryKeyFrame {
    LibraryKeyFrame {
        focus: GuiFocus {
            surface: None,
            panel: GuiPanel::Library,
        },
        overlay: None,
        status: GuiStatusBar {
            prefix_armed: false,
            toast: None,
        },
        filter_active: false,
        settings: None,
        library_list: library_list.cloned(),
        search: None,
    }
}

/// Route one keydown and paint the outcome.
fn route(
    state: &LibraryState,
    facts: &Facts,
    name: &str,
    value: &str,
    shift: bool,
    frame: &LibraryKeyFrame,
) {
    let outcome = route_library_key(&key(value, shift), Some(frame), state);
    facts
        .lock()
        .expect("facts")
        .push(format!("{name}={outcome:?}"));
}

/// The two group pills the Music fixture carries: one selected, one not.
fn pills() -> Vec<GuiMusicPillFrame> {
    vec![
        GuiMusicPillFrame {
            group_id: "g1".to_owned(),
            name: "Rock".to_owned(),
            selected: true,
            collection: true,
        },
        GuiMusicPillFrame {
            group_id: "g2".to_owned(),
            name: "Jazz".to_owned(),
            selected: false,
            collection: true,
        },
    ]
}

/// The Beastie Boys artist: one album with a cached tracklist, one without.
fn beastie() -> GuiMusicArtistFrame {
    GuiMusicArtistFrame {
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
        album: GuiMusicAlbumFrame {
            album_id: "al1".to_owned(),
            name: "Licensed To Ill".to_owned(),
            year: "1986".to_owned(),
            tracks: Some(vec![
                track("t1", "Rhymin & Stealin", 1, "4:08"),
                track("t2", "The New Style", 2, "4:35"),
            ]),
            total: None,
        },
        menu: Vec::new(),
    }
}

/// The carried Music content: two group pills, one selected, the union the
/// list keys move through, and the Hero's facts while an album is picked.
fn music_list(artists: Vec<GuiMusicArtistFrame>) -> GuiLibraryList {
    GuiLibraryList {
        tabs: vec![GuiLibraryTabFrame {
            position: 1,
            label: "Music".to_owned(),
        }],
        current_tab: Some(1),
        destination: GuiDestinationList::Music(GuiMusicList {
            lib_idx: 0,
            pills: pills(),
            collection_names: vec!["Rock".to_owned()],
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

/// The Home content the pane-walk contracts read (design D12): no Music
/// content, the flat list's rows.
fn home_list() -> GuiLibraryList {
    GuiLibraryList {
        tabs: vec![GuiLibraryTabFrame {
            position: 1,
            label: "Music".to_owned(),
        }],
        current_tab: None,
        destination: GuiDestinationList::Home(crate::bridge::frame::library::GuiHomeList {
            loading: false,
            rows: Vec::new(),
        }),
    }
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

/// The pane focus, filter, and open facts the assertions read.
fn ui_facts(state: &LibraryState) -> String {
    let ui = state.ui.peek().clone();
    format!(
        "pane={:?} filter_open={} filter={:?} focused_pill={:?} pill_select={:?} cursor={:?} open={:?} track_cursor={:?} dismissed={}",
        ui.pane,
        ui.filter_open,
        ui.filter,
        ui.focused_pill,
        ui.pill_select,
        ui.cursor,
        ui.open_nodes,
        ui.track_cursor,
        ui.hero_dismissed,
    )
}

/// Carry the union the list keys move through, with the list pane focused and
/// the list measured at 40 px.
fn carry_list(state: &LibraryState, open: &[&str]) {
    carry(
        state,
        music_list(vec![beastie(), brand()]),
        music_detail(None),
    );
    let mut ui = state.ui;
    ui.with_mut(|ui| {
        ui.pane = Some(LibraryPane::List);
        ui.open_nodes = open.iter().map(|id| (*id).to_owned()).collect();
    });
    state.measure_library_list(40.0);
}

/// The frame with the carried Library content, read back from the sections.
fn carried_frame(state: &LibraryState) -> LibraryKeyFrame {
    frame(Some(
        &state.sections.library_list.peek().clone().expect("list"),
    ))
}

// A cursor move below the rows in view scrolls its row into view.

mod gating;
mod list;
mod panes;
mod pills;
