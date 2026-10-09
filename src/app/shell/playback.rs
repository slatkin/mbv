use super::Model;
use super::{PanelFocus, palette};
use mbv_components::PlaybackProjection;
use mbv_render::components::chrome_player::TransportAvailability;
use mbv_ui_msg::PlaybackRequest;

impl crate::app::App {
    pub(in crate::app) fn slotless_playback_title_parts(
        &self,
    ) -> Option<mbv_queue::PlaybackTitleParts> {
        let cast_title = self
            .cast_attachment
            .as_ref()
            .and_then(crate::app::App::cast_now_playing_title);
        if let Some(title) = cast_title {
            return Some(mbv_queue::PlaybackTitleParts {
                title: mbv_queue::PlaybackTitlePart {
                    role: mbv_queue::PlaybackTitlePartRole::Title,
                    text: title,
                },
                context: None,
            });
        }
        let session = self.connected_session_state.as_ref()?;
        let title = session.now_playing.clone()?;
        // An Emby session's now-playing payload names the show for an episode;
        // with it, the slotless path draws the same two-part title the local
        // queue item draws (remote-session-overlay-parity, task 2.1).
        let context = (session.now_playing_item_type.as_deref() == Some("Episode"))
            .then(|| session.now_playing_series_name.clone())
            .flatten()
            .filter(|name| !name.trim().is_empty())
            .map(|text| mbv_queue::PlaybackTitlePart {
                role: mbv_queue::PlaybackTitlePartRole::Context,
                text,
            });
        Some(mbv_queue::PlaybackTitleParts {
            title: mbv_queue::PlaybackTitlePart {
                role: mbv_queue::PlaybackTitlePartRole::Title,
                text: title,
            },
            context,
        })
    }
}

impl Model {
    /// The shared transport projection both playback panels consume (task
    /// 3.5, D10): one builder so the queue-column panel and the right-column
    /// strip cannot drift in the facts they show. The caller sets its own
    /// panel surface identity on top.
    pub(in crate::app) fn transport_projection(&mut self) -> PlaybackProjection {
        // Presentation: a selected-but-unconfirmed slot already paints as the
        // playhead here, at a fresh start, so the panel's title, artwork and
        // time describe the same item the queue row highlights.
        let state = self.app.displayed_playback_state();
        // The typed title parts are built from the queue item itself (D1/D4)
        // whenever the active slot is addressable locally; the fallback
        // title below covers only the targets the queue cannot address.
        // One queue lookup feeds both: the parts and the plain title
        // describe the same slot.
        let active_item = state
            .active_idx
            .filter(|_| state.active)
            .and_then(|idx| self.app.playback_queue().item_at(idx));
        let slotless_title_parts = (state.active && active_item.is_none())
            .then(|| self.app.slotless_playback_title_parts())
            .flatten();
        let title_parts = active_item
            .map(|item| self.app.playback_title_parts(item))
            .or_else(|| slotless_title_parts.clone());
        let title = if state.active {
            active_item
                .map(|item| item.title().to_string())
                .or_else(|| slotless_title_parts.map(|parts| parts.title.text))
        } else if let Some(cast) = self.app.cast_attachment.as_ref() {
            crate::app::App::cast_now_playing_title(cast)
        } else {
            self.app
                .connected_session_state
                .as_ref()
                .and_then(|session| session.now_playing.clone())
        };
        let now_playing_title = title.map(|title| (title, palette::Role::PlaybackValueFg.color()));
        let show_controls = state.active
            || self.app.connected_session_id.is_some()
            || self.app.cast_attachment.is_some();
        let (prev_available, next_available) = self.app.transport_prev_next_available();
        PlaybackProjection {
            state,
            show_controls,
            // The panel's own identity plus the site's focus bit; the painter
            // resolves the fill through the surface table. Each sync sets the
            // site's surface on top of this default.
            panel: palette::Surface::PlaybackPanel,
            panel_focused: matches!(self.app.effective_panel_focus(), PanelFocus::Queue),
            now_playing_title,
            title_parts,
            title_site: self.app.queue_card_projection.title_site,
            status_indicators: self.app.build_status_indicator_spans(),
            idle_feed_title: self.app.idle_feed.as_ref().and_then(|feed| {
                feed.items.get(feed.current_index).map(|item| {
                    (
                        item.title.clone(),
                        item.link.as_deref().is_some_and(|link| !link.is_empty()),
                    )
                })
            }),
            use_nerd_fonts: self.app.use_nerd_fonts,
            availability: TransportAvailability {
                stop: self.app.connected_session_id.is_some() || state.active,
                previous: prev_available,
                next: next_available,
            },
        }
    }

    pub(in crate::app) fn handle_playback_request(&mut self, request: &PlaybackRequest) {
        use crate::app::dispatch::action::Command;
        match request {
            PlaybackRequest::TogglePlayPause => self.dispatch_playback(&Command::TogglePlayPause),
            PlaybackRequest::Stop => self.dispatch_playback(&Command::Stop),
            PlaybackRequest::Previous => self.dispatch_playback(&Command::PreviousTrack),
            PlaybackRequest::Next => self.dispatch_playback(&Command::NextTrack),
            PlaybackRequest::SeekTo(fraction) => self.app.seek_to_fraction(*fraction),
            PlaybackRequest::ToggleMute => self.dispatch_playback(&Command::ToggleMute),
            PlaybackRequest::VolumeDelta(delta) => {
                self.dispatch_playback(&Command::AdjustVolume(*delta));
            }
            PlaybackRequest::CycleAudio => {
                self.dispatch_playback(&Command::CycleAudio);
            }
            PlaybackRequest::CycleSubtitle => {
                self.dispatch_playback(&Command::CycleOrToggleSubtitle);
            }
        }
    }

    fn dispatch_playback(&mut self, command: &crate::app::dispatch::action::Command) {
        let _ = self.app.dispatch(command);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mbv_ui_msg::Msg;
    use rstest::rstest;
    use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};

    /// A watched Emby session's now-playing episode draws the same two-part
    /// title local playback draws when the payload names the show; every
    /// payload without that fact keeps the one-part session-name title
    /// (remote-session-overlay-parity, task 2.1).
    #[rstest]
    #[case::episode_with_series_name("Episode", Some("Show"), Some("Show"))]
    #[case::episode_without_series_name("Episode", None, None)]
    #[case::movie_with_stray_series_name("Movie", Some("Show"), None)]
    fn slotless_title_parts_split_only_episode_series_names(
        #[case] item_type: &str,
        #[case] series_name: Option<&str>,
        #[case] expected_context: Option<&str>,
    ) {
        let app = crate::app::tests::make_app_stub();
        let mut session = mbv_emby::test_support::make_session("tv", "mbv");
        session.now_playing = Some("Episode name".into());
        session.now_playing_item_type = Some(item_type.to_string());
        session.now_playing_series_name = series_name.map(str::to_string);
        let mut app = app;
        app.connected_session_state = Some(session);

        let parts = app.slotless_playback_title_parts().expect("title present");

        assert_eq!(parts.title.text, "Episode name");
        assert_eq!(
            parts.context.map(|part| part.text).as_deref(),
            expected_context
        );
    }

    #[test]
    fn playback_chrome_request_routes_through_shell_authority() {
        let app = crate::app::tests::make_app_stub();
        let mut model = Model::new(app);
        model.handle_playback_request(&PlaybackRequest::VolumeDelta(5));
        assert!(matches!(
            Msg::Playback(PlaybackRequest::VolumeDelta(5)),
            Msg::Playback(PlaybackRequest::VolumeDelta(_))
        ));
        let _ = Event::<mbv_ui_msg::UserEvent>::Keyboard(KeyEvent {
            code: Key::Char('m'),
            modifiers: KeyModifiers::NONE,
        });
    }
}
