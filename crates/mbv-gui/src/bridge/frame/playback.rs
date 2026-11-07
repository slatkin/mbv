//! The Queue playback section of a frame: the transport band's facts (design
//! D4). Each field mirrors one fact of `QueuePlaybackProjection`.

use mbv::{
    PlaybackChapter, PlaybackTargetChoice, QueuePlaybackProjection, SegmentMark, TrackChip,
    TrackChips, TrackChoice, TrickplayGrid,
};
use mbv_emby_model::TICKS_PER_SECOND;
use mbv_ui_model::panel_targets::SessionTargetKey;
use mbv_ui_model::playback::PlaybackState;
use mbv_ui_model::playback_target::NowPlayingStatus;
use serde::Serialize;

use super::theme::GuiRole;
/// The Queue playback band's facts for one frame.
#[derive(Debug, Clone, PartialEq, Serialize)]
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
    /// The seek row's left clock, formatted by the TUI's shared duration
    /// formatter. The front end paints the text verbatim.
    pub elapsed_clock: String,
    /// The seek row's right clock (the time left in the active item), in the
    /// same format. The front end paints the minus sign before it.
    pub remaining_clock: String,
    /// Whether the controls and seekbar carry playback.
    pub show_controls: bool,
    /// The plain now-playing title, if there is one.
    pub now_playing_title: Option<String>,
    /// The now-playing parent line: the series of an episode, the artist of a
    /// track, the subscription of a feed entry, the show of an
    /// Audiobookshelf episode. `None` when the item has no container fact.
    pub parent_line: Option<String>,
    /// The season-and-episode badge the artwork paints ("S2 E5"). `None` for
    /// every kind but an Emby episode with an ordinal known.
    pub episode_badge: Option<String>,
    /// The active item's wall-clock end time, formatted in the local zone:
    /// the scrubber's finish clock. `None` while no item plays or the runtime
    /// is unknown.
    pub finish_clock: Option<String>,
    /// The active item's chapter boundaries with their names (design D4a):
    /// the scrubber's boundary ticks and the hover preview's chapter name.
    /// Empty while the item's fetch has not landed or carries none.
    pub chapters: Vec<GuiChapter>,
    /// The intro mark's tick bounds (the `MediaSegments` `Intro`). `None`
    /// while unknown.
    pub intro_mark: Option<GuiSegmentMark>,
    /// The credits mark's tick bounds (the `MediaSegments` `Outro`). `None`
    /// while unknown.
    pub credits_mark: Option<GuiSegmentMark>,
    /// The trickplay source description (design D4a): one tile grid per
    /// available width. `None` while unknown; the preview's tile bytes ride
    /// the hover datum from the core's cache (task 6.7).
    pub trickplay: Option<Vec<GuiTrickplayGrid>>,
    /// Which transport controls are actionable.
    pub availability: GuiTransportAvailability,
    /// The status-indicator cluster the queue band paints on its controls
    /// row: `None` while the local player is inactive — the cluster's data
    /// source is the local player's stream, so a band whose playback rides a
    /// remote output paints none, as the TUI's does.
    pub status_indicators: Option<Vec<GuiStatusIndicator>>,
    /// The visual slot's artwork (design D10): `None` unless the slot's art
    /// changed for this window — the theme's arming rule (design D7). The
    /// front end keeps the last carried value and paints it until the next.
    pub artwork: Option<GuiCardArtwork>,
    /// The track chips the canvas paints under the transport (design D4):
    /// the video stream's resolved label, and the audio and subtitle tracks
    /// each with its choices and the chosen one. `None` while the local
    /// player is inactive, as the status-indicator cluster is.
    pub track_chips: Option<GuiTrackChips>,
    /// The playback-target chooser's choices: the Emby sessions then the cast
    /// receivers the Owner offers, each marked with whether it is the Owner's
    /// attached output.
    pub target_choices: Vec<GuiTargetChoice>,
    /// The scrubber's hover preview (add-tauri-gui-client row 6.4): the
    /// pointer's fraction along the bar and the time and chapter it names.
    /// `None` while the pointer is away from the bar.
    pub hover: Option<GuiScrubberHover>,
}

/// The scrubber's hover preview (design D4): the position a move over the
/// painted bar carried, resolved to the time and chapter it names, and the
/// trickplay tile the preview paints (task 6.7).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiScrubberHover {
    /// The pointer's fraction along the painted bar.
    pub fraction: f64,
    /// The time at the pointer's position, formatted by the TUI's shared
    /// duration formatter. The front end paints the text verbatim.
    pub time_label: String,
    /// The chapter covering the pointer's position, when the active item's
    /// chapters cover it.
    pub chapter: Option<String>,
    /// The trickplay tile at the pointer's position: `None` while the active
    /// item offers none, its sheet is not yet cached, or the server does not
    /// hold it — the preview paints its placeholder then.
    pub tile: Option<GuiScrubberTile>,
}

/// The hovered trickplay tile the preview paints (task 6.7): the thumbnail's
/// place in its sheet, the thumbnail's pixel size, and the sheet's bytes as
/// the `data:image/jpeg` URL the window paints. The webview fetches nothing:
/// the bytes are the core's cache, so the CSP keeps its `data:` image source
/// and admits no remote origin (design D11).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiScrubberTile {
    /// The sheet's opaque identity (the item, the thumbnail width, and the
    /// sheet's ordinal). The window repaints the sheet it latched for this
    /// key while `sheet` is `None`.
    pub sheet_key: String,
    /// The thumbnail's pixel width within the sheet.
    pub width: u32,
    /// The thumbnail's pixel height within the sheet.
    pub height: u32,
    /// The sheet's thumbnail columns.
    pub columns: u32,
    /// The thumbnail's column in the sheet.
    pub column: u32,
    /// The thumbnail's row in the sheet.
    pub row: u32,
    /// The sheet image as a `data:image/jpeg` URL. `None` when the frame does
    /// not re-carry it — the hovered sheet is the one the window already
    /// holds — as the artwork's arming rule works (design D10).
    pub sheet: Option<String>,
}

/// One status indicator of the queue band's cluster, mirroring
/// `mbv_render::indicators::StatusIndicator`: the uppercased label and the
/// foreground role its text paints in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiStatusIndicator {
    pub label: String,
    pub role: GuiRole,
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
            elapsed_clock: projection.elapsed_clock.clone(),
            remaining_clock: projection.remaining_clock.clone(),
            show_controls: projection.show_controls,
            now_playing_title: projection.now_playing_title.clone(),
            parent_line: projection.parent_line.clone(),
            episode_badge: projection.episode_badge.clone(),
            finish_clock: projection.finish_clock.clone(),
            chapters: projection.chapters.iter().map(GuiChapter::from).collect(),
            intro_mark: projection.intro_mark.map(GuiSegmentMark::from),
            credits_mark: projection.credits_mark.map(GuiSegmentMark::from),
            trickplay: projection
                .trickplay
                .as_ref()
                .map(|grids| grids.iter().map(GuiTrickplayGrid::from).collect()),
            availability: GuiTransportAvailability {
                stop: projection.availability.stop,
                next: projection.availability.next,
                previous: projection.availability.previous,
            },
            status_indicators: projection.status_indicators.as_ref().map(|items| {
                items
                    .iter()
                    .map(|item| GuiStatusIndicator {
                        label: item.label.clone(),
                        role: GuiRole::from(item.role),
                    })
                    .collect()
            }),
            artwork: None,
            track_chips: projection.track_chips.as_ref().map(GuiTrackChips::from),
            target_choices: projection
                .target_choices
                .iter()
                .map(GuiTargetChoice::from)
                .collect(),
            hover: None,
        }
    }
}

/// The track chips' facts, mirroring `TrackChips`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiTrackChips {
    /// The video stream's resolved label. `None` for an audio-only item.
    pub video: Option<String>,
    /// The audio chip.
    pub audio: GuiTrackChip,
    /// The subtitle chip; a `chosen` of `None` is subtitles off.
    pub subtitles: GuiTrackChip,
}

impl From<&TrackChips> for GuiTrackChips {
    fn from(chips: &TrackChips) -> Self {
        Self {
            video: chips.video.clone(),
            audio: GuiTrackChip::from(&chips.audio),
            subtitles: GuiTrackChip::from(&chips.subtitles),
        }
    }
}

/// One chip's facts, mirroring `TrackChip`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiTrackChip {
    /// The choices, in the player's track order.
    pub choices: Vec<GuiTrackChoice>,
    /// The chosen choice's mpv id; `None` is off or unresolved.
    pub chosen: Option<i64>,
}

impl From<&TrackChip> for GuiTrackChip {
    fn from(chip: &TrackChip) -> Self {
        Self {
            choices: chip.choices.iter().map(GuiTrackChoice::from).collect(),
            chosen: chip.chosen,
        }
    }
}

/// One chip choice, mirroring `TrackChoice`: the mpv track id a switch
/// sends, the resolved label, and whether the subtitle track is forced.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiTrackChoice {
    pub id: i64,
    pub label: String,
    pub forced: bool,
}

impl From<&TrackChoice> for GuiTrackChoice {
    fn from(choice: &TrackChoice) -> Self {
        Self {
            id: choice.id,
            label: choice.label.clone(),
            forced: choice.forced,
        }
    }
}

/// One playback-target choice of the chooser, mirroring
/// `PlaybackTargetChoice`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiTargetChoice {
    /// The stable identity a selection sends.
    pub key: GuiTargetKey,
    /// The label the chooser paints.
    pub label: String,
    /// Whether the target is the Owner's attached playback output.
    pub is_active: bool,
}

impl From<&PlaybackTargetChoice> for GuiTargetChoice {
    fn from(choice: &PlaybackTargetChoice) -> Self {
        Self {
            key: GuiTargetKey::from(&choice.key),
            label: choice.label.clone(),
            is_active: choice.is_active,
        }
    }
}

/// A playback target's stable identity, mirroring `SessionTargetKey` variant
/// for variant: the Emby session id, or the cast receiver id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "id", rename_all = "camelCase")]
pub enum GuiTargetKey {
    Emby(String),
    Cast(String),
}

impl From<&SessionTargetKey> for GuiTargetKey {
    fn from(key: &SessionTargetKey) -> Self {
        match key {
            SessionTargetKey::Emby(id) => Self::Emby(id.clone()),
            SessionTargetKey::Cast(id) => Self::Cast(id.clone()),
        }
    }
}

/// One chapter boundary of the active item, mirroring `PlaybackChapter`:
/// the scrubber's boundary tick and the hover preview's chapter name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiChapter {
    pub name: String,
    pub start_ticks: i64,
}

impl From<&PlaybackChapter> for GuiChapter {
    fn from(chapter: &PlaybackChapter) -> Self {
        Self {
            name: chapter.name.clone(),
            start_ticks: chapter.start_ticks,
        }
    }
}

/// One intro or credits mark of the active item, mirroring `SegmentMark`:
/// the tick bounds the scrubber paints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiSegmentMark {
    pub start_ticks: i64,
    pub end_ticks: i64,
}

impl From<SegmentMark> for GuiSegmentMark {
    fn from(mark: SegmentMark) -> Self {
        Self {
            start_ticks: mark.start_ticks,
            end_ticks: mark.end_ticks,
        }
    }
}

/// One trickplay tile grid of the active item, mirroring `TrickplayGrid`:
/// the source description the preview's tile fetch reads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiTrickplayGrid {
    /// The thumbnail pixel width this grid answers.
    pub width: u32,
    /// The thumbnail pixel height.
    pub height: u32,
    /// Thumbnails per row of one tile sheet.
    pub tile_width: u32,
    /// Thumbnail rows of one tile sheet.
    pub tile_height: u32,
    /// The total thumbnails the grid holds.
    pub thumbnail_count: u32,
    /// Milliseconds between consecutive thumbnails.
    pub interval_ms: u64,
    /// The media source the tile URLs qualify with.
    pub media_source_id: String,
}

impl From<&TrickplayGrid> for GuiTrickplayGrid {
    fn from(grid: &TrickplayGrid) -> Self {
        Self {
            width: grid.width,
            height: grid.height,
            tile_width: grid.tile_width,
            tile_height: grid.tile_height,
            thumbnail_count: grid.thumbnail_count,
            interval_ms: grid.interval_ms,
            media_source_id: grid.media_source_id.clone(),
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
    /// The remaining whole minutes the rail paints as `−22m`
    /// (audit/collapsed.md G12): the item's remaining runtime floored to
    /// whole minutes and clamped at zero. `None` while the runtime is unknown
    /// (`runtime_ticks <= 0`), which is distinct from a known zero.
    pub remaining_minutes: Option<u64>,
}

impl From<PlaybackState> for GuiPlaybackState {
    fn from(state: PlaybackState) -> Self {
        Self {
            active: state.active,
            active_idx: state.active_idx,
            position_ticks: state.position_ticks,
            runtime_ticks: state.runtime_ticks,
            paused: state.paused,
            remaining_minutes: remaining_whole_minutes(state.position_ticks, state.runtime_ticks),
        }
    }
}

/// The whole minutes left in an item with a known runtime: the position clamps
/// into the runtime, so an overshot playhead leaves zero, and the division
/// floors the seconds away. `None` while the runtime is unknown.
fn remaining_whole_minutes(position_ticks: i64, runtime_ticks: i64) -> Option<u64> {
    if runtime_ticks <= 0 {
        return None;
    }
    let remaining_ticks = runtime_ticks - position_ticks.clamp(0, runtime_ticks);
    let minutes = remaining_ticks / (60 * TICKS_PER_SECOND);
    Some(u64::try_from(minutes).unwrap_or(0))
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
