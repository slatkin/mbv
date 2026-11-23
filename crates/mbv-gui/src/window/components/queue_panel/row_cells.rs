//! One queue row's canvas cells (the `rowCells` snippet of `QueuePanel`): the
//! padded queue ordinal, the one title line, the progress ring after it, and
//! the right column's duration. The row carries no second line, and the ring
//! adds no height: it sits in the row's own line, after the title (design D3 of
//! `gui-delineated-rows`). The drag ghost paints the title, the ring, and the
//! percent only. The row's colours come through the queue column's `--ef-*`
//! tokens (design D1 of `gui-queue-panel-redesign`).

use dioxus_native::prelude::*;

use crate::bridge::frame::queue::{GuiQueue, GuiQueueRow, GuiSemanticState};
use crate::window::components::queue_panel::geometry::{ORDINAL_COL_PX, RIGHT_COL_PX};
use crate::window::playhead::Playhead;
use crate::window::theme::{EfToken, ef_var};
use crate::window::truncate::{TextStyle, Truncate};

/// The progress ring's box in logical pixels (design D3).
const RING_SIZE_PX: u16 = 14;

/// The ring's radius in logical pixels (design D3): the box's half, less the
/// stroke's outward half.
const RING_RADIUS_PX: u16 = 6;

/// The ring's stroke width in logical pixels (design D3).
const RING_STROKE_PX: f32 = 2.5;

/// The ring's circumference in logical pixels: `2π × 6`.
const RING_CIRCUMFERENCE_PX: f32 = 37.7;

/// The ghost's percent in logical pixels of type.
const GHOST_PERCENT_PX: f32 = 12.0;

/// Which cells a row paints: the list row, or the drag ghost (the title, the
/// ring, and the percent only).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RowChrome {
    Row,
    Ghost,
}

/// The item row's facts a row's cells paint (the `ItemRow` extract of
/// `GuiQueueRow::Item`; the structural rows paint no cells).
#[derive(Debug, Clone, PartialEq)]
pub(super) struct ItemRow {
    pub(super) slot: u64,
    ordinal: usize,
    primary: String,
    /// The item's duration text, which the right column paints in every row
    /// state (design D3); `None` while the frame knows no runtime.
    duration: Option<String>,
    pub(super) state: GuiSemanticState,
}

impl ItemRow {
    /// The item row a display row carries, `None` for a structural row.
    pub(super) fn of(row: &GuiQueueRow) -> Option<Self> {
        match row {
            GuiQueueRow::Item {
                slot,
                ordinal,
                primary,
                duration,
                state,
                ..
            } => Some(Self {
                slot: *slot,
                ordinal: *ordinal,
                primary: primary.clone(),
                duration: duration.clone(),
                state: state.clone(),
            }),
            _ => None,
        }
    }
}

/// The colour set one row paints in (the canvas's row states, design D3):
/// the now-playing row paints the green title at weight 600 on the green
/// fill, a marked row the raised fill, the hovered row the hover
/// fill, the watched row the grey title its strike crosses in the darker
/// token, and every other row no fill and the primary text. The ordinal and
/// the right column are shared: the number and the duration are grey. The
/// ring's track and fill are shared too: the track the raised tier and the
/// fill orange except on the playing row.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct RowLook {
    /// The row button's own fill, `None` for an ordinary resting row.
    pub(super) background: Option<&'static str>,
    title: &'static str,
    weight: f32,
    strike: &'static str,
    ordinal: &'static str,
    track: &'static str,
    fill: &'static str,
}

/// The row's one look, so the row's fill and its cells read one precedence:
/// the now-playing row's look is the playback fact and no hover displaces it,
/// then a marked row keeps its raised fill under the pointer,
/// then the hovered row takes the hover fill. A hovered row paints the primary
/// text: the pointer outranks the watched row's grey.
pub(super) fn row_look(row: &ItemRow, queue: &GuiQueue) -> RowLook {
    let hovered = queue.hover == Some(row.slot);
    if matches!(row.state, GuiSemanticState::NowPlaying { .. }) {
        return RowLook {
            background: Some(ef_var(EfToken::BgGreen)),
            title: ef_var(EfToken::Green),
            weight: 600.0,
            strike: ef_var(EfToken::Bg5),
            ordinal: ef_var(EfToken::Grey1),
            track: ef_var(EfToken::Bg2),
            fill: ef_var(EfToken::Green),
        };
    }
    let background = if queue.marks.contains(&row.slot) {
        Some(ef_var(EfToken::Bg3))
    } else if hovered {
        Some(ef_var(EfToken::Bg0))
    } else {
        None
    };
    let title = if row.state == GuiSemanticState::Played && !hovered {
        ef_var(EfToken::Grey1)
    } else {
        ef_var(EfToken::Fg)
    };
    RowLook {
        background,
        title,
        weight: 400.0,
        strike: ef_var(EfToken::Bg5),
        ordinal: ef_var(EfToken::Grey1),
        track: ef_var(EfToken::Bg2),
        fill: ef_var(EfToken::Orange),
    }
}

/// The playhead's played fraction as the row's ring paints it: the whole
/// percent, so the store's per-frame prediction re-renders the one row only
/// when its painted percent moves (design D5 of `gui-queue-panel-redesign`).
fn playhead_percent(playhead: &Playhead) -> i64 {
    mbv_emby_model::saturating_i64_from_f64((playhead.fraction.cloned() * 100.0).round())
}

/// The row's progress percentage, where its state carries one: an active row
/// keeps the frame's percent, and the playing row paints the playhead's
/// predicted percent (design D5 of `gui-queue-panel-redesign`) while its slot
/// is the active slot.
fn row_progress(row: &ItemRow, playhead: &Playhead) -> Option<i64> {
    match row.state {
        GuiSemanticState::NowPlaying { tracks_progress } => {
            if !tracks_progress {
                return None;
            }
            let active_slot = playhead
                .anchor
                .cloned()
                .and_then(|anchor| anchor.active_slot)?;
            (row.slot == active_slot).then(|| playhead_percent(playhead))
        }
        GuiSemanticState::Active { progress } => progress.map(i64::from),
        _ => None,
    }
}

/// The ring's played arc in logical pixels: the percent of the circumference,
/// clamped to the circle (design D3).
fn ring_arc(percent: i64) -> f32 {
    let clamped = percent.clamp(0, 100);
    let percent = f32::from(u16::try_from(clamped).unwrap_or(0));
    RING_CIRCUMFERENCE_PX * percent / 100.0
}

/// The row's progress ring (design D3): the track circle in the row's track
/// token under the played arc in the row's fill token, turned so the arc starts
/// at the top. Each circle names `currentColor` and carries its token in its
/// own computed `color`: Blitz serialises the inline svg to usvg for painting,
/// and usvg cannot parse a `var()` in a `stroke` attribute (it drops the
/// stroke), while the serialiser resolves `currentColor` per element — the
/// collapsed rail's proven ring form.
fn progress_ring(look: &RowLook, percent: i64) -> Element {
    let arc = ring_arc(percent);
    let centre = f32::from(RING_SIZE_PX) / 2.0;
    rsx! {
        span {
            class: "relative shrink-0",
            style: "width: {RING_SIZE_PX}px; height: {RING_SIZE_PX}px;",
            "data-progress": "{percent}",
            svg {
                class: "absolute inset-0 -rotate-90",
                view_box: "0 0 {RING_SIZE_PX} {RING_SIZE_PX}",
                width: "{RING_SIZE_PX}",
                height: "{RING_SIZE_PX}",
                "aria-hidden": "true",
                circle {
                    cx: "{centre}",
                    cy: "{centre}",
                    r: "{RING_RADIUS_PX}",
                    fill: "none",
                    stroke: "currentColor",
                    style: "color: {look.track};",
                    "stroke-width": "{RING_STROKE_PX}",
                }
                circle {
                    cx: "{centre}",
                    cy: "{centre}",
                    r: "{RING_RADIUS_PX}",
                    fill: "none",
                    stroke: "currentColor",
                    style: "color: {look.fill};",
                    "stroke-width": "{RING_STROKE_PX}",
                    "stroke-linecap": "round",
                    "stroke-dasharray": "{arc} {RING_CIRCUMFERENCE_PX}",
                }
            }
        }
    }
}

/// The props of one row's cells: the item row, its tone, and which chrome
/// paints it.
#[derive(Props, Debug, Clone, PartialEq)]
pub(super) struct RowCellsProps {
    pub(super) row: ItemRow,
    pub(super) look: RowLook,
    pub(super) chrome: RowChrome,
}

/// One item row's canvas cells (the `rowCells` snippet): the padded queue
/// ordinal, the one title line with its progress ring, and the right column's
/// duration. The drag ghost paints the title, the ring, and the percent only.
#[component]
pub(super) fn RowCells(props: RowCellsProps) -> Element {
    let playhead = consume_context::<Playhead>();
    let row = &props.row;
    let look = &props.look;
    let chrome = props.chrome;
    let progress = row_progress(row, &playhead);
    let duration = row.duration.clone();
    let ordinal = row.ordinal;
    let ordinal_text = format!("{ordinal:02}");

    rsx! {
        if chrome == RowChrome::Row {
            span {
                class: "shrink-0 text-right font-mono tabular-nums",
                style: format!(
                    "width: {ORDINAL_COL_PX}px; font-size: 11px; color: {};",
                    look.ordinal
                ),
                "data-ordinal": "",
                {ordinal_text}
            }
        }
        span {
            class: "flex min-w-0 flex-1 items-center",
            style: "gap: 6px;",
            {
                let title_color = if chrome == RowChrome::Ghost {
                    ef_var(EfToken::Fg).to_owned()
                } else {
                    look.title.to_owned()
                };
                // The title is a row-flex item, so it grows into the row's
                // free width: its measure must read the room the row has,
                // not the width of the text it currently paints.
                let title_class = if row.state == GuiSemanticState::Played {
                    "min-w-0 flex-1 line-through"
                } else {
                    "min-w-0 flex-1"
                };
                let title_css = format!(
                    "font-size: 13.5px; font-weight: {}; text-decoration-color: {}; color: {};",
                    look.weight, look.strike, title_color
                );
                rsx! {
                    Truncate {
                        text: row.primary.clone(),
                        class: title_class.to_owned(),
                        css: title_css,
                        style: TextStyle::sans(13.5, look.weight),
                    }
                }
            }
            if let Some(percent) = progress {
                // The row's only progress mark: the ring after the title, on
                // the row's own line (design D3).
                {progress_ring(look, percent)}
            }
        }
        if chrome != RowChrome::Ghost
            && let Some(duration) = duration
        {
            // The right column: the item's duration in every row state
            // (design D3).
            span {
                class: "shrink-0 text-right font-mono tabular-nums",
                style: format!(
                    "width: {RIGHT_COL_PX}px; font-size: 12px; color: {};",
                    ef_var(EfToken::Grey1)
                ),
                "data-duration": "",
                {duration}
            }
        }
        if chrome == RowChrome::Ghost
            && let Some(progress) = progress
        {
            span {
                class: "shrink-0 font-mono tabular-nums",
                style: format!(
                    "font-size: {GHOST_PERCENT_PX}px; color: {};",
                    ef_var(EfToken::Orange)
                ),
                "data-ghost-percent": "",
                "{progress}%"
            }
        }
    }
}
