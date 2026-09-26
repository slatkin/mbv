use serde::{Deserialize, Serialize};

use crate::config::QueueSource;
use crate::playback_queue::{QueueItem, QueueSlotId};
use crate::player::PlayerStatus;

/// Bump ONLY when an old peer would misbehave, not when it would merely
/// fail to understand. Compatibility is exact-match, so every bump kills
/// all running daemons until the user runs `mbv -q`.
///
/// No bump -- advertise an optional capability instead:
///   - a new `CtrlCmd` variant (unknown commands are skipped, see `daemon_core`)
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
/// Version 10 bumps for the `EmbyItem` wire-shape change: `director`/`genre`
/// (required on v9 peers) were replaced by defaultable `genres`/`people`/
/// `external_urls`. A v9 peer drops every `UnifiedQueue`* command from a v10
/// client because the removed required fields fail deserialization — silently,
/// since undeserializable ctrl lines are skipped without a log.
///
/// Version 11 bumps for the `run_identity` tuple→scalar wire-shape change in
/// `PlayerEvent::Stopped`/`TrackCompleted`; a v10 peer's `[0, gen]` array fails
/// to deserialize (the field is dropped silently by serde default) so a
/// mismatched pair must be rejected at handshake.
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
/// Peer can decode the Audiobookshelf book shape in unified queue commands,
/// snapshots, and broadcasts. Static protocol support only — does not imply a
/// daemon owner is eligible to bind or play the item. Additive — no
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
    #[must_use]
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

    #[must_use]
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

    #[must_use]
    pub fn supports_lifecycle_shutdown(&self) -> bool {
        self.capabilities
            .iter()
            .any(|cap| cap == CTRL_CAP_LIFECYCLE_SHUTDOWN)
    }

    #[must_use]
    pub fn supports_audio_only(&self) -> bool {
        self.capabilities
            .iter()
            .any(|cap| cap == CTRL_CAP_AUDIO_ONLY)
    }

    #[must_use]
    pub fn supports_control_auth(&self) -> bool {
        self.capabilities
            .iter()
            .any(|cap| cap == CTRL_CAP_CONTROL_AUTH)
    }

    #[must_use]
    pub fn supports_abs_queue(&self) -> bool {
        self.capabilities
            .iter()
            .any(|cap| cap == CTRL_CAP_ABS_QUEUE)
    }

    #[must_use]
    pub fn supports_abs_progress(&self) -> bool {
        self.capabilities
            .iter()
            .any(|cap| cap == CTRL_CAP_ABS_PROGRESS)
    }

    #[must_use]
    pub fn supports_abs_book_queue(&self) -> bool {
        self.capabilities
            .iter()
            .any(|cap| cap == CTRL_CAP_ABS_BOOK_QUEUE)
    }

    #[must_use]
    pub fn supports_abs_book_progress(&self) -> bool {
        self.capabilities
            .iter()
            .any(|cap| cap == CTRL_CAP_ABS_BOOK_PROGRESS)
    }

    #[must_use]
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "queue/progress/book_queue/book_progress are four independently negotiated peer capabilities, not one state (design analysis, issue #804)"
)]
pub struct CtrlAudiobookshelfCapabilities {
    pub queue: bool,
    pub progress: bool,
    pub book_queue: bool,
    pub book_progress: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each supports_* bit is an independently negotiated optional protocol feature; no honest enum or bundle exists across queue/lifecycle/audio/auth/owner-load (design analysis, issue #804)"
)]
pub struct CtrlCompatibility {
    pub peer_protocol_version: u32,
    pub client_protocol_version: u32,
    pub supports_queue_append: bool,
    pub supports_lifecycle_shutdown: bool,
    pub supports_audio_only: bool,
    pub supports_control_auth: bool,
    pub audiobookshelf: CtrlAudiobookshelfCapabilities,
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
                audiobookshelf: CtrlAudiobookshelfCapabilities {
                    queue: true,
                    progress: true,
                    book_queue: true,
                    book_progress: true,
                },
                supports_owner_queue_load: false,
            }),
            _ => Err(format!(
                "incompatible daemon protocol version: peer={peer_protocol_version} local={CTRL_PROTOCOL_VERSION}"
            )),
        }
    }

    /// # Panics
    ///
    /// Panics if `for_peer` rejects `CTRL_PROTOCOL_VERSION`, i.e. if the
    /// "local ctrl protocol version is compatible" invariant is violated.
    /// `for_peer` accepts exactly that version, so the compatible local
    /// protocol version always resolves.
    #[must_use]
    pub fn current() -> Self {
        Self::for_peer(CTRL_PROTOCOL_VERSION).expect("local ctrl protocol version is compatible")
    }
}

// ── Unified queue wire types ──────────────────────────────────────────────

/// One slot in the unified queue representation.  The `slot_id` is the
/// stable runtime identity of the occurrence (not the item's content ID).
#[derive(Clone, Debug, Serialize, Deserialize)]
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
#[derive(Clone, Debug, Serialize, Deserialize)]
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
#[must_use]
pub fn slot_id_to_u64(id: QueueSlotId) -> u64 {
    id.raw()
}
