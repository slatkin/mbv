//! The playback-target control of the Queue header (user request 2026-11-20,
//! moved from the Queue footer): the button that names the Owner's attached
//! output and opens the core's target chooser, and the chooser's popover. It
//! leads the header, to the left of the Queue switcher.
//!
//! The button's glyph is the glyph the chooser paints for the same choice —
//! the local monitor, the Cast mark, the Emby mark, or the direct-peer mark —
//! read from the attached choice the frame carries, so the control never paints
//! a fixed Cast icon. Its label is the playback host, and its colour the local
//! or remote role.
//!
//! The popover hangs under the button (`top`), as the Queue chooser's dropdown
//! hangs under its switcher. Each row reports its `targetChoice` index — the
//! core resolves a row press against the same live choice list the frame
//! carries — under the control's one overlay key, first inserted when the
//! chooser opens, so it lands after the queue rows in the merged report: the
//! hit map's last push wins where a row paints over a queue row beneath it. The
//! window holds no open, cursor, or hover state; Esc and a press elsewhere close
//! the popover through the core's key and pointer paths.
//!
//! The control owns its one region set: the button is its base contribution and
//! the open chooser's rows are its one overlay. The control marks both in its
//! own render — the row closures reach the presentation-only popover as props —
//! so a painted chooser's rows always record and a closed chooser's overlay
//! retracts, whatever order the mount effects run in.

use dioxus_native::prelude::*;

use crate::bridge::frame::overlay::GuiOverlayFrame;
use crate::bridge::frame::playback::{GuiTargetChoice, GuiTargetKey};
use crate::bridge::frame::theme::GuiRole;
use crate::bridge::layout::{QueuePlaybackTargetId, RegionTarget};
use crate::window::brand_icons::{TargetBrand, TargetBrandIcon};
use crate::window::components::queue_panel::geometry::{MENU_DROP_PX, ROW_GAP_PX, ROW_HEIGHT_PX};
use crate::window::icons::{Check, ChevronDown, Monitor};
use crate::window::layout::region_set::{MarkHandler, use_region_set};
use crate::window::sections::Sections;
use crate::window::theme::{EfToken, ef_var, role_var};
use crate::window::truncate::{TextStyle, Truncate};

/// The popover card's padding: the Queue chooser's own inset.
const CARD_PADDING_PX: u16 = 4;

/// The glyph a target choice paints, in the chooser's rows and on the control's
/// button alike: one function, so the two cannot drift apart.
fn target_icon(choice: &GuiTargetChoice, size: u16) -> Element {
    match &choice.key {
        GuiTargetKey::Local => rsx! {
            Monitor { size, color: role_var(GuiRole::PlaybackTargetLocalFg) }
        },
        GuiTargetKey::Cast(_) => rsx! {
            TargetBrandIcon { brand: TargetBrand::Cast, size }
        },
        GuiTargetKey::Emby(_) if choice.is_direct_peer => rsx! {
            TargetBrandIcon { brand: TargetBrand::DirectPeer, size }
        },
        GuiTargetKey::Emby(_) => rsx! {
            TargetBrandIcon { brand: TargetBrand::Emby, size }
        },
    }
}

/// The playback-target control: the button, and the open chooser's popover.
/// Every paint that moved a region reports — the button's region as the
/// control's base contribution, and the open chooser's rows as its one overlay.
#[component]
pub fn QueueTargetControl() -> Element {
    let sections = consume_context::<Sections>();

    // Each paint that moved a region reports (design D1): the region set's one
    // effect watches the `playback` and `overlay` sections and the layout epoch
    // through the set's own read. The button is the base contribution; the open
    // chooser's rows are overlay 0, so they merge after the areas beneath the
    // popover and the overlay retracts while the popover is closed.
    let watch_playback = sections.playback;
    let watch_overlay = sections.overlay;
    let region_set = use_region_set(1, move || {
        let _playback = watch_playback.cloned();
        let _overlay = watch_overlay.cloned();
    });

    // The playback target chooser: the overlay section's `targetChooser`
    // variant, `None` while another overlay (or none) is up.
    let chooser = match sections.overlay.cloned() {
        Some(GuiOverlayFrame::TargetChooser { cursor, choices }) => Some((cursor, choices)),
        _ => None,
    };

    // The pill's open state: the Queue switcher's own pair while its chooser
    // hangs open, so the two controls look alike at rest, hovered, and open.
    let open_look = if chooser.is_some() {
        format!(
            " border-color: {}; background: {};",
            ef_var(EfToken::Grey0),
            ef_var(EfToken::Bg3)
        )
    } else {
        String::new()
    };

    let target_mark = region_set.mark(RegionTarget::QueuePlayback(
        QueuePlaybackTargetId::PlaybackTarget,
    ));

    let playback = sections.playback.cloned();
    // The control's colour: the flag picks the local or the remote role.
    let target_color = if playback.host_is_remote {
        role_var(GuiRole::PlaybackTargetRemoteFg)
    } else {
        role_var(GuiRole::PlaybackTargetLocalFg)
    };
    // The attached output's choice: its glyph is the chooser's glyph for it.
    // The local output is always the list's first choice, so it stands in while
    // the frame names no attached one.
    let attached = playback
        .target_choices
        .iter()
        .find(|choice| choice.is_active)
        .or_else(|| playback.target_choices.first())
        .cloned();

    rsx! {
        div {
            style: "position: relative; min-width: 0;",
            class: "flex items-center",
            "data-queue-target-anchor": "",
            button {
                "type": "button",
                class: "flex min-w-0 items-center",
                style: "height: 32px; padding: 0 8px 0 10px; gap: 8px; border-radius: 6px; border-width: 1px; border-style: solid; font-size: 13px; color: {target_color};{open_look}",
                "data-target": "playbackTarget",
                "aria-label": "Playback target",
                title: "Playback target",
                onmounted: move |event| target_mark.call(event),
                if let Some(attached) = attached.as_ref() {
                    span {
                        class: "flex shrink-0 items-center",
                        "data-target-icon": "",
                        {target_icon(attached, 16)}
                    }
                }
                if !playback.host.is_empty() {
                    // The name paints as plain clipped text, never through
                    // `Truncate`: this box is sized by the text it paints, so
                    // a measured cut would shrink the box and ratchet the name
                    // down to `…` on the next session switch (b3c8d874b's rule,
                    // kept on the control's moves).
                    span {
                        class: "min-w-0 overflow-hidden whitespace-nowrap",
                        style: "font-weight: 600;",
                        "{playback.host}"
                    }
                }
                ChevronDown { size: 12, color: "currentColor" }
            }
            if let Some((cursor, choices)) = chooser {
                {
                    // The chooser's rows mark the set's one overlay, one per
                    // painted row in row order (design D1, D2): each row's
                    // `targetChoice` index is its region target, and a closed
                    // chooser marks none, so the overlay retracts.
                    let row_marks = (0..choices.len())
                        .map(|index| {
                            region_set.mark_over(
                                0,
                                RegionTarget::QueuePlayback(QueuePlaybackTargetId::TargetChoice(
                                    index,
                                )),
                            )
                        })
                        .collect::<Vec<_>>();
                    rsx! {
                        TargetChooserPopover {
                            cursor,
                            choices,
                            row_marks,
                        }
                    }
                }
            }
        }
    }
}

/// The open playback-target chooser's props: the core's cursor, the live choice
/// list, and the marks of the rows the control reports.
#[derive(Props, Clone, PartialEq)]
struct TargetChooserPopoverProps {
    /// The core's cursor row.
    cursor: usize,
    /// The choices the frame's `targetChoices` list carries.
    choices: Vec<GuiTargetChoice>,
    /// The painted rows' marks, in row order — the closures the control that
    /// owns the chooser produced with its region set's `mark_over`. A row takes
    /// its own entry when the vector reaches it.
    #[props(default)]
    row_marks: Vec<MarkHandler>,
}

impl std::fmt::Debug for TargetChooserPopoverProps {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TargetChooserPopoverProps")
            .field("cursor", &self.cursor)
            .field("choices", &self.choices)
            .finish_non_exhaustive()
    }
}

/// The chooser's popover, hung under the button with its leading edge on the
/// button's. Each row's `onmounted` hands its event to the control's mark for
/// that row, under the row's `data-choice` index.
#[component]
fn TargetChooserPopover(props: TargetChooserPopoverProps) -> Element {
    rsx! {
        div {
            class: "absolute z-20 flex flex-col",
            role: "menu",
            // The card is the Queue chooser's own (it is authoritative): the
            // same fill, border, radius, padding, drop, and 13 px type.
            style: format!(
                "top: calc(100% + {MENU_DROP_PX}px); left: 0; min-width: 260px; max-width: 28rem; padding: {CARD_PADDING_PX}px; gap: {ROW_GAP_PX}px; background: {}; border: 1px solid {}; border-radius: 8px; --hover-veil: transparent;",
                ef_var(EfToken::Bg2),
                ef_var(EfToken::Bg4)
            ),
            "data-target-chooser": "",
            for (index, choice) in props.choices.iter().enumerate() {
                {
                    let row_background = if index == props.cursor {
                        format!(" background: {};", ef_var(EfToken::Bg3))
                    } else {
                        String::new()
                    };
                    let row_style = format!(
                        "height: {ROW_HEIGHT_PX}px; padding: 0 8px; gap: 8px; border-radius: 5px; font-size: 13px; font-weight: 500; color: {};{row_background}",
                        ef_var(EfToken::Fg)
                    );
                    rsx! {
                        button {
                            "type": "button",
                            class: "flex w-full items-center text-left",
                            role: "menuitem",
                            style: "{row_style}",
                            "data-choice": "{index}",
                            onmounted: {
                                let row_mark = props.row_marks.get(index).copied();
                                move |event| {
                                    // The control that owns the chooser marks
                                    // this row: the closure records the handle
                                    // its overlay region reads.
                                    if let Some(mark) = row_mark {
                                        mark.call(event);
                                    }
                                }
                            },
                            span {
                                class: "flex w-4 shrink-0 justify-center",
                                if choice.is_active {
                                    Check { size: 14, color: ef_var(EfToken::Green).to_owned() }
                                }
                            }
                            {target_icon(choice, 14)}
                            Truncate {
                                text: choice.label.clone(),
                                class: "min-w-0 flex-1".to_owned(),
                                style: TextStyle::sans(13.0, 500.0),
                            }
                        }
                    }
                }
            }
        }
    }
}
