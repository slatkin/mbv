//! The Queue header (task 5.6, the port of `QueueHeader`): the
//! `Up next · N` heading over the `queue ends HH:MM` line at the left, the
//! "Length / Ends at" right-column switch and the filter control at the
//! right. All of it paints the frame's `summary` section — the switch's
//! mode, the filter's text, and the caret fact are Client-core state
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
//! The switch and the filter each carry a pointer target: the switch's
//! inactive segment and the filter report the `rightColumn` and `filter`
//! regions through `QueuePanel`'s own report key, so a press resolves in
//! the core to the `t` and `/` chords' requests — the control and the
//! chord run one path. The active segment reports no target: pressing it
//! would toggle away from the mode it already shows. The port hands the
//! panel the two segments' mounted events (keyed by their mode, both set
//! at the header's mount) and the filter's mounted event, and the panel
//! picks the inactive segment's region from the summary's mode — the
//! port of the former parent's `querySelectorAll` over the painted DOM.

use dioxus_native::prelude::*;

use crate::bridge::frame::library::{GuiMetaKindFrame, GuiMetaSegmentFrame};
use crate::bridge::frame::queue::{GuiQueueSummary, GuiRightColumn};
use crate::window::components::typed_text::TypedText;
use crate::window::icons::Search;
use crate::window::playhead::{Playhead, queue_finish_text};
use crate::window::theme::canvas_var;

/// The right-column switch's two modes in paint order.
const MODES: [(GuiRightColumn, &str); 2] = [
    (GuiRightColumn::Length, "Length"),
    (GuiRightColumn::EndsAt, "Ends at"),
];

/// The wire name a mode paints in `data-mode` and
/// `data-right-column-segment`, as the mode's camelCase wire name.
fn mode_name(mode: GuiRightColumn) -> &'static str {
    match mode {
        GuiRightColumn::Length => "length",
        GuiRightColumn::EndsAt => "endsAt",
    }
}

/// The props of the Queue header (the `QueueHeader` props, plus the
/// region handles the panel's report reads).
#[derive(Props, Debug, Clone, PartialEq)]
pub struct QueueHeaderProps {
    /// The queue summary the header paints.
    pub summary: GuiQueueSummary,
    /// The two switch segments' mounted events, keyed by the mode each
    /// paints — the handles the panel's `rightColumn` region reads.
    pub right_column: Signal<Vec<(GuiRightColumn, Event<MountedData>)>>,
    /// The filter control's mounted event — the handle the `filter` region
    /// reads.
    pub filter: Signal<Option<Event<MountedData>>>,
}

/// The Queue header: the canvas's two heading lines over the switch and the
/// filter control.
#[component]
pub fn QueueHeader(props: QueueHeaderProps) -> Element {
    let playhead = consume_context::<Playhead>();
    // The queue's finish clock, `None` while no finish facts hold or the
    // summary carries no tail (design D5).
    let queue_finish = queue_finish_text(playhead.finish.cloned(), props.summary.tail_secs);
    let summary = &props.summary;
    let right_column = props.right_column;
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
                // The "Length / Ends at" switch: a recessed track the active
                // segment fills, and a press on the inactive one runs the `t`
                // chord's toggle in the core through the `rightColumn` region
                // the segment reports.
                div {
                    class: "flex items-center",
                    style: "background: {canvas_var(\"well\")}; border-radius: 6px; padding: 2px;",
                    title: "Right column (t)",
                    "data-right-column-switch": "",
                    for (mode, label) in MODES {
                        {
                            let active = summary.right_column == mode;
                            let mode_name = mode_name(mode);
                            let segment_style = format!(
                                "height: 28px; padding-inline: 10px; border-radius: 4px; font-size: 12px; background: {}; color: {};",
                                if active {
                                    canvas_var("segment-active")
                                } else {
                                    "transparent".to_owned()
                                },
                                if active {
                                    canvas_var("heading")
                                } else {
                                    canvas_var("muted")
                                },
                            );
                            let mut segment_right_column = right_column;
                            rsx! {
                                span {
                                    key: "{mode_name}",
                                    class: "flex items-center",
                                    style: "{segment_style}",
                                    "data-mode": "{mode_name}",
                                    "data-active": active.then(|| "true".to_owned()),
                                    // Only the inactive segment paints the press
                                    // target's mark; the panel reads the attribute's
                                    // holder from the summary's mode.
                                    "data-right-column-segment": (!active)
                                        .then(|| mode_name.to_owned()),
                                    onmounted: move |event| {
                                        segment_right_column.write().push((mode, event));
                                    },
                                    "{label}"
                                }
                            }
                        }
                    }
                }
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
