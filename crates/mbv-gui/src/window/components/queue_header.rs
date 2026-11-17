//! The Queue header (task 5.6, the port of `QueueHeader`): the header band
//! over the Queue filter control. It paints the frame's `summary` section —
//! the filter's text and the caret fact are Client-core state (design D4a), so
//! the header holds none.
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
//! The filter control carries a pointer target: it reports the `filter`
//! region through `QueuePanel`'s own report key, so a press resolves in the
//! core to the `/` chord's request — the control and the chord run one path.
//! The port hands the panel the filter's mounted event, and the panel picks
//! the region from it.
//!
//! The chooser, the new-queue control, and the close control are task 8.2's.

use dioxus_native::prelude::*;

use crate::bridge::frame::library::{GuiMetaKindFrame, GuiMetaSegmentFrame};
use crate::bridge::frame::queue::GuiQueueSummary;
use crate::window::components::typed_text::TypedText;
use crate::window::icons::Search;
use crate::window::playhead::{Playhead, queue_finish_text};
use crate::window::theme::canvas_var;

/// The props of the Queue header (the `QueueHeader` props, plus the
/// region handles the panel's report reads).
#[derive(Props, Debug, Clone, PartialEq)]
pub struct QueueHeaderProps {
    /// The queue summary the header paints.
    pub summary: GuiQueueSummary,
    /// The filter control's mounted event — the handle the `filter` region
    /// reads.
    pub filter: Signal<Option<Event<MountedData>>>,
}

/// The Queue header: the canvas's two heading lines over the filter control.
#[component]
pub fn QueueHeader(props: QueueHeaderProps) -> Element {
    let playhead = consume_context::<Playhead>();
    // The queue's finish clock, `None` while no finish facts hold or the
    // summary carries no tail (design D5).
    let queue_finish = queue_finish_text(playhead.finish.cloned(), props.summary.tail_secs);
    let summary = &props.summary;
    let filter = props.filter;

    rsx! {
        div {
            class: "flex items-center justify-between overflow-hidden whitespace-nowrap",
            style: "height: var(--canvas-queue-header-height); padding: 12px 8px 8px 16px; margin-inline: calc(var(--mbv-cell-col) * -2); background: var(--canvas-dim); border-bottom: 1px solid var(--canvas-border);",
            "data-queue-header": "",
            // The canvas's two heading lines, 2 px apart: the count over the
            // queue's finish clock. Both lines carry their data type (design
            // D14): the remaining count is a count and the finish clock is a
            // time.
            div {
                class: "flex flex-col",
                style: "gap: 2px;",
                span {
                    style: "font-size: 13px; line-height: 18px; font-weight: 600; color: {canvas_var(\"heading\")};",
                    "data-queue-heading": "",
                    TypedText {
                        segments: vec![
                            GuiMetaSegmentFrame {
                                text: "Up next · ".to_owned(),
                                kind: GuiMetaKindFrame::Plain,
                            },
                            GuiMetaSegmentFrame {
                                text: summary.remaining.to_string(),
                                kind: GuiMetaKindFrame::Count,
                            },
                        ],
                    }
                }
                if let Some(finish) = queue_finish {
                    span {
                        class: "font-mono",
                        style: "font-size: 11px; line-height: 16px; color: {canvas_var(\"muted\")};",
                        "data-finish-clock": "",
                        TypedText {
                            segments: vec![
                                GuiMetaSegmentFrame {
                                    text: "queue ends ".to_owned(),
                                    kind: GuiMetaKindFrame::Plain,
                                },
                                GuiMetaSegmentFrame {
                                    text: finish,
                                    kind: GuiMetaKindFrame::Time,
                                },
                            ],
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
            }
        }
    }
}
