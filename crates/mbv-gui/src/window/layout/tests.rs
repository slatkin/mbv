//! Tests of the layout reporter's merge (D9): the port's ordering contract —
//! the merged report carries the contributions in key-insertion order, the
//! TypeScript module's `Map` order, so a key registered later paints over an
//! earlier key's regions in the core's hit map (the hit map's last push
//! wins) — and the re-set semantics: re-reporting an existing key updates it
//! in place instead of moving it last. Headless documents whose root built
//! the reporter over a recorded channel; no window, no sleep. The last four
//! tests cover the re-measure (D9, D12): the window's own poll, resolve, and
//! presented-frame sequence, run headless, must leave the measured size and the
//! measured region at the values the window lays out — at the mount, after a
//! resize, after a change no trigger names, when a frame resolved before the
//! change, and never again once settled.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc;

use blitz_dom::Document as _;
use blitz_traits::shell::{ColorScheme, Viewport};
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use super::{
    LayoutContribution, LayoutEpoch, LayoutKey, LayoutReporter, carries_unresolved_change,
    element_region, frame_carries_change, note_presented_frame, track_container_size,
    use_layout_reporter,
};
use crate::art::ArtResponse;
use crate::bridge::layout::{
    LayoutRect, LayoutRegion, LayoutReport, LayoutSize, QueuePlaybackTargetId, QueueTargetId,
    RegionTarget,
};
use crate::core_thread::CoreInput;
use crate::window::channel::CoreLink;
use crate::window::net::{MbvNetProvider, NetLoads};

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

/// What the measuring root hands the test: the epoch the drain bumps, the
/// container size signal the effect writes, and the signal a section batch's
/// write stands in for.
#[derive(Default)]
struct MeasureHandoff {
    epoch: Option<LayoutEpoch>,
    container: Option<Signal<LayoutSize>>,
    half: Option<Signal<bool>>,
}

#[derive(Clone)]
struct MeasureProps {
    handoff: Rc<RefCell<MeasureHandoff>>,
    regions: Rc<RefCell<Option<LayoutRegion>>>,
}

/// The measuring root: the window's own container effect over the window's own
/// epoch, and a region effect shaped like every component's. The box fills the
/// viewport; `half` is the fact a batch writes (a panel state, a destination, a
/// row set), and it resizes both the box and its child.
fn measuring_app(props: MeasureProps) -> Element {
    let MeasureProps { handoff, regions } = props;
    let epoch = use_hook(LayoutEpoch::new);
    provide_context(epoch);
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
            let region = element_region(&mounted, RegionTarget::Queue(QueueTargetId::Filter)).await;
            *regions.borrow_mut() = region;
        });
    });
    let mut handoff = handoff.borrow_mut();
    handoff.epoch.replace(epoch);
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

/// The measuring window: the document, the epoch its drain bumps, the container
/// signal its effect writes, the batch's stand-in signal, and the child's
/// measured region.
struct MeasuringWindow {
    doc: DioxusDocument,
    epoch: LayoutEpoch,
    container: Signal<LayoutSize>,
    half: Signal<bool>,
    regions: Rc<RefCell<Option<LayoutRegion>>>,
    /// Whether a viewport change landed since the last presented frame, as the
    /// launch copy tracks it.
    viewport_changed: bool,
}

impl MeasuringWindow {
    /// A window `width` by `height`, opened the way the window opens: the tree
    /// is built, the first frame resolves it and reports the mount's change,
    /// and the measure follows.
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
            container: taken
                .container
                .expect("the root handed the container signal over"),
            half: taken.half.expect("the root handed the toggle over"),
            regions,
            viewport_changed: false,
            doc,
        };
        window.present_frame();
        window
    }

    /// Poll until the effects and the tasks they spawn have run, as a
    /// blitz-shell poll does: effects, then the render's buffered mutations.
    fn settle(&mut self) {
        poll_until_idle(&mut self.doc);
    }

    /// Resize the document the way blitz-shell's `SurfaceResized` handler
    /// does: the new viewport lands before any resolve.
    fn set_viewport(&mut self, width: u32, height: u32) {
        self.doc.inner_mut().viewport_mut().window_size = (width, height);
        self.viewport_changed = true;
    }

    /// One presented frame, exactly as the launch copy runs it: the change fact
    /// is read before the resolve, the shell resolves, and the frame reports
    /// the fact the drain bumps on. Nothing is armed, so a change the render
    /// has not applied yet cannot be answered by a frame that resolved before
    /// it.
    fn present_frame(&mut self) {
        let changed = {
            let document = self.doc.inner.borrow();
            frame_carries_change(&document, self.viewport_changed, false)
        };
        self.viewport_changed = false;
        self.doc.inner.borrow_mut().resolve(0.0);
        let epoch = self.epoch;
        self.doc
            .vdom
            .in_runtime(move || note_presented_frame(epoch, changed));
        self.settle();
    }

    /// Write the batch's stand-in fact, as a section batch writes the stores:
    /// the change reaches the document only in the render of a later poll.
    fn batch_changes_the_layout(&mut self) {
        let mut half = self.half;
        self.doc.vdom.in_runtime(move || half.set(true));
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

    /// Whether the tree still holds a change no frame has resolved.
    fn carries_a_change(&self) -> bool {
        carries_unresolved_change(&self.doc.inner.borrow())
    }
}

/// Contract (live run of the resize defect, D9, D12): after a resize the
/// container's measured size and its child's region are the values the window
/// now lays out. Blitz applies the new viewport without touching the tree, so
/// only the frame that resolved the change reports it; a measure taken on the
/// viewport event itself reads the old layout and changes nothing.
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

    window.set_viewport(820, 610);
    window.present_frame();

    assert_eq!(
        window.measured(),
        LayoutSize {
            width: 820.0,
            height: 610.0
        }
    );
    let region = window.region();
    assert_eq!((region.rect.width, region.rect.height), (410.0, 610.0));
}

/// Contract (D9): a change no trigger names — a non-queue section batch, a
/// width reset, a font, a remount — still leaves the measurements at the values
/// its own frame lays out. Nothing arms a re-measure here: the write is a
/// section store's, the render applies it, and the frame that resolves it
/// reports the change.
#[test]
fn a_change_no_trigger_names_leaves_the_measurements_at_the_resolved_size() {
    let mut window = MeasuringWindow::built(1200, 900);

    window.batch_changes_the_layout();
    window.settle();
    window.present_frame();

    assert_eq!(
        window.measured(),
        LayoutSize {
            width: 1200.0,
            height: 450.0
        }
    );
    let region = window.region();
    assert_eq!((region.rect.width, region.rect.height), (600.0, 450.0));
}

/// Contract (review of `f297509bb`, D12): a frame whose resolve predates a
/// change must not answer that change. The drain dispatches every buffered
/// event in one turn, so a `[fact, Frame]` pair reaches the tree in that order:
/// the frame resolved before the fact's DOM change existed, and the change's
/// own frame is the one that must measure it. A standing request would be taken
/// by the earlier frame, the change would be rendered afterwards, and no
/// measure would follow — the size would stay stale.
#[test]
fn a_frame_that_resolved_before_a_change_does_not_answer_it() {
    let mut window = MeasuringWindow::built(1200, 900);

    window.batch_changes_the_layout();
    // The `[fact, Frame]` turn: the frame's resolve holds no trace of the
    // fact, whose DOM change the following poll applies.
    window.present_frame();
    assert_eq!(
        window.measured(),
        LayoutSize {
            width: 1200.0,
            height: 900.0
        },
        "the earlier frame measured the pre-change layout"
    );

    // The change's own frame: the poll applied the DOM change, so this frame
    // carries it.
    window.present_frame();

    assert_eq!(
        window.measured(),
        LayoutSize {
            width: 1200.0,
            height: 450.0
        }
    );
}

/// Contract (D9, D12): a settled window presents no further measure. After the
/// change's frame the tree holds no unresolved change, so the next frame
/// carries nothing, bumps nothing, and a measure that finds no change writes
/// nothing — the chain stops instead of driving a frame loop.
#[test]
fn a_settled_window_carries_no_change_and_measures_no_further() {
    let mut window = MeasuringWindow::built(1200, 900);
    window.batch_changes_the_layout();
    window.settle();
    window.present_frame();
    assert!(!window.carries_a_change(), "no change is left unresolved");

    window.present_frame();

    assert_eq!(
        window.measured(),
        LayoutSize {
            width: 1200.0,
            height: 450.0
        }
    );
    assert!(!window.carries_a_change());
}

/// The art URL the image host's `img` requests.
const ART_URL: &str = "mbv-art://localhost/card/abc";

/// A PNG `width` by `height`, the bytes an art answer carries.
fn source_png(width: u32, height: u32) -> Vec<u8> {
    let image = image::RgbaImage::from_pixel(width, height, image::Rgba([10, 20, 30, 255]));
    let mut out = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(image)
        .write_to(&mut out, image::ImageFormat::Png)
        .expect("the test source encodes");
    out.into_inner()
}

/// The image host's handoff: the epoch the drain bumps and the container signal
/// the effect writes.
#[derive(Default)]
struct ImageHandoff {
    epoch: Option<LayoutEpoch>,
    container: Option<Signal<LayoutSize>>,
}

#[derive(Clone)]
struct ImageProps {
    handoff: Rc<RefCell<ImageHandoff>>,
}

/// The image host: the window's container effect over the window's epoch, and a
/// box whose height is the `img`'s intrinsic height — the `w-fit` slot's shape
/// (D7). The image arrives after the mount, so the box starts unmeasured.
fn image_app(props: ImageProps) -> Element {
    let ImageProps { handoff } = props;
    let epoch = use_hook(LayoutEpoch::new);
    provide_context(epoch);
    let container: Signal<LayoutSize> = use_signal(LayoutSize::default);
    let mut mounted: Signal<Option<Event<MountedData>>> = use_signal(|| None);
    track_container_size(mounted, container);
    let mut handoff = handoff.borrow_mut();
    handoff.epoch.replace(epoch);
    handoff.container.replace(container);
    rsx! {
        main { style: "width: 100vw; height: 100vh;",
            div {
                style: "width: 100%;",
                onmounted: move |event| mounted.set(Some(event)),
                img { style: "display: block; width: auto; height: auto;", src: ART_URL }
            }
        }
    }
}

/// The image host's window: the document, the epoch its drain bumps, the net
/// answer flag the launch takes, the core channel the art request arrives on,
/// and the container signal its effect writes.
struct ImageWindow {
    doc: DioxusDocument,
    epoch: LayoutEpoch,
    loads: NetLoads,
    core: mpsc::Receiver<CoreInput>,
    container: Signal<LayoutSize>,
}

impl ImageWindow {
    /// The window built and its first frame presented: the frame's resolve
    /// fetches the image, and the measure that follows reads the box without
    /// one.
    fn built() -> Self {
        let handoff = Rc::new(RefCell::new(ImageHandoff::default()));
        let (core, core_receiver) = mpsc::channel();
        let (events, _event_receiver) = futures_channel::mpsc::unbounded();
        let loads = NetLoads::new();
        let mut doc = DioxusDocument::new(
            VirtualDom::new_with_props(
                image_app,
                ImageProps {
                    handoff: Rc::clone(&handoff),
                },
            ),
            DocumentConfig {
                viewport: Some(Viewport::new(1200, 900, 1.0, ColorScheme::Light)),
                net_provider: Some(Arc::new(MbvNetProvider::new(core, events, loads.clone()))),
                ..DocumentConfig::default()
            },
        );
        doc.initial_build();
        let taken = handoff.borrow_mut();
        let mut window = Self {
            epoch: taken.epoch.expect("the root provided the epoch"),
            container: taken
                .container
                .expect("the root handed the container signal over"),
            loads,
            core: core_receiver,
            doc,
        };
        // The first frame: the mount's change is resolved, and that resolve is
        // the one that asks the provider for the image.
        window.present_frame();
        window
    }

    /// One presented frame, as the launch copy runs it: the change fact — the
    /// document's unresolved change or a net answer — is read before the
    /// resolve, the resolve applies it, and the frame reports the fact.
    fn present_frame(&mut self) {
        let changed = {
            let document = self.doc.inner.borrow();
            frame_carries_change(&document, false, false) || self.loads.take()
        };
        self.doc.inner.borrow_mut().resolve(0.0);
        let epoch = self.epoch;
        self.doc
            .vdom
            .in_runtime(move || note_presented_frame(epoch, changed));
        poll_until_idle(&mut self.doc);
    }

    /// Answer the one art request with `bytes`, as the core thread's responder
    /// does: the document's message channel holds the answer and the provider
    /// notes the arrival.
    fn answer_art_with(&self, bytes: Vec<u8>) {
        match self.core.try_recv() {
            Ok(CoreInput::Art(request)) => (request.reply)(ArtResponse::Found {
                mime: "image/png",
                bytes,
            }),
            other => panic!("the image load reached the core as an art request: {other:?}"),
        }
    }

    /// The container's measured size, read in the runtime.
    fn measured(&mut self) -> LayoutSize {
        let container = self.container;
        self.doc.vdom.in_runtime(move || container.cloned())
    }
}

/// Contract (D7, D12): an image that arrives after the mount leaves the box it
/// sizes measured at the size the image gives it. Blitz applies the image inside
/// a resolve — it clears the node's layout cache and inserts `ALL_DAMAGE`, then
/// the same resolve lays out again — so the change fact a frame reads before
/// that resolve sees nothing; the net answer's own note is what makes the frame
/// report the change and measure the new layout. Without it the `w-fit` slot
/// keeps its pre-image rect, and its hit regions with it.
#[test]
fn an_image_arrival_leaves_the_box_it_sizes_measured_at_its_size() {
    let mut window = ImageWindow::built();
    assert_eq!(window.measured().height, 0.0, "the box holds no image yet");

    window.answer_art_with(source_png(300, 200));
    window.present_frame();

    assert_eq!(
        window.measured(),
        LayoutSize {
            width: 1200.0,
            height: 200.0
        }
    );
}
