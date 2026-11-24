//! The Queue header (task 5.6, the port of `QueueHeader`; rewritten by design
//! D7 of `gui-queue-panel-redesign`): the 48 px bar over the Queue rows. It
//! paints one row of controls in the design's order — the playback target
//! control (user request 2026-11-20), the Queue switcher, the
//! new-queue control, the close control, a spacer, and the filter control at
//! the trailing edge — and nothing else (spec
//! `gui-queue-chooser` "The queue header holds the chooser and its controls").
//!
//! The switcher always paints (design D7): it is not a row of tabs and it does
//! not wait for a second entry. It carries the list glyph, the shown entry's
//! name — or `Queue` while that name is empty (user decision 2026-11-17) — the
//! shown queue's slot count, and a chevron that points up while the Queue
//! chooser's dropdown is open. The count is the shown queue's whole slot
//! count: the Queue filter narrows the painted rows, never the queue.
//!
//! Every fact the controls paint is the frame's; the filter's text and the
//! caret are Client-core state (design D4a), so the header holds none. The
//! header paints the queue column's `--ef-*` tokens (design D1) and takes its
//! hover fills from the design's own table: the bar answers the pointer with a
//! raised fill and a brighter glyph rather than the canvas veil every other
//! button takes, so the bar sets `--hover-veil: transparent` and the shared
//! veil (the `app.css` `box-shadow` rule) cannot wash that look.
//!
//! The bar clips no child: the switcher sits in its own positioning anchor and
//! the Queue chooser's dropdown mounts inside it (design D8), so the card hangs
//! under the switcher with its leading edge on the switcher's and paints over
//! the queue rows below. `QueuePanel` builds that dropdown and passes it as
//! this component's children, so the header carries no chooser facts. The title
//! keeps its own clipping, so a long name still cannot push the controls.
//!
//! Each control carries a pointer target: it reports its own region through
//! `QueuePanel`'s region set — the switcher `queueChooser`, the filter
//! `filter`, the new-queue control `newQueue`, and the close control
//! `closeQueue` — so a press resolves in the core to the request the matching
//! GUI-only chord dispatches, and the control and the chord run one path. The
//! panel marks each control in its own render and hands the closures down as
//! this component's props, so the marks record in the panel's report order.
//! The close control reports no region while it is disabled (the only local
//! queue has no close): an absent or disabled control claims no pointer, so
//! the panel passes no close mark then and this control attaches none.

use dioxus_native::prelude::*;

use crate::bridge::frame::queue::GuiQueueSummary;
use crate::window::components::queue_panel::geometry::{CONTROL_PX, HEADER_HEIGHT_PX};
use crate::window::components::queue_target_control::QueueTargetControl;
use crate::window::icons::{ChevronDown, ChevronUp, ListOrdered, Plus, Search, SquarePen, X};
use crate::window::layout::region_set::MarkHandler;
use crate::window::theme::{EfToken, ef_var};

/// The title the header paints while the shown entry has no name (the label
/// the header once led with is gone, user request 2026-11-19). The title
/// paints as plain text, never through `Truncate`: its box is sized by its own
/// text, so a measure that read a narrow first layout would cut a name such as
/// `Default` and keep the cut.
const DEFAULT_TITLE: &str = "Queue";

/// An icon button's look (design D7): 32 × 32, radius 6. Its rest and pointer
/// colours live in `assets/app.css` (`[data-queue-header] button`), keyed on
/// the header's own markers, because they read the `--ef-*` palette through
/// the typed accessor a class scanner cannot see.
const ICON_BUTTON_CLASS: &str = "flex shrink-0 items-center justify-center";

/// The Queue switcher's look (design D7): the raised rest fill, the control
/// border, and the pointer's raised pair live in `assets/app.css`
/// (`[data-queue-chooser]`); the open state's fill and border stay inline,
/// where a hover cannot override them.
const SWITCHER_CLASS: &str = "flex min-w-0 items-center";

/// The props of the Queue header (the `QueueHeader` props, plus the mark
/// closures the panel's region set reports its controls' regions from).
#[derive(Props, Debug, Clone, PartialEq)]
pub struct QueueHeaderProps {
    /// The queue summary the header's controls read.
    pub summary: GuiQueueSummary,
    /// Whether the Queue chooser's dropdown is open: the switcher's chevron
    /// points up while it is (design D7).
    pub chooser_open: bool,
    /// The Queue chooser button's mark — the closure that records the handle
    /// the `queueChooser` region reads.
    pub chooser: MarkHandler,
    /// The filter control's mark — the closure that records the handle the
    /// `filter` region reads.
    pub filter: MarkHandler,
    /// The new-queue control's mark — the closure that records the handle the
    /// `newQueue` region reads.
    pub new_queue: MarkHandler,
    /// The close control's mark — the closure that records the handle the
    /// `closeQueue` region reads. The panel passes none while the control is
    /// disabled, so the control attaches no `onmounted` and reports nothing.
    pub close: Option<MarkHandler>,
    /// The rename control's mark — the closure that records the handle the
    /// `renameQueue` region reads. The panel passes none while the shown entry
    /// is no local queue, so the control reports nothing then.
    pub rename: Option<MarkHandler>,
    /// The open Queue chooser's dropdown, built by `QueuePanel` and placed in
    /// the switcher's anchor (design D8). The header carries no chooser facts.
    pub children: Element,
}

/// The Queue header: the `QUEUE` label and the switcher on the leading edge,
/// the new-queue and close controls beside it, and the filter at the trailing
/// edge.
#[component]
pub fn QueueHeader(props: QueueHeaderProps) -> Element {
    let summary = &props.summary;
    let chooser = props.chooser;
    let filter = props.filter;
    let new_queue = props.new_queue;
    let close = props.close;
    let rename = props.rename;
    let chooser_open = props.chooser_open;
    let title = if summary.header.shown_name.is_empty() {
        DEFAULT_TITLE.to_owned()
    } else {
        summary.header.shown_name.clone()
    };
    // An icon button's geometry, radius included.
    let icon_button_style =
        format!("width: {CONTROL_PX}px; height: {CONTROL_PX}px; border-radius: 6px;");

    rsx! {
        div {
            class: "flex items-center whitespace-nowrap",
            style: format!(
                "height: {HEADER_HEIGHT_PX}px; padding: 0 8px; gap: 2px; background: {}; border-bottom: 1px solid {}; margin-inline: calc(var(--mbv-cell-col) * -2); --hover-veil: transparent;",
                ef_var(EfToken::Bg1),
                ef_var(EfToken::Bg3)
            ),
            "data-queue-header": "",
            // The playback target control (user request 2026-11-20): it leads
            // the bar, to the left of the Queue switcher, and owns its own
            // chooser popover and region reports.
            QueueTargetControl {}
            // The Queue switcher and its dropdown's anchor (design D7, D8): the
            // button names the shown entry, counts its slots, and opens the
            // dropdown; the card mounts inside this box, so its leading edge is
            // the switcher's and it drops four pixels under it. The box takes no
            // clipping of its own — the title keeps that — so the card is free to
            // hang below the bar over the rows.
            div {
                style: "position: relative; min-width: 0;",
                class: "flex items-center",
                "data-queue-switcher-anchor": "",
                {
                    let switcher_style = if chooser_open {
                        format!(
                            "height: {CONTROL_PX}px; padding: 0 6px 0 8px; gap: 6px; border-radius: 6px; border-width: 1px; border-style: solid; border-color: {}; background: {};",
                            ef_var(EfToken::Grey0),
                            ef_var(EfToken::Bg3)
                        )
                    } else {
                        format!(
                            "height: {CONTROL_PX}px; padding: 0 6px 0 8px; gap: 6px; border-radius: 6px; border-width: 1px; border-style: solid;"
                        )
                    };
                    rsx! {
                        button {
                            "type": "button",
                            class: "{SWITCHER_CLASS}",
                            style: "{switcher_style}",
                            title: "Queue chooser (t)",
                            "aria-label": "Queue chooser",
                            "aria-expanded": "{chooser_open}",
                            "data-queue-chooser": "",
                            onmounted: chooser,
                            ListOrdered {
                                size: 14,
                                stroke_width: 1.6,
                                color: ef_var(EfToken::Grey2).to_owned(),
                            }
                            span {
                                class: "min-w-0 overflow-hidden whitespace-nowrap",
                                style: format!(
                                    "font-size: 13px; font-weight: 600; color: {};",
                                    ef_var(EfToken::Fg)
                                ),
                                "data-queue-title": "",
                                "{title}"
                            }
                            // The shown queue's slot count (design D7): the
                            // canvas's chip, in mono, on the header bar's own
                            // fill.
                            span {
                                class: "shrink-0",
                                style: format!(
                                    "padding: 0 4px; border-radius: 4px; background: {}; color: {}; font-family: var(--font-mono); font-size: 11px; line-height: 16px;",
                                    ef_var(EfToken::Bg1),
                                    ef_var(EfToken::Grey2)
                                ),
                                "data-queue-count": "",
                                "{summary.header.shown_len}"
                            }
                            if chooser_open {
                                ChevronUp {
                                    size: 12,
                                    stroke_width: 1.6,
                                    color: ef_var(EfToken::Grey2).to_owned(),
                                }
                            } else {
                                ChevronDown {
                                    size: 12,
                                    stroke_width: 1.6,
                                    color: ef_var(EfToken::Grey2).to_owned(),
                                }
                            }
                        }
                    }
                }
                {props.children}
            }
            // The new-queue control: a press asks the local Owner for a new
            // Named queue, the same request `Ctrl+T` dispatches.
            button {
                "type": "button",
                class: "{ICON_BUTTON_CLASS}",
                style: "{icon_button_style}",
                title: "New queue (Ctrl+T)",
                "aria-label": "New queue",
                "data-new-queue": "",
                onmounted: new_queue,
                Plus {
                    size: 14,
                    stroke_width: 1.6,
                }
            }
            // The close control: a press closes the shown entry, the same
            // request `Ctrl+W` dispatches. It is disabled on the only local
            // queue — there is nothing to close — and a disabled control
            // reports no region and answers no pointer. The panel passes its
            // mark only while the control acts, and this control paints one of
            // two buttons: the acting one attaches the mark, the disabled one
            // attaches no `onmounted` at all. This renderer adds a listener only
            // when one appears, so a control that kept its listener would never
            // re-report after it was enabled.
            if let Some(close_mark) = close {
                button {
                    "type": "button",
                    class: "{ICON_BUTTON_CLASS}",
                    style: "{icon_button_style} opacity: 1;",
                    title: "Close queue (Ctrl+W)",
                    "aria-label": "Close queue",
                    "aria-disabled": "false",
                    "data-close-queue": "",
                    onmounted: close_mark,
                    X {
                        size: 14,
                        stroke_width: 1.6,
                    }
                }
            } else {
                button {
                    "type": "button",
                    class: "{ICON_BUTTON_CLASS}",
                    style: "{icon_button_style} opacity: 0.4;",
                    title: "Close queue (Ctrl+W)",
                    "aria-label": "Close queue",
                    "aria-disabled": "true",
                    "data-close-queue": "",
                    disabled: true,
                    X {
                        size: 14,
                        stroke_width: 1.6,
                    }
                }
            }
            // The rename control: a press opens the shown local queue's name
            // for edit, in the draft a new queue's name opens in. It is
            // disabled while the shown entry is the peer, which has no name to
            // edit, and — like the close control — attaches its mark only while
            // it acts.
            if let Some(rename_mark) = rename {
                button {
                    "type": "button",
                    class: "{ICON_BUTTON_CLASS}",
                    style: "{icon_button_style} opacity: 1;",
                    title: "Rename queue",
                    "aria-label": "Rename queue",
                    "aria-disabled": "false",
                    "data-rename-queue": "",
                    onmounted: rename_mark,
                    SquarePen {
                        size: 14,
                        stroke_width: 1.6,
                    }
                }
            } else {
                button {
                    "type": "button",
                    class: "{ICON_BUTTON_CLASS}",
                    style: "{icon_button_style} opacity: 0.4;",
                    title: "Rename queue",
                    "aria-label": "Rename queue",
                    "aria-disabled": "true",
                    "data-rename-queue": "",
                    disabled: true,
                    SquarePen {
                        size: 14,
                        stroke_width: 1.6,
                    }
                }
            }
            // The spacer: the tab's own controls stay beside it and the filter
            // takes the bar's trailing edge (design D7).
            div { class: "min-w-0 flex-1", "data-queue-spacer": "" }
            // The filter control: a magnifier button at rest, and the text entry
            // with the block caret while the entry holds the keyboard
            // (`filterActive`) or a filter narrows the rows. A press on it runs
            // the `/` chord's text entry in the core through the `filter` region
            // the mark reports.
            if summary.filter_active || !summary.filter.is_empty() {
                {
                    let filter_text = if summary.filter.is_empty() {
                        "Filter".to_owned()
                    } else {
                        summary.filter.clone()
                    };
                    let text_color = if summary.filter.is_empty() {
                        ef_var(EfToken::Grey2)
                    } else {
                        ef_var(EfToken::Fg)
                    };
                    let filter_active = summary.filter_active;
                    rsx! {
                        span {
                            class: "flex shrink-0 items-center",
                            style: format!(
                                "height: {CONTROL_PX}px; padding-inline: 6px; border-radius: 6px; gap: 4px; font-size: 12px;"
                            ),
                            title: "Filter (/)",
                            "data-filter": "",
                            onmounted: filter,
                            Search {
                                size: 14,
                                stroke_width: 1.6,
                                color: ef_var(EfToken::Grey2).to_owned(),
                            }
                            span {
                                style: "color: {text_color};",
                                "{filter_text}"
                            }
                            if filter_active {
                                span {
                                    class: "inline-block w-px shrink-0",
                                    style: format!(
                                        "height: 14px; background: {};",
                                        ef_var(EfToken::Fg)
                                    ),
                                    "data-caret": "",
                                }
                            }
                        }
                    }
                }
            } else {
                {
                    rsx! {
                        button {
                            "type": "button",
                            class: "{ICON_BUTTON_CLASS}",
                            style: "{icon_button_style}",
                            title: "Filter (/)",
                            "aria-label": "Filter queue",
                            "data-filter": "",
                            onmounted: filter,
                            Search {
                                size: 14,
                                stroke_width: 1.6,
                            }
                        }
                    }
                }
            }
        }
    }
}
