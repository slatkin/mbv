//! The Queue playback section of a frame: the transport band's facts (design
//! D4). Each field mirrors one fact of `QueuePlaybackProjection`.

use mbv::QueuePlaybackProjection;
use mbv_ui_model::playback::PlaybackState;
use mbv_ui_model::playback_target::NowPlayingStatus;
use serde::Serialize;

/// The Queue playback band's facts for one frame.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiQueuePlayback {
    /// The now-playing status that the header row shows.
    pub status: GuiNowPlayingStatus,
    /// The playback target's host label. Empty when there is none.
    pub host: String,
    /// Whether the playback target is remote. The flag picks the icon's colour.
    pub host_is_remote: bool,
    /// Whether the transport section paints. Idle collapses it.
    pub shown: bool,
    /// The displayed playback state: the seek row's position, runtime, and paused flag.
    pub playback: GuiPlaybackState,
    /// Whether the controls and seekbar carry playback.
    pub show_controls: bool,
    /// The plain now-playing title, if there is one.
    pub now_playing_title: Option<String>,
    /// Which transport controls are actionable.
    pub availability: GuiTransportAvailability,
}

impl From<&QueuePlaybackProjection> for GuiQueuePlayback {
    fn from(projection: &QueuePlaybackProjection) -> Self {
        Self {
            status: GuiNowPlayingStatus::from(projection.status),
            host: projection.host.clone(),
            host_is_remote: projection.host_is_remote,
            shown: projection.shown,
            playback: GuiPlaybackState::from(projection.playback),
            show_controls: projection.show_controls,
            now_playing_title: projection.now_playing_title.clone(),
            availability: GuiTransportAvailability {
                stop: projection.availability.stop,
                next: projection.availability.next,
                previous: projection.availability.previous,
            },
        }
    }
}

/// The now-playing status, mirroring `NowPlayingStatus`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum GuiNowPlayingStatus {
    Playing,
    Paused,
    Idle,
}

impl From<NowPlayingStatus> for GuiNowPlayingStatus {
    fn from(status: NowPlayingStatus) -> Self {
        match status {
            NowPlayingStatus::Playing => Self::Playing,
            NowPlayingStatus::Paused => Self::Paused,
            NowPlayingStatus::Idle => Self::Idle,
        }
    }
}

/// The displayed playback state, mirroring `PlaybackState`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiPlaybackState {
    /// Whether a transport is active, on any target.
    pub active: bool,
    /// The playhead's index in the queue that this state was read from.
    pub active_idx: Option<usize>,
    pub position_ticks: i64,
    pub runtime_ticks: i64,
    pub paused: bool,
}

impl From<PlaybackState> for GuiPlaybackState {
    fn from(state: PlaybackState) -> Self {
        Self {
            active: state.active,
            active_idx: state.active_idx,
            position_ticks: state.position_ticks,
            runtime_ticks: state.runtime_ticks,
            paused: state.paused,
        }
    }
}

/// Which transport controls are actionable. The fields mirror the Queue
/// playback projection's `TransportAvailability`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiTransportAvailability {
    pub stop: bool,
    pub next: bool,
    pub previous: bool,
}
