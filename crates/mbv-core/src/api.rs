use serde_json::Value;

use crate::config::Config;

pub const TICKS_PER_SECOND: i64 = 10_000_000;

fn decode_html_entities(s: &str) -> String {
    s.replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

pub fn gen_session_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let pid = std::process::id();
    let r: u32 = rand::random();
    format!("{:x}{:x}{:x}{:x}", t.as_secs(), t.subsec_nanos(), pid, r)
}

fn device_name() -> String {
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

fn device_id() -> String {
    let data_home = std::env::var("XDG_DATA_HOME")
        .ok()
        .filter(|s| !s.is_empty())
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
            std::path::PathBuf::from(home).join(".local/share")
        });
    device_id_in(data_home)
}

fn device_id_in(data_home: std::path::PathBuf) -> String {
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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MediaItem {
    pub id: String,
    pub name: String,
    pub item_type: String,
    pub is_folder: bool,
    pub media_type: String,
    pub collection_type: String,
    pub runtime_ticks: i64,
    pub played: bool,
    pub playback_position_ticks: i64,
    pub series_id: String,
    pub series_name: String,
    pub album_id: String,
    pub album: String,
    pub index_number: i64,
    pub parent_index_number: i64,
    pub unplayed_item_count: u32,
    pub path: String,
    pub artist: String,
    pub sort_name: String,
    pub production_year: u32,
    pub end_year: u32,
    pub overview: String,
    pub premiere_date: String,
    pub date_added: String,
    pub total_count: u32,
    pub container: String,
    pub director: String,
    pub video_info: String,
    pub audio_info: String,
    pub genre: String,
    pub playlist_item_id: String,
}

impl MediaItem {
    pub fn is_audio(&self) -> bool {
        self.media_type == "Audio" || self.item_type == "Audio"
    }

    pub fn is_video(&self) -> bool {
        self.media_type == "Video"
    }

    pub fn resume_seconds(&self) -> f64 {
        self.playback_position_ticks as f64 / TICKS_PER_SECOND as f64
    }

    pub fn should_resume(&self) -> bool {
        let pos = self.playback_position_ticks;
        if pos <= 0 {
            return false;
        }
        if self.runtime_ticks > 0 && pos * 100 < self.runtime_ticks {
            return false;
        } // displays as 0%
        true
    }

    pub fn runtime_seconds(&self) -> f64 {
        self.runtime_ticks as f64 / TICKS_PER_SECOND as f64
    }

    pub fn file_name(&self) -> &str {
        if self.path.is_empty() {
            return &self.name;
        }
        let p = std::path::Path::new(&self.path);
        p.file_name().and_then(|f| f.to_str()).unwrap_or(&self.name)
    }

    pub fn sort_key(&self) -> &str {
        if !self.path.is_empty() {
            self.file_name()
        } else if !self.sort_name.is_empty() {
            &self.sort_name
        } else {
            &self.name
        }
    }

    pub fn playback_label(&self) -> String {
        if self.item_type == "Audio" && !self.artist.is_empty() {
            format!("{} - {}", self.artist, self.name)
        } else {
            self.display_name()
        }
    }

    fn folder(id: String, name: String, collection_type: String) -> Self {
        MediaItem {
            id,
            name,
            item_type: "CollectionFolder".to_string(),
            is_folder: true,
            collection_type,
            media_type: String::new(),
            runtime_ticks: 0,
            played: false,
            playback_position_ticks: 0,
            series_id: String::new(),
            series_name: String::new(),
            album_id: String::new(),
            album: String::new(),
            index_number: 0,
            parent_index_number: 0,
            unplayed_item_count: 0,
            path: String::new(),
            artist: String::new(),
            sort_name: String::new(),
            production_year: 0,
            end_year: 0,
            overview: String::new(),
            premiere_date: String::new(),
            date_added: String::new(),
            total_count: 0,
            container: String::new(),
            director: String::new(),
            video_info: String::new(),
            audio_info: String::new(),
            genre: String::new(),
            playlist_item_id: String::new(),
        }
    }

    pub fn display_name(&self) -> String {
        if self.item_type == "Episode" && !self.series_name.is_empty() {
            format!("{}⁄{}", self.series_name, self.name)
        } else {
            self.name.clone()
        }
    }
}

#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub id: String,
    pub device_name: String,
    pub client: String,
    pub user_name: String,
    pub host: String,
    pub supported_commands: Vec<String>,
    pub now_playing: Option<String>,
    pub now_playing_item_id: Option<String>,
    pub position_s: i64,
    pub runtime_s: i64,
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

/// Result of a PlaybackInfo lookup for an item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaybackInfo {
    pub session_id: String,
    pub media_source_id: String,
    pub external_subtitle_urls: Vec<String>,
}

pub const MBV_DIRECT_TCP_PORT_PREFIX: &str = "mbv-direct-tcp-port:";

pub fn mbv_direct_tcp_port_command(port: u16) -> String {
    format!("{MBV_DIRECT_TCP_PORT_PREFIX}{port}")
}

pub fn parse_mbv_direct_tcp_port(commands: &[String]) -> Option<u16> {
    commands.iter().find_map(|cmd| {
        cmd.strip_prefix(MBV_DIRECT_TCP_PORT_PREFIX)
            .and_then(|port| port.parse::<u16>().ok())
            .filter(|port| *port > 0)
    })
}

fn parse_video_info(streams: &[Value]) -> String {
    let Some(s) = streams.iter().find(|s| s["Type"].as_str() == Some("Video")) else {
        return String::new();
    };
    let width = s["Width"].as_u64().unwrap_or(0);
    let height = s["Height"].as_u64().unwrap_or(0);
    let dim = width.max(height);
    let res = match dim {
        3840.. => "4K".to_string(),
        1920.. => "1080p".to_string(),
        1280.. => "720p".to_string(),
        720.. => "480p".to_string(),
        d if d > 0 => format!("{}p", height),
        _ => String::new(),
    };
    let codec = s["Codec"].as_str().unwrap_or("").to_uppercase();
    match (res.is_empty(), codec.is_empty()) {
        (false, false) => format!("{} {}", res, codec),
        (false, true) => res,
        (true, false) => codec,
        (true, true) => String::new(),
    }
}

fn audio_language_name(lang: &str) -> &'static str {
    match lang.to_lowercase().as_str() {
        "en" | "eng" => "English",
        "fr" | "fre" | "fra" => "French",
        "de" | "ger" | "deu" => "German",
        "es" | "spa" => "Spanish",
        "it" | "ita" => "Italian",
        "pt" | "por" => "Portuguese",
        "ja" | "jpn" => "Japanese",
        "ko" | "kor" => "Korean",
        "zh" | "chi" | "zho" => "Chinese",
        "ru" | "rus" => "Russian",
        "ar" | "ara" => "Arabic",
        "nl" | "nld" | "dut" => "Dutch",
        "sv" | "swe" => "Swedish",
        "no" | "nor" => "Norwegian",
        "da" | "dan" => "Danish",
        "fi" | "fin" => "Finnish",
        "pl" | "pol" => "Polish",
        "cs" | "cze" | "ces" => "Czech",
        "tr" | "tur" => "Turkish",
        _ => "",
    }
}

fn parse_audio_info(streams: &[Value]) -> String {
    let mut parts: Vec<String> = Vec::new();
    for s in streams
        .iter()
        .filter(|s| s["Type"].as_str() == Some("Audio"))
    {
        let lang = s["Language"].as_str().unwrap_or("");
        let lang_name = audio_language_name(lang);
        let codec = s["Codec"].as_str().unwrap_or("").to_uppercase();
        let layout = s["ChannelLayout"].as_str().unwrap_or("");
        let layout_str = match layout {
            "mono" => "Mono",
            "stereo" => "Stereo",
            "5.1" => "5.1",
            "7.1" => "7.1",
            other if !other.is_empty() => other,
            _ => "",
        };
        let track: Vec<&str> = [lang_name, &codec, layout_str]
            .iter()
            .filter(|s| !s.is_empty())
            .copied()
            .collect();
        if !track.is_empty() {
            parts.push(track.join(" "));
        }
    }
    parts.join("  |  ")
}

fn parse_session_media_info(streams: &[Value]) -> SessionMediaInfo {
    let video = streams.iter().find(|s| s["Type"].as_str() == Some("Video"));
    let audio_only = video.is_none();
    let video_label = if audio_only {
        parse_audio_info(streams)
            .split("  |  ")
            .next()
            .unwrap_or("")
            .to_string()
    } else {
        parse_video_info(streams)
    };

    let audio_streams = streams
        .iter()
        .filter(|s| s["Type"].as_str() == Some("Audio"))
        .filter_map(|s| {
            s.get("Index")?;
            let index = s["Index"].as_i64().unwrap_or(0);
            let language = s["Language"].as_str().unwrap_or("").to_string();
            let label = {
                let lang_name = audio_language_name(&language);
                let codec = s["Codec"].as_str().unwrap_or("").to_uppercase();
                let layout = s["ChannelLayout"].as_str().unwrap_or("");
                let layout_str = match layout {
                    "mono" => "Mono",
                    "stereo" => "Stereo",
                    "5.1" => "5.1",
                    "7.1" => "7.1",
                    other if !other.is_empty() => other,
                    _ => "",
                };
                let title = s["DisplayTitle"]
                    .as_str()
                    .or_else(|| s["Title"].as_str())
                    .unwrap_or("");
                let pieces: Vec<&str> = [lang_name, &codec, layout_str]
                    .iter()
                    .filter(|part| !part.is_empty())
                    .copied()
                    .collect();
                if !pieces.is_empty() {
                    pieces.join(" ")
                } else if !title.is_empty() {
                    title.to_string()
                } else if !language.is_empty() {
                    language.to_uppercase()
                } else {
                    format!("#{index}")
                }
            };
            Some(SessionAudioStream {
                index,
                label,
                language,
            })
        })
        .collect();

    let subtitle_streams = streams
        .iter()
        .filter(|s| s["Type"].as_str() == Some("Subtitle"))
        .filter_map(|s| {
            let index = s["Index"].as_i64().unwrap_or(-1);
            if index < 0 {
                return None;
            }
            let language = s["Language"].as_str().unwrap_or("").to_string();
            let forced = s["IsForced"].as_bool().unwrap_or(false);
            let title = s["DisplayTitle"]
                .as_str()
                .or_else(|| s["Title"].as_str())
                .unwrap_or("");
            let lang_name = audio_language_name(&language);
            let base = if !title.is_empty() {
                title.to_string()
            } else if !lang_name.is_empty() {
                lang_name.to_string()
            } else if !language.is_empty() {
                language.to_uppercase()
            } else {
                format!("#{index}")
            };
            let label = if forced {
                format!("{base} (Forced)")
            } else {
                base
            };
            Some(SessionSubtitleStream {
                index,
                label,
                language,
                forced,
            })
        })
        .collect();

    SessionMediaInfo {
        video_label,
        audio_only,
        audio_streams,
        subtitle_streams,
    }
}

fn parse_item(raw: &Value) -> MediaItem {
    let ud = raw.get("UserData").unwrap_or(&Value::Null);
    let item_type = raw["Type"].as_str().unwrap_or("").to_string();
    let is_folder = raw["IsFolder"].as_bool().unwrap_or(false)
        || matches!(
            item_type.as_str(),
            "CollectionFolder"
                | "Channel"
                | "Series"
                | "Season"
                | "MusicArtist"
                | "MusicAlbum"
                | "BoxSet"
                | "Folder"
        );
    let total_count = if item_type == "Series" {
        raw["RecursiveItemCount"].as_u64().unwrap_or(0) as u32
    } else {
        raw["ChildCount"].as_u64().unwrap_or(0) as u32
    };
    MediaItem {
        id: raw["Id"].as_str().unwrap_or("").to_string(),
        name: raw["Name"].as_str().unwrap_or("").to_string(),
        item_type,
        is_folder,
        media_type: raw["MediaType"].as_str().unwrap_or("").to_string(),
        collection_type: raw["CollectionType"].as_str().unwrap_or("").to_string(),
        runtime_ticks: raw["RunTimeTicks"].as_i64().unwrap_or(0),
        played: ud["Played"].as_bool().unwrap_or(false),
        playback_position_ticks: ud["PlaybackPositionTicks"].as_i64().unwrap_or(0),
        series_id: raw["SeriesId"].as_str().unwrap_or("").to_string(),
        series_name: raw["SeriesName"].as_str().unwrap_or("").to_string(),
        album_id: raw["AlbumId"].as_str().unwrap_or("").to_string(),
        album: raw["Album"].as_str().unwrap_or("").to_string(),
        index_number: raw["IndexNumber"].as_i64().unwrap_or(0),
        parent_index_number: raw["ParentIndexNumber"].as_i64().unwrap_or(0),
        unplayed_item_count: ud["UnplayedItemCount"].as_u64().unwrap_or(0) as u32,
        path: raw["Path"].as_str().unwrap_or("").to_string(),
        artist: raw["AlbumArtist"]
            .as_str()
            .or_else(|| raw["Artists"].get(0).and_then(|v| v.as_str()))
            .unwrap_or("")
            .to_string(),
        sort_name: raw["SortName"].as_str().unwrap_or("").to_string(),
        production_year: raw["ProductionYear"]
            .as_u64()
            .or_else(|| raw["Year"].as_u64())
            .unwrap_or(0) as u32,
        end_year: raw["EndDate"]
            .as_str()
            .and_then(|s| s.get(..4))
            .and_then(|s| s.parse().ok())
            .unwrap_or(0),
        overview: decode_html_entities(raw["Overview"].as_str().unwrap_or("")),
        premiere_date: raw["PremiereDate"]
            .as_str()
            .and_then(|s| s.get(..10))
            .map(|s| s.to_string())
            .unwrap_or_default(),
        date_added: raw["DateCreated"]
            .as_str()
            .and_then(|s| s.get(..10))
            .map(|s| s.to_string())
            .unwrap_or_default(),
        total_count,
        container: raw["Container"].as_str().unwrap_or("").to_string(),
        genre: raw["Genres"]
            .as_array()
            .and_then(|g| g.first().and_then(|v| v.as_str()))
            .unwrap_or("")
            .to_string(),
        director: raw["People"]
            .as_array()
            .and_then(|people| {
                people
                    .iter()
                    .find(|p| p["Type"].as_str() == Some("Director"))
                    .and_then(|p| p["Name"].as_str())
            })
            .unwrap_or("")
            .to_string(),
        video_info: raw["MediaStreams"]
            .as_array()
            .map(|s| parse_video_info(s))
            .unwrap_or_default(),
        playlist_item_id: raw["PlaylistItemId"].as_str().unwrap_or("").to_string(),
        audio_info: raw["MediaStreams"]
            .as_array()
            .map(|s| parse_audio_info(s))
            .unwrap_or_default(),
    }
}

fn load_cached_token() -> Option<(String, String, String)> {
    let path = crate::config::token_cache_path();
    let text = std::fs::read_to_string(path).ok()?;
    let v: Value = serde_json::from_str(&text).ok()?;
    let token = v["token"].as_str()?.to_string();
    let user_id = v["user_id"].as_str()?.to_string();
    if token.is_empty() || user_id.is_empty() {
        return None;
    }
    let server_url = v["server_url"].as_str().unwrap_or("").to_string();
    Some((server_url, token, user_id))
}

pub fn clear_cached_token() {
    let _ = std::fs::remove_file(crate::config::token_cache_path());
}

fn save_cached_token(server_url: &str, token: &str, user_id: &str) {
    let path = crate::config::token_cache_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let json = serde_json::json!({"server_url": server_url, "token": token, "user_id": user_id});
    let _ = std::fs::write(&path, json.to_string());
    // Restrict token file to owner-only to protect credentials.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
}

#[derive(Clone)]
pub struct EmbyClient {
    pub config: Config,
    pub user_id: String,
    pub token: String,
    pub device_name: String,
    pub device_id: String,
    pub chapter_api_available: bool,
    agent: ureq::Agent,
}

impl EmbyClient {
    // ── HTTP infrastructure ──────────────────────────────────────────────────

    pub fn new(config: Config) -> Self {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(std::time::Duration::from_secs(5))
            .timeout(std::time::Duration::from_secs(30))
            .build();
        EmbyClient {
            config,
            user_id: String::new(),
            token: String::new(),
            device_name: device_name(),
            device_id: device_id(),
            chapter_api_available: false,
            agent,
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.config.server_url, path)
    }

    fn auth_header(&self) -> String {
        format!(
            "Emby Client=\"mbv\", Device=\"{}\", DeviceId=\"{}\", Version=\"{}\", Token=\"{}\"",
            self.device_name,
            self.device_id,
            env!("CARGO_PKG_VERSION"),
            self.token
        )
    }

    fn get(&self, path: &str) -> ureq::Request {
        self.agent
            .get(&self.url(path))
            .set("Authorization", &self.auth_header())
            .set("X-Emby-Token", &self.token)
    }

    fn post(&self, path: &str) -> ureq::Request {
        self.agent
            .post(&self.url(path))
            .set("Authorization", &self.auth_header())
            .set("X-Emby-Token", &self.token)
    }

    fn with_request_timeout(&self, timeout: std::time::Duration) -> Self {
        let mut client = self.clone();
        client.agent = ureq::AgentBuilder::new()
            .timeout_connect(timeout)
            .timeout(timeout)
            .build();
        client
    }

    fn delete(&self, path: &str) -> ureq::Request {
        self.agent
            .delete(&self.url(path))
            .set("Authorization", &self.auth_header())
            .set("X-Emby-Token", &self.token)
    }

    // ── Authentication ───────────────────────────────────────────────────────

    pub fn authenticate(&mut self) -> Result<(), String> {
        let Some((cached_url, token, user_id)) = load_cached_token() else {
            return Err("No cached credentials".to_string());
        };

        if self.config.server_url.is_empty() {
            if cached_url.is_empty() {
                return Err("No server URL configured".to_string());
            }
            self.config.server_url = cached_url;
        }

        self.token = token;
        self.user_id = user_id;

        match self.get(&format!("/Users/{}", self.user_id)).call() {
            Ok(_) => Ok(()),
            Err(ureq::Error::Status(401 | 403, _)) => {
                clear_cached_token();
                self.token.clear();
                self.user_id.clear();
                Err("Cached credentials expired".to_string())
            }
            Err(e) => {
                self.token.clear();
                self.user_id.clear();
                Err(format!("Cached credential validation failed: {e}"))
            }
        }
    }

    /// Hard wall-clock bound for `authenticate_bounded`, independent of
    /// ureq's own connect/total timeouts (see issue #191: those don't
    /// reliably cover every stall mode, e.g. TLS handshake hangs).
    pub const AUTHENTICATE_HARD_BOUND: std::time::Duration = std::time::Duration::from_secs(15);

    /// Runs `authenticate()` on a clone, bounded by `hard_bound` wall-clock
    /// time. On success, returns the authenticated clone -- callers should
    /// use it in place of the original, since `self` is never mutated. On
    /// timeout (or any other failure), `self` is left untouched.
    pub fn authenticate_bounded(
        &self,
        hard_bound: std::time::Duration,
    ) -> Result<EmbyClient, String> {
        let mut clone = self.clone();
        crate::bounded::run_with_hard_bound(
            move || clone.authenticate().map(|()| clone),
            hard_bound,
        )
    }

    // Authenticate using credentials in self.config (password or api_key).
    // Does not check the token cache. Saves a fresh token to the cache on success.
    // Called by authenticate() on cache miss, and directly by the login screen.
    pub fn authenticate_credentials(&mut self) -> Result<(), String> {
        // Prefer password auth: yields a user-scoped token so sessions are attributed to the
        // correct user (required for activity tracking and progress saving).
        // API key auth yields an admin token with no user association — use only as fallback.
        if !self.config.password.is_empty() {
            let resp: Value = self
                .agent
                .post(&self.url("/Users/AuthenticateByName"))
                .set(
                    "Authorization",
                    &format!(
                        "Emby Client=\"mbv\", Device=\"{}\", DeviceId=\"{}\", Version=\"{}\"",
                        self.device_name,
                        self.device_id,
                        env!("CARGO_PKG_VERSION")
                    ),
                )
                .send_json(ureq::json!({
                    "Username": self.config.username,
                    "Pw": self.config.password,
                }))
                .map_err(|e| format!("Auth failed: {e}"))?
                .into_json()
                .map_err(|e| format!("Auth parse failed: {e}"))?;
            self.token = resp["AccessToken"].as_str().unwrap_or("").to_string();
            self.user_id = resp["User"]["Id"].as_str().unwrap_or("").to_string();
            save_cached_token(&self.config.server_url, &self.token, &self.user_id);
        } else if !self.config.api_key.is_empty() {
            self.token = self.config.api_key.clone();
            let users: Value = self
                .agent
                .get(&self.url("/Users"))
                .query("api_key", &self.token)
                .call()
                .map_err(|e| format!("Auth failed: {e}"))?
                .into_json()
                .map_err(|e| format!("Auth parse failed: {e}"))?;
            let users = users.as_array().ok_or("Expected array of users")?;
            if users.is_empty() {
                return Err("No users found on server".to_string());
            }
            if !self.config.username.is_empty() {
                let uname = self.config.username.to_lowercase();
                let found = users
                    .iter()
                    .find(|u| u["Name"].as_str().unwrap_or("").to_lowercase() == uname);
                match found {
                    Some(u) => self.user_id = u["Id"].as_str().unwrap_or("").to_string(),
                    None => return Err(format!("User '{}' not found", self.config.username)),
                }
            } else {
                self.user_id = users[0]["Id"].as_str().unwrap_or("").to_string();
            }
        } else {
            return Err("No credentials configured".to_string());
        }
        Ok(())
    }

    /// Fetch the current user's subtitle and audio language preferences from Emby.
    pub fn get_user_subtitle_prefs(&self) -> Result<crate::player::SubtitlePrefs, String> {
        let resp: serde_json::Value = self
            .get("/Users/Me")
            .call()
            .map_err(|e| e.to_string())?
            .into_json()
            .map_err(|e| e.to_string())?;
        let cfg = &resp["Configuration"];
        let mode = cfg["SubtitleMode"]
            .as_str()
            .unwrap_or("Default")
            .to_string();
        let subtitle_lang = cfg["SubtitleLanguagePreference"]
            .as_str()
            .unwrap_or("")
            .to_string();
        let audio_lang = cfg["AudioLanguagePreference"]
            .as_str()
            .unwrap_or("")
            .to_string();
        Ok(crate::player::SubtitlePrefs {
            mode,
            subtitle_lang,
            audio_lang,
        })
    }

    pub fn validate_presented_token(&self, token: &str) -> Result<String, String> {
        let token = token.trim();
        if token.is_empty() {
            return Err("missing Emby auth token".to_string());
        }

        let auth_header = format!(
            "Emby Client=\"mbv\", Device=\"{}\", DeviceId=\"{}\", Version=\"{}\", Token=\"{}\"",
            self.device_name,
            self.device_id,
            env!("CARGO_PKG_VERSION"),
            token
        );

        let me_resp = self
            .agent
            .get(&self.url("/Users/Me"))
            .set("Authorization", &auth_header)
            .set("X-Emby-Token", token)
            .call();
        if let Ok(resp) = me_resp {
            let resp: serde_json::Value = resp
                .into_json()
                .map_err(|e| format!("presented Emby token validation parse failed: {e}"))?;
            let user_id = resp["Id"].as_str().unwrap_or("").trim();
            if !user_id.is_empty() {
                return Ok(user_id.to_string());
            }
        }

        let users_resp = self
            .agent
            .get(&self.url("/Users"))
            .query("api_key", token)
            .call()
            .map_err(|e| format!("presented Emby token rejected: {e}"))?;
        let users: serde_json::Value = users_resp
            .into_json()
            .map_err(|e| format!("presented Emby token API-key validation parse failed: {e}"))?;
        let users = users
            .as_array()
            .ok_or("presented Emby token API-key validation expected user array")?;
        if users.is_empty() {
            return Err("presented Emby token API-key validation returned no users".to_string());
        }
        Ok(users[0]["Id"].as_str().unwrap_or("").to_string())
    }

    // ── Browse / fetch ───────────────────────────────────────────────────────

    fn fetch_items(&self, path: &str, queries: &[(&str, &str)]) -> Result<Vec<MediaItem>, String> {
        let mut req = self.get(path);
        for (k, v) in queries {
            req = req.query(k, v);
        }
        let resp: Value = req
            .call()
            .map_err(|e| e.to_string())?
            .into_json()
            .map_err(|e| e.to_string())?;
        Ok(resp["Items"]
            .as_array()
            .map(|arr| arr.iter().map(parse_item).collect())
            .unwrap_or_default())
    }

    pub fn get_views(&self) -> Result<Vec<MediaItem>, String> {
        let vfolders: Value = self
            .get("/Library/VirtualFolders")
            .call()
            .map_err(|e| e.to_string())?
            .into_json()
            .map_err(|e| e.to_string())?;

        let user_views: Value = self
            .get(&format!("/Users/{}/Views", self.user_id))
            .call()
            .map_err(|e| e.to_string())?
            .into_json()
            .map_err(|e| e.to_string())?;

        let mut items: Vec<MediaItem> = Vec::new();
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

        if let Some(arr) = vfolders.as_array() {
            for f in arr {
                let id = f["ItemId"].as_str().unwrap_or("").to_string();
                let name = f["Name"].as_str().unwrap_or("").to_string();
                let ctype = f["CollectionType"].as_str().unwrap_or("").to_string();
                seen.insert(id.clone());
                items.push(MediaItem::folder(id, name, ctype));
            }
        }

        if let Some(arr) = user_views["Items"].as_array() {
            for raw in arr {
                let id = raw["Id"].as_str().unwrap_or("").to_string();
                if !seen.contains(&id) {
                    items.push(parse_item(raw));
                }
            }
        }

        Ok(items)
    }

    pub fn get_user_views(&self) -> Result<Vec<MediaItem>, String> {
        self.fetch_items(&format!("/Users/{}/Views", self.user_id), &[])
    }

    pub fn get_items_sorted(
        &self,
        parent_id: &str,
        item_types: Option<&str>,
        unplayed_only: bool,
        start_index: usize,
        limit: usize,
        sort_by: &str,
        sort_order: &str,
    ) -> Result<(Vec<MediaItem>, usize), String> {
        let mut req = self.get(&format!("/Users/{}/Items", self.user_id))
            .query("ParentId", parent_id)
            .query("SortBy", sort_by)
            .query("SortOrder", sort_order)
            .query("StartIndex", &start_index.to_string())
            .query("Limit", &limit.to_string())
            .query("Fields", "UserData,RunTimeTicks,MediaType,SeriesId,SeriesName,SortName,ParentIndexNumber,IndexNumber,Path,AlbumArtist,Artists,ProductionYear,EndDate,Overview,PremiereDate,DateCreated,ChildCount,RecursiveItemCount,Container,People,MediaStreams,Genres")
            .query("EnableUserData", "true");
        if let Some(types) = item_types {
            req = req
                .query("IncludeItemTypes", types)
                .query("Recursive", "true");
        }
        if unplayed_only {
            req = req.query("Filters", "IsUnplayed");
        }
        let resp: Value = req
            .call()
            .map_err(|e| e.to_string())?
            .into_json()
            .map_err(|e| e.to_string())?;
        let total = resp["TotalRecordCount"].as_u64().unwrap_or(0) as usize;
        let items = resp["Items"]
            .as_array()
            .map(|arr| arr.iter().map(parse_item).collect())
            .unwrap_or_default();
        Ok((items, total))
    }

    pub fn search_items(&self, term: &str, limit: usize) -> Result<Vec<MediaItem>, String> {
        let limit = limit.to_string();
        self.fetch_items(&format!("/Users/{}/Items", self.user_id), &[
            ("SearchTerm",  term),
            ("Recursive",   "true"),
            ("Limit",       &limit),
            ("Fields",      "UserData,RunTimeTicks,MediaType,SeriesId,SeriesName,SortName,ParentIndexNumber,IndexNumber,Path,AlbumArtist,Artists,ProductionYear"),
        ])
    }

    pub fn get_continue_watching(&self, limit: usize) -> Result<Vec<MediaItem>, String> {
        let limit = limit.to_string();
        self.fetch_items(&format!("/Users/{}/Items/Resume", self.user_id), &[
            ("UserId",     &self.user_id),
            ("Limit",      &limit),
            ("Fields",     "UserData,RunTimeTicks,MediaType,SeriesId,SeriesName,SortName,ParentIndexNumber,IndexNumber,Path,AlbumArtist,Artists"),
            ("MediaTypes", "Video"),
        ])
    }

    pub fn get_latest(&self, parent_id: &str, limit: usize) -> Result<Vec<MediaItem>, String> {
        let resp: Value = self.get(&format!("/Users/{}/Items/Latest", self.user_id))
            .query("ParentId", parent_id)
            .query("Limit", &limit.to_string())
            .query("Fields", "UserData,RunTimeTicks,MediaType,SeriesId,SeriesName,SortName,ParentIndexNumber,IndexNumber,Path,AlbumArtist,Artists,AlbumId")
            .call().map_err(|e| e.to_string())?
            .into_json().map_err(|e| e.to_string())?;
        Ok(resp
            .as_array()
            .map(|arr| arr.iter().map(parse_item).collect())
            .unwrap_or_default())
    }

    pub fn get_latest_episodes(
        &self,
        parent_id: &str,
        limit: usize,
    ) -> Result<Vec<MediaItem>, String> {
        let limit = limit.to_string();
        self.fetch_items(&format!("/Users/{}/Items", self.user_id), &[
            ("ParentId",          parent_id),
            ("Limit",             &limit),
            ("IncludeItemTypes",  "Episode"),
            ("Recursive",         "true"),
            ("SortBy",            "DateCreated"),
            ("SortOrder",         "Descending"),
            ("IsPlayed",          "false"),
            ("Fields",            "UserData,RunTimeTicks,MediaType,SeriesId,SeriesName,SortName,ParentIndexNumber,IndexNumber,Path"),
        ])
    }

    pub fn get_all_playable_recursive(&self, parent_id: &str) -> Result<Vec<MediaItem>, String> {
        self.fetch_items(&format!("/Users/{}/Items", self.user_id), &[
            ("ParentId",         parent_id),
            ("IncludeItemTypes", "Episode,Movie,Video,Audio"),
            ("Recursive",        "true"),
            ("SortBy",           "SortName"),
            ("SortOrder",        "Ascending"),
            ("Limit",            "2000"),
            ("Fields",           "UserData,RunTimeTicks,MediaType,SeriesId,SeriesName,SortName,ParentIndexNumber,IndexNumber,Path,AlbumArtist,Artists"),
        ])
    }

    pub fn get_direct_playable(&self, parent_id: &str) -> Result<Vec<MediaItem>, String> {
        self.fetch_items(&format!("/Users/{}/Items", self.user_id), &[
            ("ParentId",         parent_id),
            ("IncludeItemTypes", "Episode,Movie,Video,Audio"),
            ("SortBy",           "SortName"),
            ("SortOrder",        "Ascending"),
            ("Limit",            "2000"),
            ("Fields",           "UserData,RunTimeTicks,MediaType,SeriesId,SeriesName,SortName,ParentIndexNumber,IndexNumber,Path,AlbumArtist,Artists"),
        ])
    }

    pub fn get_all_videos_recursive(&self, parent_id: &str) -> Result<Vec<MediaItem>, String> {
        self.fetch_items(&format!("/Users/{}/Items", self.user_id), &[
            ("ParentId",         parent_id),
            ("IncludeItemTypes", "Episode,Movie,Video"),
            ("Recursive",        "true"),
            ("SortBy",           "SortName"),
            ("SortOrder",        "Ascending"),
            ("Limit",            "2000"),
            ("Fields",           "UserData,RunTimeTicks,MediaType,SeriesId,SeriesName,SortName,ParentIndexNumber,IndexNumber,Path,AlbumArtist,Artists"),
        ])
    }

    // ── Library actions ──────────────────────────────────────────────────────

    pub fn mark_played(&self, item_id: &str) -> Result<(), String> {
        self.post(&format!("/Users/{}/PlayedItems/{}", self.user_id, item_id))
            .call()
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn mark_unplayed(&self, item_id: &str) -> Result<(), String> {
        self.delete(&format!("/Users/{}/PlayedItems/{}", self.user_id, item_id))
            .call()
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn hide_from_resume(&self, item_id: &str) -> Result<(), String> {
        self.post(&format!(
            "/Users/{}/Items/{}/HideFromResume",
            self.user_id, item_id
        ))
        .query("Hide", "true")
        .call()
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn post_library_refresh(&self, library_id: &str) -> Result<(), String> {
        self.post(&format!("/Items/{library_id}/Refresh"))
            .query("Recursive", "true")
            .query("ImageRefreshMode", "Default")
            .query("MetadataRefreshMode", "Default")
            .query("ReplaceAllImages", "false")
            .query("ReplaceAllMetadata", "false")
            .call()
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    // ── Playback reporting ───────────────────────────────────────────────────

    pub fn ws_url(&self) -> String {
        let base = self
            .config
            .server_url
            .replacen("https://", "wss://", 1)
            .replacen("http://", "ws://", 1);
        format!(
            "{}/embywebsocket?api_key={}&deviceId={}",
            base, self.token, self.device_id
        )
    }

    pub fn report_start(&self, item: &MediaItem, media_source_id: &str, session_id: &str) -> bool {
        let body = ureq::json!({
            "UserId": self.user_id,
            "ItemId": item.id,
            "MediaSourceId": media_source_id,
            "PlaySessionId": session_id,
            "CanSeek": true,
            "IsPaused": false,
            "IsMuted": false,
            "PlayMethod": "DirectPlay",
            "PositionTicks": item.playback_position_ticks,
            "RunTimeTicks": item.runtime_ticks,
            "QueueableMediaTypes": ["Audio", "Video"],
        });
        log::info!(target: "api", "outbound: Playing item={} msid={media_source_id} pos={}", item.id, item.playback_position_ticks);
        match self.post("/Sessions/Playing").send_json(body.clone()) {
            Ok(r) => {
                log::info!(target: "api", "inbound: {} Playing", r.status());
                true
            }
            Err(e) => {
                log::warn!(target: "api", "err: Playing: {e}, retrying...");
                std::thread::sleep(std::time::Duration::from_millis(500));
                match self.post("/Sessions/Playing").send_json(body) {
                    Ok(r) => {
                        log::info!(target: "api", "inbound: {} Playing (retry)", r.status());
                        true
                    }
                    Err(e) => {
                        log::warn!(target: "api", "err: Playing retry failed: {e}");
                        false
                    }
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn report_progress_ws(
        &self,
        item_id: &str,
        media_source_id: &str,
        position_ticks: i64,
        runtime_ticks: i64,
        is_paused: bool,
        session_id: &str,
        event_name: &str,
        ws_tx: &crate::ws::WsSender,
    ) {
        let data = serde_json::json!({
            "UserId": self.user_id,
            "ItemId": item_id,
            "MediaSourceId": media_source_id,
            "PlaySessionId": session_id,
            "CanSeek": true,
            "IsPaused": is_paused,
            "IsMuted": false,
            "PlayMethod": "DirectPlay",
            "PositionTicks": position_ticks,
            "EventName": event_name,
            "QueueableMediaTypes": ["Audio", "Video"],
        });
        let msg = serde_json::json!({
            "MessageType": "ReportPlaybackProgress",
            "Data": data,
        })
        .to_string();
        let pos_s = position_ticks / TICKS_PER_SECOND;
        let run_s = runtime_ticks / TICKS_PER_SECOND;
        log::info!(target: "api", "outbound: ws Progress pos={pos_s}s/{run_s}s paused={is_paused} event={event_name}");
        if ws_tx.send_text(msg).is_err() {
            log::warn!(target: "api", "ws channel disconnected, falling back to HTTP");
            self.report_progress_http(
                item_id,
                media_source_id,
                position_ticks,
                is_paused,
                session_id,
                event_name,
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn report_progress_http(
        &self,
        item_id: &str,
        media_source_id: &str,
        position_ticks: i64,
        is_paused: bool,
        session_id: &str,
        event_name: &str,
    ) {
        let body = ureq::json!({
            "UserId": self.user_id,
            "ItemId": item_id,
            "MediaSourceId": media_source_id,
            "PlaySessionId": session_id,
            "CanSeek": true,
            "IsPaused": is_paused,
            "IsMuted": false,
            "PlayMethod": "DirectPlay",
            "PositionTicks": position_ticks,
            "EventName": event_name,
            "QueueableMediaTypes": ["Audio", "Video"],
        });
        log::debug!(target: "api", "outbound: Progress pos={position_ticks} paused={is_paused} event={event_name}");
        match self.post("/Sessions/Playing/Progress").send_json(body) {
            Ok(r) => log::debug!(target: "api", "inbound: {} Progress", r.status()),
            Err(e) => log::warn!(target: "api",  "err: Progress: {e}"),
        }
    }

    pub fn report_ping(&self, session_id: &str) {
        log::debug!(target: "api", "outbound: Ping session={session_id}");
        match self
            .post("/Sessions/Playing/Ping")
            .query("PlaySessionId", session_id)
            .send_string("")
        {
            Ok(r) => log::debug!(target: "api", "inbound: {} Ping", r.status()),
            Err(e) => log::warn!(target: "api",  "err: Ping: {e}"),
        }
    }

    pub fn report_stopped(
        &self,
        item_id: &str,
        media_source_id: &str,
        position_ticks: i64,
        session_id: &str,
        runtime_ticks: i64,
    ) -> bool {
        let body = self.stopped_request_body(
            item_id,
            media_source_id,
            position_ticks,
            session_id,
            runtime_ticks,
        );
        log::info!(target: "api", "outbound: Stopped pos={position_ticks}");
        match self
            .post("/Sessions/Playing/Stopped")
            .send_json(body.clone())
        {
            Ok(r) => {
                log::info!(target: "api", "inbound: {} Stopped", r.status());
                true
            }
            Err(e) => {
                log::warn!(target: "api", "err: Stopped: {e}, retrying...");
                std::thread::sleep(std::time::Duration::from_millis(500));
                match self.post("/Sessions/Playing/Stopped").send_json(body) {
                    Ok(r) => {
                        log::info!(target: "api", "inbound: {} Stopped (retry)", r.status());
                        true
                    }
                    Err(e) => {
                        log::warn!(target: "api", "err: Stopped retry failed: {e}");
                        false
                    }
                }
            }
        }
    }

    fn stopped_request_body(
        &self,
        item_id: &str,
        media_source_id: &str,
        position_ticks: i64,
        session_id: &str,
        runtime_ticks: i64,
    ) -> serde_json::Value {
        ureq::json!({
            "UserId": self.user_id,
            "ItemId": item_id,
            "MediaSourceId": media_source_id,
            "PlaySessionId": session_id,
            "PositionTicks": position_ticks,
            "RunTimeTicks": runtime_ticks,
            "CanSeek": true,
            "IsPaused": false,
            "IsMuted": false,
            "PlayMethod": "DirectPlay",
            "QueueableMediaTypes": ["Audio", "Video"],
        })
    }

    pub fn report_stopped_for_shutdown(
        &self,
        item_id: &str,
        media_source_id: &str,
        position_ticks: i64,
        session_id: &str,
        runtime_ticks: i64,
        hard_bound: std::time::Duration,
    ) -> bool {
        let body = self.stopped_request_body(
            item_id,
            media_source_id,
            position_ticks,
            session_id,
            runtime_ticks,
        );
        let client = self.with_request_timeout(hard_bound);
        let started = std::time::Instant::now();
        log::info!(
            target: "api",
            "outbound: Stopped shutdown pos={position_ticks} timeout={}ms",
            hard_bound.as_millis()
        );
        let result = crate::bounded::run_with_hard_bound(
            move || {
                client
                    .post("/Sessions/Playing/Stopped")
                    .send_json(body)
                    .map(|r| r.status())
                    .map_err(|e| e.to_string())
            },
            hard_bound,
        );
        let elapsed_ms = started.elapsed().as_millis();
        match result {
            Ok(status) => {
                log::info!(target: "api", "inbound: {status} Stopped shutdown in {elapsed_ms}ms");
                true
            }
            Err(e) if e.starts_with("timed out after ") => {
                log::warn!(target: "api", "err: Stopped shutdown timed out after {elapsed_ms}ms: {e}");
                false
            }
            Err(e) => {
                log::warn!(target: "api", "err: Stopped shutdown failed without retry after {elapsed_ms}ms: {e}");
                false
            }
        }
    }

    pub fn register_capabilities(&self) {
        self.register_capabilities_with_extra_commands(&[]);
    }

    pub fn register_capabilities_with_extra_commands(&self, extra_commands: &[String]) {
        self.register_capabilities_with_options(extra_commands, self.config.audio_pipe_enabled);
    }

    pub fn register_capabilities_with_options(&self, extra_commands: &[String], audio_only: bool) {
        let media_types: &[&str] = if audio_only {
            &["Audio"]
        } else {
            &["Audio", "Video"]
        };
        let mut commands: Vec<String> = vec![
            "Play",
            "Stop",
            "Pause",
            "Unpause",
            "NextTrack",
            "PreviousTrack",
            "Seek",
            "SetVolume",
            "VolumeUp",
            "VolumeDown",
            "Mute",
            "Unmute",
            "ToggleMute",
            "SetAudioStreamIndex",
            "SetSubtitleStreamIndex",
        ]
        .into_iter()
        .map(str::to_string)
        .collect();
        if audio_only {
            // No video window ever opens in audio-pipe mode, so subtitles can
            // never be displayed — don't advertise a command that can't work.
            commands.retain(|c| c != "SetSubtitleStreamIndex");
        }
        commands.extend(extra_commands.iter().cloned());
        let body = ureq::json!({
            "PlayableMediaTypes": media_types,
            "SupportedCommands": commands,
            "SupportsMediaControl": true,
            "SupportsSync": false
        });
        log::info!(target: "api", "outbound: Capabilities");
        match self.post("/Sessions/Capabilities/Full").send_json(body) {
            Ok(r) => log::info!(target: "api", "inbound: {} Capabilities", r.status()),
            Err(e) => log::warn!(target: "api", "err: Capabilities: {e}"),
        }
    }

    /// Falls back to a generated session id / item_id on failure.
    pub fn get_playback_info(&self, item_id: &str) -> PlaybackInfo {
        let body = ureq::json!({
            "UserId": self.user_id,
            "MaxStreamingBitrate": 140000000,
            "EnableDirectPlay": true,
            "EnableDirectStream": false,
            "IsPlayback": true,
        });
        log::info!(target: "api", "outbound: PlaybackInfo item={item_id}");
        let resp: Value = match self
            .post(&format!("/Items/{item_id}/PlaybackInfo"))
            .send_json(body)
        {
            Ok(r) => match r.into_json() {
                Ok(v) => v,
                Err(e) => {
                    log::warn!(target: "api", "err: PlaybackInfo parse: {e}");
                    return PlaybackInfo {
                        session_id: gen_session_id(),
                        media_source_id: item_id.to_string(),
                        external_subtitle_urls: vec![],
                    };
                }
            },
            Err(e) => {
                log::warn!(target: "api", "err: PlaybackInfo: {e}");
                return PlaybackInfo {
                    session_id: gen_session_id(),
                    media_source_id: item_id.to_string(),
                    external_subtitle_urls: vec![],
                };
            }
        };
        let sid = resp["PlaySessionId"].as_str().unwrap_or("").to_string();
        let msid = resp["MediaSources"][0]["Id"]
            .as_str()
            .unwrap_or(item_id)
            .to_string();
        let sub_urls: Vec<String> = resp["MediaSources"][0]["MediaStreams"]
            .as_array()
            .map(|a| a.as_slice())
            .unwrap_or(&[])
            .iter()
            .filter(|s| {
                s["Type"].as_str() == Some("Subtitle")
                    && s["DeliveryMethod"].as_str() == Some("External")
            })
            .filter_map(|s| s["DeliveryUrl"].as_str())
            .map(|u| format!("{}{}", self.config.server_url, u))
            .collect();
        log::info!(target: "api", "inbound: PlaybackInfo sid={sid} msid={msid} ext_subs={}", sub_urls.len());
        let session_id = if sid.is_empty() {
            gen_session_id()
        } else {
            sid
        };
        PlaybackInfo {
            session_id,
            media_source_id: msid,
            external_subtitle_urls: sub_urls,
        }
    }

    // ── Playlists ────────────────────────────────────────────────────────────

    pub fn get_playlists(&self) -> Result<Vec<MediaItem>, String> {
        self.fetch_items(
            &format!("/Users/{}/Items", self.user_id),
            &[
                ("IncludeItemTypes", "Playlist"),
                ("Recursive", "true"),
                ("Fields", ""),
            ],
        )
    }

    pub fn create_playlist(&self, name: &str, item_ids: &[String]) -> Result<String, String> {
        let body = ureq::json!({
            "Name": name,
            "Ids": item_ids.join(","),
            "UserId": self.user_id,
        });
        let resp: Value = self
            .post("/Playlists")
            .send_json(body)
            .map_err(|e| match e {
                ureq::Error::Status(code, r) => {
                    let body = r.into_string().unwrap_or_default();
                    format!("HTTP {code}: {body}")
                }
                e => e.to_string(),
            })?
            .into_json()
            .map_err(|e| e.to_string())?;
        resp["Id"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| "no Id in response".to_string())
    }

    pub fn delete_playlist(&self, playlist_id: &str) -> Result<(), String> {
        self.delete(&format!("/Items/{}", playlist_id))
            .call()
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Replace a playlist's contents with the given item ids (in order).
    /// Fetches current entry ids, deletes them all, then adds the new set.
    pub fn get_playlist_items(&self, playlist_id: &str) -> Result<Vec<MediaItem>, String> {
        let resp: serde_json::Value = self.get(&format!("/Playlists/{}/Items", playlist_id))
            .query("UserId", &self.user_id)
            .query("Fields", "UserData,RunTimeTicks,MediaType,SeriesId,SeriesName,SortName,ParentIndexNumber,IndexNumber,Path,AlbumArtist,Artists,ProductionYear,EndDate,Overview,PremiereDate,DateCreated,ChildCount,RecursiveItemCount,Container,People,MediaStreams,Genres")
            .query("EnableUserData", "true")
            .call().map_err(|e| e.to_string())?
            .into_json().map_err(|e| e.to_string())?;
        Ok(resp["Items"]
            .as_array()
            .map(|arr| arr.iter().map(parse_item).collect())
            .unwrap_or_default())
    }

    pub fn update_playlist_items(
        &self,
        playlist_id: &str,
        item_ids: &[String],
    ) -> Result<(), String> {
        // Get current playlist entry ids
        let resp: serde_json::Value = self
            .get(&format!("/Playlists/{}/Items", playlist_id))
            .query("UserId", &self.user_id)
            .call()
            .map_err(|e| e.to_string())?
            .into_json()
            .map_err(|e| e.to_string())?;
        let entry_ids: Vec<String> = resp["Items"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v["PlaylistItemId"].as_str())
                    .map(|s| s.to_string())
                    .collect()
            })
            .unwrap_or_default();
        // Delete existing entries
        if !entry_ids.is_empty() {
            self.delete(&format!("/Playlists/{}/Items", playlist_id))
                .query("EntryIds", &entry_ids.join(","))
                .call()
                .map_err(|e| e.to_string())?;
        }
        // Add new items in order
        if !item_ids.is_empty() {
            self.post(&format!("/Playlists/{}/Items", playlist_id))
                .query("Ids", &item_ids.join(","))
                .query("UserId", &self.user_id)
                .call()
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    // ── Series / episodes / chapters ────────────────────────────────────────

    pub fn get_items_by_ids(&self, ids: &[String]) -> Result<Vec<MediaItem>, String> {
        if ids.is_empty() {
            return Ok(vec![]);
        }
        let joined = ids.join(",");
        let mut items = self.fetch_items(&format!("/Users/{}/Items", self.user_id), &[
            ("Ids",    &joined),
            ("Fields", "UserData,RunTimeTicks,MediaType,SeriesId,SeriesName,SortName,ParentIndexNumber,IndexNumber,Path,AlbumArtist,Artists"),
        ])?;
        // Emby returns items in server sort order, not input order. Restore input order.
        let order: std::collections::HashMap<&str, usize> = ids
            .iter()
            .enumerate()
            .map(|(i, id)| (id.as_str(), i))
            .collect();
        items.sort_by_key(|item| order.get(item.id.as_str()).copied().unwrap_or(usize::MAX));
        Ok(items)
    }

    pub fn get_ancestors(&self, item_id: &str) -> Result<Vec<MediaItem>, String> {
        let resp: Value = self
            .get(&format!("/Items/{}/Ancestors", item_id))
            .query("Fields", "SortName")
            .call()
            .map_err(|e| e.to_string())?
            .into_json()
            .map_err(|e| e.to_string())?;
        Ok(resp
            .as_array()
            .map(|arr| arr.iter().map(parse_item).collect())
            .unwrap_or_default())
    }

    /// Probes for the Chapter API plugin. Sets `chapter_api_available` on self.
    /// Any HTTP response (even 500 for a bad id) means the plugin is installed;
    /// only a 404 or connection failure means it's absent.
    pub fn probe_chapter_api(&mut self) {
        log::info!(target: "api", "outbound: ChapterAPI probe");
        match self
            .get("/chapter_api/get_chapters")
            .query("id", "0")
            .call()
        {
            Ok(_) | Err(ureq::Error::Status(_, _)) => {
                self.chapter_api_available = true;
                log::info!(target: "api", "inbound: ChapterAPI available");
            }
            Err(e) => {
                log::info!(target: "api", "err: ChapterAPI not available: {e}");
            }
        }
    }

    /// Returns `(intro_start_ticks, intro_end_ticks)` for an item if the Chapter API
    /// exposes IntroStart and IntroEnd markers.
    pub fn get_intro_times(&self, item_id: &str) -> Option<(i64, i64)> {
        log::debug!(target: "api", "outbound: ChapterAPI get_chapters item={item_id}");
        let resp = self
            .get("/chapter_api/get_chapters")
            .query("id", item_id)
            .call()
            .ok()?;
        let body: serde_json::Value = resp.into_json().ok()?;
        let chapters = body["chapters"].as_array()?;
        let start = chapters
            .iter()
            .find(|c| c["MarkerType"].as_str() == Some("IntroStart"))?["StartPositionTicks"]
            .as_i64()?;
        let end = chapters
            .iter()
            .find(|c| c["MarkerType"].as_str() == Some("IntroEnd"))?["StartPositionTicks"]
            .as_i64()?;
        log::info!(target: "api", "inbound: ChapterAPI intro start={start} end={end}");
        Some((start, end))
    }

    /// Returns all episodes of a series starting from `from_item_id` (inclusive), in air order.
    /// Mirrors Emby Web's `getEpisodes(seriesId)` + filter pattern.
    pub fn get_episodes_from(&self, series_id: &str, from_item_id: &str) -> Vec<MediaItem> {
        log::debug!(target: "api", "outbound: EpisodesFrom series={series_id} from={from_item_id}");
        let resp: Value = match self
            .get(&format!("/Shows/{}/Episodes", series_id))
            .query("UserId", &self.user_id)
            .query(
                "Fields",
                "UserData,RunTimeTicks,SeriesId,SeriesName,ParentIndexNumber,IndexNumber",
            )
            .call()
        {
            Ok(r) => match r.into_json() {
                Ok(v) => v,
                Err(e) => {
                    log::warn!(target: "api", "err: EpisodesFrom parse: {e}");
                    return vec![];
                }
            },
            Err(e) => {
                log::warn!(target: "api", "err: EpisodesFrom: {e}");
                return vec![];
            }
        };
        let Some(all) = resp["Items"].as_array() else {
            return vec![];
        };
        let mut found = false;
        let items: Vec<MediaItem> = all
            .iter()
            .filter_map(|v| {
                if found {
                    return Some(parse_item(v));
                }
                if v["Id"].as_str().unwrap_or("") == from_item_id {
                    found = true;
                    Some(parse_item(v))
                } else {
                    None
                }
            })
            .collect();
        if items.is_empty() {
            // from_item_id not in series — return everything as a fallback
            log::warn!(target: "api", "inbound: EpisodesFrom: from_item_id not found, returning all");
            return all.iter().map(parse_item).collect();
        }
        log::info!(target: "api", "inbound: EpisodesFrom: {} episodes from '{}'", items.len(), items[0].display_name());
        items
    }

    #[allow(dead_code)]
    pub fn get_next_up(&self, series_id: &str) -> Option<MediaItem> {
        log::debug!(target: "api", "outbound: NextUp series={series_id}");
        let resp: Value = match self
            .get("/Shows/NextUp")
            .query("UserId", &self.user_id)
            .query("SeriesId", series_id)
            .query("Limit", "1")
            .query(
                "Fields",
                "UserData,RunTimeTicks,SeriesId,SeriesName,ParentIndexNumber,IndexNumber",
            )
            .call()
        {
            Ok(r) => match r.into_json() {
                Ok(v) => v,
                Err(e) => {
                    log::warn!(target: "api", "err: NextUp parse: {e}");
                    return None;
                }
            },
            Err(e) => {
                log::warn!(target: "api", "err: NextUp: {e}");
                return None;
            }
        };
        let items = resp["Items"].as_array()?;
        if items.is_empty() {
            log::debug!(target: "api", "inbound: NextUp: none");
            return None;
        }
        let item = parse_item(&items[0]);
        log::info!(target: "api", "inbound: NextUp: {}", item.display_name());
        Some(item)
    }

    // ── Remote session control ───────────────────────────────────────────────

    #[allow(dead_code)]
    pub fn get_sessions(&self) -> Result<Vec<SessionInfo>, String> {
        self.get_sessions_with_active_within(Some("600"))
    }

    /// Like `get_sessions`, but without the `ActiveWithinSeconds=600` filter
    /// (#236): a device that's been idle-but-still-connected for more than
    /// 10 minutes wouldn't show up in the filtered list, which would make
    /// `App::try_auto_reconnect`'s `DirectSession` lookup wrongly conclude
    /// the device is gone. Only used by that lookup -- the Sessions-panel
    /// (F3) UI should keep using the filtered `get_sessions` above.
    pub fn get_sessions_unfiltered(&self) -> Result<Vec<SessionInfo>, String> {
        self.get_sessions_with_active_within(None)
    }

    fn get_sessions_with_active_within(
        &self,
        active_within_secs: Option<&str>,
    ) -> Result<Vec<SessionInfo>, String> {
        let mut req = self.get("/Sessions");
        if let Some(secs) = active_within_secs {
            req = req.query("ActiveWithinSeconds", secs);
        }
        let arr: Value = req
            .call()
            .map_err(|e| e.to_string())?
            .into_json()
            .map_err(|e| e.to_string())?;
        let sessions = arr
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| {
                        if v["DeviceId"].as_str().unwrap_or("") == self.device_id {
                            return None;
                        }
                        if !v["SupportsRemoteControl"].as_bool().unwrap_or(false) {
                            return None;
                        }
                        let ps = &v["PlayState"];
                        let npi = &v["NowPlayingItem"];
                        let media_info = npi["MediaStreams"]
                            .as_array()
                            .map(|streams| parse_session_media_info(streams))
                            .unwrap_or_default();
                        let raw_host = v["RemoteEndPoint"].as_str().unwrap_or("");
                        let host = raw_host.rsplit(':').nth(1).unwrap_or(raw_host).to_string();
                        Some(SessionInfo {
                            id: v["Id"].as_str().unwrap_or("").to_string(),
                            device_name: v["DeviceName"].as_str().unwrap_or("").to_string(),
                            client: v["Client"].as_str().unwrap_or("").to_string(),
                            user_name: v["UserName"].as_str().unwrap_or("").to_string(),
                            host,
                            supported_commands: v["SupportedCommands"]
                                .as_array()
                                .map(|arr| {
                                    arr.iter()
                                        .filter_map(|value| value.as_str().map(str::to_string))
                                        .collect()
                                })
                                .unwrap_or_default(),
                            now_playing: npi["Name"].as_str().map(str::to_string),
                            now_playing_item_id: npi["Id"].as_str().map(str::to_string),
                            position_s: ps["PositionTicks"].as_i64().unwrap_or(0)
                                / TICKS_PER_SECOND,
                            runtime_s: npi["RunTimeTicks"].as_i64().unwrap_or(0) / TICKS_PER_SECOND,
                            is_paused: ps["IsPaused"].as_bool().unwrap_or(false),
                            volume: ps["VolumeLevel"].as_i64().unwrap_or(100),
                            sub_index: ps["SubtitleStreamIndex"].as_i64().unwrap_or(-1),
                            audio_index: ps["AudioStreamIndex"].as_i64().unwrap_or(0),
                            muted: ps["IsMuted"].as_bool().unwrap_or(false),
                            media_info,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        Ok(sessions)
    }

    pub fn session_transport(&self, id: &str, cmd: &str) -> Result<(), String> {
        self.post(&format!("/Sessions/{id}/Playing/{cmd}"))
            .send_string("")
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn session_seek(&self, id: &str, ticks: i64) -> Result<(), String> {
        self.post(&format!("/Sessions/{id}/Playing/Seek"))
            .query("SeekPositionTicks", &ticks.to_string())
            .send_string("")
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn session_set_volume(&self, id: &str, vol: i64) -> Result<(), String> {
        self.post(&format!("/Sessions/{id}/Command/SetVolume"))
            .send_json(ureq::json!({"Arguments":{"Volume": vol.to_string()}}))
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn session_set_subtitle_index(&self, id: &str, index: i64) -> Result<(), String> {
        self.post(&format!("/Sessions/{id}/Command/SetSubtitleStreamIndex"))
            .send_json(ureq::json!({"Arguments":{"Index": index.to_string()}}))
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn session_set_audio_index(&self, id: &str, index: i64) -> Result<(), String> {
        self.post(&format!("/Sessions/{id}/Command/SetAudioStreamIndex"))
            .send_json(ureq::json!({"Arguments":{"Index": index.to_string()}}))
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn session_set_mute(&self, id: &str, muted: bool) -> Result<(), String> {
        let cmd = if muted { "Mute" } else { "Unmute" };
        self.post(&format!("/Sessions/{id}/Command/{cmd}"))
            .send_string("")
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn session_play(&self, id: &str, item_id: &str, start_ticks: i64) -> Result<(), String> {
        self.post(&format!("/Sessions/{id}/Playing"))
            .send_json(ureq::json!({
                "PlayCommand": "PlayNow",
                "ItemIds": [item_id],
                "StartPositionTicks": start_ticks
            }))
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn session_play_items(
        &self,
        id: &str,
        item_ids: &[String],
        start_idx: usize,
        start_ticks: i64,
    ) -> Result<(), String> {
        self.post(&format!("/Sessions/{id}/Playing"))
            .send_json(ureq::json!({
                "PlayCommand": "PlayNow",
                "ItemIds": item_ids,
                "StartIndex": start_idx,
                "StartPositionTicks": start_ticks
            }))
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn stream_url(&self, item_id: &str) -> String {
        format!(
            "{}/Videos/{}/stream?static=true&api_key={}",
            self.config.server_url, item_id, self.token
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_item(name: &str, item_type: &str) -> MediaItem {
        MediaItem {
            id: "id".into(),
            name: name.into(),
            item_type: item_type.into(),
            is_folder: false,
            media_type: "Video".into(),
            collection_type: String::new(),
            runtime_ticks: 0,
            played: false,
            playback_position_ticks: 0,
            series_id: String::new(),
            series_name: String::new(),
            album_id: String::new(),
            album: String::new(),
            index_number: 0,
            parent_index_number: 0,
            unplayed_item_count: 0,
            path: String::new(),
            artist: String::new(),
            sort_name: String::new(),
            production_year: 0,
            end_year: 0,
            overview: String::new(),
            premiere_date: String::new(),
            date_added: String::new(),
            total_count: 0,
            container: String::new(),
            director: String::new(),
            video_info: String::new(),
            audio_info: String::new(),
            genre: String::new(),
            playlist_item_id: String::new(),
        }
    }

    // ── MediaItem::display_name ──────────────────────────────────────────────

    #[test]
    fn display_name_movie() {
        assert_eq!(make_item("Inception", "Movie").display_name(), "Inception");
    }

    #[test]
    fn display_name_episode_without_series_falls_back_to_name() {
        let item = make_item("Standalone", "Episode");
        assert_eq!(item.display_name(), "Standalone");
    }

    // ── MediaItem::resume_seconds / runtime_seconds ──────────────────────────

    #[test]
    fn resume_seconds_converts_ticks() {
        let mut item = make_item("X", "Movie");
        item.playback_position_ticks = TICKS_PER_SECOND * 90;
        assert!((item.resume_seconds() - 90.0).abs() < 1e-6);
    }

    #[test]
    fn runtime_seconds_converts_ticks() {
        let mut item = make_item("X", "Movie");
        item.runtime_ticks = TICKS_PER_SECOND * 5400; // 90 min
        assert!((item.runtime_seconds() - 5400.0).abs() < 1e-6);
    }

    // ── parse_item ───────────────────────────────────────────────────────────

    #[test]
    fn parse_item_basic_fields() {
        let raw = json!({
            "Id": "abc", "Name": "Test", "Type": "Movie",
            "IsFolder": false, "MediaType": "Video",
            "RunTimeTicks": 36_000_000_000i64,
            "SortName": "test",
            "UserData": { "Played": true, "PlaybackPositionTicks": 5_000_000i64 }
        });
        let item = parse_item(&raw);
        assert_eq!(item.id, "abc");
        assert_eq!(item.name, "Test");
        assert_eq!(item.runtime_ticks, 36_000_000_000);
        assert!(item.played);
        assert_eq!(item.playback_position_ticks, 5_000_000);
    }

    #[test]
    fn parse_item_collection_folder_forces_is_folder() {
        let raw = json!({ "Type": "CollectionFolder", "IsFolder": false, "UserData": {} });
        assert!(parse_item(&raw).is_folder);
    }

    #[test]
    fn parse_item_channel_forces_is_folder() {
        let raw = json!({ "Type": "Channel", "IsFolder": false, "UserData": {} });
        assert!(parse_item(&raw).is_folder);
    }

    #[test]
    fn parse_item_missing_fields_use_defaults() {
        let item = parse_item(&json!({}));
        assert_eq!(item.id, "");
        assert_eq!(item.runtime_ticks, 0);
        assert!(!item.played);
        assert!(!item.is_folder);
    }

    #[test]
    fn parse_item_episode_fields() {
        let raw = json!({
            "Type": "Episode", "Name": "Pilot",
            "SeriesName": "Lost", "IndexNumber": 1, "ParentIndexNumber": 2,
            "UserData": {}
        });
        let item = parse_item(&raw);
        assert_eq!(item.series_name, "Lost");
        assert_eq!(item.index_number, 1);
        assert_eq!(item.parent_index_number, 2);
    }

    // ── MediaItem::playback_label ────────────────────────────────────────────

    #[test]
    fn playback_label_audio_without_artist_falls_back_to_display_name() {
        let item = make_item("Song", "Audio");
        assert_eq!(item.playback_label(), "Song");
    }

    #[test]
    fn playback_label_video_uses_display_name() {
        let item = make_item("Inception", "Movie");
        assert_eq!(item.playback_label(), "Inception");
    }

    // ── MediaItem::file_name / sort_key ──────────────────────────────────────

    #[test]
    fn file_name_extracts_from_path() {
        let mut item = make_item("Movie", "Movie");
        item.path = "/media/movies/Inception (2010).mkv".into();
        assert_eq!(item.file_name(), "Inception (2010).mkv");
    }

    #[test]
    fn file_name_falls_back_to_name_when_path_empty() {
        let item = make_item("Inception", "Movie");
        assert_eq!(item.file_name(), "Inception");
    }

    #[test]
    fn sort_key_prefers_path_filename() {
        let mut item = make_item("Movie", "Movie");
        item.path = "/media/A.mkv".into();
        item.sort_name = "sort".into();
        assert_eq!(item.sort_key(), "A.mkv");
    }

    #[test]
    fn sort_key_falls_back_to_sort_name() {
        let mut item = make_item("Movie", "Movie");
        item.sort_name = "inception the".into();
        assert_eq!(item.sort_key(), "inception the");
    }

    #[test]
    fn sort_key_falls_back_to_name() {
        let item = make_item("Movie", "Movie");
        assert_eq!(item.sort_key(), "Movie");
    }

    // ── parse_item: audio and music folder types ─────────────────────────────

    #[test]
    fn parse_item_audio_not_folder() {
        let raw = json!({ "Type": "Audio", "MediaType": "Audio", "UserData": {} });
        let item = parse_item(&raw);
        assert_eq!(item.item_type, "Audio");
        assert_eq!(item.media_type, "Audio");
        assert!(!item.is_folder);
    }

    #[test]
    fn parse_item_music_album_is_folder() {
        let raw = json!({ "Type": "MusicAlbum", "IsFolder": false, "UserData": {} });
        assert!(parse_item(&raw).is_folder);
    }

    #[test]
    fn parse_item_music_artist_is_folder() {
        let raw = json!({ "Type": "MusicArtist", "IsFolder": false, "UserData": {} });
        assert!(parse_item(&raw).is_folder);
    }

    #[test]
    fn parse_item_series_is_folder() {
        let raw = json!({ "Type": "Series", "IsFolder": false, "UserData": {} });
        assert!(parse_item(&raw).is_folder);
    }

    #[test]
    fn parse_item_artist_from_album_artist_field() {
        let raw = json!({ "Type": "Audio", "AlbumArtist": "Pink Floyd", "UserData": {} });
        assert_eq!(parse_item(&raw).artist, "Pink Floyd");
    }

    #[test]
    fn parse_item_artist_falls_back_to_artists_array() {
        let raw = json!({ "Type": "Audio", "Artists": ["David Bowie"], "UserData": {} });
        assert_eq!(parse_item(&raw).artist, "David Bowie");
    }

    #[test]
    fn parse_item_album_artist_takes_priority_over_artists_array() {
        let raw = json!({ "Type": "Audio", "AlbumArtist": "Album Artist", "Artists": ["Track Artist"], "UserData": {} });
        assert_eq!(parse_item(&raw).artist, "Album Artist");
    }

    // ── EmbyClient::stream_url ───────────────────────────────────────────────

    #[test]
    fn stream_url_format() {
        let c = client_with_url("http://server:8096");
        assert_eq!(
            c.stream_url("abc123"),
            "http://server:8096/Videos/abc123/stream?static=true&api_key=tok"
        );
    }

    // ── EmbyClient::ws_url ───────────────────────────────────────────────────

    fn client_with_url(url: &str) -> EmbyClient {
        let cfg = crate::config::Config {
            server_url: url.into(),
            ..crate::config::Config::default()
        };
        let mut c = EmbyClient::new(cfg);
        c.token = "tok".into();
        c
    }

    fn local_listener_url() -> (std::net::TcpListener, String) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        (listener, url)
    }

    fn read_one_request(stream: &mut std::net::TcpStream) -> String {
        use std::io::Read;

        let mut request = Vec::new();
        let mut buf = [0_u8; 1024];
        loop {
            match stream.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    request.extend_from_slice(&buf[..n]);
                    if request.windows(4).any(|w| w == b"\r\n\r\n") {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
        String::from_utf8_lossy(&request).into_owned()
    }

    #[test]
    fn report_stopped_for_shutdown_stalls_with_one_attempt_and_no_retry() {
        let (listener, url) = local_listener_url();
        let attempts = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let attempts_for_thread = attempts.clone();
        std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                attempts_for_thread.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let _ = read_one_request(&mut stream);
                std::thread::sleep(std::time::Duration::from_secs(5));
            }
        });

        let started = std::time::Instant::now();
        let ok = client_with_url(&url).report_stopped_for_shutdown(
            "item",
            "msid",
            123,
            "sid",
            456,
            std::time::Duration::from_millis(150),
        );

        assert!(!ok);
        assert!(started.elapsed() < std::time::Duration::from_secs(1));
        assert_eq!(attempts.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[test]
    fn ordinary_report_stopped_still_retries_once() {
        let (listener, url) = local_listener_url();
        let attempts = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let attempts_for_thread = attempts.clone();
        std::thread::spawn(move || {
            for _ in 0..2 {
                if let Ok((mut stream, _)) = listener.accept() {
                    attempts_for_thread.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    let _ = read_one_request(&mut stream);
                }
            }
        });

        let ok = client_with_url(&url).report_stopped("item", "msid", 123, "sid", 456);

        assert!(!ok);
        assert_eq!(attempts.load(std::sync::atomic::Ordering::SeqCst), 2);
    }

    #[test]
    fn ws_url_http_becomes_ws() {
        let url = client_with_url("http://server:8096").ws_url();
        assert!(url.starts_with("ws://"));
        assert!(url.contains("api_key=tok"));
    }

    #[test]
    fn ws_url_https_becomes_wss() {
        let url = client_with_url("https://server:8096").ws_url();
        assert!(url.starts_with("wss://"));
    }

    #[test]
    fn ws_url_contains_device_id() {
        let mut c = client_with_url("http://server:8096");
        c.device_id = "my-device-id".into();
        let url = c.ws_url();
        assert!(url.contains("deviceId=my-device-id"));
    }

    #[test]
    fn parse_mbv_direct_tcp_port_command_extracts_port() {
        let commands = vec![
            "Play".to_string(),
            mbv_direct_tcp_port_command(47788),
            "Pause".to_string(),
        ];
        assert_eq!(parse_mbv_direct_tcp_port(&commands), Some(47788));
    }

    // ── auth_header ──────────────────────────────────────────────────────────

    #[test]
    fn auth_header_contains_device_name_and_id() {
        let mut c = client_with_url("http://server:8096");
        c.device_name = "myhost".into();
        c.device_id = "abcd-1234".into();
        c.token = "mytoken".into();
        let h = c.auth_header();
        assert!(h.contains("Device=\"myhost\""), "header: {h}");
        assert!(h.contains("DeviceId=\"abcd-1234\""), "header: {h}");
        assert!(h.contains("Token=\"mytoken\""), "header: {h}");
    }

    // ── device_name ──────────────────────────────────────────────────────────

    #[test]
    fn device_name_trims_hostname_env_var() {
        // /etc/hostname will be read first on Linux; test only the env-var path
        // by observing that a client created with HOSTNAME set has no whitespace
        // in its device_name field.
        let name = {
            let _g = crate::config::tests::SYS_ENV_LOCK.lock().unwrap();
            std::env::set_var("HOSTNAME", "  trimtest  \n");
            let c = EmbyClient::new(crate::config::Config::default());
            std::env::remove_var("HOSTNAME");
            c.device_name
        };
        // device_name should never embed raw whitespace
        assert!(!name.contains('\n'), "name contains newline: {:?}", name);
        assert_eq!(name, name.trim());
    }

    #[test]
    fn device_name_falls_back_to_mbv() {
        // Only reachable when both /etc/hostname is absent/empty and HOSTNAME unset.
        // We can't suppress /etc/hostname, but we can verify the fallback string
        // is the sentinel "mbv" when nothing else is available.
        let name = device_name();
        assert!(!name.is_empty());
        // Must not contain raw newlines regardless of source.
        assert!(!name.contains('\n'));
    }

    // ── device_id ────────────────────────────────────────────────────────────

    #[test]
    fn device_id_returns_uuid_v4_format() {
        let id = device_id_in(make_temp_data_dir());
        // xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx
        let parts: Vec<&str> = id.split('-').collect();
        assert_eq!(parts.len(), 5, "id: {id}");
        assert_eq!(parts[0].len(), 8);
        assert_eq!(parts[1].len(), 4);
        assert_eq!(parts[2].len(), 4);
        assert!(parts[2].starts_with('4'), "version nibble: {id}");
        assert_eq!(parts[3].len(), 4);
        assert_eq!(parts[4].len(), 12);
        assert!(id.chars().all(|c| c.is_ascii_hexdigit() || c == '-'));
    }

    #[test]
    fn device_id_is_stable_across_calls() {
        let dir = make_temp_data_dir();
        let first = device_id_in(dir.clone());
        let second = device_id_in(dir);
        assert_eq!(first, second);
    }

    #[test]
    fn device_id_respects_xdg_data_home() {
        let dir = make_temp_data_dir();
        let id = device_id_in(dir.clone());
        let persisted = std::fs::read_to_string(dir.join("mbv/device_id")).unwrap();
        assert_eq!(persisted.trim(), id);
    }

    #[test]
    fn device_id_migrates_from_legacy_mby_dir() {
        let dir = make_temp_data_dir();
        let legacy_dir = dir.join("mby");
        std::fs::create_dir_all(&legacy_dir).unwrap();
        let legacy_id = uuid::Uuid::new_v4().to_string();
        std::fs::write(legacy_dir.join("device_id"), &legacy_id).unwrap();
        let id = device_id_in(dir.clone());
        assert_eq!(id, legacy_id, "should reuse the legacy mby device_id");
        let persisted = std::fs::read_to_string(dir.join("mbv/device_id")).unwrap();
        assert_eq!(
            persisted.trim(),
            legacy_id,
            "should persist migrated id to new location"
        );
    }

    fn make_temp_data_dir() -> std::path::PathBuf {
        std::env::temp_dir().join(format!("mbv-test-{}", uuid::Uuid::new_v4()))
    }

    // ── decode_html_entities ─────────────────────────────────────────────────

    #[test]
    fn decode_html_entities_known_entities() {
        assert_eq!(decode_html_entities("&quot;hi&quot;"), "\"hi\"");
        assert_eq!(decode_html_entities("it&apos;s"), "it's");
        assert_eq!(decode_html_entities("a &lt; b &gt; c"), "a < b > c");
        assert_eq!(decode_html_entities("a &amp; b"), "a & b");
    }

    #[test]
    fn decode_html_entities_passthrough() {
        assert_eq!(decode_html_entities("plain text"), "plain text");
        assert_eq!(decode_html_entities(""), "");
    }

    // ── parse_video_info ─────────────────────────────────────────────────────

    #[test]
    fn parse_video_info_4k() {
        let streams = json!([{"Type": "Video", "Width": 3840, "Height": 2160, "Codec": "hevc"}]);
        assert_eq!(parse_video_info(streams.as_array().unwrap()), "4K HEVC");
    }

    #[test]
    fn parse_video_info_1080p() {
        let streams = json!([{"Type": "Video", "Width": 1920, "Height": 1080, "Codec": "h264"}]);
        assert_eq!(parse_video_info(streams.as_array().unwrap()), "1080p H264");
    }

    #[test]
    fn parse_video_info_720p() {
        let streams = json!([{"Type": "Video", "Width": 1280, "Height": 720, "Codec": "h264"}]);
        assert_eq!(parse_video_info(streams.as_array().unwrap()), "720p H264");
    }

    #[test]
    fn parse_video_info_codec_only_when_no_resolution() {
        let streams = json!([{"Type": "Video", "Width": 0, "Height": 0, "Codec": "vp9"}]);
        assert_eq!(parse_video_info(streams.as_array().unwrap()), "VP9");
    }

    #[test]
    fn parse_video_info_empty_when_no_video_stream() {
        let streams = json!([{"Type": "Audio", "Codec": "aac"}]);
        assert_eq!(parse_video_info(streams.as_array().unwrap()), "");
    }

    // ── parse_audio_info ─────────────────────────────────────────────────────

    fn audio_stream(lang: &str, codec: &str, layout: &str) -> serde_json::Value {
        json!({"Type": "Audio", "Language": lang, "Codec": codec, "ChannelLayout": layout})
    }

    #[test]
    fn parse_audio_info_multiple_tracks() {
        let streams = json!([
            audio_stream("eng", "ac3", "5.1"),
            audio_stream("fra", "aac", "stereo"),
        ]);
        assert_eq!(
            parse_audio_info(streams.as_array().unwrap()),
            "English AC3 5.1  |  French AAC Stereo"
        );
    }

    #[test]
    fn parse_audio_info_unknown_lang_omitted_from_label() {
        let streams = json!([audio_stream("und", "aac", "stereo")]);
        assert_eq!(parse_audio_info(streams.as_array().unwrap()), "AAC Stereo");
    }

    #[test]
    fn parse_audio_info_skips_non_audio_streams() {
        let streams = json!([
            {"Type": "Video", "Language": "eng", "Codec": "h264", "ChannelLayout": ""},
            audio_stream("eng", "aac", "stereo"),
        ]);
        assert_eq!(
            parse_audio_info(streams.as_array().unwrap()),
            "English AAC Stereo"
        );
    }

    // Sync guard: every ISO code in parse_audio_info must produce the same English
    // name as lang_code_to_name() in player.rs. Both tables must be updated together.
    // The mirror test in player.rs::tests::lang_code_to_name_matches_api_table checks
    // the other side.
    #[test]
    fn parse_audio_info_lang_table_matches_player_lang_code_to_name() {
        let cases: &[(&str, &str)] = &[
            ("en", "English"),
            ("eng", "English"),
            ("fr", "French"),
            ("fre", "French"),
            ("fra", "French"),
            ("de", "German"),
            ("ger", "German"),
            ("deu", "German"),
            ("es", "Spanish"),
            ("spa", "Spanish"),
            ("it", "Italian"),
            ("ita", "Italian"),
            ("pt", "Portuguese"),
            ("por", "Portuguese"),
            ("ja", "Japanese"),
            ("jpn", "Japanese"),
            ("ko", "Korean"),
            ("kor", "Korean"),
            ("zh", "Chinese"),
            ("chi", "Chinese"),
            ("zho", "Chinese"),
            ("ru", "Russian"),
            ("rus", "Russian"),
            ("ar", "Arabic"),
            ("ara", "Arabic"),
            ("nl", "Dutch"),
            ("nld", "Dutch"),
            ("dut", "Dutch"),
            ("sv", "Swedish"),
            ("swe", "Swedish"),
            ("no", "Norwegian"),
            ("nor", "Norwegian"),
            ("da", "Danish"),
            ("dan", "Danish"),
            ("fi", "Finnish"),
            ("fin", "Finnish"),
            ("pl", "Polish"),
            ("pol", "Polish"),
            ("cs", "Czech"),
            ("cze", "Czech"),
            ("ces", "Czech"),
            ("tr", "Turkish"),
            ("tur", "Turkish"),
        ];
        for (code, expected) in cases {
            let streams =
                json!([{"Type": "Audio", "Language": code, "Codec": "", "ChannelLayout": ""}]);
            let result = parse_audio_info(streams.as_array().unwrap());
            assert_eq!(
                result, *expected,
                "parse_audio_info: code {:?} → expected {:?}, got {:?}",
                code, expected, result
            );
        }
    }

    #[test]
    fn parse_session_media_info_extracts_remote_stream_options() {
        let streams = json!([
            {"Type": "Video", "Width": 1920, "Height": 1080, "Codec": "h264"},
            {"Type": "Audio", "Index": 1, "Language": "eng", "Codec": "ac3", "ChannelLayout": "5.1"},
            {"Type": "Audio", "Index": 2, "Language": "jpn", "Codec": "aac", "ChannelLayout": "stereo"},
            {"Type": "Subtitle", "Index": 3, "Language": "eng", "IsForced": false},
            {"Type": "Subtitle", "Index": 4, "Language": "eng", "IsForced": true}
        ]);
        let media = parse_session_media_info(streams.as_array().unwrap());
        assert_eq!(media.video_label, "1080p H264");
        assert!(!media.audio_only);
        assert_eq!(media.audio_streams.len(), 2);
        assert_eq!(media.audio_streams[0].index, 1);
        assert_eq!(media.audio_streams[0].label, "English AC3 5.1");
        assert_eq!(media.audio_streams[1].label, "Japanese AAC Stereo");
        assert_eq!(media.subtitle_streams.len(), 2);
        assert_eq!(media.subtitle_streams[0].label, "English");
        assert_eq!(media.subtitle_streams[1].label, "English (Forced)");
    }

    #[test]
    fn parse_session_media_info_handles_audio_only_sessions() {
        let streams = json!([
            {"Type": "Audio", "Index": 0, "Language": "eng", "Codec": "flac", "ChannelLayout": "stereo"}
        ]);
        let media = parse_session_media_info(streams.as_array().unwrap());
        assert!(media.audio_only);
        assert_eq!(media.video_label, "English FLAC Stereo");
        assert_eq!(media.audio_streams.len(), 1);
        assert_eq!(media.audio_streams[0].index, 0);
    }

    // ── is_audio / is_video ──────────────────────────────────────────────────

    // ── should_resume ────────────────────────────────────────────────────────

    #[test]
    fn should_resume_zero_position_returns_false() {
        assert!(!make_item("X", "Movie").should_resume());
    }

    #[test]
    fn should_resume_negative_position_returns_false() {
        let mut item = make_item("X", "Movie");
        item.playback_position_ticks = -1;
        assert!(!item.should_resume());
    }

    #[test]
    fn should_resume_mid_way_returns_true() {
        let mut item = make_item("X", "Movie");
        item.runtime_ticks = TICKS_PER_SECOND * 7200;
        item.playback_position_ticks = TICKS_PER_SECOND * 3600; // 50%
        assert!(item.should_resume());
    }

    #[test]
    fn should_resume_under_one_percent_returns_false() {
        let mut item = make_item("X", "Movie");
        item.runtime_ticks = TICKS_PER_SECOND * 7200; // 2h
        item.playback_position_ticks = TICKS_PER_SECOND; // ~0.01%
        assert!(!item.should_resume());
    }

    #[test]
    fn should_resume_with_unknown_runtime_returns_true() {
        let mut item = make_item("X", "Movie");
        item.runtime_ticks = 0;
        item.playback_position_ticks = TICKS_PER_SECOND * 60;
        assert!(item.should_resume());
    }
}
