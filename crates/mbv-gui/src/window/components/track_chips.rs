//! The track chips row (task 5.4, the port of `TrackChips`; design
//! D4a): the video stream's resolved label as a label-only chip (the D4a
//! ruling gives the video chip no popover, so it is not a button), and the
//! audio and subtitle chips, each a button whose press opens the core's
//! track popover. The chips carry no data while the local player is
//! inactive — the chips' source is the local player's stream — so the row
//! paints nothing then, as the status-indicator cluster does.
//!
//! The popover paints from the frame's `trackPopover` overlay — the same
//! projection the chips read, so its rows and the chips cannot disagree —
//! anchored below the chip it belongs to and right-aligned (design DR8),
//! the chosen choice filled and checked and the core's cursor row
//! highlighted. The dialog paints the Menus artboard's chrome (B.4). Its
//! stacking class is the above-panel tier (`z-30`): the stage's top layer
//! sits outside the media box's clip, so a popover that reaches past the
//! artwork's bottom edge paints over the panel's own content and the queue
//! rows below it (`gui-art-player-overlay` D1). The full-window dialogs
//! mount later at the root and so still paint over it. The
//! window holds no open, cursor, or hover state; Esc and a press elsewhere
//! close the popover through the core's key and pointer paths. The subtitle
//! popover's Off row comes first in the frame's choice order; the window
//! paints the order it is given. There is no subtitle-offset control and no
//! video popover (the D4a rulings).
//!
//! The audio chip reports `audioChip` and the subtitle chip `subtitleChip`;
//! the video chip is label-only (the D4a ruling) and reports no region. The
//! row owns one region set: the two chips are its base contribution, and the
//! open popover's rows report their `trackPopoverChoice` indices under its one
//! overlay key, first inserted when the popover opens, so the rows land after
//! the row's own regions and after every key created before them in the merged
//! report — the hit map's last push wins where a row paints over the transport
//! band beneath it. The row marks in its own render and passes the row closures
//! to the presentation-only popover as props: the report re-runs on the row's
//! props and on the layout epoch, as the former `use_reactive` effect did. A
//! closed popover marks no row, so the overlay retracts and a stale row never
//! keeps swallowing presses on the surfaces beneath it. A press on a
//! chip or a row forwards `pointer` like every other pointer event; the
//! core resolves it against the hit map.

use dioxus_native::prelude::*;

use crate::bridge::frame::overlay::{GuiOverlayFrame, GuiTrackChipKind, GuiTrackPopoverChoice};
use crate::bridge::frame::playback::{GuiTrackChip, GuiTrackChips};
use crate::bridge::frame::theme::GuiRole;
use crate::bridge::layout::{QueuePlaybackTargetId, RegionTarget};
use crate::window::icons::{AudioLines, Check, ClosedCaption, MonitorPlay};
use crate::window::layout::region_set::{MarkHandler, RegionSet, use_region_set_for};
use crate::window::theme::{canvas_var, role_var};
use crate::window::truncate::{TextStyle, Truncate};

#[cfg(test)]
mod tests;

/// The props the `QueuePlaybackPanel` call site passes.
#[derive(Props, Debug, Clone, PartialEq)]
pub struct TrackChipsProps {
    /// The track chips the playback section carries, `None` while the local
    /// player is inactive.
    #[props(default)]
    pub chips: Option<GuiTrackChips>,
    /// The open menu or popover (design D4): the track popover paints
    /// above its chip while it is the open overlay. `None` while none is
    /// open.
    #[props(default)]
    pub overlay: Option<GuiOverlayFrame>,
}

/// The open track popover, or `None` while another overlay (or none) is up.
/// Its `chip` fact picks the chip it anchors to.
type OpenPopover = Option<(GuiTrackChipKind, usize, Vec<GuiTrackPopoverChoice>)>;

/// The open track popover the frame's overlay section carries.
fn open_popover(overlay: Option<&GuiOverlayFrame>) -> OpenPopover {
    match overlay {
        Some(GuiOverlayFrame::TrackPopover {
            chip,
            cursor,
            choices,
        }) => Some((*chip, *cursor, choices.clone())),
        _ => None,
    }
}

/// The open popover's row marks (design D1, D2): the set's one overlay, one
/// mark per painted row in row order — each row's `trackPopoverChoice` index is
/// its region target. A closed popover marks none, so its overlay retracts.
fn mark_popover_rows(region_set: &RegionSet, count: usize) -> Vec<MarkHandler> {
    (0..count)
        .map(|index| {
            region_set.mark_over(
                0,
                RegionTarget::QueuePlayback(QueuePlaybackTargetId::TrackPopoverChoice(index)),
            )
        })
        .collect()
}

/// The label a chip paints: the chosen choice's label; `Off` while the
/// subtitle chip has no chosen choice (the popover's Off row is then the
/// chosen one), the kind's name while the audio chip's choice is
/// unresolved.
fn chip_label(chip: &GuiTrackChip, kind: GuiTrackChipKind) -> String {
    let chosen = chip
        .chosen
        .and_then(|id| chip.choices.iter().find(|choice| choice.id == id));
    match chosen {
        Some(choice) => choice.label.clone(),
        None => match kind {
            GuiTrackChipKind::Subtitles => "Off".to_owned(),
            GuiTrackChipKind::Audio => "Audio".to_owned(),
        },
    }
}

/// The audio chip's pill text: the chosen track's language code (`EN`), or the
/// choice's label while the track names no language. The popover keeps the
/// full labels (`English AAC Stereo`).
fn audio_chip_text(chips: &GuiTrackChips) -> String {
    if chips.audio_lang_code.is_empty() {
        chip_label(&chips.audio, GuiTrackChipKind::Audio)
    } else {
        chips.audio_lang_code.clone()
    }
}

/// The track chips row, centred under the transport as the canvas draws it:
/// video label, audio chip, subtitle chip — one chip-height row in the
/// chips' mono face with tabular figures (the spec's chips type rule).
/// The chips report their regions each paint that moved one, and the layout
/// epoch re-reports on every layout change (D9).
#[component]
pub fn TrackChips(props: TrackChipsProps) -> Element {
    // Each paint that moved a region reports (design D1): the region set's one
    // effect re-runs on the same dependency tuple the row gave `use_reactive`
    // — the chips and the overlay frame — and on the layout epoch through the
    // set's own read. The two chips are the base contribution; the open
    // popover's rows are overlay 0, so they land last in the merged report.
    let region_set = use_region_set_for(1, (&props.chips.clone(), &props.overlay.clone()));

    let popover = open_popover(props.overlay.as_ref());

    // The chips carry no data while the local player is inactive: the row
    // paints nothing then, as the status-indicator cluster does.
    let Some(chips) = props.chips.as_ref() else {
        return rsx! {};
    };
    // An audio-only item paints its codec label alone, as the TUI's cluster
    // does: track switching and subtitles mean nothing for an audio file, so
    // no button paints and no chip region reports.
    if let Some(codec) = chips.audio_only_codec.as_ref() {
        return rsx! {
            div {
                class: "flex items-center justify-center font-mono text-xs tabular-nums",
                span {
                    class: "flex items-center rounded-full border px-2 py-0.5",
                    style: "gap: calc(var(--mbv-cell-col) / 2); border-color: var(--mbv-border); background: var(--mbv-secondary); color: {role_var(GuiRole::TextPrimary)};",
                    "data-chip": "codec",
                    AudioLines { size: 12, color: role_var(GuiRole::IndicatorResolutionFg) }
                    span { "{codec}" }
                }
            }
        };
    }
    // The marks, in the report's order (design D1): the two chips first, as
    // the former effect pushed them.
    let audio_mark = region_set.mark(RegionTarget::QueuePlayback(
        QueuePlaybackTargetId::AudioChip,
    ));
    let subtitle_mark = region_set.mark(RegionTarget::QueuePlayback(
        QueuePlaybackTargetId::SubtitleChip,
    ));
    let audio_open = popover
        .as_ref()
        .is_some_and(|(chip, _, _)| *chip == GuiTrackChipKind::Audio);
    let subtitles_open = popover
        .as_ref()
        .is_some_and(|(chip, _, _)| *chip == GuiTrackChipKind::Subtitles);
    let chip_border = |open: bool| {
        if open {
            canvas_var("open-border")
        } else {
            "var(--mbv-border)".to_owned()
        }
    };
    let chip_fill = |open: bool| {
        if open {
            canvas_var("action")
        } else {
            "var(--mbv-secondary)".to_owned()
        }
    };

    rsx! {
        div {
            class: "flex items-center justify-center font-mono text-xs tabular-nums",
            style: "gap: calc(var(--mbv-cell-col) * 2);",
            if let Some(video) = chips.video.as_ref() {
                // The video chip: the resolved stream label only — the D4a
                // ruling gives it no popover, so it is a label, not a button.
                span {
                    class: "flex items-center rounded-full border px-2 py-0.5",
                    style: "gap: calc(var(--mbv-cell-col) / 2); border-color: var(--mbv-border); background: var(--mbv-secondary); color: {role_var(GuiRole::TextPrimary)};",
                    "data-chip": "video",
                    MonitorPlay { size: 12, color: role_var(GuiRole::IndicatorResolutionFg) }
                    span { "{video}" }
                }
            }
            div {
                class: "relative",
                button {
                    "type": "button",
                    class: "flex items-center rounded-full border px-2 py-0.5",
                    style: "gap: calc(var(--mbv-cell-col) / 2); border-color: {chip_border(audio_open)}; background: {chip_fill(audio_open)}; color: {role_var(GuiRole::TextPrimary)};",
                    "aria-expanded": "{audio_open}",
                    "data-chip": "audio",
                    onmounted: move |event| audio_mark.call(event),
                    AudioLines { size: 12, color: role_var(GuiRole::IndicatorAudioFg) }
                    span { "{audio_chip_text(chips)}" }
                }
                if audio_open {
                    if let Some((kind, cursor, choices)) = popover.clone() {
                        {
                            let row_marks = mark_popover_rows(&region_set, choices.len());
                            rsx! {
                                TrackPopoverList { kind, cursor, choices, row_marks }
                            }
                        }
                    }
                }
            }
            div {
                class: "relative",
                button {
                    "type": "button",
                    class: "flex items-center rounded-full border px-2 py-0.5",
                    style: "gap: calc(var(--mbv-cell-col) / 2); border-color: {chip_border(subtitles_open)}; background: {chip_fill(subtitles_open)}; color: {role_var(GuiRole::TextPrimary)};",
                    "aria-expanded": "{subtitles_open}",
                    "data-chip": "subtitles",
                    onmounted: move |event| subtitle_mark.call(event),
                    ClosedCaption { size: 12, color: role_var(GuiRole::TextMuted) }
                    span { "{chip_label(&chips.subtitles, GuiTrackChipKind::Subtitles)}" }
                }
                if subtitles_open {
                    if let Some((kind, cursor, choices)) = popover.clone() {
                        {
                            let row_marks = mark_popover_rows(&region_set, choices.len());
                            rsx! {
                                TrackPopoverList { kind, cursor, choices, row_marks }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The track popover list's props: the chip kind the dialog names, the
/// core's cursor row, the live choice list, and the marks of the rows the row
/// reports.
#[derive(Props, Clone, PartialEq)]
struct TrackPopoverProps {
    /// Which chip's popover this is: the dialog's aria label names it.
    kind: GuiTrackChipKind,
    /// The core's cursor row.
    cursor: usize,
    /// The chip's choices, in the frame's order (the subtitle popover lists
    /// Off first).
    choices: Vec<GuiTrackPopoverChoice>,
    /// The painted rows' marks, in row order — the closures the row that owns
    /// the popover produced with its region set's `mark_over`. A row takes its
    /// own entry when the vector reaches it.
    #[props(default)]
    row_marks: Vec<MarkHandler>,
}

impl std::fmt::Debug for TrackPopoverProps {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TrackPopoverProps")
            .field("kind", &matches!(self.kind, GuiTrackChipKind::Audio))
            .field("cursor", &self.cursor)
            .field("choices", &self.choices)
            .finish_non_exhaustive()
    }
}

/// The track popover (the Menus artboard): the core's live choice list in
/// the canvas dialog below the chip, right-aligned, the chosen choice
/// filled and checked at the row's trailing edge, and the cursor row's text
/// highlighted. Each row reports its `trackPopoverChoice` index on mount,
/// through the mark the row that owns the popover handed down.
#[component]
fn TrackPopoverList(props: TrackPopoverProps) -> Element {
    let dialog_label = match props.kind {
        GuiTrackChipKind::Audio => "Audio",
        GuiTrackChipKind::Subtitles => "Subtitles",
    };
    rsx! {
        div {
            // The above-panel stacking tier (`gui-art-player-overlay` D1):
            // the popover hangs past the artwork and paints over the panel's
            // content and the queue rows, never under them.
            class: "absolute top-full right-0 z-30 mt-1 flex flex-col",
            role: "dialog",
            "aria-label": "{dialog_label}",
            style: "width: 260px; padding: 6px; gap: 2px; background: {canvas_var(\"popover\")}; border: 1px solid {canvas_var(\"popover-border\")}; border-radius: 8px; box-shadow: {canvas_var(\"popover-shadow\")};",
            for (index, choice) in props.choices.iter().enumerate() {
                {
                    let row_fill = if choice.chosen {
                        canvas_var("selected")
                    } else {
                        "none".to_owned()
                    };
                    let row_color = if choice.chosen {
                        canvas_var("heading")
                    } else if index == props.cursor {
                        role_var(GuiRole::TextFocusAccent)
                    } else {
                        canvas_var("text")
                    };
                    // A forced row keeps its tag right after the label, so its
                    // label is sized by its text; any other row's label fills the
                    // room.
                    let label_class = if choice.forced { "min-w-0" } else { "min-w-0 flex-1" };
                    rsx! {
                        button {
                            "type": "button",
                            class: "flex items-center justify-between text-left",
                            style: "height: 36px; padding: 0 10px; border-radius: 4px; font-size: 13px; background: {row_fill}; color: {row_color};",
                            "data-popover-row": "{index}",
                            onmounted: {
                                let row_mark = props.row_marks.get(index).copied();
                                move |event| {
                                    // The row that owns the popover marks this
                                    // row: the closure records the handle its
                                    // overlay region reads.
                                    if let Some(mark) = row_mark {
                                        mark.call(event);
                                    }
                                }
                            },
                            span {
                                class: "flex min-w-0 flex-1 items-center",
                                // The 13 px the row's style sets is the label's
                                // inherited size and weight: the box carries no
                                // style of its own.
                                Truncate {
                                    text: choice.label.clone(),
                                    class: label_class.to_owned(),
                                    style: TextStyle::sans(13.0, 400.0),
                                }
                                if choice.forced {
                                    span {
                                        class: "shrink-0 font-mono",
                                        style: "margin-left: 6px; font-size: 11px; color: {canvas_var(\"muted\")};",
                                        "forced"
                                    }
                                }
                            }
                            if choice.chosen {
                                Check { size: 14, stroke_width: 2.5, color: canvas_var("accent") }
                            }
                        }
                    }
                }
            }
        }
    }
}
