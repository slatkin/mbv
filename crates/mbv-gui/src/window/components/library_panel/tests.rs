//! Headless tests of the Library panel (D16): the contracts of
//! `LibraryPanel.test.ts`. The destination painters mount for real (the pill
//! rows since 7.2–7.4, the Heroes since 7.5–7.6, the destination lists since
//! 7.7), so the mount contracts assert each painter's own root hook, and the
//! rails' own region contracts ride those rows' tests. The presses are not
//! fireable headlessly: the outside-press rule is the panel's handler
//! function, tested against the mock core link, and the
//! backdrop's close is the interaction state's own contract
//! (`window/library/state/tests.rs`) driven through the painted overlay's
//! reaction. The core channel is a mock pair; no test opens a window,
//! sleeps, or reaches the core.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, mpsc};

use blitz_dom::{Document as _, LocalName};
use blitz_traits::shell::{ColorScheme, Viewport};
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};
use mbv_config::GuiPanelWidths;

use super::LibraryPanel;
pub(super) use super::press_in_panel;

use crate::bridge::frame::library::{GuiLibraryDetail, GuiLibraryList, GuiLibraryRowMenuFrame};
use crate::core_thread::CoreInput;
use crate::window::arrangement::GuiArrangement;
use crate::window::channel::CoreLink;
use crate::window::clock::FiredTimers;
use crate::window::clock::{Clock, RecordingClock};
use crate::window::components::art_image::{ArtEvents, ScaleFactor};
use crate::window::layout::LayoutEpoch;
use crate::window::library::state::LibraryState;
use crate::window::resize::PanelWidthsStore;
use crate::window::sections::{SectionRef, Sections};

/// The context handle the root hands the test: the Library store and the
/// layout epoch the re-measures read.
type FixtureHandles = (LibraryState, LayoutEpoch);

/// The layout rules the fixture resolves with, in place of the app
/// stylesheet a real window loads: the handful of utilities the panel tree
/// sizes its boxes with (the same rules the player panel's harness uses), so
/// the `Truncate` measures read laid-out widths.
const FIXTURE_CSS: &str = concat!(
    "[data-slot] { width: 100%; }",
    ".flex { display: flex; }",
    ".flex-col { flex-direction: column; }",
    ".items-center { align-items: center; }",
    ".justify-center { justify-content: center; }",
    ".justify-between { justify-content: space-between; }",
    ".min-w-0 { min-width: 0; }",
    ".flex-1 { flex: 1 1 0%; }",
    ".flex-none { flex: none; }",
    ".shrink-0 { flex-shrink: 0; }",
    ".w-full { width: 100%; }",
    ".h-full { height: 100%; }",
    ".block { display: block; }",
    ".relative { position: relative; }",
    ".inset-0 { inset: 0; }",
    ".z-10 { z-index: 10; }",
    ".m-3 { margin: 12px; }",
    ".absolute { position: absolute; }",
    ".fixed { position: fixed; }",
    ".overflow-hidden { overflow: hidden; }",
    ".truncate { width: 100%; }",
);

/// The harness's props: the arrangement the panel paints, the mock core
/// sender, and the handle the test reads the store through.
#[derive(Clone)]
struct FixtureProps {
    sender: mpsc::Sender<CoreInput>,
    arrangement: GuiArrangement,
    handle: Rc<RefCell<Option<FixtureHandles>>>,
}

/// The root: provides the Library store, the width store, and the layout
/// epoch, and paints the panel, as the app root composes it in its Library
/// column.
fn panel_app(props: FixtureProps) -> Element {
    let FixtureProps {
        sender,
        arrangement: arrangement_value,
        handle,
    } = props;
    let clock: Arc<dyn Clock> = Arc::new(RecordingClock::new());
    // The art painter's contexts (the Heroes' art painter consumes them),
    // over the clock the store holds. The stores hook once, as the art
    // painter's own harness holds them.
    let events = use_hook(ArtEvents::new);
    let fired = use_hook(FiredTimers::new);
    provide_context(events);
    provide_context(fired);
    provide_context(ScaleFactor(Signal::new(1.0)));
    provide_context(Arc::clone(&clock));
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
    // The width store with a saver that records nothing: the tests drag no
    // divider.
    provide_context(PanelWidthsStore::new(
        GuiPanelWidths {
            queue: None,
            list: None,
        },
        Arc::new(|_: &GuiPanelWidths| {}),
    ));
    let epoch = use_hook(LayoutEpoch::new);
    provide_context(epoch);
    // The divider drag store the Wide list column's divider drives (the app
    // root provides it; the tests drag no divider).
    provide_context(use_hook(
        crate::window::components::panel_divider::DividerDrag::new,
    ));
    handle.borrow_mut().replace((state, epoch));
    rsx! {
        LibraryPanel {
            arrangement: arrangement_value,
            list_width: 368.0,
            library_column: 1200.0,
        }
    }
}

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

    /// Open the Narrow overlay, as the row menu's View Detail does: the only
    /// way it paints.
    fn pick(&mut self) {
        let mut ui = self.state.ui;
        self.in_runtime(move || ui.with_mut(|ui| ui.hero_dismissed = false));
    }

    /// Resolve the document's style and layout, as the renderer's frame
    /// would, so the `Truncate` measures read laid-out widths, and let the
    /// re-measures the epoch bump drives land.
    fn lay_out(&mut self) {
        self.doc.inner.borrow_mut().resolve(0.0);
        let mut epoch = self.epoch;
        self.doc.vdom.in_runtime(move || epoch.bump());
        self.poll_until_idle();
    }

    /// Carry the Library sections, as the core's batch would, and let the
    /// panel repaint over the laid-out boxes.
    fn carry(&mut self, list: GuiLibraryList, detail: GuiLibraryDetail) {
        let mut sections = self.state.sections.clone();
        self.in_runtime(move || {
            sections.library_list.set(Some(SectionRef::new(list)));
            sections.library_detail.set(Some(SectionRef::new(detail)));
        });
        self.lay_out();
    }

    /// Carry the right-click item menu on its section, as the core's batch
    /// would, and resolve the menu card's laid-out widths.
    fn carry_row_menu(&mut self, item_id: &str) {
        let item_id = item_id.to_owned();
        let mut sections = self.state.sections.clone();
        self.in_runtime(move || {
            sections.row_menu.set(Some(GuiLibraryRowMenuFrame {
                row: mbv::GuiLibraryMenuRow::Item(item_id),
                entries: vec![crate::bridge::frame::library::GuiMusicMenuEntryFrame {
                    label: "Play".to_owned(),
                    key_hint: None,
                    index: 0,
                }],
            }));
        });
        self.lay_out();
    }

    /// Whether the selector names a node in the painted tree.
    fn presents(&self, selector: &str) -> bool {
        matches!(self.doc.inner().query_selector(selector), Ok(Some(_)))
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
    fn attr_of(&self, selector: &str, attribute: LocalName) -> Option<String> {
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

    /// Whether the first node the selector names clips its descendants, by its
    /// `class` or its `style` attribute.
    fn clips_overflow(&self, selector: &str) -> bool {
        ["class", "style"].into_iter().any(|attribute| {
            self.attr_of(selector, LocalName::from(attribute))
                .is_some_and(|value| value.contains("overflow"))
        })
    }

    /// The laid-out client rect of the first node the selector names.
    fn rect_of(&self, selector: &str) -> blitz_dom::BoundingRect {
        let dom = self.doc.inner();
        let id = dom
            .query_selector(selector)
            .expect("the selector is valid")
            .unwrap_or_else(|| panic!("{selector} paints"));
        dom.get_client_bounding_rect(id)
            .expect("the node is laid out")
    }

    /// The `data-library-region` value of every region, in paint order.
    fn regions(&self) -> Vec<String> {
        let dom = self.doc.inner();
        dom.query_selector_all("[data-library-region]")
            .expect("the selector is valid")
            .into_iter()
            .filter_map(|id| {
                dom.get_node(id)
                    .expect("the node is in the document")
                    .attr(LocalName::from("data-library-region"))
                    .map(str::to_owned)
            })
            .collect()
    }

    /// The Library commands the core link received, in send order.
    fn library_commands(&mut self) -> Vec<mbv::GuiLibraryCommand> {
        self.receiver
            .try_iter()
            .filter_map(|input| match input {
                CoreInput::Library(mbv::GuiLibraryCommand::RowsInView { .. }) => None,
                CoreInput::Library(command) => Some(command),
                _ => None,
            })
            .collect()
    }
}

/// Build the document over the panel's arrangement and let its first render
/// and effects land.
fn built(arrangement: GuiArrangement) -> Fixture {
    let (sender, receiver) = mpsc::channel::<CoreInput>();
    let handle = Rc::new(RefCell::new(None));
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(
            panel_app,
            FixtureProps {
                sender,
                arrangement,
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

/// The frame's Status bar facts: nothing armed. The panel paints the carried
/// section, so the tests seed it idle to keep the Search field and the
/// location painting.
fn seed_idle_status(fixture: &mut Fixture) {
    let mut sections = fixture.state.sections.clone();
    fixture.in_runtime(move || {
        sections
            .status
            .set(crate::bridge::frame::chrome::GuiStatusBar {
                prefix_armed: false,
            });
    });
}

pub(super) mod fixtures;

mod destination_reset;
mod destinations;
mod overlay;
mod press;
