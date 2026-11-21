//! One reported region set per surface (D1 of `gui-tech-debt-simplification`):
//! a surface's marks, its one report task, and the retract on unmount, in place
//! of the per-control handle signals and report keys every surface held.
//!
//! A set is created once, in the surface owner's own body
//! ([`use_region_set`] for a signal-driven surface, [`use_region_set_for`] for
//! one that triggers through reactive props), and provided with the overlay
//! count its surface reports. Every render starts at that hook call: it marks a
//! new render and clears the "painted this render" list, so the marks the body
//! makes are the whole of what that render reports. Each mark
//! ([`RegionSet::mark`], [`RegionSet::mark_over`], [`RegionSet::mark_list`])
//! appends its target to that list in the order the calls run, and returns the
//! `EventHandler` that stores the element's mounted handle under that target.
//! Mark order is report order. The handles and the painted list live in one
//! `CopyValue`, which is not reactive: a mark neither triggers a render nor
//! costs an `Rc`, and the set is `Clone` so a hook can hand it to its effect.
//!
//! One effect spawns one task after each render whose watched facts moved. The
//! task resolves the painted targets of the base in mark order and reports
//! them under the base key, then does the same for each overlay key in index
//! order, so a popover's regions merge after the content beneath them (the hit
//! map's last push wins) and a closed popover's key retracts, which lets the
//! next open push at the end again. A base with no marks still reports its
//! empty contribution; an overlay with no marks retracts its key. The task
//! reads the handles the latest render recorded and drops the ones whose
//! targets that render did not paint, so a row that scrolled away leaves no
//! region behind. The set retracts every key on drop.
//!
//! Marks run only in the surface owner's render, inline in its `rsx` or in
//! `let`s before it — never in a child component. Dioxus memoizes a child
//! whose props compare equal and renders a child after its parent, so a mark
//! inside a child could be skipped while its element still paints (an open
//! popover's key would retract) or land after the parent's own marks. For an
//! element a child owns, the owner marks it and passes the returned
//! `EventHandler` to the child as a prop. A mark keeps the surface's
//! paint-vs-report gates: a control that claims no pointer today is marked
//! only while it acts, so a disabled control reports no region.
//!
//! The merge rule is unchanged (D9, `window/layout.rs`): a key keeps its
//! insertion position, a re-set updates in place, and the core's hit map
//! rebuilds from the merged report each time.

use dioxus_native::prelude::*;

use super::{LayoutContribution, LayoutEpoch, LayoutKey, LayoutReporter, element_region};
use crate::bridge::layout::{LayoutRegion, ListId, ListRows, RegionTarget};
use crate::window::input::f32_from_f64;

#[cfg(test)]
use std::sync::mpsc::Receiver;

#[cfg(test)]
use crate::bridge::layout::LayoutReport;
#[cfg(test)]
use crate::core_thread::CoreInput;

/// One element's mounted handle, as a mark's closure records it.
type Handle = Event<MountedData>;

/// A mark's handler: the closure the marked element attaches to its
/// `onmounted`, and the value a child component takes as a prop. `EventHandler`
/// of this Dioxus Native version wraps the callback's argument, so the argument
/// here is the event itself.
pub type MarkHandler = EventHandler<Event<MountedData>>;

/// A measured list's capacity rule: the box's laid-out height into its whole
/// visible rows.
type WholeRows = fn(f32) -> usize;

/// The marks one surface holds (D1): the targets painted this render, in mark
/// order, and the element handle each marked target last handed over.
#[derive(Default)]
struct MarkStore {
    /// The base contribution's painted targets, in mark order.
    base: Vec<RegionTarget>,
    /// Each overlay's painted targets, in mark order.
    overlays: Vec<Vec<RegionTarget>>,
    /// The measured lists, in mark order, with each list's capacity rule.
    lists: Vec<(ListId, WholeRows)>,
    /// Each marked target's handle, from its last mounted event.
    handles: Vec<(RegionTarget, Handle)>,
    /// Each marked list's box handle, from its last mounted event.
    list_handles: Vec<(ListId, Handle)>,
}

/// Append `target` to `marks` unless it is already painted: the first mark
/// keeps its report position and a later mark of the same target keeps its
/// handle.
fn note(marks: &mut Vec<RegionTarget>, target: RegionTarget) {
    if !marks.contains(&target) {
        marks.push(target);
    }
}

impl MarkStore {
    /// Start a render: the marks that follow are the whole of what it reports.
    fn begin_render(&mut self) {
        self.base.clear();
        for marks in &mut self.overlays {
            marks.clear();
        }
        self.lists.clear();
    }

    /// Add the base contribution's `target` to this render's painted list.
    fn note_base(&mut self, target: RegionTarget) {
        note(&mut self.base, target);
    }

    /// Add overlay `index`'s `target` to this render's painted list.
    ///
    /// # Panics
    /// Panics when `index` is at or past the overlay count the set was created
    /// with: the set reports only the overlays its surface declared.
    fn note_over(&mut self, index: usize, target: RegionTarget) {
        let marks = self
            .overlays
            .get_mut(index)
            .expect("a mark_over index is below the set's overlay count");
        note(marks, target);
    }

    /// Add `list` to this render's painted lists, with its capacity rule.
    fn note_list(&mut self, list: ListId, whole_rows: WholeRows) {
        match self.lists.iter_mut().find(|(held, _)| *held == list) {
            Some(entry) => entry.1 = whole_rows,
            None => self.lists.push((list, whole_rows)),
        }
    }

    /// Record `target`'s mounted handle.
    fn record(&mut self, target: RegionTarget, event: Handle) {
        match self.handles.iter_mut().find(|(held, _)| *held == target) {
            Some(entry) => entry.1 = event,
            None => self.handles.push((target, event)),
        }
    }

    /// Record `list`'s mounted box handle.
    fn record_list(&mut self, list: ListId, event: Handle) {
        match self.list_handles.iter_mut().find(|(held, _)| *held == list) {
            Some(entry) => entry.1 = event,
            None => self.list_handles.push((list, event)),
        }
    }

    /// The latest render's marks, owned by the report task, with every handle
    /// whose target that render did not paint dropped.
    fn take_painted(&mut self) -> Painted {
        let base = self
            .base
            .iter()
            .map(|target| (*target, self.handle(*target)))
            .collect();
        let overlays = self
            .overlays
            .iter()
            .map(|marks| {
                marks
                    .iter()
                    .map(|target| (*target, self.handle(*target)))
                    .collect()
            })
            .collect();
        let lists = self
            .lists
            .iter()
            .map(|(list, whole_rows)| (*list, *whole_rows, self.list_handle(*list)))
            .collect();
        let painted = Painted {
            base,
            overlays,
            lists,
        };
        self.prune(&painted);
        painted
    }

    /// Drop the handles of targets and lists the latest render did not paint.
    fn prune(&mut self, painted: &Painted) {
        let targets = painted.targets();
        self.handles.retain(|(target, _)| targets.contains(target));
        let lists: Vec<ListId> = painted.lists.iter().map(|(list, _, _)| *list).collect();
        self.list_handles.retain(|(list, _)| lists.contains(list));
    }

    /// The handle the last mounted event recorded for `target`.
    fn handle(&self, target: RegionTarget) -> Option<Handle> {
        self.handles
            .iter()
            .find(|(held, _)| *held == target)
            .map(|(_, handle)| handle.clone())
    }

    /// The handle the last mounted event recorded for `list`.
    fn list_handle(&self, list: ListId) -> Option<Handle> {
        self.list_handles
            .iter()
            .find(|(held, _)| *held == list)
            .map(|(_, handle)| handle.clone())
    }
}

/// One render's marks, owned by the report task (D1): each painted target with
/// the handle its element last handed over, `None` while it has not mounted.
struct Painted {
    base: Vec<(RegionTarget, Option<Handle>)>,
    overlays: Vec<Vec<(RegionTarget, Option<Handle>)>>,
    lists: Vec<(ListId, WholeRows, Option<Handle>)>,
}

impl Painted {
    /// Every painted region target, base and overlays together.
    fn targets(&self) -> Vec<RegionTarget> {
        let mut targets: Vec<RegionTarget> = self.base.iter().map(|(target, _)| *target).collect();
        for marks in &self.overlays {
            targets.extend(marks.iter().map(|(target, _)| *target));
        }
        targets
    }
}

/// One surface's region reports (D1): the keys it reports under, held for its
/// life, and the marks its own render makes. `Clone`, so the hook hands copies
/// to its effect and no `Rc` is necessary.
#[derive(Clone)]
pub struct RegionSet {
    /// The key the base contribution reports under.
    base_key: LayoutKey,
    /// One key per overlay, in index order.
    overlay_keys: Vec<LayoutKey>,
    /// The marks and their handles, non-reactive.
    marks: CopyValue<MarkStore>,
}

impl RegionSet {
    /// A set for a surface that reports `overlays` overlay contributions after
    /// its base.
    fn new(overlays: usize) -> Self {
        Self {
            base_key: LayoutKey::next(),
            overlay_keys: (0..overlays).map(|_| LayoutKey::next()).collect(),
            marks: CopyValue::new(MarkStore {
                overlays: vec![Vec::new(); overlays],
                ..MarkStore::default()
            }),
        }
    }

    /// Start a render (D1): clear the painted list, so the marks that follow
    /// are the whole of what this render reports.
    fn begin_render(&self) {
        let mut marks = self.marks;
        marks.write().begin_render();
    }

    /// Mark `target` in the base contribution (D1): the target joins this
    /// render's report in mark order, and the returned handler records the
    /// element's mounted handle for it.
    #[must_use]
    pub fn mark(&self, target: RegionTarget) -> MarkHandler {
        let mut marks = self.marks;
        marks.write().note_base(target);
        self.recorder(target)
    }

    /// Mark `target` in overlay contribution `index` (D1), which reports after
    /// the base and before the overlays that follow it.
    ///
    /// # Panics
    /// Panics when `index` is at or past the overlay count the set was created
    /// with.
    #[must_use]
    pub fn mark_over(&self, index: usize, target: RegionTarget) -> MarkHandler {
        let mut marks = self.marks;
        marks.write().note_over(index, target);
        self.recorder(target)
    }

    /// Mark the measured list `list` (D1): its box joins this render's base
    /// contribution and reports `ListRows { list, whole_rows: whole_rows(height) }`
    /// from its laid-out height, or no rows while it has not mounted. The
    /// returned handler records the box's mounted handle.
    #[must_use]
    pub fn mark_list(&self, list: ListId, whole_rows: WholeRows) -> MarkHandler {
        let mut marks = self.marks;
        marks.write().note_list(list, whole_rows);
        EventHandler::new(move |event: Event<MountedData>| {
            let mut marks = marks;
            marks.write().record_list(list, event);
        })
    }

    /// The handler that records a mark's mounted handle under `target`.
    fn recorder(&self, target: RegionTarget) -> MarkHandler {
        let marks = self.marks;
        EventHandler::new(move |event: Event<MountedData>| {
            let mut marks = marks;
            marks.write().record(target, event);
        })
    }

    /// Retract every key of the set (D1): an unmounted surface owns no regions.
    fn retract(&self, reporter: &mut LayoutReporter) {
        reporter.retract_layout(self.base_key);
        for key in &self.overlay_keys {
            reporter.retract_layout(*key);
        }
    }
}

impl std::fmt::Debug for RegionSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RegionSet")
            .field("overlays", &self.overlay_keys.len())
            .finish_non_exhaustive()
    }
}

/// A surface's region set for its signal-driven reports (D1): `watch` reads
/// the surface's current sections, which subscribes the one report effect to
/// them, and `overlays` is the overlay count [`RegionSet::mark_over`] may name.
#[must_use]
pub fn use_region_set(overlays: usize, mut watch: impl FnMut() + 'static) -> RegionSet {
    let set = use_hook(|| RegionSet::new(overlays));
    set.begin_render();
    let epoch = consume_context::<LayoutEpoch>();
    let reporter = consume_context::<LayoutReporter>();
    let effect_reporter = reporter.clone();
    let effect_set = set.clone();
    // The effect runs after the DOM update, so the painted list it reads is
    // the render's own, and the layout epoch reports layout-driven moves.
    use_effect(move || {
        watch();
        let _epoch = epoch.0.cloned();
        report_marks(&effect_set, effect_reporter.clone());
    });
    let mut drop_reporter = reporter;
    let drop_set = set.clone();
    use_drop(move || drop_set.retract(&mut drop_reporter));
    set
}

/// A surface's region set for its props-driven reports (D1): `deps` is the
/// dependency tuple the surface would give `use_reactive`, and the epoch is
/// read beside it so a layout change re-reports as it does for
/// [`use_region_set`].
#[must_use]
pub fn use_region_set_for<D: Dependency>(overlays: usize, deps: D) -> RegionSet {
    let set = use_hook(|| RegionSet::new(overlays));
    set.begin_render();
    let epoch = consume_context::<LayoutEpoch>();
    let reporter = consume_context::<LayoutReporter>();
    let effect_reporter = reporter.clone();
    let effect_set = set.clone();
    use_effect(use_reactive(deps, move |_deps| {
        let _epoch = epoch.0.cloned();
        report_marks(&effect_set, effect_reporter.clone());
    }));
    let mut drop_reporter = reporter;
    let drop_set = set.clone();
    use_drop(move || drop_set.retract(&mut drop_reporter));
    set
}

/// Spawn the set's one report task: the base contribution in mark order, then
/// each overlay in index order, with an empty overlay retracting its key (D1).
fn report_marks(set: &RegionSet, mut reporter: LayoutReporter) {
    let base_key = set.base_key;
    let overlay_keys = set.overlay_keys.clone();
    let painted = {
        let mut marks = set.marks;
        marks.write().take_painted()
    };
    spawn(async move {
        let mut lists: Vec<ListRows> = Vec::new();
        for (list, whole_rows, handle) in &painted.lists {
            let rows = match handle {
                Some(handle) => list_whole_rows(handle, *whole_rows).await,
                None => 0,
            };
            lists.push(ListRows {
                list: *list,
                whole_rows: rows,
            });
        }
        reporter.report_layout(
            base_key,
            LayoutContribution {
                lists,
                regions: painted_regions(&painted.base).await,
            },
        );
        for (key, marks) in overlay_keys.iter().zip(painted.overlays.iter()) {
            if marks.is_empty() {
                reporter.retract_layout(*key);
                continue;
            }
            reporter.report_layout(
                *key,
                LayoutContribution {
                    lists: Vec::new(),
                    regions: painted_regions(marks).await,
                },
            );
        }
    });
}

/// One render's painted regions, each element measured into the region its
/// target names: the mark order is the report order.
async fn painted_regions(marks: &[(RegionTarget, Option<Handle>)]) -> Vec<LayoutRegion> {
    let mut regions: Vec<LayoutRegion> = Vec::new();
    for (target, handle) in marks {
        if let Some(handle) = handle {
            regions.extend(element_region(handle, *target).await);
        }
    }
    regions
}

/// A list box's whole visible rows: the box's laid-out height through its
/// capacity rule, or none while the box cannot measure.
async fn list_whole_rows(handle: &Handle, whole_rows: WholeRows) -> usize {
    match handle.get_client_rect().await {
        Ok(rect) => whole_rows(f32_from_f64(rect.height())),
        Err(_) => 0,
    }
}

/// The last layout report the core link sent, draining every input the
/// fixture queued.
#[cfg(test)]
#[must_use]
pub fn last_report(receiver: &Receiver<CoreInput>) -> Option<LayoutReport> {
    receiver
        .try_iter()
        .filter_map(|input| match input {
            CoreInput::Layout(report) => Some(report),
            _ => None,
        })
        .last()
}

/// The last merged report's region targets, draining every input the fixture
/// queued.
#[cfg(test)]
#[must_use]
pub fn last_reported_targets(receiver: &Receiver<CoreInput>) -> Vec<RegionTarget> {
    last_report(receiver)
        .map(|report| {
            report
                .regions
                .into_iter()
                .map(|region| region.target)
                .collect()
        })
        .unwrap_or_default()
}
