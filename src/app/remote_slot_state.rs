use super::notify_actions::ToastSeverity;
use super::{App, RemoteSlotState};
use mbv_core::remote_player::DaemonEndpoint;

impl App {
    pub(super) fn is_local_daemon(&self) -> bool {
        matches!(self.player_endpoint, Some(DaemonEndpoint::Local))
    }

    /// Whether the Player owner lives in this process (bare mode): no daemon
    /// endpoint at all, i.e. the in-process embedded mpv. Distinct from
    /// same-machine ownership (`player_owner_is_on_this_machine`) which also
    /// includes the local daemon, and from launch mode (`home_is_local_daemon`
    /// / `launched_as_remote`) which records how we started, not where the
    /// owner lives now. Derived purely from `player_endpoint == None`.
    #[allow(dead_code)]
    pub(super) fn is_in_process_player_owner(&self) -> bool {
        self.player_endpoint.is_none()
    }

    pub(super) fn player_owner_is_on_this_machine(&self) -> bool {
        !matches!(
            self.player_endpoint,
            Some(DaemonEndpoint::Tcp(_) | DaemonEndpoint::Unix(_))
        )
    }

    pub(super) fn remote_slot_state(&self) -> RemoteSlotState {
        if self.connected_session_id.is_some() {
            RemoteSlotState::AttachedSession
        } else if self.player.is_remote() {
            if self.has_remote_queue() {
                RemoteSlotState::DirectRemote
            } else {
                RemoteSlotState::LocalDaemon
            }
        } else {
            RemoteSlotState::Off
        }
    }

    fn has_sessions_panel_connection(&self) -> bool {
        self.connected_session_id.is_some()
            || self.connected_session_state.is_some()
            || self.direct_remote_connected
    }

    pub(super) fn can_disconnect_remote(&self) -> bool {
        self.has_sessions_panel_connection()
    }

    pub(super) fn disconnect_remote(&mut self) {
        if self.connected_session_id.is_some() || self.connected_session_state.is_some() {
            self.connected_session_id = None;
            self.connected_session_state = None;
            self.retire_remote_tracking(false);
            self.session_miss_count = 0;
            self.remote_pos_s = 0;
            self.flash(
                "Disconnected from remote session".to_string(),
                ToastSeverity::Success,
            );
        } else if self.direct_remote_connected {
            self.restore_local_mode("Disconnected from direct remote session");
        } else {
            self.flash("No session selected".to_string(), ToastSeverity::Neutral);
        }
    }

    pub(super) fn sessions_overlay_footer(&self) -> &'static str {
        if self.can_disconnect_remote() {
            "[↵]conn [d]disc [r]refresh [Esc]close"
        } else {
            "[↵]conn [r]refresh [Esc]close"
        }
    }
}
