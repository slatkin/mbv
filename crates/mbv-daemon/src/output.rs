//! The Owner's single playback output: local playback, one Cast receiver, or
//! one Emby Session (cast-owner-output design D1, emby-session-owner-output
//! design D1). The type system makes two active outputs unrepresentable.
//! Each remote variant keeps its own state in its own module: `cast` owns the
//! receiver's worker, identity, and connection, and `emby_session` owns the
//! device identity, the poll bookkeeping, and the loaded item.

use mbv_ctrl::OutputState;
use mbv_queue::QueueSlotId;

use self::advance::OutputAdvance;
use crate::cast::CastOutput;
use crate::emby_session::EmbySessionOutput;

pub(crate) mod advance;

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
            // Content that the Owner did not load reaches the state from the
            // observed item (task 5.1).
            now_playing: None,
        },
    }
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
}
