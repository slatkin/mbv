//! Queue-scoped request and intent types. Split from `msg.rs` (task 8.3) to
//! keep the central `Msg` file below the 800-line cap.
//!
//! The queue can be reordered by the Player between paint and dispatch, so
//! these types carry slot identity (or the focused scope), not a snapshot
//! index.

use mbv_core::playback_queue::QueueSlotId;

/// Queue requests carry slot identity, not a snapshot index. The queue can be
/// reordered by the Player between paint and dispatch.
#[derive(Debug, Clone, PartialEq)]
pub enum QueueRequest {
    Cursor {
        scope: crate::app::types_playback::QueueScope,
        slot_id: QueueSlotId,
    },
    Scope(crate::app::types_playback::QueueScope),
    Play {
        scope: crate::app::types_playback::QueueScope,
        slot_id: QueueSlotId,
    },
    Remove {
        scope: crate::app::types_playback::QueueScope,
        slot_id: QueueSlotId,
    },
    Move {
        scope: crate::app::types_playback::QueueScope,
        slot_id: QueueSlotId,
        direction: QueueMove,
    },
    Undo {
        scope: crate::app::types_playback::QueueScope,
    },
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
        scope: crate::app::types_playback::QueueScope,
        slot_id: QueueSlotId,
    },
    PlayNow,
    SavePlaylist,
    ResizeColumn(QueueColumnResize),
    StopRemoteTracking,
    ReanchorRemoteTracking,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueColumnResize {
    Narrower,
    Wider,
}
