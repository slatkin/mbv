//! The Queue section of a frame: the rows inside the viewport, and the cursor,
//! marks, and active slot (design D4). A slot id travels as its raw number, as
//! `layout.rs` does. The summary section carries the queue header's remaining
//! count, finish clock, and right-column mode.

use mbv::QueueSummaryProjection;
use mbv_queue::QueueSlotId;
use mbv_surfaces::queue::QueueSurface;
use mbv_ui_model::media_list::{
    ActiveProgress, MediaKind, MediaListRow, MediaListTrailing, MediaSemanticState,
};
use serde::Serialize;

/// The Queue section of one frame.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiQueue {
    /// The display rows that the viewport shows, in paint order.
    pub rows: Vec<GuiQueueRow>,
    /// The slot under the cursor (`QueueSlotId::raw`), if any.
    pub cursor: Option<u64>,
    /// The slots marked for a bulk action (`QueueSlotId::raw`).
    pub marks: Vec<u64>,
    /// The slot that carries the now-playing marker: the playing slot, or the
    /// pending selection while the owner confirms it. It can lie outside the
    /// viewport, so it is read from every row, not from `rows`.
    pub active: Option<u64>,
}

impl GuiQueue {
    /// Projects the Queue Surface state: the rows of a viewport `viewport_rows`
    /// tall, with the cursor, marks, active slot, zebra parity, and each row's
    /// finish clock from `summary`.
    #[must_use]
    pub fn from_surface(
        queue: &QueueSurface,
        viewport_rows: usize,
        summary: &QueueSummaryProjection,
    ) -> Self {
        let rows = queue.rows();
        let viewport = queue.list().resolve_viewport(viewport_rows.max(1));
        // The zebra alternation the TUI's wide presentation paints
        // (`mbv-render` `media_list/wide.rs`): a grouped list alternates
        // within each group from its first member, an ungrouped list
        // alternates throughout, seeded by the viewport offset and flipped
        // per item row — headings and spacers keep the box's own fill and
        // do not flip. The parity rides the row so the stripe stays stable
        // while the viewport moves.
        let grouped = rows
            .iter()
            .any(|row| matches!(row, MediaListRow::Heading { .. }));
        let mut striped = viewport.offset % 2 == 1;
        let rows_dto = rows
            .iter()
            .enumerate()
            .skip(viewport.offset)
            .take(viewport.height)
            .map(|(source_row, row)| {
                let striped = striped_state(row, source_row, grouped, rows, &mut striped);
                let mut row = GuiQueueRow::from_painted(row, striped);
                if let GuiQueueRow::Item { slot, ends_at, .. } = &mut row {
                    *ends_at = summary
                        .row_finish_clocks
                        .iter()
                        .find(|(clock_slot, _)| clock_slot.raw() == *slot)
                        .map(|(_, clock)| clock.clone());
                }
                row
            })
            .collect();
        Self {
            rows: rows_dto,
            cursor: queue.selected_target().map(|id| id.raw()),
            marks: queue.multi_selection().iter().map(|id| id.raw()).collect(),
            active: rows.iter().find_map(active_slot),
        }
    }
}

/// One row's zebra state, mirroring the TUI's alternation walk
/// (`media_list/wide.rs`): headings and spacers never stripe and never
/// flip; an ungrouped item takes the running state and flips it; a grouped
/// item alternates within its group from the group's first member.
fn striped_state<Target>(
    row: &MediaListRow<Target>,
    source_row: usize,
    grouped: bool,
    rows: &[MediaListRow<Target>],
    striped: &mut bool,
) -> bool {
    match row {
        MediaListRow::Heading { .. } | MediaListRow::Spacer => false,
        MediaListRow::Item { .. } if grouped => grouped_member_striped(rows, source_row),
        MediaListRow::Item { .. } => {
            let state = *striped;
            *striped = !*striped;
            state
        }
    }
}

/// Whether one content row of a grouped list takes the secondary zebra
/// fill rather than the surface fill: a group's members alternate from the
/// secondary fill at their first member, so every second member reverts to
/// the surface fill (`media_list/wide.rs`'s rule, mirrored for the frame).
fn grouped_member_striped<Target>(rows: &[MediaListRow<Target>], row: usize) -> bool {
    // The group's members start at the row below its header (the list's own
    // first row when it has none above the row).
    let start = (0..=row)
        .rev()
        .find(|&index| matches!(rows[index], MediaListRow::Heading { .. }))
        .map_or(0, |header| header + 1);
    (row - start).is_multiple_of(2)
}

/// The slot of a row that carries the now-playing marker.
fn active_slot(row: &MediaListRow<QueueSlotId>) -> Option<u64> {
    match row {
        MediaListRow::Item {
            target,
            semantic_state: MediaSemanticState::NowPlaying { .. },
            ..
        } => Some(target.raw()),
        MediaListRow::Item { .. } | MediaListRow::Heading { .. } | MediaListRow::Spacer => None,
    }
}

/// One display row of the Queue, mirroring `MediaListRow<QueueSlotId>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "row", rename_all = "camelCase")]
pub enum GuiQueueRow {
    /// A selectable row for one stable slot.
    Item {
        /// The stable slot (`QueueSlotId::raw`).
        slot: u64,
        primary: String,
        secondary: Option<String>,
        trailing: Option<GuiTrailing>,
        duration: Option<String>,
        /// The row's wall-clock end time, the right column's "Ends at" value,
        /// formatted in the local zone. The "Length" value is `duration`.
        /// `None` for a slot before the playhead and for a slot whose finish
        /// chain an unknown runtime broke.
        ends_at: Option<String>,
        kind: GuiMediaKind,
        state: GuiSemanticState,
        /// The row's zebra parity: the TUI's alternation paints the odd
        /// item rows in the `QueueColumn` fill inside the recessed box.
        striped: bool,
    },
    /// A structural heading row.
    Heading { text: String },
    /// A structural spacer row.
    Spacer,
}

/// The queue summary the header paints (design D4): the remaining count, the
/// queue finish clock, and the right column's mode. Mirrors
/// `QueueSummaryProjection`, whose finish arithmetic the frame reads per
/// frame; the mode is Client-core state, so it carries its default until the
/// toggle dispatch adds the core-side state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiQueueSummary {
    /// The slots after the active index: the queue's remaining play. The
    /// whole queue's slot count while no slot carries the playhead.
    pub remaining: usize,
    /// The wall-clock time the shown queue finishes, formatted in the local
    /// zone. `None` while no slot carries the playhead or a pending runtime
    /// in the chain is unknown.
    pub finish_clock: Option<String>,
    /// Which value the rows' right column paints.
    pub right_column: GuiRightColumn,
}

impl From<&QueueSummaryProjection> for GuiQueueSummary {
    fn from(summary: &QueueSummaryProjection) -> Self {
        Self {
            remaining: summary.remaining,
            finish_clock: summary.finish_clock.clone(),
            right_column: GuiRightColumn::default(),
        }
    }
}

/// Which value the queue rows' right column paints (design D4's "Length" /
/// "Ends at" switch). The default is the length mode; the toggle arrives
/// with the core-side state of a later dispatch.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum GuiRightColumn {
    /// Each row paints its total runtime.
    #[default]
    Length,
    /// Each row paints the clock time at which it ends, from the current
    /// position onward.
    EndsAt,
}

impl GuiQueueRow {
    /// Builds one display row from a `MediaListRow` and its painted zebra
    /// state.
    #[must_use]
    pub fn from_painted(row: &MediaListRow<QueueSlotId>, striped: bool) -> Self {
        match row {
            MediaListRow::Item {
                target,
                primary,
                secondary,
                trailing,
                duration,
                kind,
                semantic_state,
            } => Self::Item {
                slot: target.raw(),
                primary: primary.clone(),
                secondary: secondary.clone(),
                trailing: trailing.as_ref().map(GuiTrailing::from),
                duration: duration.clone(),
                ends_at: None,
                kind: GuiMediaKind::from(*kind),
                state: GuiSemanticState::from(semantic_state),
                striped,
            },
            MediaListRow::Heading { text } => Self::Heading { text: text.clone() },
            MediaListRow::Spacer => Self::Spacer,
        }
    }
}

/// Gutter metadata that a row paints in the right-aligned column, mirroring
/// `MediaListTrailing`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum GuiTrailing {
    /// The release year, publish date, or minutes-precision runtime.
    Gutter(String),
}

impl From<&MediaListTrailing> for GuiTrailing {
    fn from(trailing: &MediaListTrailing) -> Self {
        match trailing {
            MediaListTrailing::Gutter(text) => Self::Gutter(text.clone()),
        }
    }
}

/// Whether a row drills into a container or plays a leaf, mirroring `MediaKind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum GuiMediaKind {
    Collection,
    Media,
}

impl From<MediaKind> for GuiMediaKind {
    fn from(kind: MediaKind) -> Self {
        match kind {
            MediaKind::Collection => Self::Collection,
            MediaKind::Media => Self::Media,
        }
    }
}

/// The semantic state of a row, mirroring `MediaSemanticState`. The `state` tag
/// names the state, and `progress` is the bounded percentage where the state
/// carries one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum GuiSemanticState {
    Ordinary,
    Played,
    Active { progress: Option<u8> },
    NowPlaying { progress: Option<u8> },
}

impl From<&MediaSemanticState> for GuiSemanticState {
    fn from(state: &MediaSemanticState) -> Self {
        match state {
            MediaSemanticState::Ordinary => Self::Ordinary,
            MediaSemanticState::Played => Self::Played,
            MediaSemanticState::Active { progress } => Self::Active {
                progress: progress.map(ActiveProgress::percent),
            },
            MediaSemanticState::NowPlaying { progress } => Self::NowPlaying {
                progress: progress.map(ActiveProgress::percent),
            },
        }
    }
}
