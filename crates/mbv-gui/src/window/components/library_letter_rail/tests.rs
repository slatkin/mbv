//! Headless tests of the letter rail (D16): the contracts of
//! `LibraryLetterRail.test.ts` — a letter with no painted rows is disabled
//! and a press on it changes nothing, and a press on an enabled letter scrolls
//! the list to its first row. A headless document
//! cannot fire a DOM click event, so the press contracts ride the state
//! method the rail's press handler calls (the top bar's precedent); the
//! disabled mark is the painted attribute, asserted on the built document.
//! The core channel is a mock pair; no test opens a window, sleeps, or
//! reaches the core.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, mpsc};

use blitz_dom::{Document as _, local_name};
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use super::LibraryLetterRail;
use crate::bridge::frame::library::{
    GuiCollections, GuiDestinationList, GuiLibraryList, GuiLibraryTabFrame, GuiMusicAlbumFrame,
    GuiMusicArtistFrame, GuiMusicList, GuiRowArtFrame, GuiRowMediaKindFrame,
};
use crate::core_thread::CoreInput;
use crate::window::channel::CoreLink;
use crate::window::clock::{Clock, RecordingClock};
use crate::window::library::list::{LibraryTree, RailLetter, build_library_rows, letter_rail};
use crate::window::library::state::LibraryState;
use crate::window::sections::Sections;

/// The context handles the root hands the test: the Library store.
type FixtureHandles = LibraryState;

/// The harness's props: the rail's own props plus the mock core sender and
/// the handle the test reads the store through.
#[derive(Clone)]
struct FixtureProps {
    sender: mpsc::Sender<CoreInput>,
    letters: Vec<RailLetter>,
    handle: Rc<RefCell<Option<FixtureHandles>>>,
}

/// The root: provides the Library store over the mock core link and paints
/// the rail, as the list composes it.
fn rail_app(props: FixtureProps) -> Element {
    let FixtureProps {
        sender,
        letters,
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
    handle.borrow_mut().replace(state);
    rsx! {
        LibraryLetterRail { letters }
    }
}

struct Fixture {
    doc: DioxusDocument,
    state: LibraryState,
    /// Kept open for the fixture's life, so a command the rail sends reaches
    /// a live channel; no test asserts one.
    _receiver: mpsc::Receiver<CoreInput>,
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

    /// Carry the Music sections, as the core's batch would, so the store's
    /// row model reads the carried union at the batch's revision.
    fn carry(&mut self, list: GuiMusicList) {
        let batch = crate::bridge::sections::GuiSectionBatch {
            library_list: Some(crate::bridge::sections::GuiSection {
                revision: 1,
                value: Some(GuiLibraryList {
                    refreshing: false,
                    tabs: vec![GuiLibraryTabFrame {
                        position: 1,
                        label: "Music".to_owned(),
                    }],
                    current_tab: Some(1),
                    destination: GuiDestinationList::Music(list),
                }),
            }),
            ..crate::bridge::sections::GuiSectionBatch::default()
        };
        let mut sections = self.state.sections.clone();
        self.in_runtime(move || sections.apply_batch(&batch));
    }

    /// Whether the first node the selector names carries the attribute.
    fn has_attribute(&self, selector: &str, attribute: blitz_dom::LocalName) -> bool {
        let dom = self.doc.inner();
        dom.query_selector(selector)
            .expect("the selector is valid")
            .is_some_and(|id| {
                dom.get_node(id)
                    .expect("the node is in the document")
                    .attr(attribute)
                    .is_some()
            })
    }
}

/// Build the document over the rail's props and let its first render and
/// effects land.
fn built(letters: Vec<RailLetter>) -> Fixture {
    let (sender, receiver) = mpsc::channel::<CoreInput>();
    let handle = Rc::new(RefCell::new(None));
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(
            rail_app,
            FixtureProps {
                sender,
                letters,
                handle: Rc::clone(&handle),
            },
        ),
        DocumentConfig::default(),
    );
    doc.initial_build();
    let state = handle
        .borrow_mut()
        .take()
        .expect("the root handed the store over");
    let mut fixture = Fixture {
        doc,
        state,
        _receiver: receiver,
    };
    fixture.poll_until_idle();
    fixture
}

/// The Beastie Boys artist: one album whose tracks the content does not
/// hold.
fn beastie() -> GuiMusicArtistFrame {
    GuiMusicArtistFrame {
        art: GuiRowArtFrame::NoSource,
        media: GuiRowMediaKindFrame::Audio,
        artist_key: crate::bridge::frame::library::GuiArtistKey::Service("a1".to_owned()),
        name: "Beastie Boys".to_owned(),
        sort_letter: 'B',
        albums: vec![GuiMusicAlbumFrame {
            album_id: "al1".to_owned(),
            name: "Licensed To Ill".to_owned(),
            year: "1986".to_owned(),
            tracks: None,
            total: None,
        }],
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
            album_id: "al2".to_owned(),
            name: "One For All".to_owned(),
            year: "1990".to_owned(),
            tracks: None,
            total: None,
        }],
    }
}

/// The carried Music content: both artists, none of them open.
fn music() -> GuiMusicList {
    GuiMusicList {
        lib_idx: 0,
        collections: GuiCollections {
            target: crate::gui_core::GuiCollectionTarget::Music(0),
            all: true,
            entries: Vec::new(),
            noun: crate::gui_core::GuiCollectionNoun::Groups,
        },
        loading: false,
        failure: None,
        artists: vec![beastie(), brand()],
    }
}

/// The rail's letters: the A–Z entries over the painted rows, as the list
/// hands them over.
fn letters() -> Vec<RailLetter> {
    let artists = music().artists;
    letter_rail(&build_library_rows(LibraryTree {
        artists: &artists,
        open: &[],
        closed: &[],
        filter: "",
    }))
}

/// Contract (`LibraryLetterRail.test.ts`): a letter with no painted rows is
/// disabled, and a press on it changes nothing.
#[test]
fn a_letter_with_no_painted_rows_is_disabled_and_a_press_changes_nothing() {
    let mut fixture = built(letters());
    fixture.carry(music());

    assert!(
        fixture.has_attribute("[data-library-rail-letter=\"C\"]", local_name!("disabled"),),
        "a letter with no painted rows paints disabled"
    );
    assert!(
        !fixture.has_attribute("[data-library-rail-letter=\"B\"]", local_name!("disabled"),),
        "a letter with painted rows paints enabled"
    );

    fixture.compute(|state| state.jump_to_letter('C'));
    assert_eq!(fixture.state.music_scroll.peek().scroll_top, 0.0);
}

/// Contract (`LibraryLetterRail.test.ts`): a press on an enabled letter
/// scrolls the list to its first row and picks and selects nothing.
#[test]
fn a_press_on_a_letter_scrolls_the_list_to_its_first_row() {
    let mut fixture = built(letters());
    fixture.carry(music());
    fixture.compute(|state| state.measure_library_list(40.0));

    fixture.compute(|state| state.jump_to_letter('B'));

    assert_eq!(fixture.state.music_scroll.peek().scroll_top, 40.0);
}
