use serde::{Deserialize, Serialize};

use crate::config::{QueueSource, ServiceKind};
use crate::playback_queue::{QueueItem, QueueSlotId};
use crate::player::{PlayerCommand, PlayerEvent, PlayerStatus};

/// Bump ONLY when an old peer would misbehave, not when it would merely
/// fail to understand. Compatibility is exact-match, so every bump kills
/// all running daemons until the user runs `mbv -q`.
///
/// No bump -- advertise an optional capability instead:
///   - a new `CtrlCmd` variant (unknown commands are skipped, see daemon_core)
///   - a new `CtrlEvent` variant (unknown events are logged and ignored)
///   - a new `#[serde(default)]` field on an existing message
///
/// Bump -- an old peer parses the message and acts on it wrongly:
///   - renaming or removing a field or variant
///   - changing the meaning, units, or nullability of an existing field
///   - changing handshake order or framing
///
/// Version 9 intentionally bumps for the removed legacy `auth_token` hello
/// field and the resulting credential-free packaged-daemon handshake. The
/// source previously declared v7 while the archived v8 contract described v8;
/// that pre-existing drift is reconciled here by shipping v9 and rejecting
/// every other version.
///
/// Version 10 bumps for the EmbyItem wire-shape change: `director`/`genre`
/// (required on v9 peers) were replaced by defaultable `genres`/`people`/
/// `external_urls`. A v9 peer drops every UnifiedQueue* command from a v10
/// client because the removed required fields fail deserialization — silently,
/// since undeserializable ctrl lines are skipped without a log.
pub const CTRL_PROTOCOL_VERSION: u32 = 11;
pub const CTRL_CAP_QUEUE_STATE: &str = "queue-state";
pub const CTRL_CAP_START_INDEX: &str = "play-items-start-idx";
pub const CTRL_CAP_STATUS_ONLY: &str = "status-only";
pub const CTRL_CAP_LIFECYCLE_SHUTDOWN: &str = "lifecycle-shutdown";
/// Daemon and client exchange item-generic unified queue state and operations.
/// The only queue shape; the legacy `CtrlState`/`PlayItems`/`AdoptQueue`/
/// `LoadFeed` split-item shapes were removed (ADR 0020).
pub const CTRL_CAP_UNIFIED_QUEUE: &str = "unified-queue";
/// Capable Local daemons authenticate ctrl clients with their mbv-owned
/// Control credential rather than an Emby Service credential.
pub const CTRL_CAP_CONTROL_AUTH: &str = "control-auth";
/// Peer can decode `QueueItem::Audiobookshelf` in unified queue commands,
/// snapshots, and broadcasts. Static protocol support only — does not imply
/// a daemon owner is eligible to bind or play the item. Additive — no
/// protocol-version bump.
pub const CTRL_CAP_ABS_QUEUE: &str = "abs-queue";
/// Peer can receive the redacted provider-qualified Audiobookshelf progress
/// event. Additive — no protocol-version bump.
pub const CTRL_CAP_ABS_PROGRESS: &str = "abs-progress";
/// Peer can decode `QueueItem::AudiobookshelfBook` in unified queue commands,
/// snapshots, and broadcasts. Static protocol support only — does not imply
/// a daemon owner is eligible to bind or play the item. Additive — no
/// protocol-version bump.
pub const CTRL_CAP_ABS_BOOK_QUEUE: &str = "abs-book-queue";
/// Peer can receive the redacted provider-qualified Audiobookshelf book
/// progress event. Additive — no protocol-version bump.
pub const CTRL_CAP_ABS_BOOK_PROGRESS: &str = "abs-book-progress";
/// Peer owner is configured audio-only and cannot play video. Additive — no
/// protocol-version bump.
pub const CTRL_CAP_AUDIO_ONLY: &str = "audio-only";
/// Peer supports owner-authoritative idle queue loads and source updates.
/// Additive — no protocol-version bump.
pub const CTRL_CAP_OWNER_QUEUE_LOAD: &str = "owner-queue-load";

pub type PlaybackRequestId = u64;
pub type QueueLoadRequestId = u64;
/// Opaque owner-minted replacement lineage carried by queue snapshots and
/// source-only updates.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueueLineage(pub u64);
pub type PlaybackGeneration = u64;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CtrlHello {
    pub protocol_version: u32,
    pub app_version: String,
    pub capabilities: Vec<String>,
    /// Control credential used only when the peer advertises `control-auth`.
    #[serde(default)]
    pub control_token: Option<String>,
}

impl CtrlHello {
    pub fn current() -> Self {
        Self {
            protocol_version: CTRL_PROTOCOL_VERSION,
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            capabilities: vec![
                CTRL_CAP_QUEUE_STATE.to_string(),
                CTRL_CAP_START_INDEX.to_string(),
                CTRL_CAP_STATUS_ONLY.to_string(),
                CTRL_CAP_LIFECYCLE_SHUTDOWN.to_string(),
                CTRL_CAP_UNIFIED_QUEUE.to_string(),
                CTRL_CAP_CONTROL_AUTH.to_string(),
                CTRL_CAP_ABS_QUEUE.to_string(),
                CTRL_CAP_ABS_PROGRESS.to_string(),
                CTRL_CAP_ABS_BOOK_QUEUE.to_string(),
                CTRL_CAP_ABS_BOOK_PROGRESS.to_string(),
                CTRL_CAP_OWNER_QUEUE_LOAD.to_string(),
            ],
            control_token: None,
        }
    }

    pub fn current_control_client(control_token: String) -> Self {
        let mut hello = Self::current();
        hello.control_token = Some(control_token);
        hello
    }

    pub fn validate_peer(&self) -> Result<(), String> {
        self.compatibility()?;
        self.validate_required_capabilities()
    }

    pub fn compatibility(&self) -> Result<CtrlCompatibility, String> {
        CtrlCompatibility::for_peer(self.protocol_version)
    }

    fn validate_required_capabilities(&self) -> Result<(), String> {
        for required in [
            CTRL_CAP_QUEUE_STATE,
            CTRL_CAP_START_INDEX,
            CTRL_CAP_STATUS_ONLY,
        ] {
            if !self.capabilities.iter().any(|cap| cap == required) {
                return Err(format!(
                    "peer missing daemon protocol capability: {required}"
                ));
            }
        }
        Ok(())
    }

    pub fn supports_lifecycle_shutdown(&self) -> bool {
        self.capabilities
            .iter()
            .any(|cap| cap == CTRL_CAP_LIFECYCLE_SHUTDOWN)
    }

    pub fn supports_audio_only(&self) -> bool {
        self.capabilities
            .iter()
            .any(|cap| cap == CTRL_CAP_AUDIO_ONLY)
    }

    pub fn supports_control_auth(&self) -> bool {
        self.capabilities
            .iter()
            .any(|cap| cap == CTRL_CAP_CONTROL_AUTH)
    }

    pub fn supports_abs_queue(&self) -> bool {
        self.capabilities
            .iter()
            .any(|cap| cap == CTRL_CAP_ABS_QUEUE)
    }

    pub fn supports_abs_progress(&self) -> bool {
        self.capabilities
            .iter()
            .any(|cap| cap == CTRL_CAP_ABS_PROGRESS)
    }

    pub fn supports_abs_book_queue(&self) -> bool {
        self.capabilities
            .iter()
            .any(|cap| cap == CTRL_CAP_ABS_BOOK_QUEUE)
    }

    pub fn supports_abs_book_progress(&self) -> bool {
        self.capabilities
            .iter()
            .any(|cap| cap == CTRL_CAP_ABS_BOOK_PROGRESS)
    }

    pub fn supports_owner_queue_load(&self) -> bool {
        self.capabilities
            .iter()
            .any(|cap| cap == CTRL_CAP_OWNER_QUEUE_LOAD)
    }

    pub fn validate_control_credential(&self, expected: &str) -> Result<(), String> {
        let Some(presented) = self.control_token.as_deref() else {
            return Err("invalid Control credential".to_string());
        };
        if presented.len() == expected.len()
            && presented
                .as_bytes()
                .iter()
                .zip(expected.as_bytes())
                .fold(0u8, |difference, (&presented, &expected)| {
                    difference | (presented ^ expected)
                })
                == 0
        {
            Ok(())
        } else {
            Err("invalid Control credential".to_string())
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CtrlCompatibility {
    pub peer_protocol_version: u32,
    pub client_protocol_version: u32,
    pub supports_queue_append: bool,
    pub supports_lifecycle_shutdown: bool,
    pub supports_audio_only: bool,
    pub supports_control_auth: bool,
    pub supports_abs_queue: bool,
    pub supports_abs_progress: bool,
    pub supports_abs_book_queue: bool,
    pub supports_abs_book_progress: bool,
    pub supports_owner_queue_load: bool,
}

impl CtrlCompatibility {
    pub fn for_peer(peer_protocol_version: u32) -> Result<Self, String> {
        match peer_protocol_version {
            CTRL_PROTOCOL_VERSION => Ok(Self {
                peer_protocol_version,
                client_protocol_version: CTRL_PROTOCOL_VERSION,
                supports_queue_append: true,
                supports_lifecycle_shutdown: false,
                supports_audio_only: false,
                supports_control_auth: true,
                supports_abs_queue: true,
                supports_abs_progress: true,
                supports_abs_book_queue: true,
                supports_abs_book_progress: true,
                supports_owner_queue_load: false,
            }),
            _ => Err(format!(
                "incompatible daemon protocol version: peer={peer_protocol_version} local={CTRL_PROTOCOL_VERSION}"
            )),
        }
    }

    pub fn current() -> Self {
        Self::for_peer(CTRL_PROTOCOL_VERSION).expect("local ctrl protocol version is compatible")
    }
}

// ── Unified queue wire types ──────────────────────────────────────────────

/// One slot in the unified queue representation.  The `slot_id` is the
/// stable runtime identity of the occurrence (not the item's content ID).
#[derive(Clone, Serialize, Deserialize)]
pub struct UnifiedQueueSlot {
    pub slot_id: u64,
    pub item: QueueItem,
}

/// Summary of a pending playback transition carried in the owner snapshot
/// (design D5). Kept independent of internal queue types — the target slot is
/// a raw u64, matching `UnifiedQueueSlot::slot_id`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransitionSummary {
    pub request_id: PlaybackRequestId,
    pub generation: PlaybackGeneration,
    pub target_slot: u64,
}

/// Full queue state exchanged between unified-queue-capable peers.
#[derive(Clone, Serialize, Deserialize)]
pub struct UnifiedQueueStateData {
    pub status: PlayerStatus,
    pub slots: Vec<UnifiedQueueSlot>,
    /// `None` when nothing is playing.
    pub active_slot: Option<u64>,
    pub revision: u64,
    #[serde(default)]
    pub source: QueueSource,
    /// Owner-minted identity for the current whole-queue replacement lineage.
    #[serde(default)]
    pub lineage: QueueLineage,
    /// Transition dispatched to the Playback run and awaiting observation
    /// (design D5). `None` when no transition is in flight.
    #[serde(default)]
    pub in_flight_transition: Option<TransitionSummary>,
    /// Newest queued transition held back behind the in-flight one (design
    /// D4/D5). `None` when nothing is queued.
    #[serde(default)]
    pub queued_latest_transition: Option<TransitionSummary>,
}

/// Build an `UnifiedQueueSlot` from a `QueueSlotId`.  Callers in
/// `mbv-core` can use this at the daemon boundary; the `From` trait is
/// not exposed to keep the wire type independent of internal queue types.
pub fn slot_id_to_u64(id: QueueSlotId) -> u64 {
    id.raw()
}

// ── CtrlCmd ──────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize)]
pub enum CtrlCmd {
    Hello(CtrlHello),
    PlayerCmd(WireCommand),
    Stop,
    /// Correlated playback control. This is intentionally separate from
    /// `PlayerCmd` so guarded actions cannot silently fall back to the old,
    /// unacknowledged command path.
    PlaybackIntent(PlaybackIntent),
    /// Daemon lifecycle request: coordinated shutdown with durable queue
    /// persistence. Distinct from the player `Stop` command. Only accepted
    /// from local Unix ctrl connections.
    RequestShutdown,

    /// Reread a committed owner-local Service setup. Only packaged Unix ctrl
    /// accepts this command; setup values and credentials never cross ctrl.
    ApplyServiceSetup {
        kind: ServiceKind,
        revision: u64,
    },

    // ── Unified queue commands (require `unified-queue` capability) ─────
    /// Replace the entire queue with item-generic slots and optionally
    /// begin playback from `start_idx`.
    UnifiedQueueReplace {
        /// Legacy item-only representation retained so older peers can still
        /// decode a replacement. New peers use `slots` for stable identities.
        items: Vec<QueueItem>,
        /// Owner-assigned slot identities. Absent on legacy payloads.
        #[serde(default)]
        slots: Vec<UnifiedQueueSlot>,
        start_idx: Option<usize>,
        #[serde(default)]
        source: QueueSource,
    },
    /// Replace the owner's queue without starting playback. Requires the
    /// `owner-queue-load` capability and is correlated by request identity.
    #[serde(rename = "UnifiedQueueLoadIdle")]
    UnifiedQueueLoadIdle {
        request_id: QueueLoadRequestId,
        slots: Vec<UnifiedQueueSlot>,
        cursor: usize,
        source: QueueSource,
    },
    /// Update only the source of the owner queue if its lineage still matches.
    #[serde(rename = "UnifiedQueueSourceUpdate")]
    UnifiedQueueSourceUpdate {
        source: QueueSource,
        lineage: QueueLineage,
    },
    /// Append item-generic values to the tail of the queue.
    UnifiedQueueAppend {
        items: Vec<QueueItem>,
    },
    /// Remove the slot identified by `slot_id`.
    UnifiedQueueRemoveSlot {
        slot_id: u64,
    },
    /// Remove every listed slot as one queue edit. The owner applies all of
    /// them before publishing a single queue snapshot, so a bulk removal is
    /// never observable as a sequence of shrinking queues. Unknown slot
    /// identities are skipped; an empty list is a no-op.
    UnifiedQueueRemoveSlots {
        slot_ids: Vec<u64>,
    },
    /// Move the slot identified by `slot_id` to `to_index`.
    UnifiedQueueMoveSlot {
        slot_id: u64,
        to_index: usize,
    },
    /// Begin playback of an existing slot identified by `slot_id`.
    UnifiedQueuePlaySlot {
        slot_id: u64,
    },
    /// Clear all slots and stop playback.
    UnifiedQueueClear,
    /// Seeds queue state on a cold daemon (no queue yet) without starting
    /// playback.
    UnifiedAdoptQueue {
        items: Vec<QueueItem>,
        cursor: usize,
        source: QueueSource,
    },
}

/// Reply a gated command's `OwnerGate` failure sends. Carried by
/// [`OwnerGate::OwnerOnly`]/[`OwnerGate::NonOwnerOnly`] so
/// `send_role_gate_rejection` (daemon_control.rs) can match on it
/// exhaustively with no wildcard arm.
pub enum OwnerGateRejection {
    AdoptQueue,
    QueueLoadIdle { request_id: QueueLoadRequestId },
    QueueSourceUpdate,
}

/// Owner-role gate for a ctrl command: whether acceptance depends on the
/// daemon being the Stay-alive owner (`DaemonRole::Local`).
pub enum OwnerGate {
    /// Accepted only from the owner.
    OwnerOnly(OwnerGateRejection),
    /// Accepted only from a non-owner (e.g. a Client adopting a cold
    /// daemon's queue).
    NonOwnerOnly(OwnerGateRejection),
    /// No role gate.
    Any,
}

impl CtrlCmd {
    /// Owner-role gate for commands whose acceptance depends on whether the
    /// daemon is the Stay-alive owner (`DaemonRole::Local`).
    pub fn requires_owner(&self) -> OwnerGate {
        match self {
            CtrlCmd::UnifiedQueueLoadIdle { request_id, .. } => {
                OwnerGate::OwnerOnly(OwnerGateRejection::QueueLoadIdle {
                    request_id: *request_id,
                })
            }
            CtrlCmd::UnifiedQueueSourceUpdate { .. } => {
                OwnerGate::OwnerOnly(OwnerGateRejection::QueueSourceUpdate)
            }
            CtrlCmd::UnifiedAdoptQueue { .. } => {
                OwnerGate::NonOwnerOnly(OwnerGateRejection::AdoptQueue)
            }
            CtrlCmd::Hello(_)
            | CtrlCmd::PlayerCmd(_)
            | CtrlCmd::Stop
            | CtrlCmd::PlaybackIntent(_)
            | CtrlCmd::RequestShutdown
            | CtrlCmd::ApplyServiceSetup { .. }
            | CtrlCmd::UnifiedQueueReplace { .. }
            | CtrlCmd::UnifiedQueueAppend { .. }
            | CtrlCmd::UnifiedQueueRemoveSlot { .. }
            | CtrlCmd::UnifiedQueueRemoveSlots { .. }
            | CtrlCmd::UnifiedQueueMoveSlot { .. }
            | CtrlCmd::UnifiedQueuePlaySlot { .. }
            | CtrlCmd::UnifiedQueueClear => OwnerGate::Any,
        }
    }

    /// Whether the owner must persist its queue after handling this command.
    /// Exhaustive so a new queue-editing command cannot silently skip
    /// persistence.
    pub fn mutates_owner_queue(&self) -> bool {
        match self {
            CtrlCmd::UnifiedQueueLoadIdle { .. }
            | CtrlCmd::UnifiedQueueSourceUpdate { .. }
            | CtrlCmd::UnifiedQueueReplace { .. }
            | CtrlCmd::UnifiedQueueAppend { .. }
            | CtrlCmd::UnifiedQueueRemoveSlot { .. }
            | CtrlCmd::UnifiedQueueRemoveSlots { .. }
            | CtrlCmd::UnifiedQueueMoveSlot { .. }
            | CtrlCmd::UnifiedQueueClear => true,
            CtrlCmd::Hello(_)
            | CtrlCmd::PlayerCmd(_)
            | CtrlCmd::Stop
            | CtrlCmd::PlaybackIntent(_)
            | CtrlCmd::RequestShutdown
            | CtrlCmd::ApplyServiceSetup { .. }
            | CtrlCmd::UnifiedQueuePlaySlot { .. }
            | CtrlCmd::UnifiedAdoptQueue { .. } => false,
        }
    }

    /// Builds `UnifiedQueueReplace`, deriving the legacy `items` payload from
    /// `slots` so callers don't each re-project the same list.
    pub fn unified_queue_replace(
        slots: Vec<UnifiedQueueSlot>,
        start_idx: Option<usize>,
        source: QueueSource,
    ) -> Self {
        CtrlCmd::UnifiedQueueReplace {
            items: slots.iter().map(|slot| slot.item.clone()).collect(),
            slots,
            start_idx,
            source,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlaybackIntent {
    pub request_id: PlaybackRequestId,
    pub generation: PlaybackGeneration,
    pub action: PlaybackIntentAction,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum PlaybackIntentAction {
    Play {
        item_ids: Vec<String>,
        start_idx: usize,
        start_ticks: i64,
        source: QueueSource,
    },
    Stop,
    SetPaused {
        paused: bool,
    },
    Next,
    Previous,
}

/// Wire-stable representation of a `PlayerCommand`, serialized across the
/// daemon/TUI process seam. Kept as a distinct type (rather than serializing
/// `PlayerCommand` directly) so that renaming or restructuring in-process
/// player commands cannot silently change the wire protocol: every variant
/// here has an explicit, pinned `serde(rename)` tag, and the conversions
/// to/from `PlayerCommand` are exhaustive matches with no wildcard arm, so
/// adding a new `PlayerCommand` variant is a compile error until this type
/// (and its conversions) are updated too.
#[derive(Serialize, Deserialize)]
pub enum WireCommand {
    #[serde(rename = "TogglePause")]
    TogglePause,
    #[serde(rename = "JumpTo")]
    JumpTo(usize),
    #[serde(rename = "SetVolume")]
    SetVolume(i64),
    #[serde(rename = "Seek")]
    Seek(f64),
    #[serde(rename = "SeekAbsolute")]
    SeekAbsolute(f64),
    #[serde(rename = "SetAudio")]
    SetAudio(i64),
    #[serde(rename = "SetSub")]
    SetSub(i64),
    #[serde(rename = "SetSubtitlePrefs")]
    SetSubtitlePrefs {
        mode: String,
        subtitle_lang: String,
        audio_lang: String,
    },
    #[serde(rename = "SetMute")]
    SetMute(bool),
    #[serde(rename = "NextUpShow")]
    NextUpShow {
        item_id: String,
        show_title: String,
        ep_title: String,
        artist: String,
    },
    #[serde(rename = "NextUpDismiss")]
    NextUpDismiss,
    #[serde(rename = "SkipIntroDismiss")]
    SkipIntroDismiss,
}

impl WireCommand {
    /// Encode a `PlayerCommand` for the ctrl wire. Local-only commands have
    /// no wire form and are refused: slot-addressed queue mutation crosses
    /// exclusively as `CtrlCmd::UnifiedQueue*`, `SubmitQueue` is resolved
    /// before send, and `QueueAppend`/`LoadNew` have no legacy wire form. The
    /// unencodable command is returned to the caller, which reports the
    /// refusal — encoding must never terminate the process.
    pub fn try_from_player_command(cmd: PlayerCommand) -> Result<Self, PlayerCommand> {
        match cmd {
            PlayerCommand::TogglePause => Ok(WireCommand::TogglePause),
            PlayerCommand::SetVolume(v) => Ok(WireCommand::SetVolume(v)),
            PlayerCommand::Seek(s) => Ok(WireCommand::Seek(s)),
            PlayerCommand::SeekAbsolute(s) => Ok(WireCommand::SeekAbsolute(s)),
            PlayerCommand::SetAudio(i) => Ok(WireCommand::SetAudio(i)),
            PlayerCommand::SetSub(i) => Ok(WireCommand::SetSub(i)),
            PlayerCommand::SetSubtitlePrefs {
                mode,
                subtitle_lang,
                audio_lang,
            } => Ok(WireCommand::SetSubtitlePrefs {
                mode,
                subtitle_lang,
                audio_lang,
            }),
            PlayerCommand::SetMute(m) => Ok(WireCommand::SetMute(m)),
            PlayerCommand::NextUpShow {
                item_id,
                show_title,
                ep_title,
                artist,
            } => Ok(WireCommand::NextUpShow {
                item_id,
                show_title,
                ep_title,
                artist,
            }),
            PlayerCommand::NextUpDismiss => Ok(WireCommand::NextUpDismiss),
            PlayerCommand::SkipIntroDismiss => Ok(WireCommand::SkipIntroDismiss),
            PlayerCommand::QueueAppend { .. }
            | PlayerCommand::QueueRemove(_)
            | PlayerCommand::QueueMove(..)
            | PlayerCommand::JumpTo { .. }
            | PlayerCommand::LoadNew { .. }
            | PlayerCommand::Next
            | PlayerCommand::Previous
            | PlayerCommand::SubmitQueue { .. } => Err(cmd),
        }
    }
}

impl From<WireCommand> for PlayerCommand {
    fn from(cmd: WireCommand) -> Self {
        match cmd {
            WireCommand::TogglePause => PlayerCommand::TogglePause,
            WireCommand::JumpTo(_) => unreachable!(
                "inbound WireCommand::JumpTo is rejected via CommandRejected in daemon_control before conversion (design D6)"
            ),
            WireCommand::SetVolume(v) => PlayerCommand::SetVolume(v),
            WireCommand::Seek(s) => PlayerCommand::Seek(s),
            WireCommand::SeekAbsolute(s) => PlayerCommand::SeekAbsolute(s),
            WireCommand::SetAudio(i) => PlayerCommand::SetAudio(i),
            WireCommand::SetSub(i) => PlayerCommand::SetSub(i),
            WireCommand::SetSubtitlePrefs {
                mode,
                subtitle_lang,
                audio_lang,
            } => PlayerCommand::SetSubtitlePrefs {
                mode,
                subtitle_lang,
                audio_lang,
            },
            WireCommand::SetMute(m) => PlayerCommand::SetMute(m),
            WireCommand::NextUpShow {
                item_id,
                show_title,
                ep_title,
                artist,
            } => PlayerCommand::NextUpShow {
                item_id,
                show_title,
                ep_title,
                artist,
            },
            WireCommand::NextUpDismiss => PlayerCommand::NextUpDismiss,
            WireCommand::SkipIntroDismiss => PlayerCommand::SkipIntroDismiss,
        }
    }
}

#[derive(Serialize, Deserialize)]
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

#[cfg(test)]
mod tests;
