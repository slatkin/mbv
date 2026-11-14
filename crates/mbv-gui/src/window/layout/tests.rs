//! Tests of the layout reporter's merge (D9): the port's ordering contract —
//! the merged report carries the contributions in key-insertion order, the
//! TypeScript module's `Map` order, so a key registered later paints over an
//! earlier key's regions in the core's hit map (the hit map's last push
//! wins) — and the re-set semantics: re-reporting an existing key updates it
//! in place instead of moving it last. Headless documents whose root built
//! the reporter over a recorded channel; no window, no sleep. The last test
//! covers the resize re-measure (D9, D12): the window's own viewport,
//! resolve, and presented-frame sequence, run headless, must leave the
//! container's measured size at the size the window now has.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc;

use blitz_dom::Document as _;
use blitz_traits::shell::{ColorScheme, Viewport};
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use super::{
    LayoutContribution, LayoutEpoch, LayoutKey, LayoutReporter, RemeasureRequest,
    note_presented_frame, note_viewport_change, track_container_size, use_layout_reporter,
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
/// window's drain drives, and the container size signal the effect writes.
#[derive(Default)]
struct MeasureHandoff {
    epoch: Option<LayoutEpoch>,
    remeasure: Option<RemeasureRequest>,
    container: Option<Signal<LayoutSize>>,
}

#[derive(Clone)]
struct MeasureProps {
    handoff: Rc<RefCell<MeasureHandoff>>,
}

/// The measuring root: the window's own container effect over the window's
/// own epoch and re-measure request. The box fills the viewport, so the size
/// the effect measures is the size the document now lays out.
fn measuring_app(props: MeasureProps) -> Element {
    let MeasureProps { handoff } = props;
    let epoch = use_hook(LayoutEpoch::new);
    provide_context(epoch);
    let remeasure = use_hook(RemeasureRequest::new);
    provide_context(remeasure.clone());
    let container: Signal<LayoutSize> = use_signal(LayoutSize::default);
    let mut mounted: Signal<Option<Event<MountedData>>> = use_signal(|| None);
    track_container_size(mounted, container);
    let mut handoff = handoff.borrow_mut();
    handoff.epoch.replace(epoch);
    handoff.remeasure.replace(remeasure);
    handoff.container.replace(container);
    rsx! {
        main { style: "width: 100vw; height: 100vh;",
            div {
                style: "width: 100%; height: 100%;",
                onmounted: move |event| mounted.set(Some(event)),
            }
        }
    }
}

/// The measuring window: the document, the contexts its drain would drive,
/// and the container signal its effect writes.
struct MeasuringWindow {
    doc: DioxusDocument,
    epoch: LayoutEpoch,
    remeasure: RemeasureRequest,
    container: Signal<LayoutSize>,
}

impl MeasuringWindow {
    /// A window `width` by `height`, measured once against its first layout:
    /// the mount, the first frame's resolve, and the measure effect the
    /// resolve lets read the laid-out box.
    fn built(width: u32, height: u32) -> Self {
        let handoff = Rc::new(RefCell::new(MeasureHandoff::default()));
        let mut doc = DioxusDocument::new(
            VirtualDom::new_with_props(
                measuring_app,
                MeasureProps {
                    handoff: Rc::clone(&handoff),
                },
            ),
            DocumentConfig {
                viewport: Some(Viewport::new(width, height, 1.0, ColorScheme::Light)),
                ..DocumentConfig::default()
            },
        );
        doc.initial_build();
        // The window's first frame: the resolve lays the tree out, and the
        // mount's measure reads it.
        doc.inner.borrow_mut().resolve(0.0);
        poll_until_idle(&mut doc);
        let mut taken = handoff.borrow_mut();
        Self {
            epoch: taken.epoch.take().expect("the root provided the epoch"),
            remeasure: taken
                .remeasure
                .take()
                .expect("the root provided the re-measure request"),
            container: taken
                .container
                .take()
                .expect("the root handed the container signal over"),
            doc,
        }
    }

    /// Resize the document the way blitz-shell's `SurfaceResized` handler
    /// does: the new viewport lands before any resolve.
    fn set_viewport(&mut self, width: u32, height: u32) {
        self.doc.inner_mut().viewport_mut().window_size = (width, height);
    }

    /// One drained viewport change and the frame it asks for, in the order the
    /// window presents them: the drain's bump and request, the poll whose
    /// effects measure the layout from before the resolving frame, the
    /// `redraw` resolve, and the presented frame the drain takes the request
    /// on.
    fn present_resize(&mut self) {
        let epoch = self.epoch;
        let remeasure = self.remeasure.clone();
        self.doc
            .vdom
            .in_runtime(move || note_viewport_change(epoch, &remeasure));
        poll_until_idle(&mut self.doc);
        self.doc.inner.borrow_mut().resolve(0.0);
        let epoch = self.epoch;
        let remeasure = self.remeasure.clone();
        self.doc
            .vdom
            .in_runtime(move || note_presented_frame(epoch, &remeasure));
        poll_until_idle(&mut self.doc);
    }

    /// The container's measured size, read in the runtime as the window's
    /// own root would.
    fn measured(&mut self) -> LayoutSize {
        let container = self.container;
        self.doc.vdom.in_runtime(move || container.cloned())
    }
}

/// Contract (live run of the resize defect, D9/D12): after a resize the
/// container's measured size is the size the window now has. Blitz applies the
/// new viewport before it resolves the document, so the measure the viewport
/// bump drives reads the old layout; without the presented-frame re-measure the
/// size signal never changes, and every arrangement derived from it — the
/// panel state, the Queue panel's rows, the Library column — stays one resize
/// stale, which paints differently on every run.
#[test]
fn a_resize_leaves_the_container_measured_at_the_new_size() {
    let mut window = MeasuringWindow::built(1200, 900);
    assert_eq!(
        window.measured(),
        LayoutSize {
            width: 1200.0,
            height: 900.0
        }
    );

    window.set_viewport(820, 610);
    window.present_resize();

    assert_eq!(
        window.measured(),
        LayoutSize {
            width: 820.0,
            height: 610.0
        }
    );
}
