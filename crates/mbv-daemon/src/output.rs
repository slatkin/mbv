//! The Owner's single playback output: local playback, one Cast receiver, or
//! one Emby Session (cast-owner-output design D1, emby-session-owner-output
//! design D1). The type system makes two active outputs unrepresentable.
//! Each remote variant keeps its own state in its own module: `cast` owns the
//! receiver's worker, identity, and connection, and `emby_session` owns the
//! device identity, the poll bookkeeping, and the loaded item.

use mbv_ctrl::OutputState;

use crate::cast::CastOutput;
use crate::emby_session::EmbySessionOutput;

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
