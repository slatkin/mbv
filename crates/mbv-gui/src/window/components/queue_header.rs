//! The Queue header (task 5.6, the port of `QueueHeader`; rewritten by task
//! 8.2 of `gui-named-queues`): the header band over the Queue rows. It paints
//! the Queue title (with the chooser button around it while two or more
//! entries exist), the filter control, the new-queue control, and the close
//! control, and nothing else (spec `gui-queue-chooser` "The queue header
//! holds the chooser and its controls"). The title always paints: the shown
//! entry's name, or `Queue` while that name is empty (user decision
//! 2026-11-17). Every fact the controls paint
//! is the frame's; the filter's text and the caret are Client-core state
//! (design D4a), so the header holds none.
//!
//! The header paints the canvas's own hexes through the queue-local
//! `--canvas-*` tokens (U13): the canvas names no `mbv-theme` slot for
//! them, and the TUI's palette stays untouched.
//!
//! The header band paints `--canvas-dim` with a 1 px `--canvas-border` rule
//! under it (user decision 2026-11-15): the Library top bar's own fill and
//! border (`library_top_bar.rs`), so the queue's header reads like the tab
//! bar it sits beside. The band covers the recessed box's top edge, so the
//! box's own fill starts under the divider. The band spans the full queue
//! column — negative inline margins cancel the recessed box's two-column
//! inset — so its divider runs edge to edge like the tab bar's, the same
//! stroke at the same colour.
//!
//! Each control carries a pointer target: it reports its own region through
//! `QueuePanel`'s report keys — the chooser `queueChooser`, the filter
//! `filter`, the new-queue control `newQueue`, and the close control
//! `closeQueue` — so a press resolves in the core to the request the matching
//! GUI-only chord dispatches, and the control and the chord run one path. The
//! chooser button paints only while the Queue chooser lists two or more
//! entries (a single entry paints the title as plain text), and
//! the close control reports no region while it is disabled (the only local
//! queue has no close): an absent or disabled control claims no pointer, so
//! the panel reads neither handle then.

use dioxus_native::prelude::*;

use crate::bridge::frame::queue::GuiQueueSummary;
use crate::window::icons::{ChevronDown, ListOrdered, Plus, Search, X};
use crate::window::theme::canvas_var;
use crate::window::truncate::{TextStyle, Truncate};

/// The title the header paints while the shown entry has no name.
const DEFAULT_TITLE: &str = "Queue";

/// The props of the Queue header (the `QueueHeader` props, plus the region
/// handles the panel's report reads).
#[derive(Props, Debug, Clone, PartialEq)]
pub struct QueueHeaderProps {
    /// The queue summary the header's controls read.
    pub summary: GuiQueueSummary,
    /// The Queue chooser button's mounted event — the handle the
    /// `queueChooser` region reads.
    pub chooser: Signal<Option<Event<MountedData>>>,
    /// The filter control's mounted event — the handle the `filter` region
    /// reads.
    pub filter: Signal<Option<Event<MountedData>>>,
    /// The new-queue control's mounted event — the handle the `newQueue`
    /// region reads.
    pub new_queue: Signal<Option<Event<MountedData>>>,
    /// The close control's mounted event — the handle the `closeQueue` region
    /// reads.
    pub close: Signal<Option<Event<MountedData>>>,
}

/// The Queue header: the chooser button on the leading edge, the filter, new,
/// and close controls on the trailing edge.
#[component]
pub fn QueueHeader(props: QueueHeaderProps) -> Element {
    let summary = &props.summary;
    let chooser = props.chooser;
    let filter = props.filter;
    let new_queue = props.new_queue;
    let close = props.close;
    let title = if summary.header.shown_name.is_empty() {
        DEFAULT_TITLE.to_owned()
    } else {
        summary.header.shown_name.clone()
    };

    rsx! {
        div {
            class: "flex items-center justify-between overflow-hidden whitespace-nowrap",
            style: "height: var(--canvas-queue-header-height); padding: 12px 8px 8px 16px; margin-inline: calc(var(--mbv-cell-col) * -2); background: var(--canvas-dim); border-bottom: 1px solid var(--canvas-border);",
            "data-queue-header": "",
            div {
                class: "flex min-w-0 items-center",
                // The Queue chooser button (design D8, D9): it names the shown
                // entry and opens the dropdown. The button is absent while
                // the list holds exactly one entry; the title then paints
                // alone, so the header always names the queue.
                if summary.header.entries < 2 {
                    span {
                        class: "flex min-w-0 items-center",
                        style: "height: 44px; padding: 0 10px; gap: 8px; font-size: 13px; color: {canvas_var(\"heading\")};",
                        "data-queue-title": "",
                        Truncate {
                            text: title.clone(),
                            style: TextStyle::sans(13.0, 500.0),
                        }
                    }
                } else {
                    {
                        let mut chooser_slot = chooser;
                        rsx! {
                            button {
                                "type": "button",
                                class: "flex min-w-0 items-center",
                                style: "height: 44px; padding: 0 10px; gap: 8px; font-size: 13px; color: {canvas_var(\"heading\")};",
                                title: "Queue chooser (t)",
                                "aria-label": "Queue chooser",
                                "data-queue-chooser": "",
                                onmounted: move |event| chooser_slot.set(Some(event)),
                                ListOrdered {
                                    size: 16,
                                    color: "currentColor",
                                }
                                Truncate {
                                    text: title.clone(),
                                    style: TextStyle::sans(13.0, 500.0),
                                }
                                ChevronDown {
                                    size: 12,
                                    color: "currentColor",
                                }
                            }
                        }
                    }
                }
            }
            div {
                class: "flex items-center",
                style: "gap: 4px;",
                // The filter control: a magnifier button at rest, and the
                // text entry with the block caret while the entry holds the
                // keyboard (`filterActive`) or a filter narrows the rows. A
                // press on it runs the `/` chord's text entry in the core
                // through the `filter` region the mark reports.
                if summary.filter_active || !summary.filter.is_empty() {
                    {
                        let filter_text = if summary.filter.is_empty() {
                            "Filter".to_owned()
                        } else {
                            summary.filter.clone()
                        };
                        let text_color = if summary.filter.is_empty() {
                            canvas_var("muted")
                        } else {
                            canvas_var("heading")
                        };
                        let mut filter_slot = filter;
                        let filter_active = summary.filter_active;
                        rsx! {
                            span {
                                class: "flex items-center",
                                style: "height: 28px; padding-inline: 10px; border-radius: 4px; gap: 6px; font-size: 12px;",
                                title: "Filter (/)",
                                "data-filter": "",
                                onmounted: move |event| filter_slot.set(Some(event)),
                                Search {
                                    size: 14,
                                    color: canvas_var("muted"),
                                }
                                span {
                                    style: "color: {text_color};",
                                    "{filter_text}"
                                }
                                if filter_active {
                                    span {
                                        class: "inline-block w-px",
                                        style: "height: 12px; background: {canvas_var(\"heading\")};",
                                        "data-caret": "",
                                    }
                                }
                            }
                        }
                    }
                } else {
                    {
                        let mut filter_slot = filter;
                        rsx! {
                            button {
                                "type": "button",
                                class: "flex items-center justify-center",
                                style: "width: 44px; height: 44px; color: {canvas_var(\"muted\")};",
                                title: "Filter (/)",
                                "aria-label": "Filter queue",
                                "data-filter": "",
                                onmounted: move |event| filter_slot.set(Some(event)),
                                Search {
                                    size: 17,
                                    color: canvas_var("muted"),
                                }
                            }
                        }
                    }
                }
                // The new-queue control: a press asks the local Owner for a
                // new Named queue, the same request `Ctrl+T` dispatches.
                {
                    let mut new_queue_slot = new_queue;
                    rsx! {
                        button {
                            "type": "button",
                            class: "flex items-center justify-center",
                            style: "width: 44px; height: 44px; color: {canvas_var(\"muted\")};",
                            title: "New queue (Ctrl+T)",
                            "aria-label": "New queue",
                            "data-new-queue": "",
                            onmounted: move |event| new_queue_slot.set(Some(event)),
                            Plus {
                                size: 17,
                                color: canvas_var("muted"),
                            }
                        }
                    }
                }
                // The close control: a press closes the shown entry, the same
                // request `Ctrl+W` dispatches. It is disabled on the only
                // local queue — there is nothing to close — and a disabled
                // control reports no region.
                {
                    let close_disabled = summary.header.close_disabled;
                    let close_opacity = if close_disabled { "0.4" } else { "1" };
                    let mut close_slot = close;
                    rsx! {
                        button {
                            "type": "button",
                            class: "flex items-center justify-center",
                            style: "width: 44px; height: 44px; color: {canvas_var(\"muted\")}; opacity: {close_opacity};",
                            title: "Close queue (Ctrl+W)",
                            "aria-label": "Close queue",
                            "aria-disabled": "{close_disabled}",
                            "data-close-queue": "",
                            disabled: close_disabled,
                            onmounted: move |event| close_slot.set(Some(event)),
                            X {
                                size: 17,
                                color: canvas_var("muted"),
                            }
                        }
                    }
                }
            }
        }
    }
}
