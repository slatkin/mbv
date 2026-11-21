//! The Queue playback section: the transport band's facts (design
//! D4). Each field mirrors one fact of `QueuePlaybackProjection`, except the
//! playhead facts: they ride the `playhead` anchor section (design D5), which
//! the window predicts from between anchors.

use mbv::{
    NowPlayingMediaKind, PlaybackTargetChoice, QueueCardArtwork, QueuePlaybackProjection,
    TrackChip, TrackChips, TrackChoice,
};
use mbv_surfaces::queue::QueueSurface;
use mbv_ui_model::panel_targets::{ChooserTargetKey, SessionTargetKey};

use super::queue::active_slot;
use super::theme::GuiRole;
use crate::art::{GuiArtRef, PLACEHOLDER_PATH, art_url, card_path};
/// The Queue playback band's facts for one build.
#[derive(Debug, Clone, PartialEq)]
pub struct GuiQueuePlayback {
    /// The playback target's host label. Empty when there is none.
    pub host: String,
    /// Whether the playback target is remote. The flag picks the icon's colour.
    pub host_is_remote: bool,
    /// Whether the transport section paints. Idle collapses it.
    pub shown: bool,
    /// Whether the controls and seekbar carry playback.
    pub show_controls: bool,
    /// The plain now-playing title, if there is one.
    pub now_playing_title: Option<String>,
    /// The now-playing item's media kind: the title block paints its kind
    /// icon beside the title from this fact.
    pub now_playing_media: NowPlayingMediaKind,
    /// The now-playing parent line: the series of an episode, the artist of a
    /// track, the subscription of a feed entry, the show of an
    /// Audiobookshelf episode. `None` when the item has no container fact.
    pub parent_line: Option<String>,
    /// The season-and-episode badge the artwork paints ("S2 · E5"). `None`
    /// for every kind but an Emby episode with an ordinal known.
    pub episode_badge: Option<String>,
    /// Which transport controls are actionable.
    pub availability: GuiTransportAvailability,
    /// The status-indicator cluster the queue band paints on its controls
    /// row: `None` while the local player is inactive — the cluster's data
    /// source is the local player's stream, so a band whose playback rides a
    /// remote output paints none, as the TUI's does.
    pub status_indicators: Option<Vec<GuiStatusIndicator>>,
    /// The visual slot's artwork (design D7): the image reference the slot
    /// paints, carried on every build — a URL names the cache entry, so the
    /// reference is small and needs no arming latch. `None` paints nothing:
    /// idle, the user's hide toggle, images off, or the visualizer owning
    /// the slot.
    pub artwork: Option<GuiArtRef>,
    /// The track chips the canvas paints under the transport (design D4):
    /// the video stream's resolved label, and the audio and subtitle tracks
    /// each with its choices and the chosen one. `None` while the local
    /// player is inactive, as the status-indicator cluster is.
    pub track_chips: Option<GuiTrackChips>,
    /// The playback-target chooser's choices: the Emby sessions then the cast
    /// receivers the Owner offers, each marked with whether it is the Owner's
    /// attached output.
    pub target_choices: Vec<GuiTargetChoice>,
    /// The player panel's control facts (design D5, D7): the GUI-only facts
    /// the shared projection does not carry, grouped as the availability is.
    pub controls: GuiPlayerControls,
}

/// The player panel's GUI-only control facts (gui-art-player-overlay design
/// D5, D7). Both decide how the transport controls paint, and neither rides
/// the shared `QueuePlaybackProjection`: the core owns them for the GUI
/// Client alone.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GuiPlayerControls {
    /// Whether the panel hides its transport controls until the pointer
    /// enters the stage (design D5): the core-owned preference, read from the
    /// App.
    pub auto_hide: bool,
    /// Whether the transport can start the shown queue (design D7): true
    /// only for a shown local queue that holds slots while nothing plays. The
    /// panel paints its control rows then, even with no title.
    pub can_start: bool,
}

/// The scrubber's hover preview (design D4): the position a move over the
/// painted bar carried, resolved to the time it names.
#[derive(Debug, Clone, PartialEq)]
pub struct GuiScrubberHover {
    /// The pointer's fraction along the painted bar.
    pub fraction: f64,
    /// The time at the pointer's position, formatted by the TUI's shared
    /// duration formatter. The window paints the text verbatim.
    pub time_label: String,
}

/// One status indicator of the queue band's cluster, mirroring
/// `mbv_render::indicators::StatusIndicator`: the uppercased label and the
/// foreground role its text paints in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiStatusIndicator {
    pub label: String,
    pub role: GuiRole,
}

/// The visual slot's artwork reference for one `QueueCardArtwork` state
/// (design D7): the cached image and the bundled placeholder as the URLs the
/// `mbv-art` scheme answers, a fetch in flight as `Loading`, and the hidden
/// state as `None` — the slot paints nothing.
pub(crate) fn card_artwork_ref(artwork: &QueueCardArtwork) -> Option<GuiArtRef> {
    match artwork {
        QueueCardArtwork::Image { key, .. } => Some(GuiArtRef::Image {
            src: art_url(&card_path(key)),
        }),
        QueueCardArtwork::Placeholder => Some(GuiArtRef::Placeholder {
            src: art_url(PLACEHOLDER_PATH),
        }),
        QueueCardArtwork::Loading => Some(GuiArtRef::Loading),
        QueueCardArtwork::Hidden => None,
    }
}

impl From<&QueuePlaybackProjection> for GuiQueuePlayback {
    fn from(projection: &QueuePlaybackProjection) -> Self {
        Self {
            host: projection.host.clone(),
            host_is_remote: projection.host_is_remote,
            shown: projection.shown,
            show_controls: projection.show_controls,
            now_playing_title: projection.now_playing_title.clone(),
            now_playing_media: projection.now_playing_media,
            parent_line: projection.parent_line.clone(),
            episode_badge: projection.episode_badge.clone(),
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
            // The section build overrides this with the slot's art reference
            // (design D7); the projection alone cannot name it.
            artwork: None,
            track_chips: projection.track_chips.as_ref().map(GuiTrackChips::from),
            target_choices: projection
                .target_choices
                .iter()
                .map(GuiTargetChoice::from)
                .collect(),
            // The GUI-only facts: the section build sets them from the App
            // (design D5, D7); the projection cannot name either.
            controls: GuiPlayerControls::default(),
        }
    }
}

/// The track chips' facts, mirroring `TrackChips`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiTrackChips {
    /// The video stream's resolved label. `None` for an audio-only item.
    pub video: Option<String>,
    /// The audio chip.
    pub audio: GuiTrackChip,
    /// The subtitle chip; a `chosen` of `None` is subtitles off.
    pub subtitles: GuiTrackChip,
    /// The audio chip's pill text: the chosen track's two-letter language code,
    /// empty when none applies (the chip then paints the choice's label).
    pub audio_lang_code: String,
}

impl From<&TrackChips> for GuiTrackChips {
    fn from(chips: &TrackChips) -> Self {
        Self {
            video: chips.video.clone(),
            audio: GuiTrackChip::from(&chips.audio),
            subtitles: GuiTrackChip::from(&chips.subtitles),
            audio_lang_code: chips.audio_lang_code.clone(),
        }
    }
}

/// One chip's facts, mirroring `TrackChip`.
#[derive(Debug, Clone, PartialEq, Eq)]
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
#[derive(Debug, Clone, PartialEq, Eq)]
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiTargetChoice {
    /// The stable identity a selection sends.
    pub key: GuiTargetKey,
    /// The label the chooser paints.
    pub label: String,
    /// Whether the target is the Owner's attached playback output.
    pub is_active: bool,
    /// Whether the target is a peer device with a direct Player-owner
    /// endpoint (an `mbvd` or another mbv), which paints the mpv mark instead
    /// of the Emby one.
    pub is_direct_peer: bool,
}

impl From<&PlaybackTargetChoice> for GuiTargetChoice {
    fn from(choice: &PlaybackTargetChoice) -> Self {
        Self {
            key: GuiTargetKey::from(&choice.key),
            label: choice.label.clone(),
            is_active: choice.is_active,
            is_direct_peer: choice.is_direct_peer,
        }
    }
}

/// A playback target's stable identity, mirroring `ChooserTargetKey`: the local
/// output, the Emby session id, or the cast receiver id. The local output is a
/// unit kind (`{ "kind": "local" }`) with no id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuiTargetKey {
    Local,
    Emby(String),
    Cast(String),
}

/// The reverse of the `GuiTargetKey` projection (the `selectTarget`
/// command, and a painted chooser row's press): the stable identity a painted
/// row names, back in the shared vocabulary the selection request carries. A
/// function, not a `From` impl: the conversion is this bridge's, and an impl on
/// the shared key type would spread it crate-wide.
#[must_use]
pub(crate) fn chooser_target_key(key: &GuiTargetKey) -> ChooserTargetKey {
    match key {
        GuiTargetKey::Local => ChooserTargetKey::Local,
        GuiTargetKey::Emby(id) => ChooserTargetKey::Target(SessionTargetKey::Emby(id.clone())),
        GuiTargetKey::Cast(id) => ChooserTargetKey::Target(SessionTargetKey::Cast(id.clone())),
    }
}

impl From<&ChooserTargetKey> for GuiTargetKey {
    fn from(key: &ChooserTargetKey) -> Self {
        match key {
            ChooserTargetKey::Local => Self::Local,
            ChooserTargetKey::Target(target) => Self::from(target),
        }
    }
}

impl From<&SessionTargetKey> for GuiTargetKey {
    fn from(key: &SessionTargetKey) -> Self {
        match key {
            SessionTargetKey::Emby(id) => Self::Emby(id.clone()),
            SessionTargetKey::Cast(id) => Self::Cast(id.clone()),
        }
    }
}

/// The playhead anchor (design D5): the position, runtime, paused state, and
/// active slot at the send, from which the window predicts the position until
/// the next anchor. The core sends one on a changed fact or a new sample that
/// drifts past the tolerance; between anchors the window advances the
/// position itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GuiPlayhead {
    /// Whether a transport is active, on any target.
    pub active: bool,
    /// The slot that carries the now-playing marker (`QueueSlotId::raw`), the
    /// same whole-list marker the `queue` section's `active` carries.
    pub active_slot: Option<u64>,
    /// The reported position at the send, in ticks.
    pub position_ticks: i64,
    /// The active item's runtime, in ticks.
    pub runtime_ticks: i64,
    /// Whether the transport is paused: the prediction holds the position
    /// while it is.
    pub paused: bool,
}

impl GuiPlayhead {
    /// The anchor the tick's one `queue_playback_projection` read and the
    /// Queue Surface state's now-playing marker carry (design D5).
    #[must_use]
    pub fn from_surface(playback: &QueuePlaybackProjection, queue: &QueueSurface) -> Self {
        let state = playback.playback;
        Self {
            active: state.active,
            active_slot: queue.rows().iter().find_map(active_slot),
            position_ticks: state.position_ticks,
            runtime_ticks: state.runtime_ticks,
            paused: state.paused,
        }
    }
}

/// Which transport controls are actionable. The fields mirror the Queue
/// playback projection's `TransportAvailability`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GuiTransportAvailability {
    pub stop: bool,
    pub next: bool,
    pub previous: bool,
}
