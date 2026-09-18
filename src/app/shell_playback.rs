use super::components::{PlaybackProjection, PlaybackRequest};
use super::shell::Model;
use super::{palette, PanelFocus};

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
        let title_parts = active_item.map(|item| self.app.playback_title_parts(item));
        let title = if state.active {
            active_item
                .map(|item| item.title().to_string())
                // `effective_playback_state` reports `active` for a cast target
                // or watched remote Session, but the local queue may hold no
                // matching slot (a cast with an empty local queue, a remote
                // device playing its own selection). Fall back to the attached
                // target's now-playing title so the panel paints what the
                // legacy player chrome used to (3.9).
                .or_else(|| {
                    self.app
                        .cast_attachment
                        .as_ref()
                        .and_then(|cast| self.app.cast_now_playing_title(cast))
                })
                .or_else(|| {
                    self.app
                        .connected_session_state
                        .as_ref()
                        .and_then(|session| session.now_playing.clone())
                })
        } else if let Some(cast) = self.app.cast_attachment.as_ref() {
            self.app.cast_now_playing_title(cast)
        } else {
            self.app
                .connected_session_state
                .as_ref()
                .and_then(|session| session.now_playing.clone())
        };
        let now_playing_title = title.map(|title| (title, palette::PLAYBACK_VALUE_FG));
        let show_controls = state.active
            || self.app.connected_session_id.is_some()
            || self.app.cast_attachment.is_some();
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
            stop_available: self.app.connected_session_id.is_some() || state.active,
            prev_available: self.app.transport_prev_next_available().0,
            next_available: self.app.transport_prev_next_available().1,
        }
    }

    pub(super) fn handle_playback_request(&mut self, request: PlaybackRequest) {
        use super::action::Command;
        match request {
            PlaybackRequest::TogglePlayPause => self.dispatch_playback(Command::TogglePlayPause),
            PlaybackRequest::Stop => self.dispatch_playback(Command::Stop),
            PlaybackRequest::Previous => self.dispatch_playback(Command::PreviousTrack),
            PlaybackRequest::Next => self.dispatch_playback(Command::NextTrack),
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
