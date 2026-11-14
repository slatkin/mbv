//! The layout reporter (`replace-tauri-with-blitz` D9, the port of
//! `lib/bridge/layout.ts`): every mounted surface reports its list capacities
//! and painted regions here, and the reporter sends the single merged
//! `LayoutReport` the core expects — the core rebuilds its whole hit map from
//! each report, so a surface must never send a partial one. Reports coalesce
//! to one send per turn, and a report identical to the last one sends nothing:
//! a frame that moves only the cursor moves no region, so it reports nothing.
//!
//! `ResizeObserver` has no equivalent on Blitz, so the measurement reads the
//! [`LayoutEpoch`]: a counter that bumps on every fact that can change a
//! measured rect — a viewport change, a divider drag or reset, a panel state
//! change, a batch that changes a list's row count. Each container's effect
//! reads the epoch and measures its box through `MountedData`'s client rect,
//! which is already CSS pixels (the taffy layout's viewport is the logical
//! size) — the unit every threshold and report value names.

use std::sync::atomic::{AtomicU64, Ordering};

use dioxus_native::prelude::*;

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
        for (_, contribution) in contributions.cloned() {
            report.lists.extend(contribution.lists);
            report.regions.extend(contribution.regions);
        }
        // A report that equals the last one sends nothing (D9): the core's
        // hit map stays the one the last layout painted.
        if last_sent.cloned().as_ref() != Some(&report) {
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
        let mut contributions = self.contributions.write();
        if let Some(held) = contributions.iter_mut().find(|(held, _)| *held == key) {
            if held.1 == contribution {
                return;
            }
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
