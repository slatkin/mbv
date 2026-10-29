//! Queue-link request and intent types. Split from `msg.rs` (task 8.3) to
//! keep the central `Msg` file below the 800-line cap.
//!
//! The queue can be reordered by the Player between paint and dispatch, so
//! these types carry slot identity (or the focused link), not a snapshot
//! index.

use mbv_queue::QueueSlotId;

/// Queue requests carry slot identity, not a snapshot index. The queue can be
/// reordered by the Player between paint and dispatch.
#[derive(Debug, Clone, PartialEq)]
pub enum QueueRequest {
    Cursor {
        link: mbv_ui_model::playback::QueueLink,
        slot_id: QueueSlotId,
    },
    Play {
        link: mbv_ui_model::playback::QueueLink,
        slot_id: QueueSlotId,
    },
    Remove {
        link: mbv_ui_model::playback::QueueLink,
        slot_id: QueueSlotId,
    },
    /// Remove the selected queue slots, resolving each slot position when the
    /// shell executes the request because the queue may have changed since it
    /// was painted.
    RemoveSelection {
        link: mbv_ui_model::playback::QueueLink,
        slot_ids: Vec<QueueSlotId>,
    },
    Move {
        link: mbv_ui_model::playback::QueueLink,
        slot_id: QueueSlotId,
        direction: QueueMove,
    },
    MoveTo {
        link: mbv_ui_model::playback::QueueLink,
        slot_id: QueueSlotId,
        onto: QueueSlotId,
    },
    Undo {
        link: mbv_ui_model::playback::QueueLink,
    },
    ResizeColumnLive(u16),
    ResizeColumnEnd(u16),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueMove {
    Up,
    Down,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueIntent {
    Clear,
    Navigate {
        link: mbv_ui_model::playback::QueueLink,
        slot_id: QueueSlotId,
    },
    PlayNow,
    SavePlaylist,
    ResizeColumn(QueueColumnResize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueColumnResize {
    Narrower,
    Wider,
}
