use crate::{SessionInfo, SessionMediaInfo};

#[must_use]
pub fn make_session(device_name: &str, client: &str) -> SessionInfo {
    SessionInfo {
        id: "sess-1".into(),
        device_name: device_name.into(),
        client: client.into(),
        user_name: "user".into(),
        host: "127.0.0.1".into(),
        supported_commands: Vec::new(),
        playable_media_types: Vec::new(),
        now_playing: None,
        now_playing_item_id: None,
        now_playing_item_type: None,
        now_playing_series_id: None,
        now_playing_series_name: None,
        now_playing_logo_etag: None,
        position_s: 0,
        runtime_s: 0,
        position_ticks: 0,
        runtime_ticks: 0,
        is_paused: false,
        volume: 100,
        sub_index: -1,
        audio_index: 1,
        muted: false,
        media_info: SessionMediaInfo::default(),
    }
}
