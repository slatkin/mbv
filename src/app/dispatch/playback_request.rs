//! The `App`-side effect of one `Msg::Playback` request. The TUI shell and the
//! GUI Client both apply it here, so each transport control does one thing in
//! both Clients (design D3 of `split-tui-gui-clients`: an inherent `impl App`
//! lives in the core, never in a Client's shell).

use crate::app::App;
use crate::app::dispatch::action::{AppCommand, PanelIntent};
use mbv_ui_msg::PlaybackRequest;

impl App {
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

    /// The App-side effect of one playback request (`Msg::Playback`). The TUI
    /// shell and the GUI Client both apply it here, so each transport control
    /// does one thing in both Clients.
    pub(in crate::app) fn handle_playback_request(&mut self, request: &PlaybackRequest) {
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
            PlaybackRequest::SeekTo(fraction) => self.seek_to_fraction(*fraction),
            // The canvas's back and forward controls emit the deltas the
            // `seek_back`/`seek_forward` chords carry, so the request
            // dispatches the same intent the chord resolves to
            // (add-tauri-gui-client row 6.4).
            PlaybackRequest::SeekRelative(delta) => {
                self.dispatch_playback(&AppCommand::Playback(PlaybackIntent::SeekRelative(*delta)));
            }
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
            // The track chips' popover opener: the GUI Client's chips emit it
            // (add-tauri-gui-client row 6.2). It dispatches the same intent
            // the `open_audio_popover`/`open_subtitle_popover` chords resolve
            // to (row 6.4), so the chip press and the chord run one dispatch.
            // The TUI Client paints no track chip, so no TUI leaf produces
            // the request.
            PlaybackRequest::OpenTrackPopover(chip) => {
                self.dispatch_playback(&AppCommand::Playback(PlaybackIntent::ToggleTrackPopover(
                    *chip,
                )));
            }
            // A track popover row's press (add-tauri-gui-client rows 6.4 and
            // 6.5), the canvas header's collapse and queue-fold controls (row
            // 6.4), the player panel's right-click menu open and its entry
            // press (gui-player-overlay-menu), and one Queue chooser entry's
            // press (task 8.3 of `gui-named-queues`). Each is GUI presentation,
            // so its core half is the documented no-op of design D4a (a): the
            // GUI Client applies it after the core at its own entry
            // (`input/gui.rs`). No TUI leaf produces any of them.
            PlaybackRequest::TrackPopoverSelect(_)
            | PlaybackRequest::TogglePanelCollapse
            | PlaybackRequest::ToggleQueueFold
            | PlaybackRequest::OpenPlayerMenu { .. }
            | PlaybackRequest::PlayerMenuSelect
            | PlaybackRequest::QueueChooserEntry(_) => {}
            // The canvas queue header's filter control (row 6.5): the same
            // effect the `/` chord dispatches, so the header control and the
            // chord run one path.
            PlaybackRequest::ActivateQueueFilter => self.activate_queue_filter(),
            // The canvas queue header's three queue controls (design D8 of
            // `gui-named-queues`): each dispatches the same panel intent its
            // GUI-only chord resolves to, so the control's press and the chord
            // run one path. The TUI paints no such control; these arms are here
            // for the shared dispatch's exhaustiveness.
            PlaybackRequest::QueueChooserToggle => {
                self.dispatch_panel(PanelIntent::ToggleQueueChooser);
            }
            PlaybackRequest::NewQueue => self.dispatch_panel(PanelIntent::RequestNewQueue),
            PlaybackRequest::RenameQueue => self.dispatch_panel(PanelIntent::RequestRenameQueue),
            PlaybackRequest::CloseQueue => self.dispatch_panel(PanelIntent::RequestCloseQueue),
        }
    }

    /// Dispatch one panel intent from a GUI header control's press: the same
    /// dispatch the intent's own chord takes.
    fn dispatch_panel(&mut self, intent: PanelIntent) {
        let _ = self.dispatch(&AppCommand::Panel(intent));
    }

    fn dispatch_playback(&mut self, command: &AppCommand) {
        let _ = self.dispatch(command);
    }
}
