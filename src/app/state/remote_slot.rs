use crate::app::dispatch::notify::ToastSeverity;
use crate::app::{App, RemoteSlotState};
use mbv_ctrl::{CtrlCmd, OutputSelection};
use mbv_remote_player::DaemonEndpoint;

impl App {
    /// Whether the local Owner currently plays (design D6's Hand off gate):
    /// its status while the home link is suspended behind a peer
    /// connection, else the current link's status — the same link when no
    /// connection is attached. `active` alone only marks a loaded item; the
    /// paused flag decides audible playback, as `now_playing_status` does.
    pub(in crate::app) fn local_owner_plays(&self) -> bool {
        let status = match &self.suspended_local {
            Some(suspended) => suspended.player.status_snapshot(),
            None => self.player.status_snapshot(),
        };
        status.active && !status.paused
    }

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
        if self.has_remote_queue() {
            RemoteSlotState::DirectRemote
        } else {
            RemoteSlotState::LocalDaemon
        }
    }

    /// Whether the owner's Emby Session output advertises an audio-only
    /// playable media set (design D12). An absent or empty advertisement is
    /// unknown and therefore treated as able to play video.
    pub(in crate::app) fn session_owner_is_audio_only(&self) -> bool {
        matches!(
            &self.playback_output,
            Some(mbv_ctrl::OutputState::EmbySession {
                audio_only: true,
                ..
            })
        )
    }

    /// The item the owner's Emby Session output presents that the owner did
    /// not load (design D12). The Owner sends this from `OutputState` alone;
    /// the loaded item is presented from the queue.
    pub(in crate::app) fn emby_session_now_playing(&self) -> Option<&mbv_ctrl::SessionNowPlaying> {
        match &self.playback_output {
            Some(mbv_ctrl::OutputState::EmbySession { now_playing, .. }) => now_playing.as_ref(),
            Some(mbv_ctrl::OutputState::Cast { .. } | mbv_ctrl::OutputState::Local) | None => None,
        }
    }

    /// Direct remote control's session id, which the Sessions sidebar marks.
    /// The Stay-alive process does not count: it is not a remote session. An
    /// Emby Session output marks by device id
    /// (`sessions_panel_connected_device_id`).
    pub(in crate::app) fn sessions_panel_connected_id(&self) -> Option<&str> {
        self.direct_remote_session_id.as_deref()
    }

    /// The device id of the owner's Emby Session output, which the Sessions
    /// sidebar uses to mark its row (design D12).
    pub(in crate::app) fn sessions_panel_connected_device_id(&self) -> Option<&str> {
        match &self.playback_output {
            Some(mbv_ctrl::OutputState::EmbySession { device_id, .. }) => Some(device_id),
            Some(mbv_ctrl::OutputState::Cast { .. } | mbv_ctrl::OutputState::Local) | None => None,
        }
    }

    pub(in crate::app) fn can_disconnect_remote(&self) -> bool {
        self.direct_remote_connected
    }

    pub(in crate::app) fn disconnect_remote(&mut self) {
        if self.direct_remote_connected {
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

    /// Whether the current player connection's owner reports a remote output
    /// (a Cast receiver or an Emby Session), read synchronously from the
    /// connection's retained state — not the drained `playback_output` mirror,
    /// which stays `None` until the shell drains the cold-seed event. Launch
    /// auto-reconnect connects a session (and severs) before any drain ever
    /// happens, so the hand-back decision reads the connection's own record,
    /// which the handshake fills at the cold seed (cast-owner-output 8.2 fix).
    fn player_output_is_remote(&self) -> bool {
        matches!(
            self.player.remote().playback_output_state(),
            Some(mbv_ctrl::OutputState::Cast { .. } | mbv_ctrl::OutputState::EmbySession { .. })
        )
    }

    /// Hand the Local daemon owner's remote output back to Local (design D13):
    /// `Stop` first so no slot is active at the hand-off (D10 step 1), then
    /// `SelectPlaybackOutput(Local)` — a Cast receiver stops, an Emby Session
    /// stops, and local playback does not start. The decision reads the
    /// current owner connection's retained output state
    /// (`player_output_is_remote`), so the send happens only when that owner is
    /// the Local daemon: a remote owner's own output is not ours to stop.
    /// Best-effort like every other fire-and-forget player send.
    pub(in crate::app) fn hand_remote_output_back_to_local(&mut self) {
        if self.is_local_daemon() && self.player_output_is_remote() {
            tracing::info!(name: "sessions.output_handback", target: "sessions", "stopping the remote output and returning the Local owner's output to Local");
            let _ = self.player.send_ctrl_cmd(CtrlCmd::Stop);
            let _ = self
                .player
                .send_ctrl_cmd(CtrlCmd::SelectPlaybackOutput(OutputSelection::Local));
        }
    }

    /// Connecting to a new target severs the current one: the watched
    /// session, direct remote, and library-route slots are mutually
    /// exclusive, so a new connect tears the old target down instead of
    /// holding both. The owner's remote output is handed back to Local first
    /// (`hand_remote_output_back_to_local`); a severed cast receiver is left
    /// as it is ("the receiver owns what it plays"). No-op when nothing
    /// else is connected.
    pub(in crate::app) fn sever_active_connection(&mut self) {
        self.hand_remote_output_back_to_local();
        self.sever_connections_without_output_change();
    }

    /// The same severing without the Cast-output hand-back, used when the
    /// new target is itself a Cast receiver: the owner performs the
    /// Cast-to-Cast hand-off, and a prior `Stop` here would clear the active
    /// slot the new receiver must continue (spec scenario "A different cast
    /// target is selected while one is attached").
    pub(in crate::app) fn sever_connections_without_output_change(&mut self) {
        if self.active_route.is_some() {
            self.restore_local_mode("Local playback restored before connecting");
        }
        if self.can_disconnect_remote() {
            self.disconnect_remote();
        }
    }
}
