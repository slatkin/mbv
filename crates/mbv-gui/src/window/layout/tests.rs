//! Tests of the layout reporter's merge (D9): the port's ordering contract —
//! the merged report carries the contributions in key-insertion order, the
//! TypeScript module's `Map` order, so a key registered later paints over an
//! earlier key's regions in the core's hit map (the hit map's last push
//! wins) — and the re-set semantics: re-reporting an existing key updates it
//! in place instead of moving it last. Headless documents whose root built
//! the reporter over a recorded channel; no window, no sleep. The last two
//! tests cover the re-measure (D9, D12): the window's own viewport and section
//! batch, its poll, resolve, and presented-frame sequence, run headless, must
//! leave the measured size and the measured region at the values the window
//! lays out — at the mount, after a resize, and after a batch.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc;

use blitz_dom::Document as _;
use blitz_traits::shell::{ColorScheme, Viewport};
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use super::{
    LayoutContribution, LayoutEpoch, LayoutKey, LayoutReporter, RemeasureRequest, element_region,
    note_layout_change, note_presented_frame, track_container_size, use_layout_reporter,
};
use crate::bridge::layout::{
    LayoutRect, LayoutRegion, LayoutReport, LayoutSize, QueuePlaybackTargetId, QueueTargetId,
    RegionTarget,
};
use crate::core_thread::CoreInput;
use crate::window::channel::CoreLink;

/// One region's contribution under a distinct target, so the assertions can
/// tell the keys apart.
fn contribution(target: RegionTarget) -> LayoutContribution {
    LayoutContribution {
        lists: Vec::new(),
        regions: vec![LayoutRegion {
            rect: LayoutRect {
                x: 0.0,
                y: 0.0,
                width: 10.0,
                height: 10.0,
            },
            target,
        }],
    }
}

/// What the root hands the test: the reporter and the core-input receiver.
#[derive(Default)]
struct Handoff {
    reporter: Option<LayoutReporter>,
    inputs: Option<mpsc::Receiver<CoreInput>>,
}

#[derive(Clone)]
struct ReporterProps {
    handoff: Rc<RefCell<Handoff>>,
}

/// The root: builds the reporter over a channel the test reads.
fn reporter_app(props: ReporterProps) -> Element {
    let ReporterProps { handoff } = props;
    let (tx, rx) = mpsc::channel();
    provide_context(CoreLink::new(tx));
    let reporter = use_layout_reporter();
    let mut handoff = handoff.borrow_mut();
    handoff.reporter.replace(reporter);
    handoff.inputs.replace(rx);
    rsx! {}
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

/// A document whose root built the reporter over a channel the test reads.
fn built_reporter() -> (DioxusDocument, LayoutReporter, mpsc::Receiver<CoreInput>) {
    let handoff = Rc::new(RefCell::new(Handoff::default()));
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(
            reporter_app,
            ReporterProps {
                handoff: Rc::clone(&handoff),
            },
        ),
        DocumentConfig::default(),
    );
    doc.initial_build();
    poll_until_idle(&mut doc);
    let mut taken = handoff.borrow_mut();
    let reporter = taken
        .reporter
        .take()
        .expect("the root handed the reporter over");
    let inputs = taken
        .inputs
        .take()
        .expect("the root handed the receiver over");
    (doc, reporter, inputs)
}

/// The last merged report the reporter sent, draining the channel.
fn last_layout(inputs: &mpsc::Receiver<CoreInput>) -> LayoutReport {
    let mut last = None;
    for input in inputs.try_iter() {
        if let CoreInput::Layout(report) = input {
            last = Some(report);
        }
    }
    last.expect("the reporter sent a layout report")
}

/// The merged report's targets, in merge order.
fn merged_targets(report: &LayoutReport) -> Vec<RegionTarget> {
    report.regions.iter().map(|region| region.target).collect()
}

/// Contract (D9): the merged report carries the contributions in
/// key-insertion order — the TypeScript `Map`'s order, not the process's
/// hash order — and a re-report of an existing key updates it in place
/// instead of moving it last. The chooser's key is registered second and
/// must keep merging last over the surface's regions, so a press on a
/// chooser row resolves to the row, never to the region beneath it.
#[test]
fn the_merged_report_keeps_key_insertion_order_across_a_reset() {
    let (mut doc, mut reporter, inputs) = built_reporter();

    let surface = LayoutKey::next();
    let chooser = LayoutKey::next();
    reporter.report_layout(
        surface,
        contribution(RegionTarget::QueuePlayback(
            QueuePlaybackTargetId::VisualSlot,
        )),
    );
    reporter.report_layout(
        chooser,
        contribution(RegionTarget::QueuePlayback(
            QueuePlaybackTargetId::TargetChoice(0),
        )),
    );
    poll_until_idle(&mut doc);

    let report = last_layout(&inputs);
    assert_eq!(
        merged_targets(&report),
        vec![
            RegionTarget::QueuePlayback(QueuePlaybackTargetId::VisualSlot),
            RegionTarget::QueuePlayback(QueuePlaybackTargetId::TargetChoice(0)),
        ]
    );

    // A re-set updates in place (the `Map`'s `set`): the surface key keeps
    // its position, so the chooser's rows still merge last.
    reporter.report_layout(
        surface,
        contribution(RegionTarget::Queue(QueueTargetId::Slot(1))),
    );
    poll_until_idle(&mut doc);

    let report = last_layout(&inputs);
    assert_eq!(
        merged_targets(&report),
        vec![
            RegionTarget::Queue(QueueTargetId::Slot(1)),
            RegionTarget::QueuePlayback(QueuePlaybackTargetId::TargetChoice(0)),
        ]
    );
}

/// What the measuring root hands the test: the two layout-fact contexts the
/// window's drain drives, the container size signal the effect writes, and the
/// signal a section batch's write stands in for.
#[derive(Default)]
struct MeasureHandoff {
    epoch: Option<LayoutEpoch>,
    remeasure: Option<RemeasureRequest>,
    container: Option<Signal<LayoutSize>>,
    half: Option<Signal<bool>>,
}

#[derive(Clone)]
struct MeasureProps {
    handoff: Rc<RefCell<MeasureHandoff>>,
    regions: Rc<RefCell<Option<LayoutRegion>>>,
}

/// The measuring root: the window's own container effect over the window's
/// own epoch and re-measure request, and a region effect shaped like every
/// component's. The box fills the viewport; `half` is the fact a batch writes
/// (a panel state or a row set), and it resizes both the box and its child.
fn measuring_app(props: MeasureProps) -> Element {
    let MeasureProps { handoff, regions } = props;
    let epoch = use_hook(LayoutEpoch::new);
    provide_context(epoch);
    let remeasure = use_hook(RemeasureRequest::new);
    provide_context(remeasure.clone());
    let container: Signal<LayoutSize> = use_signal(LayoutSize::default);
    let mut container_mounted: Signal<Option<Event<MountedData>>> = use_signal(|| None);
    track_container_size(container_mounted, container);
    let half = use_signal(|| false);
    let mut child_mounted: Signal<Option<Event<MountedData>>> = use_signal(|| None);
    let epoch_signal = epoch.0;
    use_effect(move || {
        let _epoch = epoch_signal.cloned();
        let mounted = child_mounted.cloned();
        let regions = Rc::clone(&regions);
        spawn(async move {
            let Some(mounted) = mounted else {
                return;
            };
            let region =
                element_region(&mounted, RegionTarget::Queue(QueueTargetId::RightColumn)).await;
            *regions.borrow_mut() = region;
        });
    });
    let mut handoff = handoff.borrow_mut();
    handoff.epoch.replace(epoch);
    handoff.remeasure.replace(remeasure);
    handoff.container.replace(container);
    handoff.half.replace(half);
    rsx! {
        main { style: "width: 100vw; height: 100vh;",
            div {
                style: if half() {
                    "width: 100%; height: 50%;"
                } else {
                    "width: 100%; height: 100%;"
                },
                onmounted: move |event| container_mounted.set(Some(event)),
                div {
                    style: "width: 50%; height: 100%;",
                    onmounted: move |event| child_mounted.set(Some(event)),
                }
            }
        }
    }
}

/// The measuring window: the document, the contexts its drain would drive, the
/// container signal its effect writes, and the child's measured region.
struct MeasuringWindow {
    doc: DioxusDocument,
    epoch: LayoutEpoch,
    remeasure: RemeasureRequest,
    container: Signal<LayoutSize>,
    half: Signal<bool>,
    regions: Rc<RefCell<Option<LayoutRegion>>>,
}

impl MeasuringWindow {
    /// A window `width` by `height`, opened the way the window opens: the
    /// mount's measure arms one re-measure, the first frame resolves the tree,
    /// and the presented frame's bump measures it.
    fn built(width: u32, height: u32) -> Self {
        let handoff = Rc::new(RefCell::new(MeasureHandoff::default()));
        let regions = Rc::new(RefCell::new(None));
        let mut doc = DioxusDocument::new(
            VirtualDom::new_with_props(
                measuring_app,
                MeasureProps {
                    handoff: Rc::clone(&handoff),
                    regions: Rc::clone(&regions),
                },
            ),
            DocumentConfig {
                viewport: Some(Viewport::new(width, height, 1.0, ColorScheme::Light)),
                ..DocumentConfig::default()
            },
        );
        doc.initial_build();
        let taken = handoff.borrow_mut();
        let mut window = Self {
            epoch: taken.epoch.expect("the root provided the epoch"),
            remeasure: taken
                .remeasure
                .clone()
                .expect("the root provided the re-measure request"),
            container: taken
                .container
                .expect("the root handed the container signal over"),
            half: taken.half.expect("the root handed the toggle over"),
            regions,
            doc,
        };
        // The window's first frame: the mount's measure ran against the
        // unresolved tree and armed a re-measure, this resolve lays the tree
        // out, and the presented frame's bump measures the laid-out box.
        window.settle();
        window.resolve();
        window.present_frame();
        window
    }

    /// Poll until the effects and the tasks they spawn have run, as
    /// blitz-shell's poll does.
    fn settle(&mut self) {
        poll_until_idle(&mut self.doc);
    }

    /// Resolve the document, as blitz-shell's `redraw` does before it reports
    /// the frame.
    fn resolve(&mut self) {
        self.doc.inner.borrow_mut().resolve(0.0);
    }

    /// The window's `WindowEvent::Viewport` work and the frame it asks for.
    fn present_resize(&mut self, width: u32, height: u32) {
        self.doc.inner_mut().viewport_mut().window_size = (width, height);
        self.arm();
        self.settle();
        self.resolve();
        self.present_frame();
    }

    /// A section batch that changed a measured fact: the drain's bump, then the
    /// poll before the frame, the resolve, and the presented frame.
    fn present_batch(&mut self) {
        self.arm();
        self.settle();
        self.resolve();
        self.present_frame();
    }

    /// The drain's layout fact: the epoch bump and the one presented-frame
    /// re-measure it arms.
    fn arm(&mut self) {
        let epoch = self.epoch;
        let remeasure = self.remeasure.clone();
        self.doc
            .vdom
            .in_runtime(move || note_layout_change(epoch, &remeasure));
    }

    /// The drain's `WindowEvent::Frame` work: take a pending request, bump once,
    /// and let the effects measure the resolved layout.
    fn present_frame(&mut self) {
        let epoch = self.epoch;
        let remeasure = self.remeasure.clone();
        self.doc
            .vdom
            .in_runtime(move || note_presented_frame(epoch, &remeasure));
        self.settle();
    }

    /// The container's measured size, read in the runtime as the window's own
    /// root would.
    fn measured(&mut self) -> LayoutSize {
        let container = self.container;
        self.doc.vdom.in_runtime(move || container.cloned())
    }

    /// The child box's measured region.
    fn region(&self) -> LayoutRegion {
        (*self.regions.borrow()).expect("the region effect measured the child")
    }

    /// Whether a re-measure is still waiting for a frame.
    fn pending(&self) -> bool {
        self.remeasure.pending()
    }
}

/// Contract (live run of the resize defect, D9/D12): after a resize the
/// container's measured size and its child's region are the values the window
/// now lays out. Blitz applies the new viewport before it resolves the
/// document, so the measure the viewport bump drives reads the old layout;
/// without the presented-frame re-measure the size signal never changes, and
/// every arrangement derived from it — the panel state, the Queue panel's
/// rows, the Library column — stays one resize stale, which paints differently
/// on every run.
#[test]
fn a_resize_leaves_the_measurements_at_the_resolved_size() {
    let mut window = MeasuringWindow::built(1200, 900);
    assert_eq!(
        window.measured(),
        LayoutSize {
            width: 1200.0,
            height: 900.0
        }
    );

    window.present_resize(820, 610);

    assert_eq!(
        window.measured(),
        LayoutSize {
            width: 820.0,
            height: 610.0
        }
    );
    let region = window.region();
    assert_eq!((region.rect.width, region.rect.height), (410.0, 610.0));
    assert!(!window.pending(), "one re-measure per resize");
}

/// Contract (D9): a section batch that changes a measured fact — a panel state,
/// a row count — leaves the container's measured size and the child's region
/// at the values the batch's own frame lays out. The batch's DOM changes are
/// applied after the poll that runs its effects, so the bump's measure reads
/// the layout from before the change; without the armed re-measure the window
/// keeps the pre-batch geometry until some later fact bumps.
#[test]
fn a_batch_leaves_the_measurements_at_the_resolved_size() {
    let mut window = MeasuringWindow::built(1200, 900);
    let mut half = window.half;
    window.doc.vdom.in_runtime(move || half.set(true));

    window.present_batch();

    assert_eq!(
        window.measured(),
        LayoutSize {
            width: 1200.0,
            height: 450.0
        }
    );
    let region = window.region();
    assert_eq!((region.rect.width, region.rect.height), (600.0, 450.0));
    assert!(!window.pending(), "one re-measure per batch");
}
