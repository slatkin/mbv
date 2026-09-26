mod commands;
mod events;
mod protocol;

pub use commands::{
    CtrlCmd, OwnerGate, OwnerGateRejection, PlaybackIntent, PlaybackIntentAction, WireCommand,
};
pub use events::{
    AudiobookshelfBookProgressEvent, AudiobookshelfProgressEvent, CtrlEvent, DisconnectReason,
    PipePlaybackPhase, PipePlaybackStatus, PlaybackIntentEvent, PlaybackIntentOutcome,
    PlaybackIntentRejection, QueueLoadResult, ServiceSetupRejection,
};
pub use protocol::{
    slot_id_to_u64, CtrlAudiobookshelfCapabilities, CtrlCompatibility, CtrlHello,
    PlaybackGeneration, PlaybackRequestId, QueueLineage, QueueLoadRequestId, TransitionSummary,
    UnifiedQueueSlot, UnifiedQueueStateData, CTRL_CAP_ABS_BOOK_PROGRESS, CTRL_CAP_ABS_BOOK_QUEUE,
    CTRL_CAP_ABS_PROGRESS, CTRL_CAP_ABS_QUEUE, CTRL_CAP_AUDIO_ONLY, CTRL_CAP_CONTROL_AUTH,
    CTRL_CAP_LIFECYCLE_SHUTDOWN, CTRL_CAP_OWNER_QUEUE_LOAD, CTRL_CAP_QUEUE_STATE,
    CTRL_CAP_START_INDEX, CTRL_CAP_STATUS_ONLY, CTRL_CAP_UNIFIED_QUEUE, CTRL_PROTOCOL_VERSION,
};

#[cfg(test)]
use crate::{
    config::QueueSource, playback_queue::QueueItem, player::PlayerCommand, player::PlayerStatus,
};

#[cfg(test)]
mod tests;
