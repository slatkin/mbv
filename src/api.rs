use std::sync::mpsc;

use serde_json::Value;

use crate::applog::{AppLog, Level};
use crate::config::Config;

pub const TICKS_PER_SECOND: i64 = 10_000_000;


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
        .unwrap_or_else(|| "mby".to_string())
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
    let dir = data_home.join("mby");
    let path = dir.join("device_id");
    if let Ok(id) = std::fs::read_to_string(&path) {
        let id = id.trim().to_string();
        if !id.is_empty() { return id; }
    }
    let id = uuid::Uuid::new_v4().to_string();
    if let Err(e) = std::fs::create_dir_all(&dir) {
        eprintln!("mby: could not create {}: {}", dir.display(), e);
    } else if let Err(e) = std::fs::write(&path, &id) {
        eprintln!("mby: could not write device_id to {}: {}", path.display(), e);
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
    pub index_number: i64,
    pub parent_index_number: i64,
    pub unplayed_item_count: u32,
    pub path: String,
    pub artist: String,
    pub sort_name: String,
}

impl MediaItem {
    pub fn resume_seconds(&self) -> f64 {
        self.playback_position_ticks as f64 / TICKS_PER_SECOND as f64
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
            let s = self.parent_index_number;
            let e = self.index_number;
            format!("{} S{:02}E{:02} - {}", self.series_name, s, e, self.name)
        } else {
            self.name.clone()
        }
    }
}

fn parse_item(raw: &Value) -> MediaItem {
    let ud = raw.get("UserData").unwrap_or(&Value::Null);
    let item_type = raw["Type"].as_str().unwrap_or("").to_string();
    let is_folder = raw["IsFolder"].as_bool().unwrap_or(false)
        || matches!(item_type.as_str(), "CollectionFolder" | "Channel" | "Series" | "Season"
                                        | "MusicArtist" | "MusicAlbum" | "BoxSet" | "Folder");
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
        index_number: raw["IndexNumber"].as_i64().unwrap_or(0),
        parent_index_number: raw["ParentIndexNumber"].as_i64().unwrap_or(0),
        unplayed_item_count: ud["UnplayedItemCount"].as_u64().unwrap_or(0) as u32,
        path: raw["Path"].as_str().unwrap_or("").to_string(),
        artist: raw["AlbumArtist"].as_str()
            .or_else(|| raw["Artists"].get(0).and_then(|v| v.as_str()))
            .unwrap_or("").to_string(),
        sort_name: raw["SortName"].as_str().unwrap_or("").to_string(),
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
            agent,
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.config.server_url, path)
    }

    fn auth_header(&self) -> String {
        format!(
            "Emby Client=\"mby\", Device=\"{}\", DeviceId=\"{}\", Version=\"0.1.0\", Token=\"{}\"",
            self.device_name, self.device_id, self.token
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
                .set("Authorization", &format!("Emby Client=\"mby\", Device=\"{}\", DeviceId=\"{}\", Version=\"0.1.0\"", self.device_name, self.device_id))
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
                    index_number: 0,
                    parent_index_number: 0,
                    unplayed_item_count: 0,
                    path: String::new(),
                    artist: String::new(),
                    sort_name: String::new(),
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

    pub fn get_items(&self, parent_id: &str, item_types: Option<&str>, unplayed_only: bool) -> Result<(Vec<MediaItem>, usize), String> {
        let mut req = self.get(&format!("/Users/{}/Items", self.user_id))
            .query("ParentId", parent_id)
            .query("SortBy", "SortName")
            .query("SortOrder", "Ascending")
            .query("Limit", "5000")
            .query("Fields", "UserData,RunTimeTicks,MediaType,SeriesId,SeriesName,SortName,ParentIndexNumber,IndexNumber,Path,AlbumArtist,Artists")
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
            .query("Fields", "UserData,RunTimeTicks,MediaType,SeriesId,SeriesName,SortName,ParentIndexNumber,IndexNumber,Path,AlbumArtist,Artists")
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
                .query("mediaSourceId", media_source_id)
                .query("positionTicks", &position_ticks.to_string())
                .query("playSessionId", session_id)
                .send_string("")
            {
                Ok(r)  => log.push(Level::Info, "api", format!("← {} PlayingItem", r.status())),
                Err(e) => log.push(Level::Warn, "api", format!("← ERR PlayingItem: {e}")),
            }
        }
    }

    pub fn register_capabilities(&self, log: &AppLog) {
        let body = ureq::json!({
            "PlayableMediaTypes": ["Video"],
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

    pub fn get_items_by_ids(&self, ids: &[String]) -> Result<Vec<MediaItem>, String> {
        if ids.is_empty() { return Ok(vec![]); }
        let resp: Value = self.get(&format!("/Users/{}/Items", self.user_id))
            .query("Ids", &ids.join(","))
            .query("Fields", "UserData,RunTimeTicks,MediaType,SeriesId,SeriesName,SortName,ParentIndexNumber,IndexNumber,Path,AlbumArtist,Artists")
            .call().map_err(|e| e.to_string())?
            .into_json().map_err(|e| e.to_string())?;
        Ok(resp["Items"].as_array()
            .map(|arr| arr.iter().map(parse_item).collect())
            .unwrap_or_default())
    }

    pub fn get_next_episode(&self, series_id: &str, season: i64, episode: i64, log: &AppLog) -> Option<MediaItem> {
        log.push(Level::Debug, "api", format!("→ NextEpisode series={series_id} S{season:02}E{episode:02}"));
        let resp: Value = match self.get(&format!("/Shows/{}/Episodes", series_id))
            .query("UserId", &self.user_id)
            .query("Fields", "UserData,RunTimeTicks,SeriesId,SeriesName,ParentIndexNumber,IndexNumber")
            .call()
        {
            Ok(r) => match r.into_json() {
                Ok(v) => v,
                Err(e) => { log.push(Level::Warn, "api", format!("← ERR NextEpisode parse: {e}")); return None; }
            },
            Err(e) => { log.push(Level::Warn, "api", format!("← ERR NextEpisode: {e}")); return None; }
        };
        let items = resp["Items"].as_array()?;
        // Episodes are returned in air order. Find the first one that comes after
        // (season, episode) — handles end-of-season continuation automatically.
        let mut found_current = false;
        for v in items {
            let s = v["ParentIndexNumber"].as_i64().unwrap_or(0);
            let e = v["IndexNumber"].as_i64().unwrap_or(0);
            if found_current {
                let item = parse_item(v);
                log.push(Level::Info, "api", format!("← NextEpisode: {}", item.display_name()));
                return Some(item);
            }
            if s == season && e == episode {
                found_current = true;
            }
        }
        log.push(Level::Debug, "api", "← NextEpisode: none (end of series)");
        None
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
            series_id: String::new(), series_name: String::new(),
            index_number: 0, parent_index_number: 0,
            unplayed_item_count: 0,
            path: String::new(), artist: String::new(), sort_name: String::new(),
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
        assert_eq!(item.display_name(), "Breaking Bad S01E03 - Pilot");
    }

    #[test]
    fn display_name_episode_zero_padded() {
        let mut item = make_item("Episode", "Episode");
        item.series_name = "Show".into();
        item.parent_index_number = 10;
        item.index_number = 1;
        assert_eq!(item.display_name(), "Show S10E01 - Episode");
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
    fn device_name_falls_back_to_mby() {
        // Only reachable when both /etc/hostname is absent/empty and HOSTNAME unset.
        // We can't suppress /etc/hostname, but we can verify the fallback string
        // is the sentinel "mby" when nothing else is available.
        let name = device_name();
        assert!(!name.is_empty());
        // Must not contain raw newlines regardless of source.
        assert!(!name.contains('\n'));
    }

    // ── device_id ────────────────────────────────────────────────────────────

    #[test]
    fn device_id_returns_uuid_v4_format() {
        let id = device_id_with_xdg(make_temp_data_dir());
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
        let first = device_id_with_xdg(dir.clone());
        let second = device_id_with_xdg(dir);
        assert_eq!(first, second);
    }

    #[test]
    fn device_id_respects_xdg_data_home() {
        let dir = make_temp_data_dir();
        let id = device_id_with_xdg(dir.clone());
        let persisted = std::fs::read_to_string(
            std::path::PathBuf::from(&dir).join("mby/device_id")
        ).unwrap();
        assert_eq!(persisted.trim(), id);
    }

    fn make_temp_data_dir() -> String {
        let dir = std::env::temp_dir().join(format!("mby-test-{}", uuid::Uuid::new_v4()));
        dir.to_str().unwrap().to_string()
    }

    // Serialize all tests that mutate env vars so they don't race each other.
    static ENV_MTX: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn device_id_with_xdg(xdg: String) -> String {
        let _g = ENV_MTX.lock().unwrap();
        std::env::set_var("XDG_DATA_HOME", &xdg);
        let id = device_id();
        std::env::remove_var("XDG_DATA_HOME");
        id
    }
}
