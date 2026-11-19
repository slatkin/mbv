//! One queue row's canvas cells (the `rowCells` snippet of
//! `QueuePanel`): the padded queue ordinal, the one title line, the start mark,
//! and the thin progress bar painted inside the row along its bottom. The row
//! carries no right-column value (gui-named-queues task 8.4, design D9) and the
//! bar adds no height: it is positioned against the row's own box, below the
//! title line (gui-queue-panel-redesign design D9). The hovered row reserves
//! the leading slot its sibling grip covers and the trailing space its sibling
//! actions button paints over, and the drag ghost paints the title and the
//! percent only. The row's colours come through the queue column's `--ef-*`
//! tokens (design D1).

use dioxus_native::prelude::*;

use crate::bridge::frame::queue::{GuiQueue, GuiQueueRow, GuiSemanticState};
use crate::window::icons::Play;
use crate::window::playhead::Playhead;
use crate::window::theme::{EfToken, ef_var};
use crate::window::truncate::{TextStyle, Truncate};

/// The left inset of the row's progress bar in logical pixels: past the
/// ordinal's column, under the title line (design D9).
const PROGRESS_LEFT_PX: u16 = 54;

/// The right inset of the row's progress bar in logical pixels (design D9).
const PROGRESS_RIGHT_PX: u16 = 12;

/// The row's progress bar height in logical pixels (design D9).
const PROGRESS_HEIGHT_PX: u16 = 2;

/// The progress bar's distance from the row's bottom edge in logical pixels
/// (design D9).
const PROGRESS_BOTTOM_PX: u16 = 4;

/// Which cells a row paints: the list row, the hovered row (the chrome
/// replaces the ordinal), or the drag ghost (the title and the percent only).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RowChrome {
    Row,
    Hover,
    Ghost,
}

/// The item row's facts a row's cells paint (the `ItemRow` extract of
/// `GuiQueueRow::Item`; the structural rows paint no cells).
#[derive(Debug, Clone, PartialEq)]
pub(super) struct ItemRow {
    pub(super) slot: u64,
    ordinal: usize,
    primary: String,
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
                state,
                ..
            } => Some(Self {
                slot: *slot,
                ordinal: *ordinal,
                primary: primary.clone(),
                state: state.clone(),
            }),
            _ => None,
        }
    }
}

/// The colour set one row's cells paint in (the canvas's row states, design
/// D9): the now-playing row paints the green title at weight 600, the hovered
/// row the primary text, the watched row the grey title its strike crosses in
/// the darker token, and every other row the primary text. The ordinal, the
/// track, and the partial fill are shared: the number is grey, the track the
/// raised tier, and the fill orange at the watched and active rows alike.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct RowTone {
    title: &'static str,
    weight: f32,
    strike: &'static str,
    ordinal: &'static str,
    track: &'static str,
    fill: &'static str,
}

/// The row's tone under the frame's hover: the now-playing row keeps its own
/// title and fill, because its look is the playback fact, not a pointer's.
pub(super) fn tone(row: &ItemRow, hover: Option<u64>) -> RowTone {
    if row.state == GuiSemanticState::NowPlaying {
        return RowTone {
            title: ef_var(EfToken::Green),
            weight: 600.0,
            strike: ef_var(EfToken::Bg5),
            ordinal: ef_var(EfToken::Grey1),
            track: ef_var(EfToken::Bg2),
            fill: ef_var(EfToken::Green),
        };
    }
    let watched = row.state == GuiSemanticState::Played;
    let hovered = hover == Some(row.slot);
    // A hovered row paints the primary text: the pointer outranks the watched
    // row's grey.
    let title = if watched && !hovered {
        ef_var(EfToken::Grey1)
    } else {
        ef_var(EfToken::Fg)
    };
    RowTone {
        title,
        weight: 400.0,
        strike: ef_var(EfToken::Bg5),
        ordinal: ef_var(EfToken::Grey1),
        track: ef_var(EfToken::Bg2),
        fill: ef_var(EfToken::Orange),
    }
}

/// The playhead's played fraction as the row track paints it: the whole
/// percent, so the store's per-frame prediction re-renders the one row only
/// when its painted percent moves (design D5).
fn playhead_percent(playhead: &Playhead) -> i64 {
    mbv_emby_model::saturating_i64_from_f64((playhead.fraction.cloned() * 100.0).round())
}

/// The row's progress percentage, where its state carries one: an active row
/// keeps the frame's percent, and the playing row paints the playhead's
/// predicted percent (design D5) while its slot is the active slot.
fn row_progress(row: &ItemRow, playhead: &Playhead) -> Option<i64> {
    match row.state {
        GuiSemanticState::NowPlaying => {
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

/// The start mark a row paints (design D9): the shown queue's start slot is
/// one row in two states, filled while that queue plays and outlined while it
/// is stopped or resting, so the two marks stay distinct.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum StartMark {
    /// The start slot of the queue that plays: the filled mark.
    Playing,
    /// The start slot of a stopped or resting queue: the outlined mark.
    Start,
}

/// The start mark the row of `slot` paints, if any: only the shown queue's
/// start slot carries one. A queue that reports a start slot marks it filled
/// while it plays and outlined while it is stopped or resting. A queue that
/// reports no start slot — a peer without `named-queues` — marks its observed
/// active slot as the playing row while it plays and carries no outlined mark
/// (design D9).
pub(super) fn start_mark(queue: &GuiQueue, slot: u64) -> Option<StartMark> {
    if queue.start != Some(slot) {
        return None;
    }
    if queue.playing {
        return Some(StartMark::Playing);
    }
    queue.start_slot_reported.then_some(StartMark::Start)
}

/// The props of one row's cells: the item row, its tone, which chrome paints
/// it, and its start mark while it carries one.
#[derive(Props, Debug, Clone, PartialEq)]
pub(super) struct RowCellsProps {
    pub(super) row: ItemRow,
    pub(super) cell_tone: RowTone,
    pub(super) chrome: RowChrome,
    pub(super) mark: Option<StartMark>,
}

/// One item row's canvas cells (the `rowCells` snippet): the padded queue
/// ordinal, the one title line, the start mark, and the progress bar painted
/// inside the row's bottom edge. The hovered row reserves the leading slot its
/// sibling grip covers and the trailing space its sibling actions button paints
/// over, and the drag ghost paints the title and the percent only.
#[component]
pub(super) fn RowCells(props: RowCellsProps) -> Element {
    let playhead = consume_context::<Playhead>();
    let row = &props.row;
    let cell_tone = &props.cell_tone;
    let chrome = props.chrome;
    let progress = row_progress(row, &playhead);
    let progress_width = progress.map(|progress| progress.max(3));
    let mark = props.mark.filter(|_| chrome == RowChrome::Row);
    let ordinal = row.ordinal;
    let ordinal_text = format!("{ordinal:02}");

    rsx! {
        if chrome == RowChrome::Row {
            span {
                class: "shrink-0 font-mono tabular-nums",
                style: "width: 18px; font-size: 11px; color: {cell_tone.ordinal};",
                "data-ordinal": "",
                {ordinal_text}
            }
        } else if chrome == RowChrome::Hover {
            // The grip's slot: the six-dot grip replaces the ordinal.
            span {
                class: "shrink-0",
                style: "width: 14px;",
                "aria-hidden": "true",
            }
        }
        span {
            class: "flex min-w-0 flex-1 items-center",
            style: "gap: 6px;",
            // The start mark: the filled triangle while the shown queue
            // plays, the outlined one while it is stopped or resting
            // (design D9).
            if let Some(mark) = mark {
                match mark {
                    StartMark::Playing => rsx! {
                        Play {
                            size: 12,
                            fill: ef_var(EfToken::Green),
                            color: ef_var(EfToken::Green).to_owned(),
                        }
                    },
                    StartMark::Start => rsx! {
                        Play {
                            size: 12,
                            fill: "none".to_owned(),
                            color: ef_var(EfToken::Green).to_owned(),
                        }
                    },
                }
            }
            {
                let title_color = if chrome == RowChrome::Ghost {
                    ef_var(EfToken::Fg).to_owned()
                } else {
                    cell_tone.title.to_owned()
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
                    cell_tone.weight, cell_tone.strike, title_color
                );
                rsx! {
                    Truncate {
                        text: row.primary.clone(),
                        class: title_class.to_owned(),
                        css: title_css,
                        style: TextStyle::sans(13.5, cell_tone.weight),
                    }
                }
            }
        }
        if let Some(width) = progress_width
            && chrome != RowChrome::Ghost
        {
            // The row's progress bar: absolute against the row's own box, so it
            // paints along the row's bottom without adding a line to the row
            // (design D9).
            span {
                class: "absolute block",
                style: "left: {PROGRESS_LEFT_PX}px; right: {PROGRESS_RIGHT_PX}px; bottom: {PROGRESS_BOTTOM_PX}px; height: {PROGRESS_HEIGHT_PX}px; border-radius: 1px; background: {cell_tone.track};",
                "data-progress": "",
                span {
                    class: "block",
                    style: "height: {PROGRESS_HEIGHT_PX}px; border-radius: 1px; width: {width}%; background: {cell_tone.fill};",
                }
            }
        }
        if chrome == RowChrome::Ghost
            && let Some(progress) = progress
        {
            span {
                class: "shrink-0 font-mono tabular-nums",
                style: format!("font-size: 12px; color: {};", ef_var(EfToken::Orange)),
                "data-ghost-percent": "",
                "{progress}%"
            }
        }
    }
}
