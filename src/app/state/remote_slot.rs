use crate::app::dispatch::notify::ToastSeverity;
use crate::app::{App, RemoteSlotState};
use mbv_ctrl::{CtrlCmd, OutputSelection};
use mbv_remote_player::DaemonEndpoint;

impl App {
    pub(in crate::app) fn is_local_daemon(&self) -> bool {
        matches!(self.player_endpoint, Some(DaemonEndpoint::Local))
    }

    pub(in crate::app) fn player_owner_is_on_this_machine(&self) -> bool {
        !matches!(
            self.player_endpoint,
            Some(DaemonEndpoint::Tcp(_) | DaemonEndpoint::Unix(_))
        )
    }

    pub(in crate::app) fn remote_slot_state(&self) -> RemoteSlotState {
        if self.connected_session_id.is_some() {
            RemoteSlotState::AttachedSession
        } else if self.has_remote_queue() {
            RemoteSlotState::DirectRemote
        } else {
            RemoteSlotState::LocalDaemon
        }
    }

    /// Whether the attached Emby session advertises an audio-only playable
    /// media set. An absent or empty advertisement is unknown and therefore
    /// treated as able to play video.
    pub(in crate::app) fn session_owner_is_audio_only(&self) -> bool {
        self.connected_session_id.is_some()
            && self
                .connected_session_state
                .as_ref()
                .is_some_and(|session| {
                    !session.playable_media_types.is_empty()
                        && session
                            .playable_media_types
                            .iter()
                            .all(|media_type| media_type.eq_ignore_ascii_case("Audio"))
                })
    }

    /// Emby session id the Sessions sidebar should mark. Session watch and
    /// Direct remote control of a remote owner both count. The Stay-alive
    /// process does not: it is not a remote session.
    pub(in crate::app) fn sessions_panel_connected_id(&self) -> Option<&str> {
        self.connected_session_id
            .as_deref()
            .or(self.remote.direct_remote_session_id.as_deref())
    }

    pub(in crate::app) fn can_disconnect_remote(&self) -> bool {
        self.connected_session_id.is_some()
            || self.connected_session_state.is_some()
            || self.remote.direct_remote_connected
    }

    pub(in crate::app) fn disconnect_remote(&mut self) {
        if self.connected_session_id.is_some() || self.connected_session_state.is_some() {
            self.connected_session_id = None;
            self.connected_session_state = None;
            self.remote.session_miss_count = 0;
            self.remote.remote_pos_s = 0;
            self.flash(
                "Disconnected from remote session".to_string(),
                ToastSeverity::Success,
            );
        } else if self.remote.direct_remote_connected {
            self.restore_local_mode("Disconnected from direct remote session");
        } else {
            self.flash("No session selected".to_string(), ToastSeverity::Neutral);
        }
    }

    /// Send a playback-output selection to the Player owner (design D13).
    /// The owner is the only authority over the output: it performs the
    /// hand-off, tears the previous output down, and broadcasts the
    /// resulting `CtrlEvent::PlaybackOutput` that every Client mirrors into
    /// `playback_output`. Best-effort like every other fire-and-forget
    /// player send.
    pub(in crate::app) fn select_playback_output(&mut self, selection: OutputSelection) {
        let _ = self
            .player
            .send_ctrl_cmd(CtrlCmd::SelectPlaybackOutput(selection));
    }

    /// Hand the Local daemon owner's Cast output back to Local (design D13):
    /// `Stop` first so no slot is active at the hand-off (D10 step 1), then
    /// `SelectPlaybackOutput(Local)` — the receiver stops and local playback
    /// does not start. `playback_output` mirrors the current owner's last
    /// `CtrlEvent::PlaybackOutput`, so the send happens only when that owner
    /// is the Local daemon: a remote owner's own Cast output is not ours to
    /// stop. Best-effort like every other fire-and-forget player send.
    pub(in crate::app) fn hand_cast_output_back_to_local(&mut self) {
        if self.is_local_daemon() && self.output_is_cast() {
            tracing::info!(name: "sessions.output_handback", target: "sessions", "stopping the receiver and returning the Local owner's output to Local");
            let _ = self.player.send_ctrl_cmd(CtrlCmd::Stop);
            let _ = self
                .player
                .send_ctrl_cmd(CtrlCmd::SelectPlaybackOutput(OutputSelection::Local));
        }
    }

    /// Connecting to a new target severs the current one: the cast
    /// attachment, watched session, direct remote, and library-route slots
    /// are mutually exclusive, so a new connect tears the old target down
    /// instead of holding both. The owner's Cast output is handed back to
    /// Local first (`hand_cast_output_back_to_local`); the severed cast
    /// receiver is left as it is ("the receiver owns what it plays"); the
    /// dropped transport ends mbv's connection to it. No-op when nothing
    /// else is connected.
    pub(in crate::app) fn sever_active_connection(&mut self) {
        self.hand_cast_output_back_to_local();
        self.sever_connections_without_output_change();
    }

    /// The same severing without the Cast-output hand-back, used when the
    /// new target is itself a Cast receiver: the owner performs the
    /// Cast-to-Cast hand-off, and a prior `Stop` here would clear the active
    /// slot the new receiver must continue (spec scenario "A different cast
    /// target is selected while one is attached").
    pub(in crate::app) fn sever_connections_without_output_change(&mut self) {
        if self.cast_attachment.take().is_some() {
            self.stop_visualizer_capture();
        }
        if self.active_route.is_some() {
            self.restore_local_mode("Local playback restored before connecting");
        }
        if self.can_disconnect_remote() {
            self.disconnect_remote();
        }
    }
}
