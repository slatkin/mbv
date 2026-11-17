//! Headless tests of the Feeds pills (D16): the contracts of
//! `LibraryFeedsPills.test.ts` — the pills paint the core's single-select
//! view with `aria-pressed` under the shared hook, and a press sends the
//! set-pill command with that pill's object, as the frame carries it and
//! never a stringified pill. The IPC of the earlier tests is the mock
//! core link here; the asserted command is the same neutral shape. A
//! headless document cannot fire a DOM click, so the press contract rides
//! the state method the component's own handler calls one line each (the
//! list's precedent). The core channel is a mock pair; no test opens a
//! window, sleeps, or reaches the core.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, mpsc};

use blitz_dom::{Document as _, LocalName};
use blitz_traits::shell::{ColorScheme, Viewport};
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use super::LibraryFeedsPills;
use crate::bridge::frame::library::{GuiFeedsList, GuiPillFrame};
use crate::bridge::library::GuiFeedPill;
use crate::core_thread::CoreInput;
use crate::window::channel::CoreLink;
use crate::window::clock::{Clock, RecordingClock};
use crate::window::library::state::LibraryState;
use crate::window::sections::Sections;

/// The harness's props: the carried Feeds pills, the mock core sender, and
/// the handle the test reads the store through.
#[derive(Clone)]
struct FixtureProps {
    sender: mpsc::Sender<CoreInput>,
    pills: Vec<GuiPillFrame<GuiFeedPill>>,
    handle: Rc<RefCell<Option<LibraryState>>>,
}

/// The root: provides the Library store and paints the pills, as the panel
/// composes them.
fn pills_app(props: FixtureProps) -> Element {
    let FixtureProps {
        sender,
        pills,
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
        LibraryFeedsPills { pills }
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
        dom.query_selector_all("[data-library-pill]")
            .expect("the selector is valid")
            .into_iter()
            .filter_map(|id| {
                let node = dom.get_node(id).expect("the node is in the document");
                let key = node
                    .attr(LocalName::from("data-library-pill"))
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
fn built(list: &GuiFeedsList) -> Fixture {
    let (sender, receiver) = mpsc::channel::<CoreInput>();
    let handle = Rc::new(RefCell::new(None));
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(
            pills_app,
            FixtureProps {
                sender,
                pills: list.pills.clone(),
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

/// The carried Feeds content with the subscription view selected (design
/// D12).
fn frame() -> GuiFeedsList {
    let pill = |pill, name: &str, selected| GuiPillFrame {
        pill,
        name: name.to_owned(),
        selected,
        collection: false,
    };
    GuiFeedsList {
        pills: vec![
            pill(GuiFeedPill::Latest, "Latest", false),
            pill(
                GuiFeedPill::Subscription("https://sub.one".to_owned()),
                "Sub One",
                true,
            ),
        ],
        loading: false,
        failed: false,
        rows: Vec::new(),
    }
}

/// Contract (`LibraryFeedsPills.test.ts`): the pills paint the core's
/// selection and never none — every pill carries `aria-pressed`, keyed by
/// the pill's paint key.
#[test]
fn the_pills_paint_the_cores_selection_and_never_none() {
    let fixture = built(&frame());

    assert_eq!(
        fixture.pressed(),
        [
            ("latest".to_owned(), "false".to_owned()),
            ("subscription:https://sub.one".to_owned(), "true".to_owned()),
        ]
    );
}

/// Contract (`LibraryFeedsPills.test.ts`): a press sends the set-pill
/// command with the frame's pill object — the All pill's own object, as
/// the frame carries it.
#[test]
fn a_press_sends_the_set_pill_command_with_the_frames_pill_object() {
    let mut fixture = built(&frame());
    let state = fixture.state.clone();
    fixture.in_runtime(move || state.select_feeds_pill(GuiFeedPill::Latest));

    assert_eq!(
        fixture.library_commands(),
        [mbv::GuiLibraryCommand::CollectionAll {
            target: mbv::GuiCollectionTarget::Feeds,
        }]
    );
}
