//! Headless tests of the Music pills (D16): the contracts of
//! `LibraryPills.test.ts` — the pill row paints the core's carried selection
//! with `aria-pressed` and no check glyph, paints no clear control, a press
//! sends the select-collection command, the keys' Space request becomes the
//! same command, and the focus marks the pill the keys hold and sends
//! nothing for it. The IPC of the earlier tests is the mock core link
//! here; the asserted commands are the same neutral shapes. A headless
//! document cannot fire a DOM click, so the press contract rides the state
//! method the component's own handler calls one line each (the list's
//! precedent); the Space contract rides the mounted component's effect. The
//! core channel is a mock pair; no test opens a window, sleeps, or reaches
//! the core.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, mpsc};

use blitz_dom::{Document as _, LocalName};
use blitz_traits::shell::{ColorScheme, Viewport};
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use super::LibraryPills;
use crate::bridge::frame::library::{GuiMusicList, GuiMusicPillFrame};
use crate::core_thread::CoreInput;
use crate::window::channel::CoreLink;
use crate::window::clock::{Clock, RecordingClock};
use crate::window::library::state::LibraryState;
use crate::window::sections::Sections;

/// The harness's props: the carried Music content, the mock core sender, and
/// the handle the test reads the store through.
#[derive(Clone)]
struct FixtureProps {
    sender: mpsc::Sender<CoreInput>,
    pills: Vec<GuiMusicPillFrame>,
    lib_idx: usize,
    handle: Rc<RefCell<Option<LibraryState>>>,
}

/// The root: provides the Library store and paints the pills, as the panel
/// composes them.
fn pills_app(props: FixtureProps) -> Element {
    let FixtureProps {
        sender,
        pills,
        lib_idx,
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
        LibraryPills { pills, lib_idx }
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
fn built(list: &GuiMusicList) -> Fixture {
    let (sender, receiver) = mpsc::channel::<CoreInput>();
    let handle = Rc::new(RefCell::new(None));
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(
            pills_app,
            FixtureProps {
                sender,
                pills: list.pills.clone(),
                lib_idx: list.lib_idx,
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

/// The carried Music content: Hip-Hop selected, Jazz and Soul clear, in a
/// Grouped Music library.
fn music() -> GuiMusicList {
    GuiMusicList {
        lib_idx: 2,
        pills: vec![
            GuiMusicPillFrame {
                group_id: "g1".to_owned(),
                name: "Hip-Hop".to_owned(),
                selected: true,
                collection: true,
            },
            GuiMusicPillFrame {
                group_id: "g2".to_owned(),
                name: "Jazz".to_owned(),
                selected: false,
                collection: true,
            },
            GuiMusicPillFrame {
                group_id: "g3".to_owned(),
                name: "Soul".to_owned(),
                selected: false,
                collection: true,
            },
        ],
        collection_names: vec!["Hip-Hop".to_owned()],
        loading: false,
        failure: None,
        artists: Vec::new(),
    }
}

/// Contract (`LibraryPills.test.ts`): the one selected pill carries
/// `aria-pressed`, the clear ones do not, and no pill paints a check glyph.
#[test]
fn marks_the_one_selected_pill_with_aria_pressed_and_no_check_glyph() {
    let fixture = built(&music());

    assert_eq!(
        fixture
            .attr_of(
                "[data-library-pill=\"g1\"]",
                LocalName::from("aria-pressed")
            )
            .as_deref(),
        Some("true")
    );
    assert_eq!(
        fixture
            .attr_of(
                "[data-library-pill=\"g2\"]",
                LocalName::from("aria-pressed")
            )
            .as_deref(),
        Some("false")
    );
    assert!(
        !fixture.presents("[data-library-pill=\"g1\"] svg"),
        "no check glyph paints"
    );
}

/// Contract (`LibraryPills.test.ts`): the row paints no selected count or
/// clear control — the TUI's Grouped Music form has none.
#[test]
fn paints_no_selected_count_or_clear_control() {
    let fixture = built(&music());

    assert!(
        !fixture.presents("[data-library-pill-clear]"),
        "no clear control paints"
    );
}

/// Contract (`LibraryPills.test.ts`): a press on a pill toggles it — the
/// collection-toggle command with the row's library index and the pill's
/// group id.
#[test]
fn a_press_selects_the_pressed_pill() {
    let mut fixture = built(&music());
    let state = fixture.state.clone();
    fixture.in_runtime(move || state.select_library_pill(2, "g3".to_owned()));

    assert_eq!(
        fixture.library_commands(),
        [mbv::GuiLibraryCommand::CollectionToggle {
            target: mbv::GuiCollectionTarget::Music(2),
            key: "g3".to_owned(),
        }]
    );
}

/// Contract (`LibraryPills.test.ts`): a pill request selects that pill — the
/// mounted effect turns the request into the same command, and the consumed
/// request sends nothing again.
#[test]
fn a_pill_request_selects_that_pill() {
    let mut fixture = built(&music());
    let state = fixture.state.clone();
    fixture.in_runtime(move || state.request_library_pill_select("g2".to_owned()));

    assert_eq!(
        fixture.library_commands(),
        [mbv::GuiLibraryCommand::CollectionToggle {
            target: mbv::GuiCollectionTarget::Music(2),
            key: "g2".to_owned(),
        }]
    );
}
