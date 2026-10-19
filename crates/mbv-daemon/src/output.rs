//! The Owner's single playback output: local playback, one Cast receiver, or
//! one Emby Session (cast-owner-output design D1, emby-session-owner-output
//! design D1). The type system makes two active outputs unrepresentable.
//! Each remote variant keeps its own state in its own module: `cast` owns the
//! receiver's worker, identity, and connection, and `emby_session` owns the
//! device identity, the poll bookkeeping, and the loaded item.

use mbv_ctrl::OutputState;
use mbv_ctrl::player::PlayerStatus;
use mbv_queue::{PlaybackQueue, QueueSlotId};

use self::advance::OutputAdvance;
use crate::cast::{self, CastOutput};
use crate::emby_session::{EmbySessionOutput, status as emby_session_status};

pub(crate) mod advance;
pub(crate) mod handoff;

/// The single playback output a Player owner plays through (design D1). The
/// remote sides are boxed: they carry the worker sender, the connection, and
/// the loaded item while `Local` carries nothing.
#[derive(Default)]
pub(crate) enum PlaybackOutput {
    #[default]
    Local,
    Cast(Box<CastOutput>),
    /// One Emby Session (emby-session-owner-output design D1): the device's
    /// own app plays the item and the Owner observes it by poll, so the state
    /// here is the device identity and the observation, not a media transport.
    EmbySession(Box<EmbySessionOutput>),
}

/// The ctrl `OutputState` an output projects (design D3).
pub(crate) fn output_state(output: &PlaybackOutput) -> OutputState {
    match output {
        PlaybackOutput::Local => OutputState::Local,
        PlaybackOutput::Cast(cast) => OutputState::Cast {
            receiver_id: cast.receiver.id.clone(),
            name: cast.receiver.friendly_name.clone(),
            connection: cast.connection,
        },
        PlaybackOutput::EmbySession(session) => OutputState::EmbySession {
            device_id: session.device_id.clone(),
            name: session.name.clone(),
            connection: session.connection,
            audio_only: session.audio_only(),
            // Content the Owner did not load reaches the state from the
            // observed item; the loaded item is presented from the queue
            // (design D3, D10).
            now_playing: session.now_playing(),
        },
    }
}

/// The `PlayerStatus` a remote output with nothing to present projects (design
/// D8, D10): no active playback, no position, no now-playing metadata. The
/// queue length and the queue's own active marker stay coherent, so status-only
/// consumers never index a slot the queue no longer holds. Shared by the Cast
/// receiver and the Emby Session, which both present a report of their own
/// state rather than a local run.
pub(crate) fn not_presenting(base: &PlayerStatus, queue: &PlaybackQueue) -> PlayerStatus {
    let mut inactive = base.clone();
    inactive.active = false;
    inactive.paused = false;
    inactive.position_ticks = 0;
    inactive.clear_current_item_metadata();
    inactive.queue_len = queue.len();
    inactive.current_idx = queue.active_index().unwrap_or(0);
    inactive
}

impl PlaybackOutput {
    /// The advance state of the active remote output (emby-session-owner-output
    /// design D6), if one is under way. Local playback never advances the queue
    /// from an output report.
    pub(super) fn advance(&self) -> Option<&OutputAdvance> {
        match self {
            PlaybackOutput::Local => None,
            PlaybackOutput::Cast(cast) => cast.advance.as_ref(),
            PlaybackOutput::EmbySession(session) => session.advance.as_ref(),
        }
    }

    /// The active remote output's advance, to step it (design D6); `None` for
    /// local playback, which never advances the queue.
    pub(super) fn advance_mut(&mut self) -> Option<&mut OutputAdvance> {
        match self {
            PlaybackOutput::Local => None,
            PlaybackOutput::Cast(cast) => cast.advance.as_mut(),
            PlaybackOutput::EmbySession(session) => session.advance.as_mut(),
        }
    }

    /// Starts or ends the active remote output's advance (design D6): it lives
    /// on the output, so an output change, a stop, or a replacing load ends it
    /// with the output. Local playback has no advance to replace.
    pub(super) fn set_advance(&mut self, advance: Option<OutputAdvance>) {
        match self {
            PlaybackOutput::Local => {}
            PlaybackOutput::Cast(cast) => cast.advance = advance,
            PlaybackOutput::EmbySession(session) => session.advance = advance,
        }
    }

    /// The slot of the item the active remote output loaded (design D6), so the
    /// shared advance steps from the slot that ended; `None` for local playback
    /// and for a remote output that loaded nothing.
    pub(super) fn loaded_slot(&self) -> Option<QueueSlotId> {
        match self {
            PlaybackOutput::Local => None,
            PlaybackOutput::Cast(cast) => cast.loaded.as_ref().map(|loaded| loaded.slot),
            PlaybackOutput::EmbySession(session) => {
                session.loaded.as_ref().map(|loaded| loaded.slot)
            }
        }
    }

    /// Whether the output's presented item is the queue's active slot
    /// (queue-content-shuffle): the Shuffle pins the active slot first only
    /// when the presented item is that slot. Local playback plays only items
    /// the Owner loaded, so it always presents a queue slot; a remote output
    /// can present foreign content — media the Owner did not load — which
    /// names no queue slot (PR #957 review).
    pub(super) fn presents_active_queue_slot(&self, queue: &PlaybackQueue) -> bool {
        match self {
            PlaybackOutput::Local => true,
            PlaybackOutput::Cast(cast) => cast::presents_active_queue_slot(cast, queue),
            PlaybackOutput::EmbySession(session) => {
                emby_session_status::presents_active_queue_slot(session, queue)
            }
        }
    }
}
