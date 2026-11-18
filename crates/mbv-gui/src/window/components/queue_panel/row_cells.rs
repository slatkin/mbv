//! One queue row's canvas cells (the `rowCells` snippet of
//! `QueuePanel`): the padded queue ordinal, the one title line over its
//! progress track, and the start mark. The row carries no right-column value
//! (gui-named-queues task 8.4, design D9). The hovered row reserves the
//! leading slot its sibling grip covers and the trailing space its sibling
//! actions button paints over, and the drag ghost paints the title and the
//! percent only. The row's colours come through the queue-local `--canvas-*`
//! tokens (U13).

use dioxus_native::prelude::*;

use crate::bridge::frame::queue::{GuiQueue, GuiQueueRow, GuiSemanticState};
use crate::window::icons::Play;
use crate::window::playhead::Playhead;
use crate::window::theme::canvas_var;
use crate::window::truncate::{TextStyle, Truncate};

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
    state: GuiSemanticState,
    pub(super) striped: bool,
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
                striped,
                ..
            } => Some(Self {
                slot: *slot,
                ordinal: *ordinal,
                primary: primary.clone(),
                state: state.clone(),
                striped: *striped,
            }),
            _ => None,
        }
    }
}

/// The colour set one row's cells paint in (the canvas's row states): the
/// filled active row reads the dark ink, the hovered row the canvas's hover
/// text, the watched row the muted text with a strike, and every other row
/// the canvas's row text.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct RowTone {
    title: String,
    weight: f32,
    strike: String,
    ordinal: String,
    track: String,
    fill: String,
}

/// The row's tone under the frame's hover: the playing row keeps its own
/// fill, because a highlight marks focus only.
pub(super) fn tone(row: &ItemRow, hover: Option<u64>) -> RowTone {
    if row.state == GuiSemanticState::NowPlaying {
        return RowTone {
            title: canvas_var("heading"),
            weight: 600.0,
            strike: canvas_var("strike"),
            ordinal: canvas_var("accent"),
            track: canvas_var("track-current"),
            fill: canvas_var("accent"),
        };
    }
    let watched = row.state == GuiSemanticState::Played;
    let hovered = hover == Some(row.slot);
    RowTone {
        title: if hovered {
            canvas_var("heading")
        } else if watched {
            canvas_var("muted")
        } else {
            canvas_var("text")
        },
        weight: 400.0,
        strike: canvas_var("strike"),
        ordinal: if hovered {
            canvas_var("heading")
        } else {
            canvas_var("ordinal")
        },
        track: canvas_var("track"),
        fill: canvas_var("partial"),
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
/// ordinal, the one title line over its progress track, and the start mark.
/// The hovered row reserves the leading slot its sibling grip covers and the
/// trailing space its sibling actions button paints over, and the drag ghost
/// paints the title and the percent only.
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
            class: "flex min-w-0 flex-1 flex-col justify-center",
            style: "gap: 3px;",
            span {
                class: "flex min-w-0 items-center",
                style: "gap: 6px;",
                // The start mark: the filled triangle while the shown queue
                // plays, the outlined one while it is stopped or resting
                // (design D9).
                if let Some(mark) = mark {
                    match mark {
                        StartMark::Playing => rsx! {
                            Play {
                                size: 12,
                                fill: canvas_var("accent"),
                                color: canvas_var("accent"),
                            }
                        },
                        StartMark::Start => rsx! {
                            Play {
                                size: 12,
                                fill: "none".to_owned(),
                                color: canvas_var("accent"),
                            }
                        },
                    }
                }
                {
                    let title_color = if chrome == RowChrome::Ghost {
                        canvas_var("heading")
                    } else {
                        cell_tone.title.clone()
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
                span {
                    class: "block shrink-0",
                    style: "height: 2px; border-radius: 1px; background: {cell_tone.track};",
                    "data-progress": "",
                    span {
                        class: "block",
                        style: "height: 2px; border-radius: 1px; width: {width}%; background: {cell_tone.fill};",
                    }
                }
            }
        }
        if chrome == RowChrome::Ghost
            && let Some(progress) = progress
        {
            span {
                class: "shrink-0 font-mono tabular-nums",
                style: "font-size: 12px; color: var(--canvas-partial);",
                "data-ghost-percent": "",
                "{progress}%"
            }
        }
    }
}
