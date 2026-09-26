use serde::{Deserialize, Serialize};

use crate::config::ServiceKind;
use crate::ctrl::{
    CtrlHello, PlaybackGeneration, PlaybackRequestId, QueueLoadRequestId, UnifiedQueueStateData,
};
use crate::player::{PlayerEvent, PlayerStatus};

#[derive(Debug, Serialize, Deserialize)]
pub enum CtrlEvent {
    Hello(CtrlHello),
    Player(PlayerEvent),
    StatusOnly(PlayerStatus),
    #[serde(rename = "Disconnected")]
    Disconnected {
        reason: DisconnectReason,
    },
    /// A command the daemon received over the ctrl socket was not acted on;
    /// the payload is a human-readable, server-computed reason. Generic by
    /// design so future rejection reasons (not just audio-only mode) can
    /// reuse it — see #90.
    CommandRejected(String),
    PlaybackIntent(PlaybackIntentEvent),
    /// Observed pipe-startup progress for a direct daemon client. The daemon
    /// owns this status; it never represents downstream audibility.
    PipePlaybackStatus(PipePlaybackStatus),
    /// The daemon accepted a coordinated shutdown request after durably
    /// persisting its authoritative queue.
    ShutdownAccepted,
    /// The daemon rejected a coordinated shutdown request. The reason
    /// indicates why (e.g. TCP transport, persistence failure).
    ShutdownRejected {
        reason: String,
    },
    ServiceSetupApplied {
        kind: ServiceKind,
        revision: u64,
    },
    ServiceSetupRejected {
        kind: ServiceKind,
        revision: u64,
        reason: ServiceSetupRejection,
    },

    // ── Unified queue events (require `unified-queue` capability) ───────
    /// Full item-generic queue state.  Sent on initial connection and
    /// after every queue mutation.
    UnifiedQueueState(UnifiedQueueStateData),
    /// Result of an idle whole-queue load, correlated with its request.
    #[serde(rename = "UnifiedQueueLoadResult")]
    UnifiedQueueLoadResult {
        request_id: QueueLoadRequestId,
        result: QueueLoadResult,
    },

    /// Redacted, provider-qualified Audiobookshelf progress. Sent only to
    /// peers advertising `abs-progress`. See `AudiobookshelfProgressEvent`
    /// for the field-level redaction contract this event upholds.
    AudiobookshelfProgress(AudiobookshelfProgressEvent),

    /// Redacted, provider-qualified Audiobookshelf book progress. Sent only to
    /// peers advertising `abs-book-progress`. Keyed by `library_item_id` only,
    /// so it can never be matched against an episode-shaped event.
    AudiobookshelfBookProgress(AudiobookshelfBookProgressEvent),
}

/// Redacted, provider-qualified Audiobookshelf progress: identity,
/// acknowledged position/completion, and setup generation only. Excludes
/// API key, Authorization header, resolved source URL, and playback
/// `sessionId` — those never cross the ctrl wire.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudiobookshelfProgressEvent {
    pub library_item_id: String,
    pub episode_id: String,
    /// Acknowledged playback position in ticks.
    pub position_ticks: i64,
    /// Mirrors Audiobookshelf `is_finished` completion state.
    pub is_finished: bool,
    /// Setup generation the acknowledged progress was produced under;
    /// receivers discard progress from a stale generation.
    pub setup_generation: u64,
}

/// Book-shaped counterpart to `AudiobookshelfProgressEvent`: identity,
/// acknowledged position/completion, and setup generation only, keyed by
/// `library_item_id` with no `episode_id`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudiobookshelfBookProgressEvent {
    pub library_item_id: String,
    /// Acknowledged playback position in ticks.
    pub position_ticks: i64,
    /// Mirrors Audiobookshelf `is_finished` completion state.
    pub is_finished: bool,
    /// Setup generation the acknowledged progress was produced under;
    /// receivers discard progress from a stale generation.
    pub setup_generation: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum QueueLoadResult {
    Accepted,
    Rejected { reason: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServiceSetupRejection {
    UnsupportedService,
    RevisionMismatch,
    StorageUnavailable,
    TransitionRejected,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PipePlaybackStatus {
    pub request_id: PlaybackRequestId,
    pub generation: PlaybackGeneration,
    pub phase: PipePlaybackPhase,
    /// Approximate time until the configured local estimate expires. Only
    /// present for `OutputBuffering`; it is not an observed downstream value.
    pub estimated_remaining_ms: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PipePlaybackPhase {
    Resolving,
    PlayerOpening,
    /// mbv observed mpv's `PlaybackRestart` event for this generation.
    OutputStarted,
    OutputBuffering,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaybackIntentEvent {
    pub request_id: PlaybackRequestId,
    pub generation: PlaybackGeneration,
    pub outcome: PlaybackIntentOutcome,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlaybackIntentOutcome {
    Accepted,
    Applied,
    Coalesced {
        canonical_request_id: PlaybackRequestId,
    },
    Superseded,
    Rejected {
        reason: PlaybackIntentRejection,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlaybackIntentRejection {
    EmptyTarget,
    InvalidTarget,
    ResolutionFailed,
    AudioOnly,
    Unavailable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DisconnectReason {
    #[serde(rename = "TakenOverByEmbyRemote")]
    TakenOverByEmbyRemote,
    /// The daemon is shutting down deliberately (`mbv -q`, tray Quit).
    /// Unlike `TakenOverByEmbyRemote`, this reason means the connection is
    /// about to close, and it is not an Emby-authority notification.
    #[serde(rename = "DaemonShutdown")]
    DaemonShutdown,
}
