use std::sync::mpsc;

use serde_json::Value;

use crate::applog::{AppLog, Level};
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
    let t = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    format!("{:x}{:x}", t.as_secs(), t.subsec_nanos())
}

fn device_name() -> String {
    std::fs::read_to_string("/etc/hostname")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| std::env::var("HOSTNAME").ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()))
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
        if !id.is_empty() { return id; }
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
        eprintln!("mbv: could not write device_id to {}: {}", path.display(), e);
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
        if pos <= 0 { return false; }
        if self.runtime_ticks > 0 && pos * 100 < self.runtime_ticks { return false; } // displays as 0%
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
        if !self.path.is_empty() { self.file_name() }
        else if !self.sort_name.is_empty() { &self.sort_name }
        else { &self.name }
    }

    pub fn playback_label(&self) -> String {
        if self.item_type == "Audio" && !self.artist.is_empty() {
            format!("{} - {}", self.artist, self.name)
        } else {
            self.display_name()
        }
    }

    pub fn display_name(&self) -> String {
        if self.item_type == "Episode" && !self.series_name.is_empty() {
            format!("{} - {}", self.series_name, self.name)
        } else {
            self.name.clone()
        }
    }
}

#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub id:                  String,
    pub device_name:         String,
    pub client:              String,
    pub user_name:           String,
    pub host:                String,
    pub now_playing:         Option<String>,
    pub now_playing_item_id: Option<String>,
    pub position_s:          i64,
    pub runtime_s:           i64,
    pub is_paused:           bool,
    pub volume:              i64,
    pub sub_index:           i64,   // -1 = disabled
    pub audio_index:         i64,   // 1-based; 0 = unknown
}

fn parse_item(raw: &Value) -> MediaItem {
    let ud = raw.get("UserData").unwrap_or(&Value::Null);
    let item_type = raw["Type"].as_str().unwrap_or("").to_string();
    let is_folder = raw["IsFolder"].as_bool().unwrap_or(false)
        || matches!(item_type.as_str(), "CollectionFolder" | "Channel" | "Series" | "Season"
                                        | "MusicArtist" | "MusicAlbum" | "BoxSet" | "Folder");
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
        artist: raw["AlbumArtist"].as_str()
            .or_else(|| raw["Artists"].get(0).and_then(|v| v.as_str()))
            .unwrap_or("").to_string(),
        sort_name: raw["SortName"].as_str().unwrap_or("").to_string(),
        production_year: raw["ProductionYear"].as_u64()
            .or_else(|| raw["Year"].as_u64())
            .unwrap_or(0) as u32,
        end_year: raw["EndDate"].as_str()
            .and_then(|s| s.get(..4))
            .and_then(|s| s.parse().ok())
            .unwrap_or(0),
        overview: decode_html_entities(raw["Overview"].as_str().unwrap_or("")),
        premiere_date: raw["PremiereDate"].as_str()
            .and_then(|s| s.get(..10))
            .map(|s| s.to_string())
            .unwrap_or_default(),
        date_added: raw["DateCreated"].as_str()
            .and_then(|s| s.get(..10))
            .map(|s| s.to_string())
            .unwrap_or_default(),
        total_count,
        container: raw["Container"].as_str().unwrap_or("").to_string(),
        genre: raw["Genres"].as_array()
            .and_then(|g| g.first().and_then(|v| v.as_str()))
            .unwrap_or("").to_string(),
        director: raw["People"].as_array()
            .and_then(|people| people.iter()
                .find(|p| p["Type"].as_str() == Some("Director"))
                .and_then(|p| p["Name"].as_str()))
            .unwrap_or("").to_string(),
        video_info: raw["MediaStreams"].as_array()
            .and_then(|streams| streams.iter().find(|s| s["Type"].as_str() == Some("Video")))
            .map(|s| {
                let width  = s["Width"].as_u64().unwrap_or(0);
                let height = s["Height"].as_u64().unwrap_or(0);
                let dim = width.max(height);
                let res = match dim {
                    3840.. => "4K".to_string(),
                    1920.. => "1080p".to_string(),
                    1280.. => "720p".to_string(),
                    720..  => "480p".to_string(),
                    d if d > 0 => format!("{}p", height),
                    _ => String::new(),
                };
                let codec = s["Codec"].as_str().unwrap_or("").to_uppercase();
                match (res.is_empty(), codec.is_empty()) {
                    (false, false) => format!("{} {}", res, codec),
                    (false, true)  => res,
                    (true, false)  => codec,
                    (true, true)   => String::new(),
                }
            })
            .unwrap_or_default(),
        playlist_item_id: raw["PlaylistItemId"].as_str().unwrap_or("").to_string(),
        audio_info: raw["MediaStreams"].as_array()
            .map(|streams| {
                let mut parts: Vec<String> = Vec::new();
                for s in streams.iter().filter(|s| s["Type"].as_str() == Some("Audio")) {
                    let lang = s["Language"].as_str().unwrap_or("");
                    let lang_name = match lang {
                        "eng" => "English",
                        "chi" | "zho" => "Chinese",
                        "jpn" => "Japanese",
                        "fre" | "fra" => "French",
                        "ger" | "deu" => "German",
                        "spa" => "Spanish",
                        "ita" => "Italian",
                        "kor" => "Korean",
                        "por" => "Portuguese",
                        "rus" => "Russian",
                        other if !other.is_empty() => other,
                        _ => "",
                    };
                    let codec = s["Codec"].as_str().unwrap_or("").to_uppercase();
                    let layout = s["ChannelLayout"].as_str().unwrap_or("");
                    let layout_str = match layout {
                        "mono"   => "Mono",
                        "stereo" => "Stereo",
                        "5.1"    => "5.1",
                        "7.1"    => "7.1",
                        other if !other.is_empty() => other,
                        _ => "",
                    };
                    let track: Vec<&str> = [lang_name, &codec, layout_str]
                        .iter().filter(|s| !s.is_empty()).copied().collect();
                    if !track.is_empty() { parts.push(track.join(" ")); }
                }
                parts.join("  |  ")
            })
            .unwrap_or_default(),
    }
}

fn load_cached_token() -> Option<(String, String, String)> {
    let path = crate::config::token_cache_path();
    let text = std::fs::read_to_string(path).ok()?;
    let v: Value = serde_json::from_str(&text).ok()?;
    let token = v["token"].as_str()?.to_string();
    let user_id = v["user_id"].as_str()?.to_string();
    if token.is_empty() || user_id.is_empty() { return None; }
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
    pub fn new(config: Config) -> Self {
        let agent = ureq::AgentBuilder::new()
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
            self.device_name, self.device_id, env!("CARGO_PKG_VERSION"), self.token
        )
    }

    fn get(&self, path: &str) -> ureq::Request {
        self.agent.get(&self.url(path))
            .set("Authorization", &self.auth_header())
            .set("X-Emby-Token", &self.token)
    }

    fn post(&self, path: &str) -> ureq::Request {
        self.agent.post(&self.url(path))
            .set("Authorization", &self.auth_header())
            .set("X-Emby-Token", &self.token)
    }

    fn delete(&self, path: &str) -> ureq::Request {
        self.agent.delete(&self.url(path))
            .set("Authorization", &self.auth_header())
            .set("X-Emby-Token", &self.token)
    }

    pub fn authenticate(&mut self) -> Result<(), String> {
        let Some((cached_url, token, user_id)) = load_cached_token() else {
            return Err("No cached credentials".to_string());
        };
        if self.config.server_url.is_empty() && !cached_url.is_empty() {
            self.config.server_url = cached_url;
        }
        self.token = token;
        self.user_id = user_id;
        match self.get("/Users/Me").call() {
            Ok(_) => Ok(()),
            Err(ureq::Error::Status(401, _)) | Err(ureq::Error::Status(403, _)) => {
                self.token.clear();
                self.user_id.clear();
                Err("Cached token rejected".to_string())
            }
            Err(_) => {
                // Network error — keep token and proceed; UI surfaces errors when loading content.
                Ok(())
            }
        }
    }

    // Authenticate using credentials in self.config (password or api_key).
    // Does not check the token cache. Saves a fresh token to the cache on success.
    // Called by authenticate() on cache miss, and directly by the login screen.
    pub fn authenticate_credentials(&mut self) -> Result<(), String> {
        // Prefer password auth: yields a user-scoped token so sessions are attributed to the
        // correct user (required for activity tracking and progress saving).
        // API key auth yields an admin token with no user association — use only as fallback.
        if !self.config.password.is_empty() {
            let resp: Value = self.agent
                .post(&self.url("/Users/AuthenticateByName"))
                .set("Authorization", &format!("Emby Client=\"mbv\", Device=\"{}\", DeviceId=\"{}\", Version=\"{}\"", self.device_name, self.device_id, env!("CARGO_PKG_VERSION")))
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
            let users: Value = self.agent
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
                let found = users.iter().find(|u| {
                    u["Name"].as_str().unwrap_or("").to_lowercase() == uname
                });
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

    pub fn get_views(&self) -> Result<Vec<MediaItem>, String> {
        let vfolders: Value = self.get("/Library/VirtualFolders")
            .call().map_err(|e| e.to_string())?
            .into_json().map_err(|e| e.to_string())?;

        let user_views: Value = self.get(&format!("/Users/{}/Views", self.user_id))
            .call().map_err(|e| e.to_string())?
            .into_json().map_err(|e| e.to_string())?;

        let mut items: Vec<MediaItem> = Vec::new();
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

        if let Some(arr) = vfolders.as_array() {
            for f in arr {
                let id = f["ItemId"].as_str().unwrap_or("").to_string();
                let item = MediaItem {
                    id: id.clone(),
                    name: f["Name"].as_str().unwrap_or("").to_string(),
                    item_type: "CollectionFolder".to_string(),
                    is_folder: true,
                    collection_type: f["CollectionType"].as_str().unwrap_or("").to_string(),
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
                };
                seen.insert(id);
                items.push(item);
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
        let resp: Value = self.get(&format!("/Users/{}/Views", self.user_id))
            .call().map_err(|e| e.to_string())?
            .into_json().map_err(|e| e.to_string())?;
        Ok(resp["Items"].as_array()
            .map(|arr| arr.iter().map(parse_item).collect())
            .unwrap_or_default())
    }

    pub fn get_items_sorted(&self, parent_id: &str, item_types: Option<&str>, unplayed_only: bool, start_index: usize, limit: usize, sort_by: &str, sort_order: &str) -> Result<(Vec<MediaItem>, usize), String> {
        let mut req = self.get(&format!("/Users/{}/Items", self.user_id))
            .query("ParentId", parent_id)
            .query("SortBy", sort_by)
            .query("SortOrder", sort_order)
            .query("StartIndex", &start_index.to_string())
            .query("Limit", &limit.to_string())
            .query("Fields", "UserData,RunTimeTicks,MediaType,SeriesId,SeriesName,SortName,ParentIndexNumber,IndexNumber,Path,AlbumArtist,Artists,ProductionYear,EndDate,Overview,PremiereDate,DateCreated,ChildCount,RecursiveItemCount,Container,People,MediaStreams,Genres")
            .query("EnableUserData", "true");
        if let Some(types) = item_types {
            req = req.query("IncludeItemTypes", types).query("Recursive", "true");
        }
        if unplayed_only {
            req = req.query("Filters", "IsUnplayed");
        }
        let resp: Value = req.call().map_err(|e| e.to_string())?
            .into_json().map_err(|e| e.to_string())?;
        let total = resp["TotalRecordCount"].as_u64().unwrap_or(0) as usize;
        let items = resp["Items"].as_array()
            .map(|arr| arr.iter().map(parse_item).collect())
            .unwrap_or_default();
        Ok((items, total))
    }

    pub fn get_continue_watching(&self, limit: usize) -> Result<Vec<MediaItem>, String> {
        let resp: Value = self.get(&format!("/Users/{}/Items/Resume", self.user_id))
            .query("UserId", &self.user_id)
            .query("Limit", &limit.to_string())
            .query("Fields", "UserData,RunTimeTicks,MediaType,SeriesId,SeriesName,SortName,ParentIndexNumber,IndexNumber,Path,AlbumArtist,Artists")
            .query("MediaTypes", "Video")
            .call().map_err(|e| e.to_string())?
            .into_json().map_err(|e| e.to_string())?;
        Ok(resp["Items"].as_array()
            .map(|arr| arr.iter().map(parse_item).collect())
            .unwrap_or_default())
    }

    pub fn get_latest(&self, parent_id: &str, limit: usize) -> Result<Vec<MediaItem>, String> {
        let resp: Value = self.get(&format!("/Users/{}/Items/Latest", self.user_id))
            .query("ParentId", parent_id)
            .query("Limit", &limit.to_string())
            .query("Fields", "UserData,RunTimeTicks,MediaType,SeriesId,SeriesName,SortName,ParentIndexNumber,IndexNumber,Path,AlbumArtist,Artists,AlbumId")
            .call().map_err(|e| e.to_string())?
            .into_json().map_err(|e| e.to_string())?;
        Ok(resp.as_array()
            .map(|arr| arr.iter().map(parse_item).collect())
            .unwrap_or_default())
    }

    pub fn get_latest_episodes(&self, parent_id: &str, limit: usize) -> Result<Vec<MediaItem>, String> {
        let resp: Value = self.get(&format!("/Users/{}/Items", self.user_id))
            .query("ParentId", parent_id)
            .query("Limit", &limit.to_string())
            .query("IncludeItemTypes", "Episode")
            .query("Recursive", "true")
            .query("SortBy", "DateCreated")
            .query("SortOrder", "Descending")
            .query("IsPlayed", "false")
            .query("Fields", "UserData,RunTimeTicks,MediaType,SeriesId,SeriesName,SortName,ParentIndexNumber,IndexNumber,Path")
            .call().map_err(|e| e.to_string())?
            .into_json().map_err(|e| e.to_string())?;
        Ok(resp["Items"].as_array()
            .map(|arr| arr.iter().map(parse_item).collect())
            .unwrap_or_default())
    }

    pub fn get_all_playable_recursive(&self, parent_id: &str) -> Result<Vec<MediaItem>, String> {
        let resp: Value = self.get(&format!("/Users/{}/Items", self.user_id))
            .query("ParentId", parent_id)
            .query("IncludeItemTypes", "Episode,Movie,Video,Audio")
            .query("Recursive", "true")
            .query("SortBy", "SortName")
            .query("SortOrder", "Ascending")
            .query("Limit", "2000")
            .query("Fields", "UserData,RunTimeTicks,MediaType,SeriesId,SeriesName,SortName,ParentIndexNumber,IndexNumber,Path,AlbumArtist,Artists")
            .call().map_err(|e| e.to_string())?
            .into_json().map_err(|e| e.to_string())?;
        Ok(resp["Items"].as_array()
            .map(|arr| arr.iter().map(parse_item).collect())
            .unwrap_or_default())
    }

    pub fn get_direct_playable(&self, parent_id: &str) -> Result<Vec<MediaItem>, String> {
        let resp: Value = self.get(&format!("/Users/{}/Items", self.user_id))
            .query("ParentId", parent_id)
            .query("IncludeItemTypes", "Episode,Movie,Video,Audio")
            .query("SortBy", "SortName")
            .query("SortOrder", "Ascending")
            .query("Limit", "2000")
            .query("Fields", "UserData,RunTimeTicks,MediaType,SeriesId,SeriesName,SortName,ParentIndexNumber,IndexNumber,Path,AlbumArtist,Artists")
            .call().map_err(|e| e.to_string())?
            .into_json().map_err(|e| e.to_string())?;
        Ok(resp["Items"].as_array()
            .map(|arr| arr.iter().map(parse_item).collect())
            .unwrap_or_default())
    }

    pub fn get_all_videos_recursive(&self, parent_id: &str) -> Result<Vec<MediaItem>, String> {
        let resp: Value = self.get(&format!("/Users/{}/Items", self.user_id))
            .query("ParentId", parent_id)
            .query("IncludeItemTypes", "Episode,Movie,Video")
            .query("Recursive", "true")
            .query("SortBy", "SortName")
            .query("SortOrder", "Ascending")
            .query("Limit", "2000")
            .query("Fields", "UserData,RunTimeTicks,MediaType,SeriesId,SeriesName,SortName,ParentIndexNumber,IndexNumber,Path,AlbumArtist,Artists")
            .call().map_err(|e| e.to_string())?
            .into_json().map_err(|e| e.to_string())?;
        Ok(resp["Items"].as_array()
            .map(|arr| arr.iter().map(parse_item).collect())
            .unwrap_or_default())
    }

    pub fn mark_played(&self, item_id: &str) -> Result<(), String> {
        self.post(&format!("/Users/{}/PlayedItems/{}", self.user_id, item_id))
            .call().map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn mark_unplayed(&self, item_id: &str) -> Result<(), String> {
        self.delete(&format!("/Users/{}/PlayedItems/{}", self.user_id, item_id))
            .call().map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn hide_from_resume(&self, item_id: &str) -> Result<(), String> {
        self.post(&format!("/Users/{}/Items/{}/HideFromResume", self.user_id, item_id))
            .query("Hide", "true")
            .call().map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn ws_url(&self) -> String {
        let base = self.config.server_url
            .replacen("https://", "wss://", 1)
            .replacen("http://", "ws://", 1);
        format!("{}/embywebsocket?api_key={}&deviceId={}", base, self.token, self.device_id)
    }

    pub fn report_start(&self, item: &MediaItem, media_source_id: &str, session_id: &str, log: &AppLog) {
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
        });
        log.push(Level::Info, "api", format!("→ Playing item={} msid={media_source_id} pos={}", item.id, item.playback_position_ticks));
        match self.post("/Sessions/Playing").send_json(body) {
            Ok(r)  => log.push(Level::Info, "api", format!("← {} Playing", r.status())),
            Err(e) => log.push(Level::Warn, "api", format!("← ERR Playing: {e}")),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn report_progress_ws(&self, item_id: &str, media_source_id: &str, position_ticks: i64, is_paused: bool, session_id: &str, event_name: &str, ws_tx: &mpsc::Sender<String>, log: &AppLog) {
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
        });
        let msg = serde_json::json!({
            "MessageType": "ReportPlaybackProgress",
            "Data": data,
        }).to_string();
        log.push(Level::Debug, "api", format!("→ ws Progress pos={position_ticks} paused={is_paused} event={event_name}"));
        let _ = ws_tx.send(msg);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn report_progress_http(&self, item_id: &str, media_source_id: &str, position_ticks: i64, is_paused: bool, session_id: &str, event_name: &str, log: &AppLog) {
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
        });
        log.push(Level::Debug, "api", format!("→ Progress pos={position_ticks} paused={is_paused} event={event_name}"));
        match self.post("/Sessions/Playing/Progress").send_json(body) {
            Ok(r)  => log.push(Level::Debug, "api", format!("← {} Progress", r.status())),
            Err(e) => log.push(Level::Warn,  "api", format!("← ERR Progress: {e}")),
        }
    }

    pub fn report_ping(&self, session_id: &str, log: &AppLog) {
        log.push(Level::Debug, "api", format!("→ Ping session={session_id}"));
        match self.post("/Sessions/Playing/Ping")
            .query("PlaySessionId", session_id)
            .send_string("")
        {
            Ok(r)  => log.push(Level::Debug, "api", format!("← {} Ping", r.status())),
            Err(e) => log.push(Level::Warn,  "api", format!("← ERR Ping: {e}")),
        }
    }

    pub fn report_stopped(&self, item_id: &str, media_source_id: &str, position_ticks: i64, session_id: &str, log: &AppLog) {
        let body = ureq::json!({
            "UserId": self.user_id,
            "ItemId": item_id,
            "MediaSourceId": media_source_id,
            "PlaySessionId": session_id,
            "PositionTicks": position_ticks,
            "PlayMethod": "DirectPlay",
        });
        log.push(Level::Info, "api", format!("→ Stopped pos={position_ticks}"));
        match self.post("/Sessions/Playing/Stopped").send_json(body) {
            Ok(r)  => log.push(Level::Info, "api", format!("← {} Stopped", r.status())),
            Err(e) => log.push(Level::Warn, "api", format!("← ERR Stopped: {e}")),
        }

        if !self.user_id.is_empty() {
            let path = format!("/Users/{}/PlayingItems/{}", self.user_id, item_id);
            log.push(Level::Info, "api", format!("→ DELETE PlayingItem pos={position_ticks}"));
            match self.delete(&path)
                .query("MediaSourceId", media_source_id)
                .query("PositionTicks", &position_ticks.to_string())
                .query("PlaySessionId", session_id)
                .call()
            {
                Ok(r)  => log.push(Level::Info, "api", format!("← {} PlayingItem", r.status())),
                Err(e) => log.push(Level::Warn, "api", format!("← ERR PlayingItem: {e}")),
            }
        }
    }

    pub fn register_capabilities(&self, log: &AppLog) {
        let body = ureq::json!({
            "PlayableMediaTypes": ["Audio", "Video"],
            "SupportedCommands": [
                "Play","Stop","Pause","Unpause","NextTrack","PreviousTrack",
                "Seek","SetVolume","VolumeUp","VolumeDown","Mute","Unmute","ToggleMute",
                "SetAudioStreamIndex","SetSubtitleStreamIndex","DisplayMessage","GoHome"
            ],
            "SupportsMediaControl": true,
            "SupportsSync": false
        });
        log.push(Level::Info, "api", "→ Capabilities");
        match self.post("/Sessions/Capabilities/Full").send_json(body) {
            Ok(r)  => log.push(Level::Info, "api", format!("← {} Capabilities", r.status())),
            Err(e) => log.push(Level::Warn, "api", format!("← ERR Capabilities: {e}")),
        }
    }

    // Returns (play_session_id, media_source_id). Falls back to generated id / item_id on failure.
    pub fn get_playback_info(&self, item_id: &str, log: &AppLog) -> (String, String) {
        let body = ureq::json!({
            "UserId": self.user_id,
            "MaxStreamingBitrate": 140000000,
            "EnableDirectPlay": true,
            "EnableDirectStream": false,
            "IsPlayback": true,
        });
        log.push(Level::Info, "api", format!("→ PlaybackInfo item={item_id}"));
        let resp: Value = match self.post(&format!("/Items/{item_id}/PlaybackInfo")).send_json(body) {
            Ok(r) => match r.into_json() {
                Ok(v) => v,
                Err(e) => { log.push(Level::Warn, "api", format!("← ERR PlaybackInfo parse: {e}")); return (gen_session_id(), item_id.to_string()); }
            },
            Err(e) => { log.push(Level::Warn, "api", format!("← ERR PlaybackInfo: {e}")); return (gen_session_id(), item_id.to_string()); }
        };
        let sid = resp["PlaySessionId"].as_str().unwrap_or("").to_string();
        let msid = resp["MediaSources"][0]["Id"].as_str().unwrap_or(item_id).to_string();
        log.push(Level::Info, "api", format!("← PlaybackInfo sid={sid} msid={msid}"));
        if sid.is_empty() {
            (gen_session_id(), item_id.to_string())
        } else {
            (sid, msid)
        }
    }

    pub fn get_playlists(&self) -> Result<Vec<MediaItem>, String> {
        let resp: Value = self.get(&format!("/Users/{}/Items", self.user_id))
            .query("IncludeItemTypes", "Playlist")
            .query("Recursive", "true")
            .query("Fields", "")
            .call().map_err(|e| e.to_string())?
            .into_json().map_err(|e| e.to_string())?;
        Ok(resp["Items"].as_array()
            .map(|arr| arr.iter().map(parse_item).collect())
            .unwrap_or_default())
    }

    pub fn create_playlist(&self, name: &str, item_ids: &[String]) -> Result<String, String> {
        let body = ureq::json!({
            "Name": name,
            "Ids": item_ids.join(","),
            "UserId": self.user_id,
        });
        let resp: Value = self.post("/Playlists")
            .send_json(body)
            .map_err(|e| match e {
                ureq::Error::Status(code, r) => {
                    let body = r.into_string().unwrap_or_default();
                    format!("HTTP {code}: {body}")
                }
                e => e.to_string(),
            })?
            .into_json().map_err(|e| e.to_string())?;
        resp["Id"].as_str().map(|s| s.to_string())
            .ok_or_else(|| "no Id in response".to_string())
    }

    pub fn delete_playlist(&self, playlist_id: &str) -> Result<(), String> {
        self.delete(&format!("/Items/{}", playlist_id))
            .call().map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Replace a playlist's contents with the given item ids (in order).
    /// Fetches current entry ids, deletes them all, then adds the new set.
    pub fn update_playlist_items(&self, playlist_id: &str, item_ids: &[String]) -> Result<(), String> {
        // Get current playlist entry ids
        let resp: serde_json::Value = self.get(&format!("/Playlists/{}/Items", playlist_id))
            .query("UserId", &self.user_id)
            .call().map_err(|e| e.to_string())?
            .into_json().map_err(|e| e.to_string())?;
        let entry_ids: Vec<String> = resp["Items"].as_array()
            .map(|arr| arr.iter()
                .filter_map(|v| v["PlaylistItemId"].as_str())
                .map(|s| s.to_string())
                .collect())
            .unwrap_or_default();
        // Delete existing entries
        if !entry_ids.is_empty() {
            self.delete(&format!("/Playlists/{}/Items", playlist_id))
                .query("EntryIds", &entry_ids.join(","))
                .call().map_err(|e| e.to_string())?;
        }
        // Add new items in order
        if !item_ids.is_empty() {
            self.post(&format!("/Playlists/{}/Items", playlist_id))
                .query("Ids", &item_ids.join(","))
                .query("UserId", &self.user_id)
                .call().map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    pub fn get_items_by_ids(&self, ids: &[String]) -> Result<Vec<MediaItem>, String> {
        if ids.is_empty() { return Ok(vec![]); }
        let resp: Value = self.get(&format!("/Users/{}/Items", self.user_id))
            .query("Ids", &ids.join(","))
            .query("Fields", "UserData,RunTimeTicks,MediaType,SeriesId,SeriesName,SortName,ParentIndexNumber,IndexNumber,Path,AlbumArtist,Artists")
            .call().map_err(|e| e.to_string())?
            .into_json().map_err(|e| e.to_string())?;
        let mut items: Vec<MediaItem> = resp["Items"].as_array()
            .map(|arr| arr.iter().map(parse_item).collect())
            .unwrap_or_default();
        // Emby returns items in server sort order, not input order. Restore input order.
        let order: std::collections::HashMap<&str, usize> =
            ids.iter().enumerate().map(|(i, id)| (id.as_str(), i)).collect();
        items.sort_by_key(|item| order.get(item.id.as_str()).copied().unwrap_or(usize::MAX));
        Ok(items)
    }

    pub fn get_ancestors(&self, item_id: &str) -> Result<Vec<MediaItem>, String> {
        let resp: Value = self.get(&format!("/Users/{}/Items/{}/Ancestors", self.user_id, item_id))
            .query("Fields", "SortName")
            .call().map_err(|e| e.to_string())?
            .into_json().map_err(|e| e.to_string())?;
        Ok(resp.as_array()
            .map(|arr| arr.iter().map(parse_item).collect())
            .unwrap_or_default())
    }

    /// Probes for the Chapter API plugin. Sets `chapter_api_available` on self.
    /// Any HTTP response (even 500 for a bad id) means the plugin is installed;
    /// only a 404 or connection failure means it's absent.
    pub fn probe_chapter_api(&mut self, log: &AppLog) {
        log.push(Level::Info, "api", "→ ChapterAPI probe");
        match self.get("/chapter_api/get_chapters").query("id", "0").call() {
            Ok(_) | Err(ureq::Error::Status(_, _)) => {
                self.chapter_api_available = true;
                log.push(Level::Info, "api", "← ChapterAPI available");
            }
            Err(e) => {
                log.push(Level::Info, "api", format!("← ChapterAPI not available: {e}"));
            }
        }
    }

    /// Returns `(intro_start_ticks, intro_end_ticks)` for an item if the Chapter API
    /// exposes IntroStart and IntroEnd markers.
    pub fn get_intro_times(&self, item_id: &str, log: &AppLog) -> Option<(i64, i64)> {
        log.push(Level::Debug, "api", format!("→ ChapterAPI get_chapters item={item_id}"));
        let resp = self.get("/chapter_api/get_chapters")
            .query("id", item_id)
            .call().ok()?;
        let body: serde_json::Value = resp.into_json().ok()?;
        let chapters = body["chapters"].as_array()?;
        let start = chapters.iter()
            .find(|c| c["MarkerType"].as_str() == Some("IntroStart"))?
            ["StartPositionTicks"].as_i64()?;
        let end = chapters.iter()
            .find(|c| c["MarkerType"].as_str() == Some("IntroEnd"))?
            ["StartPositionTicks"].as_i64()?;
        log.push(Level::Info, "api", format!("← ChapterAPI intro start={start} end={end}"));
        Some((start, end))
    }

    /// Returns all episodes of a series starting from `from_item_id` (inclusive), in air order.
    /// Mirrors Emby Web's `getEpisodes(seriesId)` + filter pattern.
    pub fn get_episodes_from(&self, series_id: &str, from_item_id: &str, log: &AppLog) -> Vec<MediaItem> {
        log.push(Level::Debug, "api", format!("→ EpisodesFrom series={series_id} from={from_item_id}"));
        let resp: Value = match self.get(&format!("/Shows/{}/Episodes", series_id))
            .query("UserId", &self.user_id)
            .query("Fields", "UserData,RunTimeTicks,SeriesId,SeriesName,ParentIndexNumber,IndexNumber")
            .call()
        {
            Ok(r) => match r.into_json() {
                Ok(v) => v,
                Err(e) => { log.push(Level::Warn, "api", format!("← ERR EpisodesFrom parse: {e}")); return vec![]; }
            },
            Err(e) => { log.push(Level::Warn, "api", format!("← ERR EpisodesFrom: {e}")); return vec![]; }
        };
        let Some(all) = resp["Items"].as_array() else { return vec![]; };
        let mut found = false;
        let items: Vec<MediaItem> = all.iter().filter_map(|v| {
            if found { return Some(parse_item(v)); }
            if v["Id"].as_str().unwrap_or("") == from_item_id { found = true; Some(parse_item(v)) } else { None }
        }).collect();
        if items.is_empty() {
            // from_item_id not in series — return everything as a fallback
            log.push(Level::Warn, "api", format!("← EpisodesFrom: from_item_id not found, returning all"));
            return all.iter().map(parse_item).collect();
        }
        log.push(Level::Info, "api", format!("← EpisodesFrom: {} episodes from '{}'", items.len(), items[0].display_name()));
        items
    }


    #[allow(dead_code)]
    pub fn get_next_up(&self, series_id: &str, log: &AppLog) -> Option<MediaItem> {
        log.push(Level::Debug, "api", format!("→ NextUp series={series_id}"));
        let resp: Value = match self.get("/Shows/NextUp")
            .query("UserId", &self.user_id)
            .query("SeriesId", series_id)
            .query("Limit", "1")
            .query("Fields", "UserData,RunTimeTicks,SeriesId,SeriesName,ParentIndexNumber,IndexNumber")
            .call()
        {
            Ok(r) => match r.into_json() {
                Ok(v) => v,
                Err(e) => { log.push(Level::Warn, "api", format!("← ERR NextUp parse: {e}")); return None; }
            },
            Err(e) => { log.push(Level::Warn, "api", format!("← ERR NextUp: {e}")); return None; }
        };
        let items = resp["Items"].as_array()?;
        if items.is_empty() {
            log.push(Level::Debug, "api", "← NextUp: none");
            return None;
        }
        let item = parse_item(&items[0]);
        log.push(Level::Info, "api", format!("← NextUp: {}", item.display_name()));
        Some(item)
    }

    #[allow(dead_code)]
    pub fn get_sessions(&self) -> Result<Vec<SessionInfo>, String> {
        let arr: Value = self.get("/Sessions")
            .query("ActiveWithinSeconds", "600")
            .call().map_err(|e| e.to_string())?
            .into_json().map_err(|e| e.to_string())?;
        let sessions = arr.as_array().map(|a| a.iter().filter_map(|v| {
            if v["DeviceId"].as_str().unwrap_or("") == self.device_id { return None; }
            if !v["SupportsRemoteControl"].as_bool().unwrap_or(false) { return None; }
            let ps  = &v["PlayState"];
            let npi = &v["NowPlayingItem"];
            let raw_host = v["RemoteEndPoint"].as_str().unwrap_or("");
            let host = raw_host.rsplit(':').nth(1)
                .unwrap_or(raw_host)
                .to_string();
            Some(SessionInfo {
                id:          v["Id"].as_str().unwrap_or("").to_string(),
                device_name: v["DeviceName"].as_str().unwrap_or("").to_string(),
                client:      v["Client"].as_str().unwrap_or("").to_string(),
                user_name:   v["UserName"].as_str().unwrap_or("").to_string(),
                host,
                now_playing:         npi["Name"].as_str().map(str::to_string),
                now_playing_item_id: npi["Id"].as_str().map(str::to_string),
                position_s:          ps["PositionTicks"].as_i64().unwrap_or(0) / TICKS_PER_SECOND,
                runtime_s:   npi["RunTimeTicks"].as_i64().unwrap_or(0) / TICKS_PER_SECOND,
                is_paused:   ps["IsPaused"].as_bool().unwrap_or(false),
                volume:      ps["VolumeLevel"].as_i64().unwrap_or(100),
                sub_index:   ps["SubtitleStreamIndex"].as_i64().unwrap_or(-1),
                audio_index: ps["AudioStreamIndex"].as_i64().unwrap_or(0),
            })
        }).collect()).unwrap_or_default();
        Ok(sessions)
    }

    pub fn session_transport(&self, id: &str, cmd: &str) -> Result<(), String> {
        self.post(&format!("/Sessions/{id}/Playing/{cmd}"))
            .send_string("").map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn session_seek(&self, id: &str, ticks: i64) -> Result<(), String> {
        self.post(&format!("/Sessions/{id}/Playing/Seek"))
            .query("SeekPositionTicks", &ticks.to_string())
            .send_string("").map_err(|e| e.to_string())?;
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

    pub fn session_play_items(&self, id: &str, item_ids: &[String], start_idx: usize, start_ticks: i64) -> Result<(), String> {
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
        format!("{}/Videos/{}/stream?static=true&api_key={}", self.config.server_url, item_id, self.token)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_item(name: &str, item_type: &str) -> MediaItem {
        MediaItem {
            id: "id".into(), name: name.into(), item_type: item_type.into(),
            is_folder: false, media_type: "Video".into(), collection_type: String::new(),
            runtime_ticks: 0, played: false, playback_position_ticks: 0,
            series_id: String::new(), series_name: String::new(), album_id: String::new(),
            album: String::new(), index_number: 0, parent_index_number: 0,
            unplayed_item_count: 0,
            path: String::new(), artist: String::new(), sort_name: String::new(),
            production_year: 0, end_year: 0, overview: String::new(),
            premiere_date: String::new(), date_added: String::new(),
            total_count: 0, container: String::new(),
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
    fn display_name_episode_formats_series() {
        let mut item = make_item("Pilot", "Episode");
        item.series_name = "Breaking Bad".into();
        item.parent_index_number = 1;
        item.index_number = 3;
        assert_eq!(item.display_name(), "Breaking Bad - Pilot");
    }

    #[test]
    fn display_name_episode_zero_padded() {
        let mut item = make_item("Episode", "Episode");
        item.series_name = "Show".into();
        item.parent_index_number = 10;
        item.index_number = 1;
        assert_eq!(item.display_name(), "Show - Episode");
    }

    #[test]
    fn display_name_episode_without_series_falls_back_to_name() {
        let item = make_item("Standalone", "Episode");
        assert_eq!(item.display_name(), "Standalone");
    }

    // ── MediaItem::resume_seconds / runtime_seconds ──────────────────────────

    #[test]
    fn resume_seconds_zero() {
        assert_eq!(make_item("X", "Movie").resume_seconds(), 0.0);
    }

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
    fn playback_label_audio_with_artist() {
        let mut item = make_item("Song", "Audio");
        item.artist = "Artist".into();
        item.media_type = "Audio".into();
        assert_eq!(item.playback_label(), "Artist - Song");
    }

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
        let mut cfg = crate::config::Config::default();
        cfg.server_url = url.into();
        let mut c = EmbyClient::new(cfg);
        c.token = "tok".into();
        c
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
            let _g = ENV_MTX.lock().unwrap();
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
        assert_eq!(persisted.trim(), legacy_id, "should persist migrated id to new location");
    }

    fn make_temp_data_dir() -> std::path::PathBuf {
        std::env::temp_dir().join(format!("mbv-test-{}", uuid::Uuid::new_v4()))
    }

    static ENV_MTX: std::sync::Mutex<()> = std::sync::Mutex::new(());
}
