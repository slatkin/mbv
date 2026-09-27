pub use crate::config::Config;
use mbv_ids::{EmbySessionId, MediaSourceId};

#[must_use]
pub fn gen_session_id() -> EmbySessionId {
    EmbySessionId::new(uuid::Uuid::new_v4().simple().to_string())
}

#[must_use]
pub fn device_name() -> String {
    std::fs::read_to_string("/etc/hostname")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| {
            std::env::var("HOSTNAME")
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        })
        .unwrap_or_else(|| "mbv".to_string())
}

/// Return mbv's stable, non-secret device identifier.
pub fn device_id() -> String {
    let data_home = std::env::var("XDG_DATA_HOME")
        .ok()
        .filter(|s| !s.is_empty())
        .map_or_else(
            || {
                let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
                std::path::PathBuf::from(home).join(".local/share")
            },
            std::path::PathBuf::from,
        );
    device_id_in(&data_home)
}

fn device_id_in(data_home: &std::path::Path) -> String {
    let dir = data_home.join("mbv");
    let path = dir.join("device_id");
    if let Ok(id) = std::fs::read_to_string(&path) {
        let id = id.trim().to_string();
        if !id.is_empty() {
            return id;
        }
    }
    // Migrate device_id from the old "mby" directory so Emby recognises this as the same client.
    let legacy = data_home.join("mby").join("device_id");
    let id = std::fs::read_to_string(&legacy)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    if let Err(e) = std::fs::create_dir_all(&dir) {
        eprintln!("mbv: could not create {}: {}", dir.display(), e);
    } else if let Err(e) = std::fs::write(&path, &id) {
        eprintln!(
            "mbv: could not write device_id to {}: {}",
            path.display(),
            e
        );
    }
    id
}

#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub id: String,
    pub device_name: String,
    pub client: String,
    pub user_name: String,
    pub host: String,
    pub supported_commands: Vec<String>,
    /// Media kinds this session advertises as playable. An empty list means
    /// that the capability is unknown, rather than that the session is audio-only.
    pub playable_media_types: Vec<String>,
    pub now_playing: Option<String>,
    pub now_playing_item_id: Option<String>,
    pub position_s: i64,
    pub runtime_s: i64,
    pub position_ticks: i64,
    pub runtime_ticks: i64,
    pub is_paused: bool,
    pub volume: i64,
    pub sub_index: i64,   // -1 = disabled
    pub audio_index: i64, // stream index; 0 = unknown
    pub muted: bool,
    pub media_info: SessionMediaInfo,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SessionMediaInfo {
    pub video_label: String,
    pub audio_only: bool,
    pub audio_streams: Vec<SessionAudioStream>,
    pub subtitle_streams: Vec<SessionSubtitleStream>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionAudioStream {
    pub index: i64,
    pub label: String,
    pub language: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionSubtitleStream {
    pub index: i64,
    pub label: String,
    pub language: String,
    pub forced: bool,
}

/// Result of a `PlaybackInfo` lookup for an item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaybackInfo {
    pub session_id: EmbySessionId,
    pub media_source_id: MediaSourceId,
    pub external_subtitle_urls: Vec<String>,
}

pub const MBV_DIRECT_TCP_PORT_PREFIX: &str = "mbv-direct-tcp-port:";

#[must_use]
pub fn mbv_direct_tcp_port_command(port: u16) -> String {
    format!("{MBV_DIRECT_TCP_PORT_PREFIX}{port}")
}

#[must_use]
pub fn parse_mbv_direct_tcp_port(commands: &[String]) -> Option<u16> {
    commands.iter().find_map(|cmd| {
        cmd.strip_prefix(MBV_DIRECT_TCP_PORT_PREFIX)
            .and_then(|port| port.parse::<u16>().ok())
            .filter(|port| *port > 0)
    })
}

pub(crate) use crate::api::types_parsing::load_cached_token;
#[cfg(test)]
pub use crate::api::types_parsing::save_cached_token;
pub use crate::api::types_parsing::{
    clear_cached_token, parse_audio_info, parse_item, parse_session_media_info, parse_video_info,
};

#[derive(Clone)]
pub struct EmbyClient {
    pub config: Config,
    pub user_id: String,
    pub token: String,
    pub device_name: String,
    pub device_id: String,
    pub(super) agent: ureq::Agent,
    /// True when tests installed an in-memory transport; never replaced by
    /// `with_request_timeout`.
    pub(super) mock_agent: bool,
}

impl std::fmt::Debug for EmbyClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The auth token is never rendered; `agent` has no stable redaction.
        f.debug_struct("EmbyClient")
            .field("config", &self.config)
            .field("user_id", &self.user_id)
            .field("token", &"<redacted>")
            .field("device_name", &self.device_name)
            .field("device_id", &self.device_id)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_redacts_token() {
        let mut client = EmbyClient::new(Config::default());
        let token = "emby-secret-token-123";
        client.token = token.to_string();
        let rendered = format!("{client:?}");
        assert!(rendered.contains("EmbyClient"));
        assert!(!rendered.contains(token));
    }
}
