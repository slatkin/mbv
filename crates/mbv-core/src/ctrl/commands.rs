use serde::{Deserialize, Serialize};

use crate::config::{QueueSource, ServiceKind};
use crate::ctrl::{
    CtrlHello, PlaybackGeneration, PlaybackRequestId, QueueLineage, QueueLoadRequestId,
    UnifiedQueueSlot,
};
use crate::playback_queue::QueueItem;
use crate::player::PlayerCommand;

// ── CtrlCmd ──────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
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
/// `send_role_gate_rejection` (`daemon_control.rs`) can match on it
/// exhaustively with no wildcard arm.
#[derive(Debug)]
pub enum OwnerGateRejection {
    AdoptQueue,
    QueueLoadIdle { request_id: QueueLoadRequestId },
    QueueSourceUpdate,
}

/// Owner-role gate for a ctrl command: whether acceptance depends on the
/// daemon being the Stay-alive owner (`DaemonRole::Local`).
#[derive(Debug)]
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
    #[must_use]
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
    #[must_use]
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
    #[must_use]
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
#[derive(Debug, Serialize, Deserialize)]
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
