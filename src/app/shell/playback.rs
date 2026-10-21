use super::Model;
use super::{PanelFocus, palette};
use mbv_components::PlaybackProjection;
use mbv_render::components::chrome_player::TransportAvailability;
use mbv_ui_msg::PlaybackRequest;

impl crate::app::App {
    pub(in crate::app) fn slotless_playback_title_parts(
        &self,
    ) -> Option<mbv_queue::PlaybackTitleParts> {
        let now_playing = self.emby_session_now_playing()?;
        let title = now_playing.name.clone();
        // An Emby Session's now-playing payload names the show for an episode;
        // with it, the slotless path draws the same two-part title the local
        // queue item draws (remote-session-overlay-parity, task 2.1;
        // emby-session-owner-output task 7.2).
        let context = (now_playing.item_type == "Episode")
            .then(|| now_playing.series_name.clone())
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
        let title = state
            .active
            .then(|| {
                active_item
                    .map(|item| item.title().to_string())
                    .or_else(|| slotless_title_parts.map(|parts| parts.title.text))
            })
            .flatten();
        let now_playing_title = title.map(|title| (title, palette::Role::PlaybackValueFg.color()));
        let show_controls = state.active || self.app.output_is_cast();
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
                stop: state.active,
                previous: prev_available,
                next: next_available,
            },
        }
    }

    pub(in crate::app) fn handle_playback_request(&mut self, request: &PlaybackRequest) {
        use crate::app::dispatch::action::AppCommand;
        use crate::app::dispatch::action::PlaybackIntent;
        match request {
            PlaybackRequest::TogglePlayPause => {
                self.dispatch_playback(&AppCommand::Playback(PlaybackIntent::TogglePlayPause));
            }
            PlaybackRequest::Stop => {
                self.dispatch_playback(&AppCommand::Playback(PlaybackIntent::Stop));
            }
            PlaybackRequest::Previous => {
                self.dispatch_playback(&AppCommand::Playback(PlaybackIntent::PreviousTrack));
            }
            PlaybackRequest::Next => {
                self.dispatch_playback(&AppCommand::Playback(PlaybackIntent::NextTrack));
            }
            PlaybackRequest::SeekTo(fraction) => self.app.seek_to_fraction(*fraction),
            PlaybackRequest::ToggleMute => {
                self.dispatch_playback(&AppCommand::Playback(PlaybackIntent::ToggleMute));
            }
            PlaybackRequest::VolumeDelta(delta) => {
                self.dispatch_playback(&AppCommand::Playback(PlaybackIntent::AdjustVolume(*delta)));
            }
            PlaybackRequest::CycleAudio => {
                self.dispatch_playback(&AppCommand::Playback(PlaybackIntent::CycleAudio));
            }
            PlaybackRequest::CycleSubtitle => {
                self.dispatch_playback(&AppCommand::Playback(
                    PlaybackIntent::CycleOrToggleSubtitle,
                ));
            }
        }
    }

    fn dispatch_playback(&mut self, command: &crate::app::dispatch::action::AppCommand) {
        let _ = self.app.dispatch(command);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::tests::QueueViewTestExt;
    use mbv_ui_msg::Msg;
    use rstest::rstest;
    use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};

    /// The owner's Emby Session output presents an episode outside the local
    /// queue; its now-playing payload draws the same two-part title local
    /// playback draws when it names the show, and every payload without that
    /// fact keeps the one-part title (remote-session-overlay-parity, task 2.1;
    /// emby-session-owner-output task 7.2 reads the `OutputState` source).
    #[rstest]
    #[case::episode_with_series_name("Episode", Some("Show"), Some("Show"))]
    #[case::episode_without_series_name("Episode", None, None)]
    #[case::movie_with_stray_series_name("Movie", Some("Show"), None)]
    fn slotless_title_parts_split_only_episode_series_names(
        #[case] item_type: &str,
        #[case] series_name: Option<&str>,
        #[case] expected_context: Option<&str>,
    ) {
        let mut app = crate::app::tests::make_app_stub();
        app.playback_output = Some(mbv_ctrl::OutputState::EmbySession {
            device_id: "device-1".to_string(),
            name: "Living Room TV".to_string(),
            connection: mbv_ctrl::OutputConnection::Connected,
            audio_only: false,
            now_playing: Some(mbv_ctrl::SessionNowPlaying {
                item_id: "device-episode".to_string(),
                item_type: item_type.to_string(),
                name: "Episode name".to_string(),
                series_id: Some("device-series".to_string()),
                series_name: series_name.map(str::to_string),
                logo_tag: None,
            }),
        });

        let parts = app.slotless_playback_title_parts().expect("title present");

        assert_eq!(parts.title.text, "Episode name");
        assert_eq!(
            parts.context.map(|part| part.text).as_deref(),
            expected_context
        );
    }

    /// Review r7: foreign content on the owner's Emby Session output must
    /// present its own title, not the owner's loaded queue slot. The Owner's
    /// status still carries that slot's index; the projection must treat the
    /// output as slotless whenever it presents `now_playing`. Guards commit
    /// bc7690c38 (present foreign Emby Session content as slotless).
    #[test]
    fn foreign_emby_session_content_presents_its_own_transport_title() {
        use mbv_emby_model::test_support::make_item;

        let mut app = crate::app::tests::make_app_stub();
        let mut loaded = make_item("Loaded Slot", "Movie");
        loaded.id = "loaded-slot".into();
        app.local_view.adopt_items(vec![loaded], 0);
        app.player.update_status(|status| {
            status.active = true;
            status.current_idx = 0;
            status.queue_len = 1;
        });
        app.playback_output = Some(mbv_ctrl::OutputState::EmbySession {
            device_id: "device-1".to_string(),
            name: "Living Room TV".to_string(),
            connection: mbv_ctrl::OutputConnection::Connected,
            audio_only: false,
            now_playing: Some(mbv_ctrl::SessionNowPlaying {
                item_id: "foreign-item".to_string(),
                item_type: "Movie".to_string(),
                name: "Foreign Item".to_string(),
                series_id: None,
                series_name: None,
                logo_tag: None,
            }),
        });

        let mut model = Model::new(app);
        let projection = model.transport_projection();

        assert_eq!(
            projection
                .title_parts
                .as_ref()
                .map(|parts| parts.title.text.as_str()),
            Some("Foreign Item")
        );
        assert!(projection.state.active_idx.is_none());
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
