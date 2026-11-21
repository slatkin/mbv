//! The layout reporter (`replace-tauri-with-blitz` D9, the port of
//! `lib/bridge/layout.ts`): every mounted surface reports its list capacities
//! and painted regions here, and the reporter sends the single merged
//! `LayoutReport` the core expects — the core rebuilds its whole hit map from
//! each report, so a surface must never send a partial one. Reports coalesce
//! to one send per turn, and a report identical to the last one sends nothing:
//! a frame that moves only the cursor moves no region, so it reports nothing.
//!
//! `ResizeObserver` has no equivalent on Blitz, so the measurement reads the
//! [`LayoutEpoch`]: a counter that bumps when a presented frame resolved a
//! change that can move a measured rect. The measure never runs on the fact
//! itself, only after the resolve ([`note_presented_frame`]): Blitz applies a
//! viewport in its own window-event handler and our DOM changes after the poll
//! that runs the effects, so a measure taken on the fact reads the layout from
//! before the frame that resolves it — the container keeps its old size, the
//! write is the no-op the reporter's dedup expects, and nothing measures
//! again. So the launch copy, which is the one place that knows a resolve
//! happened, carries the fact on the frame ([`frame_carries_change`]): the
//! drain bumps the epoch for exactly the frames that resolved a change, a
//! viewport change, or a resolve a hit test needed early. A measure that finds
//! nothing new writes nothing, so the next frame carries no change and the
//! window settles. Each container's effect reads the epoch and measures its box
//! through `MountedData`'s client rect, which is already CSS pixels (the taffy
//! layout's viewport is the logical size) — the unit every threshold and report
//! value names.

pub mod region_set;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use blitz_dom::{BaseDocument, Document as _};
use blitz_traits::shell::Viewport;
use dioxus_native::DioxusDocument;
use dioxus_native::prelude::*;
use dioxus_native::winit::window::Window;

use crate::bridge::layout::{
    LayoutRect, LayoutRegion, LayoutReport, LayoutSize, ListRows, RegionTarget,
};
use crate::window::channel::CoreLink;
use crate::window::input::f32_from_f64;

/// The layout epoch (D9): a counter the root bumps on every fact that can
/// change a measured rect. Every measurement effect reads it, so a bump
/// re-measures the rects the change can move.
#[derive(Clone, Copy, Debug, Default)]
pub struct LayoutEpoch(pub Signal<u64>);

impl LayoutEpoch {
    /// The epoch at zero: no viewport, drag, or state change has landed yet.
    #[must_use]
    pub fn new() -> Self {
        Self(Signal::new(0))
    }

    /// One layout fact landed (D9): every measurement effect re-measures.
    pub fn bump(&mut self) {
        *self.0.write() += 1;
    }
}

/// Whether the document holds a tree change no resolve has applied yet (D9,
/// gap `G-Hit`): every mutation inserts damage on the changed node's parent and
/// flags the ancestors (`Node::mark_damaged`), and a resolve rebuilds the paint
/// trees of the damaged subtrees and clears both, so the root's flags are the
/// document's own record of an applied-but-unresolved change.
#[must_use]
pub fn carries_unresolved_change(document: &BaseDocument) -> bool {
    let root = document.root_node();
    root.has_damaged_descendants() || root.damage().is_some_and(|damage| !damage.is_empty())
}

/// The layout size a window shows at `surface` physical pixels and `scale`, in
/// CSS pixels: the size every threshold in [`crate::window::arrangement`] and
/// [`crate::window::resize`] names (D9). The launch copy reads it from the
/// document viewport after each resize and the root reads it from the window it
/// opens, so both name one size.
#[must_use]
pub fn window_layout_size(surface: (u32, u32), scale: f64) -> LayoutSize {
    LayoutSize {
        width: f32_from_f64(f64::from(surface.0) / scale),
        height: f32_from_f64(f64::from(surface.1) / scale),
    }
}

/// The layout size a document viewport lays out, in CSS pixels (D9): the same
/// size [`window_layout_size`] gives for the window it was created from.
#[must_use]
pub fn viewport_layout_size(viewport: &Viewport) -> LayoutSize {
    window_layout_size(viewport.window_size, f64::from(viewport.scale()))
}

/// The layout size `window` shows now, in CSS pixels (D9): its surface over its
/// scale factor. The root seeds its first render from it, so the first frame is
/// right even when the shell's first resize landed before the tree built.
#[must_use]
pub fn current_layout_size(window: &dyn Window) -> LayoutSize {
    let surface = window.surface_size();
    window_layout_size((surface.width, surface.height), window.scale_factor())
}

/// The root's layout-size signal as the launch copy holds it (D9).
///
/// The launch copy is the one place that sees a resize, and Blitz applies the
/// new size to the document's viewport in its own handler without touching the
/// tree. So the launch copy writes the size: [`apply_viewport_size`] renders it
/// into the tree before the frame that resolves at the new size, which is what
/// makes the first frame at a new size lay out that size and a continuous drag
/// present every frame. A channel hop cannot do that — it lands a turn after
/// the resolve, which is the resize jerk (`e978b31aa`).
///
/// The root hands its signal over while its tree builds. A resize that arrives
/// before that writes nothing; the root seeds its own first render from the
/// window it opened, so the first frame is right either way.
#[derive(Clone, Default)]
pub struct ViewportSize(Rc<RefCell<Option<Signal<LayoutSize>>>>);

impl std::fmt::Debug for ViewportSize {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ViewportSize")
            .field("provided", &self.0.borrow().is_some())
            .finish()
    }
}

impl ViewportSize {
    /// A handle no root has provided its signal through yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The root handed its signal over (D9): called once, while its tree builds.
    pub fn provide(&self, size: Signal<LayoutSize>) {
        *self.0.borrow_mut() = Some(size);
    }

    /// Write `size` into the root's signal, if the root has built.
    pub fn set(&self, size: LayoutSize) {
        if let Some(mut signal) = *self.0.borrow() {
            signal.set(size);
        }
    }
}

/// The root's layout-size signal (D9): the one value the window picks its panel
/// states, its arrangement, and its column widths from. The launch copy writes a
/// resize into it through [`ViewportSize`] ([`apply_viewport_size`]), and
/// [`track_container_size`] writes the box it measures. The first render seeds it
/// from the window the root opened, because the shell's first resize can land
/// before the tree builds.
#[must_use]
pub fn use_container_size(viewport: ViewportSize, window: &Arc<dyn Window>) -> Signal<LayoutSize> {
    let size: Signal<LayoutSize> = use_signal(|| current_layout_size(window.as_ref()));
    use_hook(move || viewport.provide(size));
    size
}

/// Apply a resize to the window's tree (D9): the shell has already put the new
/// size on the document's viewport, so the size signal is written from the
/// vdom's runtime and the tree is rendered — all before the frame that resolves
/// at the new size. That frame therefore lays out the arrangement, panel state,
/// and column widths the size picks.
///
/// The measure tasks left pending by the previous frame read the layout from
/// before this resolve, so they run first, and the size is written after them;
/// the frame that carries the change then measures the resolved layout, and the
/// measured container keeps reporting the same size through
/// [`track_container_size`].
pub fn apply_viewport_size(document: &mut DioxusDocument, size: ViewportSize, layout: LayoutSize) {
    document.poll(None);
    document.vdom.in_runtime(move || size.set(layout));
    document.poll(None);
}

/// Whether the frame that is about to resolve carries a change no measurement
/// has read (D9, D12), the launch copy's frame fact.
///
/// A frame must bump the [`LayoutEpoch`] exactly when its own resolve applies
/// something a measure has not seen — never when it was merely presented, and
/// never on a standing request, because a request armed after a frame's resolve
/// would be taken by that frame and the change it stood for would never be
/// measured. The three facts are the document's unresolved tree change (every
/// mutation, from a section batch, a divider step, a width reset, a remount, or
/// a net answer that reaches the tree), a viewport change ([`super::channel::
/// WindowEvent::Viewport`], which Blitz applies without touching the tree), and
/// a resolve the hit-test gap needed early (`G-Hit`), which applied a change no
/// frame has reported yet.
#[must_use]
pub fn frame_carries_change(
    document: &BaseDocument,
    viewport_changed: bool,
    resolved_early: bool,
) -> bool {
    viewport_changed || resolved_early || carries_unresolved_change(document)
}

/// One presented frame's layout facts (D10, D12): the launch copy reports
/// whether the frame's resolve applied a change ([`frame_carries_change`]), and
/// a frame that did bumps the epoch once, so every measurement effect re-reads
/// its box against the layout this frame painted. A frame that resolved nothing
/// new bumps nothing: a measure that finds no change writes nothing, so the
/// next frame carries no change and an idle window's frames stop costing
/// measures.
pub fn note_presented_frame(mut epoch: LayoutEpoch, changed: bool) {
    if changed {
        epoch.bump();
    }
}

/// One surface's share of the merged report (D9): its list capacities and
/// regions.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LayoutContribution {
    /// The whole visible rows of each list the surface painted.
    pub lists: Vec<ListRows>,
    /// The painted regions, each with the semantic target a press on it
    /// resolves to.
    pub regions: Vec<LayoutRegion>,
}

/// A surface's report key (D9): one per component instance, held for the
/// instance's life, as the TypeScript module's symbol was.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LayoutKey(u64);

impl LayoutKey {
    /// A key no other surface holds.
    #[must_use]
    pub fn next() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        Self(NEXT.fetch_add(1, Ordering::Relaxed))
    }
}

/// The layout reporter (D9). The root creates it once and provides it as
/// context; its send effect merges the contributions into one report and
/// pushes it through the [`CoreLink`] when it differs from the last one
/// sent. The merge walks the contributions in key-insertion order — the
/// TypeScript module's `Map` order, held in a `Vec` instead of a `HashMap`,
/// whose iteration order is the process's hash order, not paint order — so
/// a key registered later merges after an earlier key and its regions paint
/// over them in the core's hit map (the hit map's last push wins).
#[derive(Clone, Debug)]
pub struct LayoutReporter {
    link: CoreLink,
    contributions: Signal<Vec<(LayoutKey, LayoutContribution)>>,
    container_size: Signal<LayoutSize>,
    last_sent: Signal<Option<LayoutReport>>,
}

/// The layout reporter as a hook (D9): create once in the root and provide as
/// context. The send effect reads the contributions and the container size, so
/// every write in one task poll coalesces into one merged report.
#[must_use]
pub fn use_layout_reporter() -> LayoutReporter {
    let link = consume_context::<CoreLink>();
    let reporter = use_hook(|| LayoutReporter {
        link,
        contributions: Signal::new(Vec::new()),
        container_size: Signal::new(LayoutSize::default()),
        last_sent: Signal::new(None),
    });
    // The signals are `Copy`, so the effect captures them by copy and the
    // report's own dedup signal stays out of its subscriptions.
    let contributions = reporter.contributions;
    let container_size = reporter.container_size;
    let mut last_sent = reporter.last_sent;
    let link = CoreLink::clone(&reporter.link);
    use_effect(move || {
        let mut report = LayoutReport {
            lists: Vec::new(),
            regions: Vec::new(),
            container_size: container_size.cloned(),
        };
        for (_, contribution) in contributions.read().iter() {
            report.lists.extend_from_slice(&contribution.lists);
            report.regions.extend_from_slice(&contribution.regions);
        }
        // A report that equals the last one sends nothing (D9): the core's
        // hit map stays the one the last layout painted.
        if last_sent.peek().as_ref() != Some(&report) {
            link.layout(report.clone());
            last_sent.set(Some(report));
        }
    });
    reporter
}

impl LayoutReporter {
    /// The panel container's last reported painted size (D9): the measured
    /// size the window picks the panel state from. Zero while no container
    /// has reported — the pre-attach states paint no panel.
    #[must_use]
    pub fn container_size(&self) -> LayoutSize {
        self.container_size.cloned()
    }

    /// Records the panel container's painted size and schedules the merged
    /// report (D9): the container's own measurement effect calls this, and the
    /// report carries the size so the capacity and regions it merges stay
    /// matched to the layout the size painted. An unchanged size is a no-op.
    pub fn report_container_size(&mut self, size: LayoutSize) {
        if self.container_size.cloned() != size {
            self.container_size.set(size);
        }
    }

    /// Registers or replaces one surface's contribution and schedules the
    /// merged report (D9). `key` is the [`LayoutKey`] the caller holds for the
    /// surface's life. A contribution identical to the last one reported under
    /// the key is a no-op.
    pub fn report_layout(&mut self, key: LayoutKey, contribution: LayoutContribution) {
        let unchanged = self
            .contributions
            .peek()
            .iter()
            .any(|(held, last)| *held == key && *last == contribution);
        if unchanged {
            return;
        }
        let mut contributions = self.contributions.write();
        if let Some(held) = contributions.iter_mut().find(|(held, _)| *held == key) {
            // A re-set updates in place: the TypeScript `Map`'s `set` keeps
            // the key's original insertion position, so the surface keeps
            // its paint layer in the merged report.
            held.1 = contribution;
            return;
        }
        contributions.push((key, contribution));
    }

    /// Removes a surface's contribution (D9): an unmounted surface owns no
    /// regions, so the next merged report drops them.
    pub fn retract_layout(&mut self, key: LayoutKey) {
        self.contributions.write().retain(|(held, _)| *held != key);
    }
}

/// The container-size effect (D9): the port of the container's
/// `ResizeObserver`. Call it from the container component's body with the box's
/// `onmounted` data and the container's size signal; the effect reads the
/// [`LayoutEpoch`], measures the box's client rect, and writes the size into
/// the signal and the merged report. The effect runs after the render, so the
/// rect is the laid-out one.
///
/// The size signal is also the one [`apply_viewport_size`] writes at a resize
/// (D9), so the window picks its arrangement from the new size on the first
/// frame at that size and from this measurement from the frame after it. The
/// two agree to the pixel on this window (the container is `h-dvh`, the
/// viewport's own height, and winit reports no safe-area insets off mobile), so
/// the refinement changes nothing that paints; it keeps the reporter's size the
/// measured one.
///
/// The first measure runs before any resolve has placed the box, so it reads a
/// zero rect and writes nothing (the size signal's default). The frame that
/// resolves the mount's mutations carries the change, so its bump measures the
/// laid-out box; no mount-time request is needed for it.
pub fn track_container_size(
    mounted: Signal<Option<Event<MountedData>>>,
    mut size: Signal<LayoutSize>,
) {
    let Some(LayoutEpoch(epoch)) = try_consume_context::<LayoutEpoch>() else {
        tracing::debug!(
            target: "gui_layout",
            "no layout epoch is provided; the container's size stays unmeasured"
        );
        return;
    };
    let reporter = try_consume_context::<LayoutReporter>();
    let report_size = reporter.as_ref().map(|reporter| reporter.container_size);
    use_effect(move || {
        // Reading the epoch subscribes the re-measure to every layout change
        // (D9): a resize, a divider drag, or a panel state change re-measures.
        let _epoch = epoch.cloned();
        let mounted = mounted.cloned();
        spawn(async move {
            let Some(event) = mounted else {
                return;
            };
            let Ok(rect) = event.get_client_rect().await else {
                return;
            };
            let measured = LayoutSize {
                width: f32_from_f64(rect.width()),
                height: f32_from_f64(rect.height()),
            };
            if size.cloned() != measured {
                size.set(measured);
            }
            if let Some(mut report_size) = report_size {
                report_size.set(measured);
            }
        });
    });
}

/// The width-only measure effect: the same re-measure as
/// [`track_container_size`] (a [`LayoutEpoch`] subscription, the box's client
/// rect, a 0.5 px dedupe) for a box whose width alone matters — the top bar
/// and its tab strip. It writes only `width`, never the layout reporter's
/// container size. Without a [`LayoutEpoch`] the box is measured once, at
/// mount.
pub fn track_width(mounted: Signal<Option<Event<MountedData>>>, mut width: Signal<f32>) {
    let epoch = try_consume_context::<LayoutEpoch>().map(|epoch| epoch.0);
    use_effect(move || {
        // Reading the epoch re-measures the box on every layout change.
        let _epoch = epoch.as_ref().map(Signal::cloned);
        let mounted = mounted.cloned();
        spawn(async move {
            let Some(event) = mounted else {
                return;
            };
            let Ok(rect) = event.get_client_rect().await else {
                return;
            };
            let measured = f32_from_f64(rect.width());
            if (*width.peek() - measured).abs() > 0.5 {
                width.set(measured);
            }
        });
    });
}

/// One element's painted region (D9), `None` while the element is not
/// mounted. The rect is the element's bounding box in window coordinates —
/// already CSS pixels — and `target` is the semantic target a press on the
/// region resolves to.
pub async fn element_region(
    mounted: &Event<MountedData>,
    target: RegionTarget,
) -> Option<LayoutRegion> {
    let Ok(rect) = mounted.get_client_rect().await else {
        return None;
    };
    Some(LayoutRegion {
        rect: LayoutRect {
            x: f32_from_f64(rect.origin.x),
            y: f32_from_f64(rect.origin.y),
            width: f32_from_f64(rect.width()),
            height: f32_from_f64(rect.height()),
        },
        target,
    })
}

#[cfg(test)]
mod tests;
