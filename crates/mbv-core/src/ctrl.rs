use serde::{Deserialize, Serialize};

use crate::api::EmbyItem;
use crate::config::QueueSource;
use crate::playback_queue::{FeedEntry, QueueItem, QueueSlotId};
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
pub const CTRL_PROTOCOL_VERSION: u32 = 7;
pub const CTRL_CAP_QUEUE_STATE: &str = "queue-state";
pub const CTRL_CAP_START_INDEX: &str = "play-items-start-idx";
pub const CTRL_CAP_STATUS_ONLY: &str = "status-only";
pub const CTRL_CAP_LIFECYCLE_SHUTDOWN: &str = "lifecycle-shutdown";
pub const CTRL_CAP_SHARED_MBV_STATE: &str = "shared-mbv-state-v1";
/// Daemon supports playing feed entries (RSS/podcast/YouTube) via the legacy `LoadFeed` wire command.
pub const CTRL_CAP_FEED_PLAYBACK: &str = "feed-playback";
/// Daemon and client exchange item-generic unified queue state and operations.
/// Capable peers use `UnifiedQueueState` and unified-queue `CtrlCmd` variants;
/// legacy peers use `CtrlState`, `PlayItems`, `AdoptQueue`, and `PlayerCmd`
/// queue mutations.  Additive — no protocol-version bump.
pub const CTRL_CAP_UNIFIED_QUEUE: &str = "unified-queue";
/// Capable Local daemons authenticate ctrl clients with their mbv-owned
/// Control credential rather than an Emby Service credential.
pub const CTRL_CAP_CONTROL_AUTH: &str = "control-auth";

pub type PlaybackRequestId = u64;
pub type PlaybackGeneration = u64;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CtrlHello {
    pub protocol_version: u32,
    pub app_version: String,
    pub capabilities: Vec<String>,
    pub auth_token: Option<String>,
    /// Control credential used only when the peer advertises `control-auth`.
    /// Defaulting this field keeps deferred legacy peers wire-compatible.
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
                CTRL_CAP_FEED_PLAYBACK.to_string(),
                CTRL_CAP_UNIFIED_QUEUE.to_string(),
                CTRL_CAP_CONTROL_AUTH.to_string(),
            ],
            auth_token: None,
            control_token: None,
        }
    }

    pub fn current_client(auth_token: String) -> Self {
        let mut hello = Self::current();
        hello.auth_token = Some(auth_token);
        hello
    }

    pub fn current_control_client(control_token: String) -> Self {
        let mut hello = Self::current();
        hello.control_token = Some(control_token);
        hello
    }

    pub fn compatible_client(auth_token: String, compatibility: CtrlCompatibility) -> Self {
        let mut hello = Self::current_client(auth_token);
        hello.protocol_version = compatibility.client_protocol_version;
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

    pub fn supports_feed_playback(&self) -> bool {
        self.capabilities
            .iter()
            .any(|cap| cap == CTRL_CAP_FEED_PLAYBACK)
    }

    pub fn supports_unified_queue(&self) -> bool {
        self.capabilities
            .iter()
            .any(|cap| cap == CTRL_CAP_UNIFIED_QUEUE)
    }

    pub fn supports_control_auth(&self) -> bool {
        self.capabilities
            .iter()
            .any(|cap| cap == CTRL_CAP_CONTROL_AUTH)
    }

    pub fn validate_control_credential(&self, expected: &str) -> Result<(), String> {
        if self.control_token.as_deref() == Some(expected) {
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
    pub supports_feed_playback: bool,
    pub supports_unified_queue: bool,
    pub supports_control_auth: bool,
}

impl CtrlCompatibility {
    pub fn for_peer(peer_protocol_version: u32) -> Result<Self, String> {
        match peer_protocol_version {
            CTRL_PROTOCOL_VERSION => Ok(Self {
                peer_protocol_version,
                client_protocol_version: CTRL_PROTOCOL_VERSION,
                supports_queue_append: true,
                supports_lifecycle_shutdown: false,
                supports_feed_playback: true,
                supports_unified_queue: true,
                supports_control_auth: true,
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
// Used only when both peers advertise `unified-queue` capability.
// Legacy peers use the old `CtrlState` / `PlayItems` / `AdoptQueue` shapes.

/// One slot in the unified queue representation.  The `slot_id` is the
/// stable runtime identity of the occurrence (not the item's content ID).
#[derive(Clone, Serialize, Deserialize)]
pub struct UnifiedQueueSlot {
    pub slot_id: u64,
    pub item: QueueItem,
}

/// Full queue state exchanged between unified-queue-capable peers.
/// Replaces the legacy `CtrlState` (which split items and feed_items)
/// for capable connections.
#[derive(Clone, Serialize, Deserialize)]
pub struct UnifiedQueueStateData {
    pub status: PlayerStatus,
    pub slots: Vec<UnifiedQueueSlot>,
    /// `None` when nothing is playing.
    pub active_slot: Option<u64>,
    pub revision: u64,
    #[serde(default)]
    pub source: QueueSource,
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
    /// **Legacy compatibility.** Replaced by `UnifiedQueueReplace` for
    /// peers advertising `unified-queue`.  Kept so older clients can
    /// still adopt a queue the daemon already holds.
    AdoptQueue {
        items: Vec<EmbyItem>,
        cursor: usize,
        source: QueueSource,
    },
    /// **Legacy compatibility.** Replaced by `UnifiedQueueReplace` for
    /// peers advertising `unified-queue`.  Kept so older clients can
    /// still start playback by Emby item ID.
    PlayItems {
        item_ids: Vec<String>,
        start_idx: usize,
        start_ticks: i64,
        source: QueueSource,
    },
    Stop,
    /// Correlated playback control. This is intentionally separate from
    /// `PlayerCmd` so guarded actions cannot silently fall back to the old,
    /// unacknowledged command path.
    PlaybackIntent(PlaybackIntent),
    /// Daemon lifecycle request: coordinated shutdown with durable queue
    /// persistence. Distinct from the player `Stop` command. Only accepted
    /// from local Unix ctrl connections.
    RequestShutdown,

    // ── Unified queue commands (require `unified-queue` capability) ─────
    /// Replace the entire queue with item-generic slots and optionally
    /// begin playback from `start_idx`.
    UnifiedQueueReplace {
        items: Vec<QueueItem>,
        start_idx: Option<usize>,
    },
    /// Append item-generic values to the tail of the queue.
    UnifiedQueueAppend {
        items: Vec<QueueItem>,
    },
    /// Remove the slot identified by `slot_id`.
    UnifiedQueueRemoveSlot {
        slot_id: u64,
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
    /// Unified-capable counterpart to the legacy `AdoptQueue`: seeds queue
    /// state on a cold daemon (no queue yet) without starting playback.
    UnifiedAdoptQueue {
        items: Vec<QueueItem>,
        cursor: usize,
        source: QueueSource,
    },
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
    #[serde(rename = "QueueAppend")]
    QueueAppend { items: Vec<EmbyItem> },
    #[serde(rename = "PlaylistRemove")]
    QueueRemove(usize),
    #[serde(rename = "PlaylistMove")]
    QueueMove(usize, usize),
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
    #[serde(rename = "LoadNew")]
    LoadNew {
        url: String,
        start_pos: f64,
        item: Box<EmbyItem>,
    },
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
    #[serde(rename = "ReplacePlaylist")]
    ReplaceQueue {
        items: Vec<EmbyItem>,
        start_idx: usize,
    },
    /// **Legacy wire-compat.** Play a single feed entry. Additive — requires `feed-playback` capability.
    /// Decoded at the daemon boundary into the unified queue path; current code never emits this.
    #[serde(rename = "LoadFeed")]
    LoadFeed { entry: FeedEntry },
}

impl From<PlayerCommand> for WireCommand {
    fn from(cmd: PlayerCommand) -> Self {
        match cmd {
            PlayerCommand::TogglePause => WireCommand::TogglePause,
            PlayerCommand::JumpTo(idx) => WireCommand::JumpTo(idx),
            PlayerCommand::QueueAppend { items } => WireCommand::QueueAppend {
                items: items
                    .iter()
                    .filter_map(|qi| qi.as_emby().cloned())
                    .collect(),
            },
            PlayerCommand::QueueRemove(idx) => WireCommand::QueueRemove(idx),
            PlayerCommand::QueueMove(from, to) => WireCommand::QueueMove(from, to),
            PlayerCommand::SetVolume(v) => WireCommand::SetVolume(v),
            PlayerCommand::Seek(s) => WireCommand::Seek(s),
            PlayerCommand::SeekAbsolute(s) => WireCommand::SeekAbsolute(s),
            PlayerCommand::SetAudio(i) => WireCommand::SetAudio(i),
            PlayerCommand::SetSub(i) => WireCommand::SetSub(i),
            PlayerCommand::SetSubtitlePrefs {
                mode,
                subtitle_lang,
                audio_lang,
            } => WireCommand::SetSubtitlePrefs {
                mode,
                subtitle_lang,
                audio_lang,
            },
            PlayerCommand::SetMute(m) => WireCommand::SetMute(m),
            PlayerCommand::LoadNew {
                url,
                start_pos,
                item,
            } => WireCommand::LoadNew {
                url,
                start_pos,
                item,
            },
            PlayerCommand::NextUpShow {
                item_id,
                show_title,
                ep_title,
                artist,
            } => WireCommand::NextUpShow {
                item_id,
                show_title,
                ep_title,
                artist,
            },
            PlayerCommand::NextUpDismiss => WireCommand::NextUpDismiss,
            PlayerCommand::SkipIntroDismiss => WireCommand::SkipIntroDismiss,
            PlayerCommand::ReplaceQueue { items, start_idx } => {
                WireCommand::ReplaceQueue { items, start_idx }
            }
            // SubmitQueue is a local-only command — it is never serialized to the wire.
            PlayerCommand::SubmitQueue { .. } => {
                unreachable!("SubmitQueue is local-only; never sent over ctrl")
            }
        }
    }
}

impl From<WireCommand> for PlayerCommand {
    fn from(cmd: WireCommand) -> Self {
        match cmd {
            WireCommand::TogglePause => PlayerCommand::TogglePause,
            WireCommand::JumpTo(idx) => PlayerCommand::JumpTo(idx),
            WireCommand::QueueAppend { items } => PlayerCommand::QueueAppend {
                items: items
                    .into_iter()
                    .map(|e| QueueItem::Emby(Box::new(e)))
                    .collect(),
            },
            WireCommand::QueueRemove(idx) => PlayerCommand::QueueRemove(idx),
            WireCommand::QueueMove(from, to) => PlayerCommand::QueueMove(from, to),
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
            WireCommand::LoadNew {
                url,
                start_pos,
                item,
            } => PlayerCommand::LoadNew {
                url,
                start_pos,
                item,
            },
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
            WireCommand::ReplaceQueue { items, start_idx } => {
                PlayerCommand::ReplaceQueue { items, start_idx }
            }
            // LoadFeed is intercepted at the daemon control boundary
            // before reaching this conversion; it routes through the
            // unified queue path instead of PlayerCommand::LoadFeed.
            WireCommand::LoadFeed { .. } => {
                unreachable!(
                    "LoadFeed is intercepted at daemon boundary; never converted to PlayerCommand"
                )
            }
        }
    }
}

#[derive(Serialize, Deserialize)]
pub enum CtrlEvent {
    Hello(CtrlHello),
    Player(PlayerEvent),
    /// **Legacy compatibility.** Sent to peers that do not advertise
    /// `unified-queue`.  Replaced by `UnifiedQueueState` for capable
    /// connections.  Never emitted on a unified-queue session.
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
    /// The daemon accepted a coordinated shutdown request after durably
    /// persisting its authoritative queue.
    ShutdownAccepted,
    /// The daemon rejected a coordinated shutdown request. The reason
    /// indicates why (e.g. TCP transport, persistence failure).
    ShutdownRejected {
        reason: String,
    },

    // ── Unified queue events (require `unified-queue` capability) ───────
    /// Full item-generic queue state.  Sent on initial connection and
    /// after every queue mutation to peers advertising `unified-queue`.
    /// Legacy peers receive `CtrlState` instead.
    UnifiedQueueState(UnifiedQueueStateData),
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

/// **Legacy compatibility.** Split-item queue state sent to peers that
/// do not advertise `unified-queue`.  Replaced by `UnifiedQueueStateData`
/// for capable connections.  The `items` and `feed_items` split is
/// preserved here for backward wire compatibility only; the daemon's
/// internal authority is a single `PlaybackQueue<QueueItem>`.
#[derive(Serialize, Deserialize)]
pub struct CtrlState {
    pub status: PlayerStatus,
    pub items: Vec<EmbyItem>,
    pub cursor: usize,
    pub source: QueueSource,
    /// Live feed entries at the tail of the queue, after all Emby items.
    /// Additive: legacy clients that don't know this field will deserialize
    /// it as an empty vec (serde `default`). Capable clients (advertising
    /// `feed-playback`) use it to render and manage feed entries.
    #[serde(default)]
    pub feed_items: Vec<FeedEntry>,
}

#[cfg(test)]
mod tests {
    include!("ctrl_tests.rs");
}
