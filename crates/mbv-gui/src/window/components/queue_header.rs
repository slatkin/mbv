//! The Queue header (task 5.6, the port of `QueueHeader`; rewritten by design
//! D7 of `gui-queue-panel-redesign`): the 48 px bar over the Queue rows. It
//! paints one row of controls in the design's order — the `QUEUE` label, the
//! Queue switcher, the new-queue control, the close control, a spacer, and the
//! filter control at the trailing edge — and nothing else (spec
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
//! button takes, so `assets/app.css` excludes the header's buttons from it.
//!
//! The bar clips no child: the switcher sits in its own positioning anchor and
//! the Queue chooser's dropdown mounts inside it (design D8), so the card hangs
//! under the switcher with its leading edge on the switcher's and paints over
//! the queue rows below. The title keeps its own clipping, so a long name still
//! cannot push the controls.
//!
//! Each control carries a pointer target: it reports its own region through
//! `QueuePanel`'s report keys — the switcher `queueChooser`, the filter
//! `filter`, the new-queue control `newQueue`, and the close control
//! `closeQueue` — so a press resolves in the core to the request the matching
//! GUI-only chord dispatches, and the control and the chord run one path. The
//! close control reports no region while it is disabled (the only local queue
//! has no close): an absent or disabled control claims no pointer, so the panel
//! reads neither handle then.

use dioxus_native::prelude::*;

use crate::bridge::frame::queue::GuiQueueSummary;
use crate::window::components::queue_chooser_menu::{QueueChooserMenu, QueueChooserMenuState};
use crate::window::icons::{ChevronDown, ChevronUp, ListOrdered, Plus, Search, X};

/// The title the header paints while the shown entry has no name. The title
/// paints as plain text, never through `Truncate`: its box is sized by its own
/// text, so a measure that read a narrow first layout would cut a name such as
/// `Default` and keep the cut.
const DEFAULT_TITLE: &str = "Queue";

/// The `QUEUE` label's text (design D7).
const QUEUE_LABEL: &str = "QUEUE";

/// An icon button's look (design D7): 32 × 32, radius 6, the muted glyph. Its
/// fill and glyph colour live in the class list, never inline, so the `hover:`
/// pair can raise them — an inline colour would win over the pointer's.
const ICON_BUTTON_CLASS: &str = "flex shrink-0 items-center justify-center text-[var(--ef-grey2)]";

/// An icon button's pointer look: the raised fill and the brighter glyph.
const ICON_BUTTON_HOVER: &str = "hover:bg-[var(--ef-bg3)] hover:text-[var(--ef-fg)]";

/// The close control's enabled hover: the destructive fill and glyph (design
/// D7). A disabled close claims nothing, so it paints the rest look alone.
const CLOSE_HOVER: &str = "hover:bg-[var(--ef-bg-red)] hover:text-[var(--ef-red)]";

/// The Queue switcher's look (design D7): the raised rest fill and the control
/// border, both in the class list so the pointer can raise them, and the open
/// state's fill and border inline, which a hover cannot override.
const SWITCHER_CLASS: &str = "flex min-w-0 items-center bg-[var(--ef-bg2)] border-[var(--ef-bg4)] hover:bg-[var(--ef-bg3)] hover:border-[var(--ef-bg5)]";

/// An icon button's geometry, radius included.
const ICON_BUTTON_STYLE: &str = "width: 32px; height: 32px; border-radius: 6px;";

/// The props of the Queue header (the `QueueHeader` props, plus the region
/// handles the panel's report reads).
#[derive(Props, Debug, Clone, PartialEq)]
pub struct QueueHeaderProps {
    /// The queue summary the header's controls read.
    pub summary: GuiQueueSummary,
    /// The open Queue chooser's dropdown facts: its cursor, its entries in
    /// paint order, and the rename draft. `None` while it is closed, and the
    /// switcher's chevron follows it (design D7).
    pub chooser_menu: Option<QueueChooserMenuState>,
    /// The painted chooser entries' mounted events, by their entry index — the
    /// handles the panel's `queueChooserEntry` regions read.
    pub chooser_row_slot: Signal<Vec<Option<Event<MountedData>>>>,
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
    let chooser_menu = props.chooser_menu.clone();
    let chooser_open = chooser_menu.is_some();
    let title = if summary.header.shown_name.is_empty() {
        DEFAULT_TITLE.to_owned()
    } else {
        summary.header.shown_name.clone()
    };

    rsx! {
        div {
            class: "flex items-center whitespace-nowrap",
            style: "height: var(--canvas-queue-header-height); padding: 0 12px; gap: 4px; background: var(--ef-bg1); border-bottom: 1px solid var(--ef-bg3); margin-inline: calc(var(--mbv-cell-col) * -2);",
            "data-queue-header": "",
            // The `QUEUE` label (design D7): the bar's own name, in the canvas's
            // letter-spaced small caps.
            span {
                class: "shrink-0",
                style: "font-size: 11px; font-weight: 600; letter-spacing: 0.08em; color: var(--ef-grey2);",
                "data-queue-label": "",
                "{QUEUE_LABEL}"
            }
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
                    let mut chooser_slot = chooser;
                    let switcher_style = if chooser_open {
                        "height: 32px; padding: 0 8px 0 10px; gap: 8px; border-radius: 6px; border-width: 1px; border-style: solid; border-color: var(--ef-grey0); background: var(--ef-bg3);"
                    } else {
                        "height: 32px; padding: 0 8px 0 10px; gap: 8px; border-radius: 6px; border-width: 1px; border-style: solid;"
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
                            onmounted: move |event| chooser_slot.set(Some(event)),
                            ListOrdered {
                                size: 14,
                                stroke_width: 1.6,
                                color: "var(--ef-grey2)".to_owned(),
                            }
                            span {
                                class: "min-w-0 overflow-hidden whitespace-nowrap",
                                style: "font-size: 13px; font-weight: 600; color: var(--ef-fg);",
                                "data-queue-title": "",
                                "{title}"
                            }
                            // The shown queue's slot count (design D7): the
                            // canvas's chip, in mono, on the header bar's own
                            // fill.
                            span {
                                class: "shrink-0",
                                style: "padding: 0 4px; border-radius: 4px; background: var(--ef-bg1); color: var(--ef-grey2); font-family: var(--font-mono); font-size: 11px; line-height: 16px;",
                                "data-queue-count": "",
                                "{summary.header.shown_len}"
                            }
                            if chooser_open {
                                ChevronUp {
                                    size: 12,
                                    stroke_width: 1.6,
                                    color: "var(--ef-grey2)".to_owned(),
                                }
                            } else {
                                ChevronDown {
                                    size: 12,
                                    stroke_width: 1.6,
                                    color: "var(--ef-grey2)".to_owned(),
                                }
                            }
                        }
                    }
                }
                if let Some(menu) = chooser_menu {
                    QueueChooserMenu {
                        cursor: menu.cursor,
                        entries: menu.entries,
                        renaming: menu.renaming,
                        row_slot: props.chooser_row_slot,
                    }
                }
            }
            // The new-queue control: a press asks the local Owner for a new
            // Named queue, the same request `Ctrl+T` dispatches.
            {
                let mut new_queue_slot = new_queue;
                rsx! {
                    button {
                        "type": "button",
                        class: "{ICON_BUTTON_CLASS} {ICON_BUTTON_HOVER}",
                        style: "{ICON_BUTTON_STYLE}",
                        title: "New queue (Ctrl+T)",
                        "aria-label": "New queue",
                        "data-new-queue": "",
                        onmounted: move |event| new_queue_slot.set(Some(event)),
                        Plus {
                            size: 16,
                            stroke_width: 1.6,
                        }
                    }
                }
            }
            // The close control: a press closes the shown entry, the same
            // request `Ctrl+W` dispatches. It is disabled on the only local
            // queue — there is nothing to close — and a disabled control
            // reports no region and answers no pointer.
            {
                let close_disabled = summary.header.close_disabled;
                let close_opacity = if close_disabled { "0.4" } else { "1" };
                let close_class = if close_disabled {
                    ICON_BUTTON_CLASS.to_owned()
                } else {
                    format!("{ICON_BUTTON_CLASS} {CLOSE_HOVER}")
                };
                let mut close_slot = close;
                rsx! {
                    button {
                        "type": "button",
                        class: "{close_class}",
                        style: "{ICON_BUTTON_STYLE} opacity: {close_opacity};",
                        title: "Close queue (Ctrl+W)",
                        "aria-label": "Close queue",
                        "aria-disabled": "{close_disabled}",
                        "data-close-queue": "",
                        disabled: close_disabled,
                        onmounted: move |event| close_slot.set(Some(event)),
                        X {
                            size: 16,
                            stroke_width: 1.6,
                        }
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
                        "var(--ef-grey2)"
                    } else {
                        "var(--ef-fg)"
                    };
                    let mut filter_slot = filter;
                    let filter_active = summary.filter_active;
                    rsx! {
                        span {
                            class: "flex shrink-0 items-center",
                            style: "height: 32px; padding-inline: 8px; border-radius: 6px; gap: 6px; font-size: 12px;",
                            title: "Filter (/)",
                            "data-filter": "",
                            onmounted: move |event| filter_slot.set(Some(event)),
                            Search {
                                size: 16,
                                stroke_width: 1.6,
                                color: "var(--ef-grey2)".to_owned(),
                            }
                            span {
                                style: "color: {text_color};",
                                "{filter_text}"
                            }
                            if filter_active {
                                span {
                                    class: "inline-block w-px shrink-0",
                                    style: "height: 14px; background: var(--ef-fg);",
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
                            class: "{ICON_BUTTON_CLASS} {ICON_BUTTON_HOVER}",
                            style: "{ICON_BUTTON_STYLE}",
                            title: "Filter (/)",
                            "aria-label": "Filter queue",
                            "data-filter": "",
                            onmounted: move |event| filter_slot.set(Some(event)),
                            Search {
                                size: 16,
                                stroke_width: 1.6,
                            }
                        }
                    }
                }
            }
        }
    }
}
