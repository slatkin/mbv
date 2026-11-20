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
//! carries — under a second report key first inserted when the chooser opens,
//! so it lands after the queue rows in the merged report: the hit map's last
//! push wins where a row paints over a queue row beneath it. The window holds
//! no open, cursor, or hover state; Esc and a press elsewhere close the popover
//! through the core's key and pointer paths.

use dioxus_native::prelude::*;

use crate::bridge::frame::overlay::GuiOverlayFrame;
use crate::bridge::frame::playback::{GuiTargetChoice, GuiTargetKey};
use crate::bridge::frame::theme::GuiRole;
use crate::bridge::layout::{LayoutRegion, QueuePlaybackTargetId, RegionTarget};
use crate::window::brand_icons::{TargetBrand, TargetBrandIcon};
use crate::window::components::row_handle::record_row_handle;
use crate::window::icons::{Check, ChevronDown, Monitor};
use crate::window::layout::{
    LayoutContribution, LayoutEpoch, LayoutKey, LayoutReporter, element_region,
};
use crate::window::sections::Sections;
use crate::window::theme::role_var;
use crate::window::truncate::{TextStyle, Truncate};

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
/// Every paint that moved a region reports — the button's region under the
/// control's key, and the open chooser's rows under the chooser's.
#[component]
pub fn QueueTargetControl() -> Element {
    let sections = consume_context::<Sections>();
    let epoch = consume_context::<LayoutEpoch>();
    let reporter = consume_context::<LayoutReporter>();
    let key = use_hook(LayoutKey::next);
    let chooser_key = use_hook(LayoutKey::next);

    let mut target_mounted: Signal<Option<Event<MountedData>>> = use_signal(|| None);
    // The open chooser's painted rows, by their choice index.
    let chooser_rows: Signal<Vec<Option<Event<MountedData>>>> = use_signal(Vec::new);

    // No region outlives the control: the contributions retract on unmount.
    let mut drop_reporter = reporter.clone();

    // The playback target chooser: the overlay section's `targetChooser`
    // variant, `None` while another overlay (or none) is up.
    let chooser = match sections.overlay.cloned() {
        Some(GuiOverlayFrame::TargetChooser { cursor, choices }) => Some((cursor, choices)),
        _ => None,
    };

    let watch_playback = sections.playback;
    let watch_overlay = sections.overlay;
    let watch_epoch = epoch.0;
    use_effect(move || {
        let _playback = watch_playback.cloned();
        let overlay_open = watch_overlay.cloned();
        let _epoch = watch_epoch.cloned();
        let chooser_open = matches!(
            overlay_open.as_ref(),
            Some(GuiOverlayFrame::TargetChooser { .. })
        );
        let rows = chooser_rows.cloned();
        let target = target_mounted.cloned();
        let mut effect_reporter = reporter.clone();
        spawn(async move {
            let mut regions: Vec<LayoutRegion> = Vec::new();
            if let Some(mounted) = target {
                regions.extend(
                    element_region(
                        &mounted,
                        RegionTarget::QueuePlayback(QueuePlaybackTargetId::PlaybackTarget),
                    )
                    .await,
                );
            }
            effect_reporter.report_layout(
                key,
                LayoutContribution {
                    lists: Vec::new(),
                    regions,
                },
            );
            if chooser_open {
                let mut chooser_regions: Vec<LayoutRegion> = Vec::new();
                for (index, mounted) in rows.iter().enumerate() {
                    if let Some(mounted) = mounted {
                        chooser_regions.extend(
                            element_region(
                                mounted,
                                RegionTarget::QueuePlayback(QueuePlaybackTargetId::TargetChoice(
                                    index,
                                )),
                            )
                            .await,
                        );
                    }
                }
                effect_reporter.report_layout(
                    chooser_key,
                    LayoutContribution {
                        lists: Vec::new(),
                        regions: chooser_regions,
                    },
                );
            } else {
                effect_reporter.retract_layout(chooser_key);
            }
        });
    });

    use_drop(move || {
        drop_reporter.retract_layout(key);
        drop_reporter.retract_layout(chooser_key);
    });

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
                style: "height: 32px; padding: 0 8px; gap: 6px; border-radius: 6px; font-size: 13px; color: {target_color};",
                "data-target": "playbackTarget",
                "aria-label": "Playback target",
                title: "Playback target",
                onmounted: move |event| target_mounted.set(Some(event)),
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
                        "{playback.host}"
                    }
                }
                ChevronDown { size: 12, color: "currentColor" }
            }
            if let Some((cursor, choices)) = chooser {
                TargetChooserPopover {
                    cursor,
                    choices,
                    chooser_rows,
                }
            }
        }
    }
}

/// The open playback-target chooser's props: the core's cursor and live
/// choice list, and the row handles the chooser's regions read.
#[derive(Props, Clone, PartialEq)]
struct TargetChooserPopoverProps {
    /// The core's cursor row.
    cursor: usize,
    /// The choices the frame's `targetChoices` list carries.
    choices: Vec<GuiTargetChoice>,
    /// The painted rows' mounted events, by their choice index.
    chooser_rows: Signal<Vec<Option<Event<MountedData>>>>,
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
/// button's. Each row's `onmounted` hands its event to the control's
/// chooser-regions signal, under the row's `data-choice` index.
#[component]
fn TargetChooserPopover(props: TargetChooserPopoverProps) -> Element {
    rsx! {
        div {
            class: "absolute z-20 flex min-w-56 flex-col rounded-md border py-1 shadow-md",
            // The header's type is not the popover's: it keeps the 16 px body
            // face its rows painted in, which is the face the rows' `Truncate`
            // measure names.
            style: "top: 36px; left: 0; font-family: var(--font-sans); font-size: 16px; background: var(--mbv-popover); border-color: var(--mbv-border);",
            "data-target-chooser": "",
            for (index, choice) in props.choices.iter().enumerate() {
                {
                    let row_color = if index == props.cursor {
                        role_var(GuiRole::TextFocusAccent)
                    } else {
                        role_var(GuiRole::TextPrimary)
                    };
                    rsx! {
                        button {
                            "type": "button",
                            class: "flex w-full items-center text-left",
                            style: "padding: calc(var(--mbv-cell-row) / 4) calc(var(--mbv-cell-col) * 1); gap: var(--mbv-cell-col); color: {row_color};",
                            "data-choice": "{index}",
                            onmounted: {
                                let chooser_rows = props.chooser_rows;
                                move |event| record_row_handle(chooser_rows, index, event)
                            },
                            span {
                                class: "flex w-4 shrink-0 justify-center",
                                if choice.is_active {
                                    Check { size: 14, color: role_var(GuiRole::Accent) }
                                }
                            }
                            {target_icon(choice, 14)}
                            Truncate {
                                text: choice.label.clone(),
                                class: "min-w-0 flex-1".to_owned(),
                                // The chooser button names no font size: the
                                // label inherits the document's 16 px body face.
                                style: TextStyle::sans(16.0, 400.0),
                            }
                        }
                    }
                }
            }
        }
    }
}
