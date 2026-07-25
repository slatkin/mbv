use super::{App, RemoteSlotState};

impl App {
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
            self.session_miss_count = 0;
            self.remote_pos_s = 0;
            self.flash_status("Disconnected from remote session".to_string());
        } else if self.direct_remote_connected {
            self.restore_local_mode("Disconnected from direct remote session");
        } else {
            self.flash_status("No session connected".to_string());
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
