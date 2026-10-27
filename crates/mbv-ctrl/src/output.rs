use serde::{Deserialize, Serialize};

/// Playback output requested by `CtrlCmd::SelectPlaybackOutput`
/// (cast-owner-output design D3, emby-session-owner-output design D3). The
/// Client supplies the Cast address from its own discovery, and only the
/// Emby device identity of an Emby Session; the Owner is the only authority
/// over the resulting output state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub enum OutputSelection {
    Local,
    Cast {
        receiver_id: String,
        name: String,
        host: String,
        port: u16,
    },
    /// An Emby Session, named by the device that owns it. The Owner resolves
    /// that device's current session id itself (emby-session-owner-output
    /// design D4), so a stale session id cannot reach it.
    EmbySession {
        device_id: String,
        name: String,
    },
}

/// Connection state of the Owner's remote playback output.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub enum OutputConnection {
    Connecting,
    Connected,
    Disconnected,
}

/// Content that an Emby Session plays but the Owner did not load
/// (emby-session-owner-output design D3): the device's own item. The Owner
/// sends it only when that item changes, not on every `StatusOnly` broadcast.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SessionNowPlaying {
    pub item_id: String,
    pub item_type: String,
    pub name: String,
    pub series_id: Option<String>,
    pub series_name: Option<String>,
    /// The item's own logo image tag. An episode's logo lives on its series
    /// and is keyed by the series id alone, so it needs no tag.
    pub logo_tag: Option<String>,
}

/// The Owner's active playback output, carried by `CtrlEvent::PlaybackOutput`
/// on cold seed and on every change (design D3). Slot, position, and pause
/// state keep travelling on `UnifiedQueueState` and `StatusOnly`, which have
/// no output-specific shape.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub enum OutputState {
    Local,
    Cast {
        receiver_id: String,
        name: String,
        connection: OutputConnection,
    },
    /// `audio_only` is the Session's own view of what it can play, and
    /// `now_playing` is content that the Owner did not load.
    EmbySession {
        device_id: String,
        name: String,
        connection: OutputConnection,
        audio_only: bool,
        now_playing: Option<SessionNowPlaying>,
    },
}
