use super::components::{PlaybackComponent, PlaybackProjection, PlaybackRequest};
use super::shell::Model;
use super::{palette, PanelFocus, PanelMode};

impl Model {
    pub(super) fn sync_playback(&mut self) {
        let id = super::components::ComponentId::Playback;
        if !self.application.mounted(&id) {
            self.application
                .mount(id.clone(), Box::new(PlaybackComponent::new()), vec![])
                .expect("mount Playback");
        }
        let state = self.app.effective_playback_state();
        let title = if state.active {
            self.app
                .playback_queue()
                .item_at(state.active_idx)
                .map(|item| item.title().to_string())
                // `effective_playback_state` reports `active` for a cast target,
                // but the local queue may hold no matching slot (a cast with an
                // empty local queue). Fall back to the cast title so the
                // component paints what the legacy player chrome used to (3.9).
                .or_else(|| {
                    self.app
                        .cast_attachment
                        .as_ref()
                        .and_then(|cast| self.app.cast_now_playing_title(cast))
                })
        } else if let Some(cast) = self.app.cast_attachment.as_ref() {
            self.app.cast_now_playing_title(cast)
        } else {
            self.app
                .connected_session_state
                .as_ref()
                .and_then(|session| session.now_playing.clone())
        };
        let now_playing_title = title
            .clone()
            .map(|title| (title, palette::PLAYBACK_VALUE_FG));
        let show_controls = state.active
            || self.app.connected_session_id.is_some()
            || self.app.cast_attachment.is_some();
        let focused = matches!(self.app.effective_panel_focus(), PanelFocus::Queue);
        let projection = PlaybackProjection {
            state,
            player_area: self.app.layout.playback.player_area,
            show_controls,
            player_h: self.app.layout.playback.player_area.height.max(4),
            panel_bg: if focused {
                palette::SURFACE_FOCUSED
            } else {
                palette::SURFACE_PLAYBACK
            },
            narrow_player: self.app.effective_panel_mode() == PanelMode::QueueOnly,
            now_playing_title: now_playing_title.clone(),
            title_parts: now_playing_title
                .as_ref()
                .map(|(title, color)| self.app.playback_title_parts(title, *color))
                .unwrap_or_default(),
            status_indicators: self.app.build_status_indicator_spans(),
            throbber: self.app.now_playing_throbber_span(),
            idle_feed_title: self.app.idle_feed.as_ref().and_then(|feed| {
                feed.items.get(feed.current_index).map(|item| {
                    (
                        item.title.clone(),
                        item.link.as_deref().is_some_and(|link| !link.is_empty()),
                    )
                })
            }),
            use_nerd_fonts: self.app.use_nerd_fonts,
            stop_available: self.app.connected_session_id.is_some() || state.active,
            next_available: self.app.transport_prev_next_available().1,
        };
        if let Some(comp) = self.application.get_component_mut(&id) {
            if let Some(playback) = comp.as_any_mut().downcast_mut::<PlaybackComponent>() {
                playback.set_projection(projection);
            }
        }
    }

    pub(super) fn render_playback_component(&mut self, frame: &mut ratatui::Frame) {
        let id = super::components::ComponentId::Playback;
        if self.application.mounted(&id) {
            self.application.view(&id, frame, frame.area());
        }
    }

    pub(super) fn handle_playback_request(&mut self, request: PlaybackRequest) {
        use super::action::Command;
        match request {
            PlaybackRequest::TogglePlayPause => self.dispatch_playback(Command::TogglePlayPause),
            PlaybackRequest::Stop => self.dispatch_playback(Command::Stop),
            PlaybackRequest::Previous => self.dispatch_playback(Command::PreviousTrack),
            PlaybackRequest::Next => self.dispatch_playback(Command::NextTrack),
            PlaybackRequest::SeekRelative(seconds) => {
                self.dispatch_playback(Command::SeekRelative(seconds as f64));
            }
            PlaybackRequest::SeekTo(fraction) => self.app.seek_to_fraction(fraction),
            PlaybackRequest::ToggleMute => self.dispatch_playback(Command::ToggleMute),
            PlaybackRequest::VolumeDelta(delta) => {
                self.dispatch_playback(Command::AdjustVolume(delta));
            }
            PlaybackRequest::CycleAudio => {
                self.dispatch_playback(Command::ToggleMuteOrCycleAudio);
            }
            PlaybackRequest::CycleSubtitle => {
                self.dispatch_playback(Command::CycleOrToggleSubtitle);
            }
            PlaybackRequest::ToggleVisualizer => {
                let _ = self.app.dispatch(super::action::Command::ToggleVisualizer);
            }
        }
    }

    fn dispatch_playback(&mut self, command: super::action::Command) {
        let _ = self.app.dispatch(command);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::components::Msg;
    use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};

    #[test]
    fn playback_chrome_request_routes_through_shell_authority() {
        let app = super::super::tests::make_app_stub();
        let mut model = Model::new(app);
        model.handle_playback_request(PlaybackRequest::VolumeDelta(5));
        assert!(matches!(
            Msg::Playback(PlaybackRequest::VolumeDelta(5)),
            Msg::Playback(PlaybackRequest::VolumeDelta(_))
        ));
        let _ = Event::<super::super::components::UserEvent>::Keyboard(KeyEvent {
            code: Key::Char('m'),
            modifiers: KeyModifiers::NONE,
        });
    }
}
