//! One queue row's canvas cells (the `rowCells` snippet of
//! `QueuePanel`): the padded queue ordinal, the one title line over
//! its progress track, and the right-column value. The hovered row reserves
//! the leading and trailing slots its sibling chrome covers, and the drag
//! ghost paints the title and the percent only. The row's colours come
//! through the queue-local `--canvas-*` tokens (U13).

use dioxus_native::prelude::*;

use crate::bridge::frame::library::{GuiMetaKindFrame, GuiMetaSegmentFrame};
use crate::bridge::frame::queue::{GuiMediaKind, GuiQueueRow, GuiSemanticState};
use crate::window::components::typed_text::TypedText;
use crate::window::icons::Play;
use crate::window::playhead::Playhead;
use crate::window::theme::canvas_var;
use crate::window::truncate::{TextStyle, Truncate};

/// Which cells a row paints: the list row, the hovered row (the chrome
/// replaces the ordinal and the right value), or the drag ghost (the title
/// and the percent only).
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
    duration: Option<String>,
    kind: GuiMediaKind,
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
                duration,
                kind,
                state,
                striped,
                ..
            } => Some(Self {
                slot: *slot,
                ordinal: *ordinal,
                primary: primary.clone(),
                duration: duration.clone(),
                kind: *kind,
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

/// The row's right-column text: the media row's duration. A collection row
/// carries none.
fn right_value(row: &ItemRow) -> Option<String> {
    if row.kind == GuiMediaKind::Media {
        return row.duration.clone();
    }
    None
}

/// The props of one row's cells: the item row, its tone, and which chrome
/// paints it.
#[derive(Props, Debug, Clone, PartialEq)]
pub(super) struct RowCellsProps {
    pub(super) row: ItemRow,
    pub(super) cell_tone: RowTone,
    pub(super) chrome: RowChrome,
}

/// One item row's canvas cells (the `rowCells` snippet): the padded queue
/// ordinal, the one title line over its progress track, and the
/// right-column value. The hovered row reserves the leading and trailing
/// slots its sibling chrome covers, and the drag ghost paints the title and
/// the percent only.
#[component]
pub(super) fn RowCells(props: RowCellsProps) -> Element {
    let playhead = consume_context::<Playhead>();
    let row = &props.row;
    let cell_tone = &props.cell_tone;
    let chrome = props.chrome;
    let progress = row_progress(row, &playhead);
    let progress_width = progress.map(|progress| progress.max(3));
    let right = if chrome == RowChrome::Row {
        right_value(row)
    } else {
        None
    };
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
                if row.state == GuiSemanticState::NowPlaying && chrome == RowChrome::Row {
                    Play {
                        size: 12,
                        fill: canvas_var("accent"),
                        color: canvas_var("accent"),
                    }
                }
                {
                    let title_color = if chrome == RowChrome::Ghost {
                        canvas_var("heading")
                    } else {
                        cell_tone.title.clone()
                    };
                    let title_class = if row.state == GuiSemanticState::Played {
                        "min-w-0 line-through"
                    } else {
                        "min-w-0"
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
        if let Some(right) = right {
            // The right column repeats a time down the list, so it paints the
            // time token at the repeat opacity; the current row keeps its own
            // ink at full strength (design D14).
            if row.state == GuiSemanticState::NowPlaying {
                span {
                    class: "shrink-0 font-mono text-xs tabular-nums",
                    style: "color: var(--canvas-ink);",
                    "data-right-value": "",
                    {right}
                }
            } else {
                span {
                    class: "shrink-0 font-mono text-xs tabular-nums",
                    "data-right-value": "",
                    TypedText {
                        segments: vec![GuiMetaSegmentFrame {
                            text: right.clone(),
                            kind: GuiMetaKindFrame::Time,
                        }],
                        repeat: true,
                    }
                }
            }
        } else if chrome == RowChrome::Ghost {
            if let Some(progress) = progress {
                span {
                    class: "shrink-0 font-mono tabular-nums",
                    style: "font-size: 12px; color: var(--canvas-partial);",
                    "data-ghost-percent": "",
                    "{progress}%"
                }
            }
        } else if chrome == RowChrome::Hover {
            // The actions button's slot: it replaces the right-column value.
            span {
                class: "shrink-0",
                style: "width: 30px;",
                "aria-hidden": "true",
            }
        }
    }
}
