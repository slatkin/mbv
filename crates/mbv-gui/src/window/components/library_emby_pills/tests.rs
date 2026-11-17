//! Headless tests of the feed-group Emby pills (D16): the contracts of
//! `LibraryEmbyPills.test.ts` — the pills paint the core's selection with
//! `aria-pressed` under the destination's own hook, a press sends the
//! collection command the collections frame names (gui-collection-dropdown
//! design D6), and the keys' pill request becomes the same command. The IPC
//! of the earlier tests is
//! the mock core link here; the asserted commands are the same neutral
//! shapes. A headless document cannot fire a DOM click, so the press
//! contract rides the state method the component's own handler calls one
//! line each (the list's precedent); the request contract rides the mounted
//! component's effect. The core channel is a mock pair; no test opens a
//! window, sleeps, or reaches the core.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, mpsc};

use blitz_dom::{Document as _, LocalName};
use blitz_traits::shell::{ColorScheme, Viewport};
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use super::LibraryEmbyPills;
use crate::bridge::frame::library::{GuiCollectionEntry, GuiCollectionsFrame, GuiEmbyList};
use crate::core_thread::CoreInput;
use crate::window::channel::CoreLink;
use crate::window::clock::{Clock, RecordingClock};
use crate::window::library::state::LibraryState;
use crate::window::sections::Sections;

/// The harness's props: the carried Emby collections, the mock core sender,
/// and the handle the test reads the store through.
#[derive(Clone)]
struct FixtureProps {
    sender: mpsc::Sender<CoreInput>,
    collections: GuiCollectionsFrame,
    handle: Rc<RefCell<Option<LibraryState>>>,
}

/// The root: provides the Library store and paints the pills, as the panel
/// composes them.
fn pills_app(props: FixtureProps) -> Element {
    let FixtureProps {
        sender,
        collections,
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
        LibraryEmbyPills { collections }
    }
}

struct Fixture {
    doc: DioxusDocument,
    state: LibraryState,
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

    /// The painted pills' (key, pressed) pairs, in paint order.
    fn pressed(&self) -> Vec<(String, String)> {
        let dom = self.doc.inner();
        dom.query_selector_all("[data-library-emby-pill]")
            .expect("the selector is valid")
            .into_iter()
            .filter_map(|id| {
                let node = dom.get_node(id).expect("the node is in the document");
                let key = node
                    .attr(LocalName::from("data-library-emby-pill"))
                    .map(str::to_owned)?;
                let pressed = node
                    .attr(LocalName::from("aria-pressed"))
                    .map(str::to_owned)?;
                Some((key, pressed))
            })
            .collect()
    }

    /// The Library commands the core link received, in send order.
    fn library_commands(&mut self) -> Vec<mbv::GuiLibraryCommand> {
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
fn built(list: &GuiEmbyList) -> Fixture {
    let (sender, receiver) = mpsc::channel::<CoreInput>();
    let handle = Rc::new(RefCell::new(None));
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(
            pills_app,
            FixtureProps {
                sender,
                collections: list
                    .collections
                    .clone()
                    .expect("the feed-group destination carries its collections"),
                handle: Rc::clone(&handle),
            },
        ),
        DocumentConfig {
            viewport: Some(Viewport::new(1200, 900, 1.0, ColorScheme::Light)),
            ..DocumentConfig::default()
        },
    );
    doc.initial_build();
    let state = handle
        .borrow_mut()
        .take()
        .expect("the root handed the store over");
    let mut fixture = Fixture {
        doc,
        state,
        receiver,
    };
    fixture.poll_until_idle();
    fixture
}

/// The carried Emby content in the feed-group mode with `All` selected (row
/// 8.1).
fn emby_frame() -> GuiEmbyList {
    GuiEmbyList {
        lib_idx: 3,
        library_name: "Movies".to_owned(),
        collections: Some(GuiCollectionsFrame {
            target: mbv::GuiCollectionTarget::EmbyFeed(3),
            all: true,
            entries: vec![GuiCollectionEntry {
                key: "folder-1".to_owned(),
                name: "Family".to_owned(),
                checked: false,
            }],
            noun: mbv::GuiCollectionNoun::Folders,
            order: Some(mbv::GuiDateOrder::Newest),
        }),
        collection_names: vec!["All".to_owned()],
        feed_group: true,
        loading: false,
        failed: false,
        rows: Vec::new(),
    }
}

/// Contract (`LibraryEmbyPills.test.ts`): the pills paint the core's
/// selection and never none — every pill carries `aria-pressed`, and the
/// selected one is the core's.
#[test]
fn the_pills_paint_the_cores_selection_and_never_none() {
    let fixture = built(&emby_frame());

    assert_eq!(
        fixture.pressed(),
        [
            (String::new(), "true".to_owned()),
            ("folder-1".to_owned(), "false".to_owned()),
        ]
    );
}

/// Contract (`LibraryEmbyPills.test.ts`): a press sends the collection-toggle
/// command with the pill's folder.
#[test]
fn a_press_sends_the_set_feed_group_command_with_the_pills_group() {
    let mut fixture = built(&emby_frame());
    let state = fixture.state.clone();
    let collections = emby_frame()
        .collections
        .expect("the feed-group destination carries its collections");
    fixture.in_runtime(move || {
        state.press_collection_pill(&collections, "folder-1");
    });

    assert_eq!(
        fixture.library_commands(),
        [mbv::GuiLibraryCommand::CollectionToggle {
            target: mbv::GuiCollectionTarget::EmbyFeed(3),
            key: "folder-1".to_owned(),
        }]
    );
}

/// Contract (`LibraryEmbyPills.test.ts`): the keys' pill request sends the
/// collection-toggle command — the mounted effect consumes the request, so one
/// key sends one command.
#[test]
fn the_keys_pill_request_sends_the_set_feed_group_command() {
    let mut fixture = built(&emby_frame());
    let state = fixture.state.clone();
    fixture.in_runtime(move || {
        let mut emby_ui = state.emby_ui;
        emby_ui.with_mut(|ui| {
            ui.pill_select = Some("folder-1".to_owned());
        });
    });

    assert_eq!(
        fixture.library_commands(),
        [mbv::GuiLibraryCommand::CollectionToggle {
            target: mbv::GuiCollectionTarget::EmbyFeed(3),
            key: "folder-1".to_owned(),
        }]
    );
}
