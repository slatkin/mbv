//! The Queue playback projection every Client consumes (`extract-client-core`
//! design D5, row 6.5): the transport facts the queue column's transport band
//! paints, computed once from the `App` so both Clients read the same title,
//! availability, and idle-collapse rules. The TUI shell's `PlaybackProjection`
//! builds its title and availability from the same helpers.

use mbv_emby_model::TICKS_PER_SECOND;
use mbv_render::components::chrome_player::TransportAvailability;
use mbv_render::components::indicators::{
    StatusIndicator, short_resolution_label, status_indicator_items,
};
use mbv_ui_model::panel_targets::SessionTargetKey;
use mbv_ui_model::playback::PlaybackState;
use mbv_ui_model::playback_target::NowPlayingStatus;
use mbv_ui_model::ui_util::{fmt_duration_short, fmt_wall_clock};
use time::UtcOffset;

use crate::app::state::panel_targets::PanelTarget;
use crate::app::{App, queue_summary_projection::playback_finish_epoch};

/// One playback-target choice of the canvas's chooser: the stable identity a
/// selection sends (`SessionTargetKey`, the F3 target panel's own identity),
/// the label the chooser paints, and whether the target is the Owner's
/// attached playback output.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlaybackTargetChoice {
    /// The Emby session id or the cast receiver id, qualified by its channel.
    pub key: SessionTargetKey,
    /// The Emby session's device name, or the cast receiver's friendly name.
    pub label: String,
    /// Whether this target is the Owner's attached playback output: a Cast
    /// receiver by its receiver id, an Emby Session by the output's device id.
    pub is_active: bool,
}

/// The track chips the canvas paints under the transport: the video stream's
/// resolved label, and the audio and subtitle tracks each with its choices
/// and the chosen one (design D4). The local player's status is the source,
/// so the chips are `None` while the local player is inactive — the same rule
/// the status-indicator cluster follows, and a band whose playback rides a
/// remote output carries none.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TrackChips {
    /// The video stream's resolved label (`"4K"`, `"FHD"`, ...).
    /// `None` for an audio-only item: no video stream resolves a label, as
    /// the TUI's cluster drops the audio and subtitle labels for one.
    pub video: Option<String>,
    /// The audio chip: the track choices and the chosen track's mpv id.
    pub audio: TrackChip,
    /// The subtitle chip: the track choices and the chosen track's mpv id;
    /// a `chosen` of `None` is subtitles off.
    pub subtitles: TrackChip,
}

/// One chip's facts: its choices and the chosen one.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TrackChip {
    /// The choices, in the player's track order, each carrying the mpv track
    /// id a switch sends and the resolved label the player parsed.
    pub choices: Vec<TrackChoice>,
    /// The chosen choice's mpv id; `None` is off or unresolved (id 0).
    pub chosen: Option<i64>,
}

/// One chip choice: the mpv track id a switch sends, the resolved label, and
/// whether the subtitle track is forced (an audio choice carries `false`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrackChoice {
    pub id: i64,
    pub label: String,
    pub forced: bool,
}

/// One chapter boundary of the active item (design D4a): the scrubber's
/// boundary tick and the hover preview's chapter name. Both providers'
/// chapter lists map into it — Emby's `Chapters` and Audiobookshelf's
/// `media.chapters`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlaybackChapter {
    pub name: String,
    pub start_ticks: i64,
}

/// One intro or credits mark of the active item (design D4a): the
/// `MediaSegments` `Intro`/`Outro` tick bounds the scrubber paints.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SegmentMark {
    pub start_ticks: i64,
    pub end_ticks: i64,
}

/// One trickplay tile grid of the active item (design D4a): the source
/// description the scrubber preview's tile fetch (task 6.7) reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrickplayGrid {
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

/// Where one position lands inside a trickplay grid (design D4a): the tile
/// sheet the thumbnail lives in and the thumbnail's place in that sheet —
/// the crop the scrubber preview paints.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TrickplayTile {
    /// The sheet's ordinal in the grid's sheet series (the tile fetch's
    /// `index`).
    pub sheet: u32,
    /// The thumbnail's column in the sheet.
    pub column: u32,
    /// The thumbnail's row in the sheet.
    pub row: u32,
}

impl TrickplayGrid {
    /// The tile `ticks` names, `None` when the grid carries no usable
    /// geometry (a zero interval, tile width, tile height, or thumbnail
    /// count).
    ///
    /// The thumbnail the position names is `ticks / interval_ms`, clamped to
    /// the grid's last thumbnail, so a position at or past the runtime's end
    /// paints the last frame instead of nothing. The sheet is that
    /// thumbnail's ordinal divided by the sheet's capacity
    /// (`tile_width × tile_height`), and the tile within the sheet is the
    /// remainder — the arithmetic the Emby web client's preview uses.
    #[must_use]
    pub fn tile_at(&self, ticks: i64) -> Option<TrickplayTile> {
        let per_sheet = u64::from(self.tile_width).checked_mul(u64::from(self.tile_height))?;
        if per_sheet == 0 || self.interval_ms == 0 || self.thumbnail_count == 0 {
            return None;
        }
        let interval_ms = i64::try_from(self.interval_ms).ok()?;
        let milliseconds = ticks.max(0) / mbv_emby_model::TICKS_PER_MILLISECOND;
        let thumbnail = u64::try_from(milliseconds / interval_ms).ok()?;
        let thumbnail = thumbnail.min(u64::from(self.thumbnail_count) - 1);
        let within = thumbnail % per_sheet;
        Some(TrickplayTile {
            sheet: u32::try_from(thumbnail / per_sheet).ok()?,
            column: u32::try_from(within % u64::from(self.tile_width)).ok()?,
            row: u32::try_from(within / u64::from(self.tile_width)).ok()?,
        })
    }
}

/// The Queue playback band's facts for one tick. The header row paints in
/// every state, idle included. The transport section collapses while idle
/// (task 3.6): `shown` is false, and the section reserves no rows and reports
/// no regions.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct QueuePlaybackProjection {
    /// The now-playing status the header row shows. The header paints in
    /// every state.
    pub status: NowPlayingStatus,
    /// The playback target's host label, painted after the target icon while
    /// a remote target owns playback. Empty when the label is empty.
    pub host: String,
    /// Whether the playback target is remote. The flag picks the icon's colour.
    pub host_is_remote: bool,
    /// Whether the transport section paints. Idle collapses it.
    pub shown: bool,
    /// The displayed playback state: the seek row's position, runtime, and
    /// paused flag.
    pub playback: PlaybackState,
    /// The seek row's left clock: the playhead's elapsed time, formatted by
    /// the TUI's shared duration formatter (`M:SS`, `H:MM:SS` at an hour).
    pub elapsed_clock: String,
    /// The seek row's right clock: the time left in the active item in the
    /// same format, `0:00` when the runtime is unknown. The painter adds the
    /// minus sign.
    pub remaining_clock: String,
    /// Whether the controls and seekbar carry playback
    /// (`PlaybackProjection::show_controls`). Without it the seekbar paints an
    /// empty track and resolves no seek.
    pub show_controls: bool,
    /// The plain now-playing title. The controls and seek row paint only with
    /// a title.
    pub now_playing_title: Option<String>,
    /// Which transport controls are actionable.
    pub availability: TransportAvailability,
    /// The status-indicator cluster the queue band paints on its controls
    /// row (the resolution, audio, and subtitle facts the local player
    /// resolves): `None` while the local player is inactive — the cluster's
    /// data source is the local player's stream, so a band whose playback
    /// rides a remote output paints none, as the TUI's does.
    pub status_indicators: Option<Vec<StatusIndicator>>,
    /// The track chips the canvas paints under the transport (design D4):
    /// the video stream's resolved label, and the audio and subtitle tracks
    /// each with its choices and the chosen one. `None` while the local
    /// player is inactive, as the cluster is.
    pub track_chips: Option<TrackChips>,
    /// The playback-target chooser's choices (design D4a, "Cast and session
    /// choices"): the Emby sessions then the cast receivers the Owner
    /// offers, the cast half gated on the Owner's playback-output capability
    /// — the same merge the F3 target panel builds — each marked with whether
    /// it is the Owner's attached output.
    pub target_choices: Vec<PlaybackTargetChoice>,
    /// The now-playing parent line: the context part of the title parts, the
    /// series of an episode, the artist of a track, the subscription of a
    /// feed entry, the show of an Audiobookshelf episode.
    pub parent_line: Option<String>,
    /// The season-and-episode badge the artwork paints (design D4a): an Emby
    /// episode's ordinals, `"S2 · E5"` — the canvas's middle-dot separator
    /// (`main` audit 7). `None` for every other kind, and for an episode with
    /// neither ordinal known.
    pub episode_badge: Option<String>,
    /// The active item's wall-clock end time (design D4a's scrubber finish
    /// clock), formatted in the local zone: the displayed playback state's
    /// remaining runtime added to the clock. `None` while no item plays or
    /// the runtime is unknown.
    pub finish_clock: Option<String>,
    /// The active item's chapter boundaries with their names (design D4a):
    /// the scrubber's boundary ticks and the hover preview's chapter name.
    /// Empty while the item's fetch has not landed or carries none.
    pub chapters: Vec<PlaybackChapter>,
    /// The intro mark's tick bounds (the `MediaSegments` `Intro`). `None`
    /// while unknown.
    pub intro_mark: Option<SegmentMark>,
    /// The credits mark's tick bounds (the `MediaSegments` `Outro`). `None`
    /// while unknown.
    pub credits_mark: Option<SegmentMark>,
    /// The trickplay source description (design D4a): one tile grid per
    /// available width. `None` while unknown; the tile bytes are task 6.7's
    /// fetch.
    pub trickplay: Option<Vec<TrickplayGrid>>,
}

impl App {
    /// Project the queue column's transport facts (design D5). Read-only: the
    /// Client's Queue playback panel adopts the result each tick.
    #[must_use]
    pub fn queue_playback_projection(&self) -> QueuePlaybackProjection {
        let playback = self.displayed_playback_state();
        let status = self.now_playing_status();
        let (host, host_is_remote) = self.playback_host_label_and_remote();
        let parts = self.now_playing_title_parts(playback);
        let active_item = playback
            .active_idx
            .and_then(|idx| self.playback_queue().item_at(idx));
        let episode_badge = active_item.and_then(|item| match item {
            mbv_queue::QueueItem::Emby(item) => episode_badge(item),
            _ => None,
        });
        // The scrubber's decoration (design D4a): the cached metadata the
        // active item's Client-side fetch landed, if any. The projection
        // only reads the cache; a Client's frame path issues the fetch.
        let decorations = active_item
            .map(mbv_queue::QueueItem::content_id)
            .and_then(|key| self.playback_metadata_cache.get(&key));
        QueuePlaybackProjection {
            status,
            host,
            host_is_remote,
            shown: status != NowPlayingStatus::Idle,
            playback,
            elapsed_clock: fmt_duration_short(playback.position_ticks / TICKS_PER_SECOND),
            remaining_clock: fmt_duration_short(
                (playback.runtime_ticks - playback.position_ticks).max(0) / TICKS_PER_SECOND,
            ),
            show_controls: playback.active || self.output_is_cast(),
            now_playing_title: parts.as_ref().map(|parts| parts.title.text.clone()),
            parent_line: parts
                .as_ref()
                .and_then(|parts| parts.context.as_ref().map(|part| part.text.clone())),
            episode_badge,
            finish_clock: playback_finish_clock(playback),
            chapters: decorations.map_or_else(Vec::new, |d| d.chapters.clone()),
            intro_mark: decorations.and_then(|d| d.intro),
            credits_mark: decorations.and_then(|d| d.credits),
            trickplay: decorations.and_then(|d| d.trickplay.clone()),
            availability: self.transport_availability(playback),
            status_indicators: crate::app::LocalPlaybackTarget::indicator_data(self)
                .map(|data| status_indicator_items(&data)),
            track_chips: self.track_chips(),
            target_choices: self.playback_target_choices(),
        }
    }

    /// The now-playing title parts for `state`: the active queue item's parts,
    /// or the Emby Session's parts for slotless output. `None` while idle.
    pub(in crate::app) fn now_playing_title_parts(
        &self,
        state: PlaybackState,
    ) -> Option<mbv_queue::PlaybackTitleParts> {
        if !state.active {
            return None;
        }
        state
            .active_idx
            .and_then(|idx| self.playback_queue().item_at(idx))
            .map(|item| self.playback_title_parts(item))
            .or_else(|| self.slotless_playback_title_parts())
    }

    /// The plain now-playing title for `state`: the active queue item's title,
    /// or the Emby Session's title for slotless output. `None` while idle.
    pub(in crate::app) fn now_playing_title_text(&self, state: PlaybackState) -> Option<String> {
        self.now_playing_title_parts(state)
            .map(|parts| parts.title.text)
    }

    /// The track chips (design D4): the local player's resolved video label
    /// and track lists. `None` while the local player is inactive, as the
    /// status-indicator cluster is — the chips' source is the local player's
    /// stream, so a band whose playback rides a remote output carries none.
    pub(in crate::app) fn track_chips(&self) -> Option<TrackChips> {
        let status = self.player.status_snapshot();
        if !status.active {
            return None;
        }
        let video = (!status.video_is_image && status.video_height > 0).then(|| {
            short_resolution_label(
                u64::try_from(status.video_height).expect("guarded positive resolution height"),
            )
            .to_string()
        });
        Some(TrackChips {
            video,
            audio: TrackChip {
                choices: status
                    .audio_tracks
                    .iter()
                    .map(|(id, label)| TrackChoice {
                        id: *id,
                        label: label.clone(),
                        forced: false,
                    })
                    .collect(),
                chosen: (status.audio_id != 0).then_some(status.audio_id),
            },
            subtitles: TrackChip {
                choices: status
                    .sub_tracks
                    .iter()
                    .map(|(id, label, forced)| TrackChoice {
                        id: *id,
                        label: label.clone(),
                        forced: *forced,
                    })
                    .collect(),
                chosen: (status.sub_id != 0).then_some(status.sub_id),
            },
        })
    }

    /// The playback-target chooser's choices (design D4a, "Cast and session
    /// choices"): the kept `panel_targets` merge — Emby sessions then cast
    /// receivers, the cast half gated on the Owner's playback-output
    /// capability — each marked with whether it is the Owner's attached
    /// output: a Cast receiver by its receiver id, an Emby Session by the
    /// output's device id.
    pub(in crate::app) fn playback_target_choices(&self) -> Vec<PlaybackTargetChoice> {
        let attached = self.playback_output.as_ref();
        self.panel_targets
            .iter()
            .map(|target| {
                let key = target.key();
                let label = match target {
                    PanelTarget::Emby(session) => session.device_name.clone(),
                    PanelTarget::Cast(receiver) => receiver.friendly_name.clone(),
                };
                let is_active = match (attached, &key) {
                    (
                        Some(mbv_ctrl::OutputState::Cast { receiver_id, .. }),
                        SessionTargetKey::Cast(id),
                    ) => receiver_id == id,
                    (
                        Some(mbv_ctrl::OutputState::EmbySession { device_id, .. }),
                        SessionTargetKey::Emby(id),
                    ) => self
                        .sessions
                        .iter()
                        .any(|session| &session.id == id && &session.device_id == device_id),
                    _ => false,
                };
                PlaybackTargetChoice {
                    key,
                    label,
                    is_active,
                }
            })
            .collect()
    }

    /// Which transport controls are actionable for `state`: Stop while a
    /// transport is active, and Previous and Next from the owner's queue
    /// position.
    pub(in crate::app) fn transport_availability(
        &self,
        state: PlaybackState,
    ) -> TransportAvailability {
        let (previous, next) = self.transport_prev_next_available();
        TransportAvailability {
            stop: state.active,
            previous,
            next,
        }
    }
}

/// The playhead's wall-clock end time (design D4a's scrubber finish clock),
/// formatted in the local zone: the displayed playback state's remaining
/// runtime added to the clock. `None` while no item plays or the runtime is
/// unknown.
fn playback_finish_clock(state: PlaybackState) -> Option<String> {
    if !state.active {
        return None;
    }
    let offset = UtcOffset::current_local_offset().unwrap_or(UtcOffset::UTC);
    playback_finish_epoch(
        crate::app::queue_summary_projection::now_unix_secs(),
        state.position_ticks,
        state.runtime_ticks,
    )
    .map(|secs| fmt_wall_clock(secs, offset))
}

/// The season-and-episode badge the artwork paints: an Emby episode's
/// ordinals, `"S2 · E5"` — the canvas's middle-dot separator, the season
/// dropped when unknown, `None` when neither is known and for every
/// non-episode kind (design D4a, "Non-Emby kinds": Audiobookshelf, Feed, and
/// Twitch items carry no ordinal).
fn episode_badge(item: &mbv_emby_model::EmbyItem) -> Option<String> {
    if item.item_type != "Episode" {
        return None;
    }
    let season = (item.parent_index_number > 0).then(|| format!("S{}", item.parent_index_number));
    let episode = (item.index_number > 0).then(|| format!("E{}", item.index_number));
    match (season, episode) {
        (Some(season), Some(episode)) => Some(format!("{season} · {episode}")),
        (Some(season), None) | (None, Some(season)) => Some(season),
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests;
