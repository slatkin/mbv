//! The Queue section: the rows inside the viewport, and the cursor,
//! marks, and active slot (design D4). A slot id travels as its raw number, as
//! `layout.rs` does. The summary section carries the queue header's chooser
//! facts, tail offset, filter text, and totals.

use mbv::{QueueEndOffsets, QueueHeaderProjection, QueueSummaryProjection};
use mbv_queue::QueueSlotId;
use mbv_surfaces::queue::{QueueRowDrag, QueueRowInsertion, QueueSurface};
use mbv_text::fuzzy_match::{WordMatcher, word_match_score};
use mbv_ui_model::media_list::{
    ActiveProgress, MediaKind, MediaListRow, MediaListTrailing, MediaSemanticState,
};

/// The Queue section.
#[derive(Debug, Clone, PartialEq, Eq)]
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
    /// add-tauri-gui-client row 6.5): the window paints that row's grip and
    /// actions button. `None` while the pointer rests on no row.
    pub hover: Option<u64>,
    /// The row drag in progress, if any: the drag ghost and insertion line.
    pub drag: Option<GuiQueueDrag>,
    /// The scroll position the window paints as a scroll bar, `None` while
    /// every row fits the viewport. The core owns the scroll, so the bar is a
    /// readout of it, never a control.
    pub scroll: Option<GuiQueueScroll>,
}

/// The queue list's scroll position: the viewport's first row, the rows it
/// shows, and the rows the list holds, structural rows included. The window
/// draws the bar's thumb from the three.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GuiQueueScroll {
    /// The first painted row's index in the projected rows.
    pub offset: usize,
    /// The rows the viewport shows.
    pub visible: usize,
    /// The projected rows the list holds.
    pub total: usize,
}

impl GuiQueueScroll {
    /// The position of a viewport over `total` rows, `None` when every row
    /// fits so no bar is due.
    fn of(offset: usize, visible: usize, total: usize) -> Option<Self> {
        (total > visible).then_some(Self {
            offset,
            visible,
            total,
        })
    }

    /// The thumb's top edge and length as percentages of the track: the share
    /// of the list above the viewport and the share the viewport shows. The
    /// two stay inside the track, so a thumb at the end of the list ends at
    /// the track's end.
    #[must_use]
    pub fn thumb_percent(&self) -> (f64, f64) {
        let share = |rows: usize| {
            let rows = f64::from(u32::try_from(rows).unwrap_or(u32::MAX));
            let total = f64::from(u32::try_from(self.total).unwrap_or(u32::MAX).max(1));
            (rows / total * 100.0).min(100.0)
        };
        let length = share(self.visible);
        let top = share(self.offset).min(100.0 - length);
        (top, length)
    }
}

/// The row drag the window paints (add-tauri-gui-client row 6.5, design D5):
/// the dragged row's content as the ghost, the final index the drop would
/// leave it at, and the insertion line's anchor. `None` while no drag runs.
#[derive(Debug, Clone, PartialEq, Eq)]
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

/// One row drag's insertion line (add-tauri-gui-client row 6.5): the row it
/// anchors on and the side it paints. A stable slot anchors it, so the line
/// keeps its place while the rows above it move.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuiQueueInsertion {
    /// The line paints above this slot's row.
    Before { slot: u64 },
    /// The line paints below this slot's row.
    After { slot: u64 },
}

impl GuiQueueDrag {
    /// Projects the Surface state's drag over the list's rows: the ghost is
    /// the dragged row's own content, so a drag of a row outside the painted
    /// viewport still paints what it moves. `None` when the rows no longer
    /// hold the dragged slot (a content refresh can drop it).
    fn from_surface(queue: &QueueSurface, drag: QueueRowDrag) -> Option<Self> {
        let row = queue.rows().iter().find(
            |row| matches!(row, MediaListRow::Item { target, .. } if *target == drag.slot_id),
        )?;
        let ordinal = queue
            .item_index_of(drag.slot_id)
            .map_or(0, |index| index + 1);
        let ghost = GuiQueueRow::from_painted(row, ordinal);
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
    /// tall, with the cursor, marks, and active slot. The
    /// summary's filter text narrows the projected rows locally — the library's
    /// one word-local acceptance rule (`mbv-text`) decides what matches, and the
    /// Surface state and the canonical queue stay whole.
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
        let scroll = GuiQueueScroll::of(viewport.0, viewport.1, kept.len());
        let rows_dto = kept
            .iter()
            .skip(viewport.0)
            .take(viewport.1)
            .map(|row| {
                // The canvas's row number is the slot's canonical queue
                // position, not its index in the painted viewport: the
                // filter keeps the number a row carried before it.
                let ordinal = row
                    .selectable_target()
                    .and_then(|target| queue.item_index_of(*target))
                    .map_or(0, |index| index + 1);
                GuiQueueRow::from_painted(row, ordinal)
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
                .and_then(|drag| GuiQueueDrag::from_surface(queue, drag)),
            scroll,
        }
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
/// cursor, the filter's fresh re-anchor each time it changes. Returns the
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
#[derive(Debug, Clone, PartialEq, Eq)]
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
        kind: GuiMediaKind,
        state: GuiSemanticState,
    },
    /// A structural heading row.
    Heading { text: String },
    /// A structural spacer row.
    Spacer,
}

/// The queue summary the header and the folded line paint (design D4): the
/// remaining count and the queue's tail offset the folded queue keeps, the
/// Queue chooser facts the header's controls read (design D9 of
/// `gui-named-queues`), the filter text with its caret fact, and the totals
/// the footer shows. Mirrors `QueueSummaryProjection`'s remaining, next-item,
/// total, and filter facts; the tail offset comes from the shown queue's end
/// offsets (design D5), which the window turns into the finish wall clock.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiQueueSummary {
    /// The rows that do not carry the playhead: the canvas's remaining count
    /// counts every non-playhead row, rows before the playhead included —
    /// the whole queue's slot count while no slot carries the playhead
    /// (align U1.3; the projection's `QueueSummaryProjection::remaining`).
    pub remaining: usize,
    /// The title of the first slot after the playhead (review of
    /// add-tauri-gui-client row 6.6, finding P2): the compact strip's
    /// next-item line. It comes from the queue, not from `GuiQueue::rows`,
    /// because the strip paints no Queue list and its viewport need not hold
    /// the playhead row. `None` while no slot follows the playhead.
    pub next_title: Option<String>,
    /// The shown queue's tail (design D5): the queue's finish as seconds
    /// after the end of the active item, which the window adds to the wall
    /// clock now plus the active item's remaining time. `None` while no slot
    /// carries the playhead or a pending runtime in the chain is unknown.
    pub tail_secs: Option<i64>,
    /// The queue filter's text, which narrows the projected rows locally.
    /// Empty text filters nothing.
    pub filter: String,
    /// Whether the filter's text entry holds the keyboard: the caret fact
    /// the filter control paints.
    pub filter_active: bool,
    /// The shown queue's total time in seconds: every slot's known runtime
    /// added up, which the footer paints at the right of its line. The filter
    /// narrows no total (design D9 of `gui-named-queues`).
    pub total_secs: i64,
    /// Whether a slot of the shown queue has an unknown runtime, so
    /// [`Self::total_secs`] is only a lower bound.
    pub total_partial: bool,
    /// The marked rows' total time in seconds, or `None` while no row of the
    /// shown queue is marked: the footer then shows the queue's total alone.
    pub marked_secs: Option<i64>,
    /// Whether a marked row has an unknown runtime, so [`Self::marked_secs`]
    /// is only a lower bound.
    pub marked_partial: bool,
    /// The Queue chooser facts the header's controls read (design D9 of
    /// `gui-named-queues`). They travel as one group because each of the
    /// header's controls is a function of them: the switcher's label and
    /// count, and the close control's state.
    pub header: GuiQueueHeaderFacts,
}

/// The Queue header's chooser facts for one frame (design D7 of
/// `gui-queue-panel-redesign`, spec `gui-queue-chooser` "The queue header holds
/// the chooser and its controls"): what the header's controls paint. The
/// switcher paints at all times, so these facts carry no entry count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiQueueHeaderFacts {
    /// The name of the shown entry: the switcher's label. Empty while no entry
    /// shows, which the header paints as `Queue`.
    pub shown_name: String,
    /// The shown entry's slot count: the switcher's count chip (design D7 of
    /// `gui-queue-panel-redesign`). The Queue filter narrows no count, so it
    /// counts every slot of the shown queue.
    pub shown_len: usize,
    /// Whether the close control is disabled: the only local queue has no
    /// close (spec `gui-queue-chooser` "The close control closes the shown
    /// entry").
    pub close_disabled: bool,
}

impl GuiQueueSummary {
    /// The summary section from the projection's remaining, next-item, total,
    /// and filter facts, the shown queue's end offsets' tail (design D5), and
    /// the header's chooser facts (design D9 of `gui-named-queues`). Consumes
    /// `summary`: the frame reads it once, so its owned strings move instead
    /// of a second clone.
    #[must_use]
    pub fn build(
        summary: QueueSummaryProjection,
        offsets: &QueueEndOffsets,
        header: &QueueHeaderProjection,
    ) -> Self {
        Self {
            remaining: summary.remaining,
            next_title: summary.next_title,
            tail_secs: offsets.tail,
            filter: summary.filter,
            filter_active: summary.filter_active,
            total_secs: summary.total_secs,
            total_partial: summary.total_partial,
            marked_secs: summary.marked_secs,
            marked_partial: summary.marked_partial,
            header: GuiQueueHeaderFacts {
                shown_name: header.shown_name.clone(),
                shown_len: header.shown_len,
                close_disabled: header.close_disabled,
            },
        }
    }
}

impl GuiQueueRow {
    /// Builds one display row from a `MediaListRow` and its 1-based canonical
    /// queue position (`ordinal`; structural rows ignore it).
    #[must_use]
    pub fn from_painted(row: &MediaListRow<QueueSlotId>, ordinal: usize) -> Self {
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
                kind: GuiMediaKind::from(*kind),
                state: GuiSemanticState::from(semantic_state),
            },
            MediaListRow::Heading { text } => Self::Heading { text: text.clone() },
            MediaListRow::Spacer => Self::Spacer,
        }
    }
}

/// Gutter metadata that a row paints in the right-aligned column, mirroring
/// `MediaListTrailing`.
#[derive(Debug, Clone, PartialEq, Eq)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
/// carries one. The now-playing state carries no percentage (design D5): the
/// window reads the playhead's played fraction for the row whose slot is the
/// active slot, and only where `tracks_progress` says the item has progress to
/// show — music is a no-progress media type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuiSemanticState {
    Ordinary,
    Played,
    Active { progress: Option<u8> },
    NowPlaying { tracks_progress: bool },
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
                tracks_progress: progress.is_some(),
            },
        }
    }
}
