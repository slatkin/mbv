//! The Queue playback projection every Client consumes (`add-gui-front-end`
//! design D5, row 6.5): the transport facts the queue column's transport band
//! paints, computed once from the `App` so both Clients read the same title,
//! availability, and idle-collapse rules. The TUI shell's `PlaybackProjection`
//! builds its title and availability from the same helpers.

use mbv_render::components::chrome_player::TransportAvailability;
use mbv_ui_model::playback::PlaybackState;
use mbv_ui_model::playback_target::NowPlayingStatus;

use crate::app::App;

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
    /// Whether the controls and seekbar carry playback
    /// (`PlaybackProjection::show_controls`). Without it the seekbar paints an
    /// empty track and resolves no seek.
    pub show_controls: bool,
    /// The plain now-playing title. The controls and seek row paint only with
    /// a title.
    pub now_playing_title: Option<String>,
    /// Which transport controls are actionable.
    pub availability: TransportAvailability,
}

impl App {
    /// Project the queue column's transport facts (design D5). Read-only: the
    /// Client's Queue playback panel adopts the result each tick.
    #[must_use]
    pub fn queue_playback_projection(&self) -> QueuePlaybackProjection {
        let playback = self.displayed_playback_state();
        let status = self.now_playing_status();
        let (host, host_is_remote) = self.playback_host_label_and_remote();
        QueuePlaybackProjection {
            status,
            host,
            host_is_remote,
            shown: status != NowPlayingStatus::Idle,
            playback,
            show_controls: playback.active || self.output_is_cast(),
            now_playing_title: self.now_playing_title_text(playback),
            availability: self.transport_availability(playback),
        }
    }

    /// The plain now-playing title for `state`: the active queue item's title,
    /// or the Emby Session's title for slotless output. `None` while idle.
    pub(in crate::app) fn now_playing_title_text(&self, state: PlaybackState) -> Option<String> {
        if !state.active {
            return None;
        }
        state
            .active_idx
            .and_then(|idx| self.playback_queue().item_at(idx))
            .map(|item| item.title().to_string())
            .or_else(|| {
                self.slotless_playback_title_parts()
                    .map(|parts| parts.title.text)
            })
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
