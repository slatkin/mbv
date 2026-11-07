//! The Queue section of a frame: the rows inside the viewport, and the cursor,
//! marks, and active slot (design D4). A slot id travels as its raw number, as
//! `layout.rs` does. The summary section carries the queue header's remaining
//! count, finish clock, right-column mode, and filter text.

use mbv::QueueSummaryProjection;
use mbv_queue::QueueSlotId;
use mbv_surfaces::queue::{QueueRowDrag, QueueRowInsertion, QueueSurface};
use mbv_text::fuzzy_match::{WordMatcher, word_match_score};
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
    /// The row the pointer rests on (the canvas row hover,
    /// add-tauri-gui-client row 6.5): the front end paints that row's grip and
    /// actions button. `None` while the pointer rests on no row.
    pub hover: Option<u64>,
    /// The row drag in progress, if any: the drag ghost and insertion line.
    pub drag: Option<GuiQueueDrag>,
}

/// The row drag one frame paints (add-tauri-gui-client row 6.5, design D5):
/// the dragged row's content as the ghost, the final index the drop would
/// leave it at, and the insertion line's anchor. `None` while no drag runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiQueueDrag {
    /// The dragged slot (`QueueSlotId::raw`).
    pub slot: u64,
    /// The dragged row's content, painted as the drag ghost.
    pub ghost: GuiQueueRow,
    /// The slot's final queue index if the drop applied now.
    pub drop_index: usize,
    /// The insertion line's anchor.
    pub insertion: GuiQueueInsertion,
}

/// One frame's insertion line (add-tauri-gui-client row 6.5): the row it
/// anchors on and the side it paints. A stable slot anchors it, so the line
/// keeps its place while the rows above it move.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "at", rename_all = "camelCase")]
pub enum GuiQueueInsertion {
    /// The line paints above this slot's row.
    Before { slot: u64 },
    /// The line paints below this slot's row.
    After { slot: u64 },
}

impl GuiQueueDrag {
    /// Projects the Surface state's drag over the list's rows: the ghost is
    /// the dragged row's own content — its Ends-at value included, so the
    /// ghost paints the same right column as the row it lifts — so a drag of
    /// a row outside the painted viewport still paints what it moves. `None`
    /// when the rows no longer hold the dragged slot (a content refresh can
    /// drop it).
    fn from_surface(
        queue: &QueueSurface,
        drag: QueueRowDrag,
        summary: &QueueSummaryProjection,
    ) -> Option<Self> {
        let row = queue.rows().iter().find(
            |row| matches!(row, MediaListRow::Item { target, .. } if *target == drag.slot_id),
        )?;
        let ordinal = queue
            .item_index_of(drag.slot_id)
            .map_or(0, |index| index + 1);
        let mut ghost = GuiQueueRow::from_painted(row, false, ordinal);
        apply_ends_at(&mut ghost, summary);
        Some(Self {
            slot: drag.slot_id.raw(),
            ghost,
            drop_index: drag.drop_index,
            insertion: match drag.insertion {
                QueueRowInsertion::Before(slot_id) => GuiQueueInsertion::Before {
                    slot: slot_id.raw(),
                },
                QueueRowInsertion::After(slot_id) => GuiQueueInsertion::After {
                    slot: slot_id.raw(),
                },
            },
        })
    }
}

impl GuiQueue {
    /// Projects the Queue Surface state: the rows of a viewport `viewport_rows`
    /// tall, with the cursor, marks, active slot, zebra parity, and each row's
    /// finish clock from `summary`. The summary's filter text narrows the
    /// projected rows locally — the library's one word-local acceptance rule
    /// (`mbv-text`) decides what matches, and the Surface state and the
    /// canonical queue stay whole.
    #[must_use]
    pub fn from_surface(
        queue: &QueueSurface,
        viewport_rows: usize,
        summary: &QueueSummaryProjection,
    ) -> Self {
        let rows = queue.rows();
        let filter = summary.filter.as_str();
        let matcher = (!filter.is_empty()).then(WordMatcher::new);
        let kept: Vec<&MediaListRow<QueueSlotId>> = match &matcher {
            // While the filter applies, only matching item rows project; a
            // structural row with no matching children would be noise.
            Some(matcher) => rows
                .iter()
                .filter(|row| row_matches_filter(matcher, row, filter))
                .collect(),
            None => rows.iter().collect(),
        };
        let viewport = if matcher.is_some() {
            let cursor = queue.selected_target().copied();
            filtered_viewport(&kept, cursor, viewport_rows.max(1))
        } else {
            let viewport = queue.list().resolve_viewport(viewport_rows.max(1));
            (viewport.offset, viewport.height)
        };
        // The zebra alternation the TUI's wide presentation paints
        // (`mbv-render` `media_list/wide.rs`): a grouped list alternates
        // within each group from its first member, an ungrouped list
        // alternates throughout, seeded by the viewport offset and flipped
        // per item row — headings and spacers keep the box's own fill and
        // do not flip. The parity rides the row so the stripe stays stable
        // while the viewport moves; under a filter it re-seeds over the kept
        // rows, whose list the front end paints.
        let grouped = kept
            .iter()
            .any(|row| matches!(row, MediaListRow::Heading { .. }));
        let mut striped = viewport.0 % 2 == 1;
        let rows_dto = kept
            .iter()
            .enumerate()
            .skip(viewport.0)
            .take(viewport.1)
            .map(|(source_row, row)| {
                let striped = striped_state(row, source_row, grouped, &kept, &mut striped);
                // The canvas's row number is the slot's canonical queue
                // position, not its index in the painted viewport: the
                // filter keeps the number a row carried before it.
                let ordinal = row
                    .selectable_target()
                    .and_then(|target| queue.item_index_of(*target))
                    .map_or(0, |index| index + 1);
                let mut row = GuiQueueRow::from_painted(row, striped, ordinal);
                apply_ends_at(&mut row, summary);
                row
            })
            .collect();
        Self {
            rows: rows_dto,
            cursor: queue.selected_target().map(|id| id.raw()),
            marks: queue.multi_selection().iter().map(|id| id.raw()).collect(),
            // The now-playing marker reads the whole list, filtered view or
            // not: it is the playback fact, not the filter's.
            active: rows.iter().find_map(active_slot),
            hover: queue.hover().map(QueueSlotId::raw),
            drag: queue
                .row_drag()
                .and_then(|drag| GuiQueueDrag::from_surface(queue, drag, summary)),
        }
    }
}

/// Fill a row's Ends-at value from the summary's per-slot finish clocks (the
/// right column's "Ends at" datum). Shared by the painted rows and the drag
/// ghost, so the ghost paints the same right column as its row.
fn apply_ends_at(row: &mut GuiQueueRow, summary: &QueueSummaryProjection) {
    if let GuiQueueRow::Item { slot, ends_at, .. } = row {
        *ends_at = summary
            .row_finish_clocks
            .iter()
            .find(|(clock_slot, _)| clock_slot.raw() == *slot)
            .map(|(_, clock)| clock.clone());
    }
}

/// Whether one row matches the queue filter: the item's title, or its
/// secondary line when one is present, must accept the query under the
/// library's word-local fuzzy rule. A structural row never matches.
fn row_matches_filter(matcher: &WordMatcher, row: &MediaListRow<QueueSlotId>, query: &str) -> bool {
    match row {
        MediaListRow::Item {
            primary, secondary, ..
        } => {
            word_match_score(matcher, primary, query).is_some()
                || secondary
                    .as_deref()
                    .is_some_and(|text| word_match_score(matcher, text, query).is_some())
        }
        MediaListRow::Heading { .. } | MediaListRow::Spacer => false,
    }
}

/// The viewport over the kept rows that keeps the cursor row fully visible:
/// the window opens at the top and scrolls only far enough to hold the
/// cursor, the filter's fresh re-anchor each frame it changes. Returns the
/// offset and height the kept list slices with.
fn filtered_viewport(
    kept: &[&MediaListRow<QueueSlotId>],
    cursor: Option<QueueSlotId>,
    height: usize,
) -> (usize, usize) {
    let cursor_position = cursor.and_then(|slot| {
        kept.iter()
            .position(|row| matches!(row, MediaListRow::Item { target, .. } if *target == slot))
    });
    let offset = match cursor_position {
        Some(position) if position >= height => position + 1 - height,
        _ => 0,
    };
    (offset.min(kept.len().saturating_sub(height)), height)
}

/// One row's zebra state, mirroring the TUI's alternation walk
/// (`media_list/wide.rs`): headings and spacers never stripe and never
/// flip; an ungrouped item takes the running state and flips it; a grouped
/// item alternates within its group from the group's first member.
fn striped_state<Target>(
    row: &MediaListRow<Target>,
    source_row: usize,
    grouped: bool,
    rows: &[&MediaListRow<Target>],
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
fn grouped_member_striped<Target>(rows: &[&MediaListRow<Target>], row: usize) -> bool {
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
        /// The row's 1-based position in the queue: the canvas's `01`-style
        /// row number, read from the canonical queue order rather than the
        /// painted viewport, so a scrolled or filtered viewport still numbers
        /// rows by their queue positions.
        ordinal: usize,
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
/// queue finish clock, the right column's mode, and the filter text with its
/// caret fact. Mirrors `QueueSummaryProjection`, whose finish arithmetic the
/// frame reads per frame; the mode, filter text, and caret fact are the
/// Client-core state the projections carry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiQueueSummary {
    /// The slots after the active index: the queue's remaining play. The
    /// whole queue's slot count while no slot carries the playhead.
    pub remaining: usize,
    /// The title of the first slot after the playhead (review of
    /// add-tauri-gui-client row 6.6, finding P2): the compact strip's
    /// next-item line. It comes from the queue, not from `GuiQueue::rows`,
    /// because the strip paints no Queue list and its viewport need not hold
    /// the playhead row. `None` while no slot follows the playhead.
    pub next_title: Option<String>,
    /// The wall-clock time the shown queue finishes, formatted in the local
    /// zone. `None` while no slot carries the playhead or a pending runtime
    /// in the chain is unknown.
    pub finish_clock: Option<String>,
    /// Which value the rows' right column paints.
    pub right_column: GuiRightColumn,
    /// The queue filter's text, which narrows the projected rows locally.
    /// Empty text filters nothing.
    pub filter: String,
    /// Whether the filter's text entry holds the keyboard: the caret fact
    /// the filter control paints.
    pub filter_active: bool,
}

impl From<&QueueSummaryProjection> for GuiQueueSummary {
    fn from(summary: &QueueSummaryProjection) -> Self {
        Self {
            remaining: summary.remaining,
            next_title: summary.next_title.clone(),
            finish_clock: summary.finish_clock.clone(),
            right_column: GuiRightColumn::from(summary.right_column),
            filter: summary.filter.clone(),
            filter_active: summary.filter_active,
        }
    }
}

/// Which value the queue rows' right column paints (design D4's "Length" /
/// "Ends at" switch), mirroring `QueueRightColumn` variant for variant.
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

impl From<mbv::QueueRightColumn> for GuiRightColumn {
    fn from(mode: mbv::QueueRightColumn) -> Self {
        match mode {
            mbv::QueueRightColumn::Length => Self::Length,
            mbv::QueueRightColumn::EndsAt => Self::EndsAt,
        }
    }
}

impl GuiQueueRow {
    /// Builds one display row from a `MediaListRow`, its painted zebra state,
    /// and its 1-based canonical queue position (`ordinal`; structural rows
    /// ignore it).
    #[must_use]
    pub fn from_painted(row: &MediaListRow<QueueSlotId>, striped: bool, ordinal: usize) -> Self {
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
                ordinal,
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
