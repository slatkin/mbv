use super::notify_actions::ToastSeverity;
use super::{App, RemoteSlotState};
use mbv_core::remote_player::DaemonEndpoint;

impl App {
    pub(super) fn is_local_daemon(&self) -> bool {
        matches!(self.player_endpoint, Some(DaemonEndpoint::Local))
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

    /// Whether the attached Emby session advertises an audio-only playable
    /// media set. An absent or empty advertisement is unknown and therefore
    /// treated as able to play video.
    // This query is introduced ahead of the explicit-play guard that consumes
    // it in the next implementation unit.
    pub(super) fn session_owner_is_audio_only(&self) -> bool {
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

    pub(super) fn can_disconnect_remote(&self) -> bool {
        self.connected_session_id.is_some()
            || self.connected_session_state.is_some()
            || self.direct_remote_connected
    }

    pub(super) fn disconnect_remote(&mut self) {
        if self.connected_session_id.is_some() || self.connected_session_state.is_some() {
            self.connected_session_id = None;
            self.connected_session_state = None;
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

    /// Connecting to a new target severs the current one: the cast
    /// attachment, watched session, direct remote, and library-route slots
    /// are mutually exclusive, so a new connect tears the old target down
    /// instead of holding both. The severed cast receiver is left as it is
    /// ("the receiver owns what it plays"); the dropped transport ends
    /// mbv's connection to it. No-op when nothing is connected.
    pub(super) fn sever_active_connection(&mut self) {
        if self.cast_attachment.take().is_some() {
            self.stop_visualizer_worker();
        }
        if self.active_route.is_some() {
            self.restore_local_mode("Local playback restored before connecting");
        }
        if self.can_disconnect_remote() {
            self.disconnect_remote();
        }
    }
}
