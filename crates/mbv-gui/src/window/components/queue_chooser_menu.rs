//! The Queue chooser's dropdown (task 8.3 of `gui-named-queues`; restyled by
//! design D8 of `gui-queue-panel-redesign`): the local Owner's Named queues in
//! queue order, then the connected peer Player owner, each with the marks and
//! the slot count the frame carries, and the rename draft while one is open
//! (spec `gui-queue-chooser` "The chooser lists local queues and connected
//! peers").
//!
//! The window holds no open, cursor, or rename state: the frame's `queueChooser`
//! section carries the core's cursor, the entries, and the draft, and a press
//! forwards like every other pointer event — the core resolves it against the
//! hit map. The card is a child of the switcher's own anchor box, so its leading
//! edge is the switcher's and it drops four pixels under it (design D8), and it
//! paints the design's own card and item chrome: a queue-row-height entry in the
//! raised fill on the cursor and under the pointer, so the two queue overlays
//! read as one family. The card answers the pointer with those fills alone, so it sets
//! `--hover-veil: transparent` and the shared veil does not wash them.
//!
//! Each row paints, in the design's order: the shown entry's check, the name —
//! or the name open for edit — then at the trailing end the playing mark, the
//! current queue's dot, or the peer's brand mark, and the queue's slot count in
//! mono last. The check takes the played green, the playing mark the aqua the
//! design gives it, and the count the secondary grey, so a count never reads as
//! a mark. The rename draft paints in place of its entry's label, with the
//! block caret the Queue filter's entry paints, because the core holds the text
//! (design D8).
//!
//! The component is presentation-only. The surface that owns the dropdown marks
//! its `[data-chooser-entry]` rows: `QueuePanel` passes one closure per painted
//! row, from its region set's `mark_over` at the dropdown's position in its
//! `rsx`, so the entries report under the panel's overlay key, after the queue's
//! regions in the merged report, and win the hit map where the dropdown paints
//! over a queue row beneath it (the hit map's last push wins). Reporting from
//! the owner, not from this child's mount effect, keeps that order even when
//! the first painted frame already carries the open chooser: a child's mount
//! effect runs before the parent's, so a child-held key would land first and
//! the entries would lose to the regions beneath them. `QueuePanel` builds the
//! card and mounts it in the header's switcher anchor, which changes nothing
//! about that order: the key belongs to the panel, not to where the card mounts.

use dioxus_native::prelude::*;

use crate::bridge::frame::overlay::{GuiOverlayFrame, GuiQueueChooserEntry, GuiQueueChooserRename};
use crate::window::brand_icons::{TargetBrand, TargetBrandIcon};
use crate::window::components::queue_panel::geometry::{MENU_DROP_PX, ROW_GAP_PX, ROW_HEIGHT_PX};
use crate::window::icons::{AudioLines, Check};
use crate::window::layout::region_set::MarkHandler;
use crate::window::theme::{EfToken, ef_var};

/// The current queue's mark: the dot's diameter in logical pixels.
const CURRENT_MARK_PX: u16 = 8;

/// One entry's height and the gap between two items are the queue rows' own
/// (`ROW_HEIGHT_PX` and `ROW_GAP_PX`: the delineated row's 40 px and its 0 px
/// gap, design D3 of `gui-delineated-rows`); the card's padding is its own
/// inset.
const CARD_PADDING_PX: u16 = 4;

/// The open Queue chooser's dropdown facts, as the frame's overlay carries
/// them: the core's cursor, the entries in paint order, and the rename draft
/// while one is open. The header's switcher anchor paints the dropdown from
/// these, so the card needs no second match on the overlay.
#[derive(Clone, Debug, PartialEq)]
pub struct QueueChooserMenuState {
    pub cursor: usize,
    pub entries: Vec<GuiQueueChooserEntry>,
    pub renaming: Option<GuiQueueChooserRename>,
}

impl QueueChooserMenuState {
    /// The open chooser's facts, or `None` while another overlay (or none) is
    /// up.
    #[must_use]
    pub fn from_overlay(overlay: Option<&GuiOverlayFrame>) -> Option<Self> {
        match overlay {
            Some(GuiOverlayFrame::QueueChooser {
                cursor,
                entries,
                renaming,
            }) => Some(Self {
                cursor: *cursor,
                entries: entries.clone(),
                renaming: renaming.clone(),
            }),
            _ => None,
        }
    }
}

/// The props of the Queue chooser's dropdown: the core's cursor, the frame's
/// entries in paint order, the rename draft while one is open, and the marks of
/// the rows the panel's entry regions read.
#[derive(Props, Debug, Clone, PartialEq)]
pub struct QueueChooserMenuProps {
    /// The core's cursor row.
    pub cursor: usize,
    /// The entries the frame carries: local queues in queue order, then the
    /// peer.
    pub entries: Vec<GuiQueueChooserEntry>,
    /// The rename draft, while a name is open for edit.
    pub renaming: Option<GuiQueueChooserRename>,
    /// The painted rows' marks, in row order — the closures `QueuePanel`
    /// produced with its region set's `mark_over`. A row takes its own entry.
    pub row_marks: Vec<MarkHandler>,
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

/// The Queue chooser's dropdown: one entry per queue at the queue rows' height
/// and gap, the cursor row filled, the shown entry checked, the current queue
/// dotted, the playing queue marked in aqua, the peer entry brand-marked, each
/// with its slot count, and the rename draft's caret after the text it edits.
/// It fills its positioning ancestor — the switcher's anchor — dropping four
/// pixels under it with its leading edge on the switcher's (design D8).
#[component]
pub fn QueueChooserMenu(props: QueueChooserMenuProps) -> Element {
    rsx! {
        div {
            class: "absolute z-20 flex flex-col",
            role: "menu",
            "aria-label": "Queue chooser",
            style: format!(
                "top: calc(100% + {MENU_DROP_PX}px); left: 0; min-width: 260px; max-width: 28rem; padding: {CARD_PADDING_PX}px; gap: {ROW_GAP_PX}px; background: {}; border: 1px solid {}; border-radius: 8px; --hover-veil: transparent;",
                ef_var(EfToken::Bg2),
                ef_var(EfToken::Bg4)
            ),
            "data-queue-chooser-menu": "",
            for (index, entry) in props.entries.iter().enumerate() {
                {
                    let paint = EntryPaint::of(entry, props.renaming.as_ref());
                    let cursor = props.cursor == index;
                    let row_background = if cursor {
                        format!(" background: {};", ef_var(EfToken::Bg3))
                    } else {
                        String::new()
                    };
                    let row_style = format!(
                        "height: {ROW_HEIGHT_PX}px; padding: 0 8px; gap: 8px; border-radius: 5px; font-size: 13px; font-weight: 500; color: {};{row_background}",
                        ef_var(EfToken::Fg)
                    );
                    let label_align = if paint.draft {
                        " justify-content: flex-end;"
                    } else {
                        ""
                    };
                    let label_style = if paint.selected {
                        format!(
                            "padding: 0 2px; border-radius: 2px; background: {}; color: {};",
                            ef_var(EfToken::Green),
                            ef_var(EfToken::BgDim)
                        )
                    } else {
                        String::new()
                    };
                    let key = entry_key(entry);
                    let label = entry.label.clone();
                    let len = entry.len;
                    let shown = entry.shown;
                    let current = entry.current;
                    let playing = entry.playing;
                    let peer = entry.queue.is_none();
                    let row_mark = props.row_marks.get(index).copied();
                    rsx! {
                        button {
                            "type": "button",
                            class: "flex w-full items-center justify-between text-left",
                            role: "menuitem",
                            style: "{row_style}",
                            "data-chooser-entry": "{key}",
                            "aria-label": "{label}",
                            onmounted: move |event| {
                                // The panel marks this row: the closure records the
                                // handle its overlay region reads.
                                if let Some(mark) = row_mark {
                                    mark.call(event);
                                }
                            },
                            div {
                                class: "flex min-w-0 items-center",
                                style: "gap: 8px;",
                                // The shown entry's check: the leading slot holds
                                // every entry's mark, so the labels line up.
                                span {
                                    class: "flex w-4 shrink-0 justify-center",
                                    "data-chooser-shown": "{shown}",
                                    if shown {
                                        Check {
                                            size: 14,
                                            color: ef_var(EfToken::Green).to_owned(),
                                        }
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
                                    style: "gap: 2px; font-size: 13px;{label_align}",
                                    span {
                                        class: "shrink-0",
                                        style: "{label_style}",
                                        "data-chooser-label": "",
                                        "{paint.label}"
                                    }
                                    if paint.draft && !paint.selected {
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
                            // The entry's marks and its slot count (design D8):
                            // the playing queue's aqua mark, or the current
                            // queue's dot, or the peer's brand, then the count in
                            // mono.
                            div {
                                class: "flex shrink-0 items-center",
                                style: "gap: 6px;",
                                if playing {
                                    span {
                                        class: "flex items-center",
                                        title: "Playing",
                                        "data-chooser-playing": "",
                                        AudioLines {
                                            size: 14,
                                            color: ef_var(EfToken::Aqua).to_owned(),
                                        }
                                    }
                                } else if current {
                                    span {
                                        class: "inline-block rounded-full",
                                        style: format!(
                                            "width: {CURRENT_MARK_PX}px; height: {CURRENT_MARK_PX}px; background: {};",
                                            ef_var(EfToken::Green)
                                        ),
                                        title: "Current queue",
                                        "data-chooser-current": "",
                                    }
                                }
                                // The peer entry's own mark: a peer owns no Named
                                // queue, so it trails with the direct-peer brand
                                // icon the playback-target chooser paints for it.
                                if peer {
                                    TargetBrandIcon { brand: TargetBrand::DirectPeer, size: 16 }
                                }
                                span {
                                    style: format!(
                                        "font-family: var(--font-mono); font-size: 11px; color: {};",
                                        ef_var(EfToken::Grey2)
                                    ),
                                    "data-chooser-count": "",
                                    "{len}"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
