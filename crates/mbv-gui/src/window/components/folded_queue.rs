//! The panel's folded queue (task 5.7, the port of `FoldedQueue.svelte`;
//! add-tauri-gui-client row 6.6, design D10): the expanded artboard after
//! the `q` toggle. The canvas replaces the queue's rows with one line — the
//! remaining count, the queue finish clock, and the control that expands
//! the queue again.
//!
//! The expand control reports the `queueFold` target, the id the core's one
//! `q` toggle runs from (the canvas paints no header fold control; `q`
//! folds the queue). The line keeps the queue box's fill and its
//! two-column inset, so the fold does not move the neighbouring bands'
//! edges.

use dioxus_native::prelude::*;

use crate::bridge::frame::theme::GuiRole;
use crate::bridge::layout::{QueuePlaybackTargetId, RegionTarget};
use crate::window::icons::ChevronDown;
use crate::window::layout::{
    LayoutContribution, LayoutEpoch, LayoutKey, LayoutReporter, element_region,
};
use crate::window::playhead::{Playhead, queue_finish_text};
use crate::window::sections::Sections;
use crate::window::theme::role_var;

/// The folded queue's one line: the Queue label, the remaining count, the
/// finish clock at the trailing edge, and the expand control.
#[component]
pub fn FoldedQueue() -> Element {
    let sections = consume_context::<Sections>();
    let playhead = consume_context::<Playhead>();
    let epoch = consume_context::<LayoutEpoch>();
    let reporter = consume_context::<LayoutReporter>();
    let key = use_hook(LayoutKey::next);

    let mut expand_mounted: Signal<Option<Event<MountedData>>> = use_signal(|| None);

    // No region outlives the line: the contribution retracts on unmount, as
    // the Svelte `onMount` teardown did.
    let mut drop_reporter = reporter.clone();

    // Each paint that moved a region reports (design D4): this effect runs
    // after the DOM update, and the layout epoch reports layout-driven moves.
    // The reporter dedupes an unchanged contribution.
    let watch_summary = sections.summary;
    let watch_epoch = epoch.0;
    use_effect(move || {
        let _summary = watch_summary.cloned();
        let _epoch = watch_epoch.cloned();
        let expand = expand_mounted.cloned();
        let mut effect_reporter = reporter.clone();
        spawn(async move {
            // The painted regions (design D4): the expand control, the
            // line's one interactive target. The line itself is
            // informational, so it reports none.
            let regions = match expand.as_ref() {
                Some(expand) => element_region(
                    expand,
                    RegionTarget::QueuePlayback(QueuePlaybackTargetId::QueueFold),
                )
                .await
                .into_iter()
                .collect(),
                None => Vec::new(),
            };
            effect_reporter.report_layout(
                key,
                LayoutContribution {
                    lists: Vec::new(),
                    regions,
                },
            );
        });
    });

    use_drop(move || {
        drop_reporter.retract_layout(key);
    });

    let summary = sections.summary.cloned();
    // The queue's finish clock, `None` while no finish facts hold or the
    // summary carries no tail (design D5).
    let queue_finish = queue_finish_text(playhead.finish.cloned(), summary.tail_secs);

    rsx! {
        // The folded queue's one line, on the queue box's fill and its
        // two-column inset: the Queue label, the remaining count, the
        // finish clock at the trailing edge, and the expand control.
        div {
            class: "flex shrink-0 items-center overflow-hidden whitespace-nowrap",
            style: "height: var(--mbv-cell-row); margin-inline: calc(var(--mbv-cell-col) * 2); padding-inline: calc(var(--mbv-cell-col) * 2); gap: var(--mbv-cell-col); background: var(--mbv-queuePanel);",
            span {
                class: "text-xs font-semibold tracking-widest uppercase",
                style: "color: {role_var(GuiRole::TextMuted)};",
                "Queue"
            }
            span {
                class: "font-mono text-xs tabular-nums",
                style: "color: {role_var(GuiRole::TextSecondary)};",
                "data-remaining": "",
                "{summary.remaining}"
            }
            span { class: "flex-1" }
            if let Some(finish) = queue_finish {
                span {
                    class: "font-mono text-xs tabular-nums",
                    style: "color: {role_var(GuiRole::TextMuted)};",
                    "data-finish-clock": "",
                    "{finish}"
                }
            }
            button {
                "type": "button",
                class: "flex items-center justify-center rounded-sm p-1",
                "data-target": "queueFold",
                "aria-label": "Expand queue",
                title: "Expand queue (q)",
                onmounted: move |event| expand_mounted.set(Some(event)),
                ChevronDown {
                    size: 16,
                    color: role_var(GuiRole::TextMuted),
                }
            }
        }
    }
}
