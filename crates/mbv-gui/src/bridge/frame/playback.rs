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
    /// The visual slot's artwork (design D10): `None` unless the slot's art
    /// changed for this window — the theme's arming rule (design D7). The
    /// front end keeps the last carried value and paints it until the next.
    pub artwork: Option<GuiCardArtwork>,
}

/// The Queue playback visual slot's artwork one frame carries (design D10).
/// The image and placeholder payloads are `data:` URLs: the artwork the
/// window paints is the core's encoded cache bytes, never a fetch the
/// webview makes (the CSP allows a `data:` image source for exactly this).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "detail", rename_all = "camelCase")]
pub enum GuiCardArtwork {
    /// The slot paints this image: the cached artwork's display encode, as a
    /// `data:image/jpeg` URL.
    Image(String),
    /// The slot paints the bundled placeholder, as a `data:image/webp` URL.
    Placeholder(String),
    /// The slot paints the loading fill: a fetch is in flight or awaits its
    /// retry.
    Loading,
    /// The slot paints nothing: idle, the user's hide toggle, images off, or
    /// the visualizer owning the slot.
    Hidden,
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
            artwork: None,
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
