use serde::{Deserialize, Serialize};

/// Playback output requested by `CtrlCmd::SelectPlaybackOutput`
/// (cast-owner-output design D3). The Client supplies the Cast address from
/// its own discovery; the Owner is the only authority over the resulting
/// output state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutputSelection {
    Local,
    Cast {
        receiver_id: String,
        name: String,
        host: String,
        port: u16,
    },
}

/// Connection state of the Owner's Cast output.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CastConnection {
    Connecting,
    Connected,
    Disconnected,
}

/// The Owner's active playback output, carried by `CtrlEvent::PlaybackOutput`
/// on cold seed and on every change (design D3). Slot, position, and pause
/// state keep travelling on `UnifiedQueueState` and `StatusOnly`, which have
/// no output-specific shape.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutputState {
    Local,
    Cast {
        receiver_id: String,
        name: String,
        connection: CastConnection,
    },
}
