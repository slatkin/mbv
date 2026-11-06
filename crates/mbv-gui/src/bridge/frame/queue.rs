//! The Queue section of a frame: the rows inside the viewport, and the cursor,
//! marks, and active slot (design D4). A slot id travels as its raw number, as
//! `layout.rs` does.

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
    /// tall, with the cursor, marks, and active slot.
    #[must_use]
    pub fn from_surface(queue: &QueueSurface, viewport_rows: usize) -> Self {
        let viewport = queue.list().resolve_viewport(viewport_rows.max(1));
        Self {
            rows: queue
                .rows()
                .iter()
                .skip(viewport.offset)
                .take(viewport.height)
                .map(GuiQueueRow::from)
                .collect(),
            cursor: queue.selected_target().map(|id| id.raw()),
            marks: queue.multi_selection().iter().map(|id| id.raw()).collect(),
            active: queue.rows().iter().find_map(active_slot),
        }
    }
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
        kind: GuiMediaKind,
        state: GuiSemanticState,
    },
    /// A structural heading row.
    Heading { text: String },
    /// A structural spacer row.
    Spacer,
}

impl From<&MediaListRow<QueueSlotId>> for GuiQueueRow {
    fn from(row: &MediaListRow<QueueSlotId>) -> Self {
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
