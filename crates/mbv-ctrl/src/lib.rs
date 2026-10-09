//! Player-owner protocol vocabulary for Client-to-owner communication over ctrl.
//!
//! Commands, events, capability negotiation, and the unified queue snapshot travel
//! here. The canonical queue itself lives in `mbv-queue` and is held by the Player
//! owner; this crate defines the vocabulary and never drives playback.

mod commands;
mod events;
mod owner_action;
mod protocol;

#[doc(inline)]
pub use commands::{
    CtrlCmd, Direction, OwnerGate, OwnerGateRejection, PlaybackIntent, PlaybackIntentAction,
    TransportCommand, WireCommand,
};
#[doc(inline)]
pub use events::{
    AudiobookshelfBookProgressEvent, AudiobookshelfProgressEvent, CtrlEvent, DisconnectReason,
    PipePlaybackPhase, PipePlaybackStatus, PlaybackIntentEvent, PlaybackIntentOutcome,
    PlaybackIntentRejection, QueueLoadResult, QueueOpOutcome, ServiceSetupRejection,
};
#[doc(inline)]
pub use owner_action::OwnerAction;
#[doc(inline)]
pub use protocol::{
    CTRL_CAP_ABS_BOOK_PROGRESS, CTRL_CAP_ABS_BOOK_QUEUE, CTRL_CAP_ABS_PROGRESS, CTRL_CAP_ABS_QUEUE,
    CTRL_CAP_ANSWERED_QUEUE_OPS, CTRL_CAP_AUDIO_ONLY, CTRL_CAP_CONTROL_AUTH,
    CTRL_CAP_LIFECYCLE_SHUTDOWN, CTRL_CAP_OWNER_ACTION, CTRL_CAP_OWNER_QUEUE_LOAD,
    CTRL_CAP_PIN_SWAP, CTRL_CAP_PINNED_SURFACE, CTRL_CAP_QUEUE_STATE, CTRL_CAP_SERVICE_SETUP_ADMIN,
    CTRL_CAP_START_INDEX, CTRL_CAP_STATUS_ONLY, CTRL_CAP_UNIFIED_QUEUE, CTRL_PROTOCOL_VERSION,
    PlaybackGeneration, PlaybackRequestId, ProgressUpdate, QueueLoadRequestId, QueueOpId,
    SWAP_TOKEN_ENV, TransitionSummary, UnifiedQueueSlot, UnifiedQueueStateData, slot_id_to_u64,
};

mod error;
mod hello;
#[doc(inline)]
pub use error::CtrlError;
#[doc(inline)]
pub use hello::{CtrlAudiobookshelfCapabilities, CtrlCompatibility, CtrlHello};

pub mod player;
