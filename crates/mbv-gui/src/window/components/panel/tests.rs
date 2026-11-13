//! Headless tests of the player panel (D16): the contracts of
//! `Panel.test.ts` that `Panel` itself owns — the four states the container
//! size and the core's two toggles pick, and the composition each state
//! paints. The assertions over each child's own painted content (the strip's
//! next-item line, the folded queue's summary, the rail's ring and remaining
//! time, the regions the transport reports) arrived with the children's
//! rows: 5.2 (`QueuePlaybackPanel`), 5.5 (`QueuePanel`), 5.6 (`QueueFooter`),
//! and 5.7 (`FoldedQueue`, `CompactStrip`, `CollapsedRail`). The core
//! channel is a mock pair; no test opens a window, sleeps, or reaches the core.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};

use blitz_dom::Document as _;
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use super::Panel;
use crate::bridge::frame::GuiPanelToggles;
use crate::bridge::layout::{LayoutSize, QueuePlaybackTargetId, RegionTarget};
use crate::core_thread::CoreInput;
use crate::window::channel::CoreLink;
use crate::window::clock::{Clock, FiredTimers, RecordingClock};
use crate::window::components::art_image::{ArtEvents, ScaleFactor};
use crate::window::layout::LayoutEpoch;
use crate::window::playhead::{Playhead, SystemTimes, Times};
use crate::window::sections::Sections;

/// A container tall enough for the expanded artboard (the test's `TALL`).
const TALL: LayoutSize = LayoutSize {
    width: 640.0,
    height: 900.0,
};

/// A container shorter than the expanded artboard's 520 px (the test's
/// `SHORT`).
const SHORT: LayoutSize = LayoutSize {
    width: 640.0,
    height: 480.0,
};

#[derive(Clone)]
struct FixtureProps {
    sender: Sender<CoreInput>,
    container: LayoutSize,
    handle: Rc<RefCell<Option<FixtureHandles>>>,
}

/// The context handles the root hands the test: the section stores, the
/// container size the test drives, and the playhead.
type FixtureHandles = (Sections, Signal<LayoutSize>, Playhead);

/// The root: provides every context the panel's tree consumes and paints the
/// panel, as `App` composes it inside its Queue column.
fn panel_app(props: FixtureProps) -> Element {
    provide_context(CoreLink::new(props.sender));
    let reporter = crate::window::layout::use_layout_reporter();
    provide_context(reporter);
    let epoch = use_hook(LayoutEpoch::new);
    provide_context(epoch);
    let sections = use_hook(Sections::new);
    provide_context(sections.clone());
    let clock: Arc<dyn Clock> = Arc::new(RecordingClock::new());
    // The art painter's retry clock (D7): the same context the app root
    // provides.
    provide_context(Arc::clone(&clock));
    let times: Arc<dyn Times> = Arc::new(SystemTimes::new());
    let playhead = use_hook(move || Playhead::new(Arc::clone(&clock), times));
    provide_context(playhead.clone());
    provide_context(use_hook(ArtEvents::new));
    provide_context(use_hook(FiredTimers::new));
    provide_context(ScaleFactor(Signal::new(1.0)));
    let container = use_signal(move || props.container);
    props
        .handle
        .borrow_mut()
        .replace((sections, container, playhead));
    rsx! {
        Panel {
            container: container.cloned(),
            library_visible: false,
        }
    }
}

struct Fixture {
    doc: DioxusDocument,
    sections: Sections,
    receiver: Receiver<CoreInput>,
}

impl Fixture {
    /// Whether the selector names a node in the painted tree.
    fn presents(&self, selector: &str) -> bool {
        let dom = self.doc.inner();
        matches!(dom.query_selector(selector), Ok(Some(_)))
    }

    /// Poll the document until the effects have run and their re-renders
    /// landed.
    fn poll_until_idle(&mut self) {
        for _ in 0..8 {
            if !self.doc.poll(None) {
                return;
            }
        }
        panic!("document did not become idle");
    }

    /// The region targets of the last merged layout report.
    fn last_reported_targets(&self) -> Vec<RegionTarget> {
        self.receiver
            .try_iter()
            .filter_map(|input| match input {
                CoreInput::Layout(report) => Some(report),
                _ => None,
            })
            .last()
            .map(|report| {
                report
                    .regions
                    .into_iter()
                    .map(|region| region.target)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Write the core's two toggles, as the drain task would, and let the
    /// re-render land.
    fn set_toggles(&mut self, collapsed: bool, queue_folded: bool) {
        let mut sections = self.sections.clone();
        self.doc.vdom.in_runtime(move || {
            sections.panel.set(GuiPanelToggles {
                collapsed,
                queue_folded,
                ui_reset_epoch: 0,
            });
        });
        self.poll_until_idle();
    }
}

fn built(container: LayoutSize) -> Fixture {
    let (sender, receiver) = channel::<CoreInput>();
    let handle = Rc::new(RefCell::new(None));
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(
            panel_app,
            FixtureProps {
                sender,
                container,
                handle: Rc::clone(&handle),
            },
        ),
        DocumentConfig::default(),
    );
    doc.initial_build();
    let (sections, _container, _playhead) = handle
        .borrow_mut()
        .take()
        .expect("the root handed the section stores over");
    let mut fixture = Fixture {
        doc,
        sections,
        receiver,
    };
    fixture.poll_until_idle();
    fixture
}

/// Contract (`Panel.test.ts`): a tall container renders the expanded
/// artboard — the queue rows (not their folded summary) and the footer line
/// mount under the `expanded` state. The artboard's own painted content
/// (the title, the rows, the footer pills) asserts with its row's landing.
#[test]
fn a_tall_container_renders_the_expanded_artboard() {
    let fixture = built(TALL);
    assert!(
        fixture.presents("[data-panel-state=\"expanded\"]"),
        "the expanded state paints"
    );
    assert!(
        fixture.presents("[data-queue-list]"),
        "the expanded artboard paints the queue rows"
    );
    assert!(
        fixture.presents("[data-footer-line]"),
        "the expanded artboard paints the footer line"
    );
    assert!(
        !fixture.presents("[data-panel-child=\"FoldedQueue\"]"),
        "the expanded artboard does not fold the queue"
    );
}

/// Contract (`Panel.test.ts`): a container shorter than 520 px renders the
/// compact strip — the strip mounts under the `compactStrip` state and the
/// expanded artboard does not.
#[test]
fn a_container_shorter_than_520px_renders_the_compact_strip() {
    let fixture = built(SHORT);
    assert!(
        fixture.presents("[data-panel-state=\"compactStrip\"]"),
        "the compact strip state paints"
    );
    assert!(
        fixture.presents("[data-target=\"visualSlot\"]"),
        "the compact strip paints the strip"
    );
    assert!(
        !fixture.presents("[data-target=\"playbackTarget\"]"),
        "the compact strip paints no expanded artboard"
    );
}

/// Contract (`Panel.test.ts`): the queue-fold toggle replaces the rows with
/// the folded summary — the `expandedWithoutQueue` state paints the folded
/// queue and not the queue rows.
#[test]
fn the_queue_fold_toggle_replaces_the_queue_rows_with_the_folded_summary() {
    let mut fixture = built(TALL);
    fixture.set_toggles(false, true);
    assert!(
        fixture.presents("[data-panel-state=\"expandedWithoutQueue\"]"),
        "the folded state paints"
    );
    assert!(
        fixture.presents("[data-target=\"queueFold\"]"),
        "the folded state paints the one-line summary"
    );
    assert!(
        !fixture.presents("[data-queue-list]"),
        "the folded state paints no queue rows"
    );
}

/// Contract (`Panel.test.ts`): the collapse toggle renders the 72 px rail —
/// the rail mounts with no state wrapper around it (the rail paints its own
/// `data-panel-state`), and no expanded artboard mounts.
#[test]
fn the_collapse_toggle_renders_the_rail() {
    let mut fixture = built(TALL);
    fixture.set_toggles(true, false);
    assert!(
        fixture.presents("[data-panel-state=\"rail\"]"),
        "the collapsed state paints the rail"
    );
    // The rail paints its own state attribute, and no state wrapper mounts
    // around it (the Svelte contract's `expanded`/`compactStrip` wrappers
    // name the other two states).
    assert!(
        !fixture.presents("[data-panel-state=\"expanded\"]"),
        "the rail mounts no expanded artboard"
    );
    assert!(
        !fixture.presents("[data-target=\"playbackTarget\"]"),
        "the rail paints no expanded artboard"
    );
}

/// Contract (`Panel.test.ts`): the compact strip reports the still's slot
/// and the transport's two controls — `visualSlot`, `playPause`, `next` —
/// its whole region list, since no expanded artboard mounts beside it.
#[test]
fn the_compact_strip_reports_the_still_and_transport_regions() {
    let fixture = built(SHORT);

    assert_eq!(
        fixture.last_reported_targets(),
        vec![
            RegionTarget::QueuePlayback(QueuePlaybackTargetId::VisualSlot),
            RegionTarget::QueuePlayback(QueuePlaybackTargetId::PlayPause),
            RegionTarget::QueuePlayback(QueuePlaybackTargetId::Next),
        ]
    );
}

/// Contract (`Panel.test.ts`): the folded queue reports the expand control's
/// `queueFold` target — the line's one interactive target.
#[test]
fn the_folded_queue_reports_the_expand_control_region() {
    let mut fixture = built(TALL);
    fixture.set_toggles(false, true);

    let targets = fixture.last_reported_targets();
    assert!(
        targets.contains(&RegionTarget::QueuePlayback(
            QueuePlaybackTargetId::QueueFold
        )),
        "{targets:?}"
    );
}

/// Contract (`Panel.test.ts`): the rail reports the transport's two controls,
/// the artwork slot, and the expand control, in paint order — its whole
/// region list, since the queue and the footer belong to the expanded
/// artboard.
#[test]
fn the_rail_reports_the_transport_artwork_and_expand_regions() {
    let mut fixture = built(TALL);
    fixture.set_toggles(true, false);

    assert_eq!(
        fixture.last_reported_targets(),
        vec![
            RegionTarget::QueuePlayback(QueuePlaybackTargetId::PlayPause),
            RegionTarget::QueuePlayback(QueuePlaybackTargetId::Next),
            RegionTarget::QueuePlayback(QueuePlaybackTargetId::VisualSlot),
            RegionTarget::QueuePlayback(QueuePlaybackTargetId::Collapse),
        ]
    );
}
