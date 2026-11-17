//! The Queue chooser's dropdown (task 8.3 of `gui-named-queues`, design D8):
//! the local Owner's Named queues in queue order, then the connected peer
//! Player owner, each with the marks the frame carries, and the rename draft
//! while one is open (spec `gui-queue-chooser` "The chooser lists local queues
//! and connected peers").
//!
//! The window holds no open, cursor, or rename state: the frame's `queueChooser`
//! section carries the core's cursor, the entries, and the draft, and a press
//! forwards like every other pointer event — the core resolves it against the
//! hit map. The card anchors under the Queue header at the chooser button's
//! leading edge and paints the row menu's own canvas chrome (the `Menus`
//! artboard's fill, border, radius, shadow, and rows at the size the panel needs), so the two
//! queue overlays read as one family.
//!
//! The marks: the shown entry leads with the canvas's check, the current queue —
//! the one that holds the playhead — trails with a filled accent dot, or with
//! the audio-lines mark in its place while the local Owner plays it, and the
//! peer entry trails with the direct-peer brand mark, so a peer is never read as
//! a local queue and binds no rename. The rename draft paints in place of its
//! entry's label, with the block caret the Queue filter's entry paints, because
//! the core holds the text (design D8).
//!
//! `QueuePanel` measures the `[data-chooser-entry]` rows through their mounted
//! handles and reports their entry regions under its own second report key,
//! inserted when the chooser opens — the row menu's technique — so the entries
//! land after the queue's regions in the merged report and win the hit map where
//! the dropdown paints over a queue row beneath it (the hit map's last push
//! wins). A child-held key would land first, before the parent's, and lose.

use dioxus_native::prelude::*;

use crate::bridge::frame::overlay::{GuiQueueChooserEntry, GuiQueueChooserRename};
use crate::window::brand_icons::{TargetBrand, TargetBrandIcon};
use crate::window::components::menu_card::MENU_CARD_STYLE;
use crate::window::components::row_handle::record_row_handle;
use crate::window::icons::{AudioLines, Check};
use crate::window::theme::canvas_var;

/// The current queue's mark: the accent dot's diameter in logical pixels.
const CURRENT_MARK_PX: u16 = 8;

/// The props of the Queue chooser's dropdown: the core's cursor, the frame's
/// entries in paint order, the rename draft while one is open, and the handles
/// the panel's entry regions read.
#[derive(Props, Debug, Clone, PartialEq)]
pub struct QueueChooserMenuProps {
    /// The core's cursor row.
    pub cursor: usize,
    /// The entries the frame carries: local queues in queue order, then the
    /// peer.
    pub entries: Vec<GuiQueueChooserEntry>,
    /// The rename draft, while a name is open for edit.
    pub renaming: Option<GuiQueueChooserRename>,
    /// The painted rows' mounted events, by their entry index.
    pub row_slot: Signal<Vec<Option<Event<MountedData>>>>,
}

/// One entry's paint facts: the text it paints, and whether that text is the
/// rename draft's rather than the entry's own label.
struct EntryPaint {
    label: String,
    draft: bool,
    /// Whether the draft's whole text is selected: it paints highlighted and
    /// carries no caret.
    selected: bool,
}

impl EntryPaint {
    /// The facts one entry paints: its label, or the draft's text while its name
    /// is open for edit.
    fn of(entry: &GuiQueueChooserEntry, renaming: Option<&GuiQueueChooserRename>) -> Self {
        let draft = renaming.filter(|draft| Some(draft.queue) == entry.queue);
        Self {
            label: draft.map_or_else(|| entry.label.clone(), |draft| draft.text.clone()),
            draft: draft.is_some(),
            selected: draft.is_some_and(|draft| draft.selected),
        }
    }
}

/// The entry's `data-chooser-entry` value: the local queue's `NamedQueueId::raw`,
/// or `peer` for the connected peer Player owner.
fn entry_key(entry: &GuiQueueChooserEntry) -> String {
    entry
        .queue
        .map_or_else(|| "peer".to_owned(), |raw| raw.to_string())
}

/// The Queue chooser's dropdown: one 40 px row per entry in canvas chrome, the
/// cursor row filled, the shown entry checked, the current queue dotted, the peer
/// entry brand-marked, and the rename draft's caret after the text it edits.
#[component]
pub fn QueueChooserMenu(props: QueueChooserMenuProps) -> Element {
    rsx! {
        div {
            class: "absolute z-20 flex flex-col",
            role: "menu",
            "aria-label": "Queue chooser",
            style: format!(
                "top: var(--canvas-queue-header-height); left: 16px; width: calc(100% - 32px); max-width: 28rem; padding: 8px; gap: 4px; {MENU_CARD_STYLE}"
            ),
            "data-queue-chooser-menu": "",
            for (index, entry) in props.entries.iter().enumerate() {
                {
                    let paint = EntryPaint::of(entry, props.renaming.as_ref());
                    let cursor = props.cursor == index;
                    let row_color = if cursor {
                        canvas_var("heading")
                    } else {
                        canvas_var("text")
                    };
                    let row_style = format!(
                        "height: 40px; padding: 0 14px; gap: 10px; border-radius: 6px; font-size: 15px; background: {}; color: {row_color};",
                        if cursor { "var(--canvas-selected)" } else { "none" },
                    );
                    let label_align = if paint.draft { " justify-content: flex-end;" } else { "" };
                    let label_style = if paint.selected {
                        format!(
                            "padding: 0 2px; border-radius: 2px; background: {}; color: {};",
                            canvas_var("accent"),
                            canvas_var("heading"),
                        )
                    } else {
                        String::new()
                    };
                    let key = entry_key(entry);
                    let label = entry.label.clone();
                    let shown = entry.shown;
                    let current = entry.current;
                    let playing = entry.playing;
                    let peer = entry.queue.is_none();
                    let row_slot = props.row_slot;
                    rsx! {
                        button {
                            "type": "button",
                            class: "flex w-full items-center justify-between text-left",
                            role: "menuitem",
                            style: "{row_style}",
                            "data-chooser-entry": "{key}",
                            "aria-label": "{label}",
                            onmounted: move |event| {
                                record_row_handle(row_slot, index, event);
                            },
                            div {
                                class: "flex min-w-0 items-center",
                                style: "gap: 8px;",
                                // The shown entry's check: the leading slot holds
                                // every entry's mark, so the labels line up.
                                span {
                                    class: "flex w-5 shrink-0 justify-center",
                                    "data-chooser-shown": "{shown}",
                                    if shown {
                                        Check { size: 18, color: canvas_var("accent") }
                                    }
                                }
                                // The entry's label, or the name open for edit:
                                // highlighted while all of it is selected, else
                                // with the filter's block caret after it. Plain
                                // text, never `Truncate`: a content-sized
                                // `Truncate` box keeps the cut of its first
                                // measure, which cut a name as it was typed. A
                                // draft aligns to the end, so a name longer than
                                // the row keeps its caret in view.
                                div {
                                    class: "flex min-w-0 items-center overflow-hidden whitespace-nowrap",
                                    style: "gap: 2px; font-size: 15px;{label_align}",
                                    span {
                                        class: "shrink-0",
                                        style: "{label_style}",
                                        "data-chooser-label": "",
                                        "{paint.label}"
                                    }
                                    if paint.draft && !paint.selected {
                                        span {
                                            class: "inline-block w-px shrink-0",
                                            style: "height: 16px; background: {canvas_var(\"heading\")};",
                                            "data-caret": "",
                                        }
                                    }
                                }
                            }
                            div {
                                class: "flex shrink-0 items-center",
                                style: "gap: 4px;",
                                // The current queue — the one that holds the
                                // playhead — trails with the accent dot, and
                                // with the audio-lines mark in its place while
                                // the local Owner plays it.
                                if playing {
                                    span {
                                        class: "flex items-center",
                                        title: "Playing",
                                        "data-chooser-playing": "",
                                        AudioLines { size: 18, color: canvas_var("accent") }
                                    }
                                } else if current {
                                    span {
                                        class: "inline-block rounded-full",
                                        style: "width: {CURRENT_MARK_PX}px; height: {CURRENT_MARK_PX}px; background: {canvas_var(\"accent\")};",
                                        title: "Current queue",
                                        "data-chooser-current": "",
                                    }
                                }
                                // The peer entry's own mark: a peer owns no Named
                                // queue, so it trails with the direct-peer brand
                                // icon the playback-target chooser paints for it.
                                if peer {
                                    TargetBrandIcon { brand: TargetBrand::DirectPeer, size: 18 }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
