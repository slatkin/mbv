#[derive(Debug, Serialize, Deserialize)]
pub enum CtrlEvent {
    Hello(CtrlHello),
    Player(PlayerEvent),
    State(CtrlState),
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DisconnectReason {
    #[serde(rename = "TakenOverByEmbyRemote")]
    TakenOverByEmbyRemote,
    /// The daemon is shutting down deliberately (`mbv -q`, tray Quit).
    /// Unlike `TakenOverByEmbyRemote`, this reason means the connection is
    /// about to close, and it is not an Emby-authority notification.
    #[serde(rename = "DaemonShutdown")]
    DaemonShutdown,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CtrlState {
    pub status: PlayerStatus,
    pub items: Vec<MediaItem>,
    pub cursor: usize,
    pub source: QueueSource,
    /// Parallel to `items`: the daemon-assigned slot ID for each entry.
    /// Present for v8 peers, omitted for v7 peers.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub slot_ids: Vec<QueueSlotId>,
    /// Monotonic daemon queue revision; bumped on every structural mutation.
    /// Omitted when zero (v7 peers, before any mutation).
    #[serde(default, skip_serializing_if = "QueueRevision::is_default")]
    pub revision: QueueRevision,
    /// The daemon's active (playing) slot, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_slot_id: Option<QueueSlotId>,
}

impl CtrlState {
    /// Construct a `CtrlState` from v7-compatible fields, filling the new
    /// v8 slot-aware fields with their defaults. Callers that need to
    /// populate `slot_ids`, `revision`, and `active_slot_id` should use
    /// the struct literal directly.
    pub fn v7(status: PlayerStatus, items: Vec<MediaItem>, cursor: usize, source: QueueSource) -> Self {
        Self {
            status,
            items,
            cursor,
            source,
            slot_ids: Vec::new(),
            revision: QueueRevision::default(),
            active_slot_id: None,
        }
    }
}
