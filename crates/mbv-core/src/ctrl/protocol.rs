use serde::{Deserialize, Serialize};

use crate::player::PlayerStatus;
use mbv_queue::{QueueItem, QueueLineage, QueueSlotId, QueueSource};

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
pub type PlaybackGeneration = u64;

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
