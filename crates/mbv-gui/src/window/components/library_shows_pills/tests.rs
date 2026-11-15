//! Headless tests of the Shows mode pills (D16): the contracts of
//! `LibraryShowsPills.test.ts` — the pills paint the core's single-select
//! view mode with `aria-pressed` under the shared hook, and a press sends
//! the set-mode command with that pill's mode. The IPC of the earlier client
//! tests is the mock core link here; the asserted command is the same
//! neutral shape. A headless document cannot fire a DOM click, so the press
//! contract rides the state method the component's own handler calls one
//! line each (the list's precedent). The core channel is a mock pair; no
//! test opens a window, sleeps, or reaches the core.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, mpsc};

use blitz_dom::{Document as _, LocalName};
use blitz_traits::shell::{ColorScheme, Viewport};
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use super::LibraryShowsPills;
use crate::bridge::frame::library::{GuiShowsList, GuiShowsModeFrame, GuiShowsPillFrame};
use crate::core_thread::CoreInput;
use crate::window::channel::CoreLink;
use crate::window::clock::{Clock, RecordingClock};
use crate::window::library::state::LibraryState;
use crate::window::sections::Sections;

/// The harness's props: the carried Shows pills and library index, the mock
/// core sender, and the handle the test reads the store through.
#[derive(Clone)]
struct FixtureProps {
    sender: mpsc::Sender<CoreInput>,
    pills: Vec<GuiShowsPillFrame>,
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
        LibraryShowsPills { pills, lib_idx }
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
fn built(list: &GuiShowsList) -> Fixture {
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

/// The carried Shows content with `all` selected (design D12).
fn shows_frame() -> GuiShowsList {
    let pill = |mode, name: &str, selected| GuiShowsPillFrame {
        mode,
        name: name.to_owned(),
        selected,
        collection: false,
    };
    GuiShowsList {
        lib_idx: 0,
        pills: vec![
            pill(GuiShowsModeFrame::Latest, "Latest", false),
            pill(GuiShowsModeFrame::Upcoming, "Upcoming", false),
            pill(GuiShowsModeFrame::All, "All series", true),
        ],
        collection_names: vec!["All series".to_owned()],
        loading: false,
        series: Vec::new(),
        upcoming_failed: false,
        rows: Vec::new(),
    }
}

/// Contract (`LibraryShowsPills.test.ts`): the pills paint the core's
/// selection and never none — every pill carries `aria-pressed`, and the
/// selected one is the core's active mode.
#[test]
fn the_pills_paint_the_cores_selection_and_never_none() {
    let fixture = built(&shows_frame());

    assert_eq!(
        fixture.pressed(),
        [
            ("latest".to_owned(), "false".to_owned()),
            ("upcoming".to_owned(), "false".to_owned()),
            ("all".to_owned(), "true".to_owned()),
        ]
    );
}

/// Contract (`LibraryShowsPills.test.ts`): a press sends the set-mode
/// command with the pill's mode.
#[test]
fn a_press_sends_the_set_mode_command_with_the_pills_mode() {
    let mut fixture = built(&shows_frame());
    let state = fixture.state.clone();
    fixture.in_runtime(move || state.select_shows_pill(0, GuiShowsModeFrame::Upcoming));

    assert_eq!(
        fixture.library_commands(),
        [mbv::GuiLibraryCommand::ShowsSetMode {
            lib_idx: 0,
            mode: mbv::GuiShowsMode::Upcoming,
        }]
    );
}
