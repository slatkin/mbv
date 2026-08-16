use serde_json::Value;

use crate::config::Config;

pub const TICKS_PER_SECOND: i64 = 10_000_000;

/// Inclusive lower-bound percentage of known runtime at which a saved
/// position qualifies for resume. Exactly this percent qualifies.
pub const RESUME_THRESHOLD_PERCENT: i64 = 6;

/// Shared resume-eligibility predicate used by both Emby items and feed
/// entries. A positive saved position with unknown runtime (`runtime_ticks
/// <= 0`) is always resumable. Zero and negative positions never qualify.
/// For a known runtime the position must be at least `RESUME_THRESHOLD_PERCENT`
/// (inclusive) of runtime. Uses `i128` multiplication to avoid overflow.
pub fn should_resume(position_ticks: i64, runtime_ticks: i64) -> bool {
    if position_ticks <= 0 {
        return false;
    }
    if runtime_ticks > 0 {
        (position_ticks as i128) * 100
            >= (runtime_ticks as i128) * (RESUME_THRESHOLD_PERCENT as i128)
    } else {
        true
    }
}

/// Decode common XML/HTML entities (`&amp;`, `&lt;`, `&gt;`, `&quot;`, `&apos;`)
/// and numeric character references (`&#NNN;` / `&#xHHHH;`) in a single
/// left-to-right scan. Anything unrecognized (e.g. a stray `&` or an
/// unsupported named entity) is left untouched rather than erroring.
pub fn decode_entities(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(amp_idx) = rest.find('&') {
        result.push_str(&rest[..amp_idx]);
        let tail = &rest[amp_idx..];
        let Some(semi_idx) = tail.find(';') else {
            result.push('&');
            rest = &tail[1..];
            continue;
        };
        let entity = &tail[1..semi_idx];
        let decoded_char = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ if entity.starts_with('#') => {
                let num_part = &entity[1..];
                let code_point = if let Some(hex) = num_part
                    .strip_prefix('x')
                    .or_else(|| num_part.strip_prefix('X'))
                {
                    u32::from_str_radix(hex, 16).ok()
                } else {
                    num_part.parse::<u32>().ok()
                };
                code_point.and_then(char::from_u32)
            }
            _ => None,
        };
        match decoded_char {
            Some(ch) => {
                result.push(ch);
                rest = &tail[semi_idx + 1..];
            }
            None => {
                // Unrecognized entity: leave the leading '&' untouched and
                // keep scanning from just after it.
                result.push('&');
                rest = &tail[1..];
            }
        }
    }
    result.push_str(rest);
    result
}

/// Convert a snippet of HTML (common in Audiobookshelf episode/podcast
/// descriptions) into plain terminal text: block tags become paragraph
/// breaks, links keep their visible text plus the URL as `text (URL)`,
/// and entities are decoded. Inline styling/formatting tags are dropped.
pub fn html_to_text(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let mut rest = html;

    // Href of the open <a> tag; text between `<a ...>` and `</a>` is kept,
    // then the href follows in parentheses on closing.
    let mut pending_link: Option<String> = None;

    while let Some(lt) = rest.find('<') {
        result.push_str(&rest[..lt]);
        let after = &rest[lt..];
        let Some(gt) = after.find('>') else {
            result.push_str(after);
            break;
        };
        let tag = &after[1..gt];
        let lower = tag.trim().to_ascii_lowercase();

        if let Some(name) = lower.strip_prefix('/') {
            if is_block_tag(name.trim()) {
                result.push('\n');
            } else if name.trim() == "a" {
                if let Some(href) = pending_link.take() {
                    if !result.is_empty() && !result.ends_with(' ') {
                        result.push(' ');
                    }
                    result.push('(');
                    result.push_str(&href);
                    result.push(')');
                }
            }
        } else {
            let name = lower
                .trim_end_matches('/')
                .split_whitespace()
                .next()
                .unwrap_or("");
            if name == "a" {
                pending_link = extract_href(&lower);
            } else if is_block_tag(name) {
                result.push('\n');
            }
        }
        rest = &after[gt + 1..];
    }
    result.push_str(rest);

    result = decode_entities(&result);
    result = trim_blank_lines(&result);
    result.trim().to_string()
}

fn is_block_tag(name: &str) -> bool {
    matches!(name, "p" | "div" | "li" | "ul" | "ol" | "br")
}

/// Collapse runs of blank lines (and trailing spaces) down to single
/// newlines, trimming each line.
fn trim_blank_lines(text: &str) -> String {
    text.split('\n')
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Extract the `href="..."` value from an `<a ...>` tag body.
fn extract_href(tag_body: &str) -> Option<String> {
    let key = "href=\"";
    let start = tag_body.find(key)? + key.len();
    let end = tag_body[start..].find('"')?;
    Some(decode_entities(&tag_body[start..start + end]))
}

pub fn gen_session_id() -> EmbySessionId {
    EmbySessionId::new(uuid::Uuid::new_v4().simple().to_string())
}

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
pub struct EmbyItem {
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

impl EmbyItem {
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
        should_resume(self.playback_position_ticks, self.runtime_ticks)
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
        EmbyItem {
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
            format!("{} {}", self.series_name, self.name)
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

/// Result of a PlaybackInfo lookup for an item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaybackInfo {
    pub session_id: EmbySessionId,
    pub media_source_id: MediaSourceId,
    pub external_subtitle_urls: Vec<String>,
}

pub const MBV_DIRECT_TCP_PORT_PREFIX: &str = "mbv-direct-tcp-port:";
pub const MBV_SHARED_DATA_TCP_PORT_PREFIX: &str = "mbv-shared-data-tcp-port:";

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

pub fn mbv_shared_data_tcp_port_command(port: u16) -> String {
    format!("{MBV_SHARED_DATA_TCP_PORT_PREFIX}{port}")
}

pub fn parse_mbv_shared_data_tcp_port(commands: &[String]) -> Option<u16> {
    commands.iter().find_map(|cmd| {
        cmd.strip_prefix(MBV_SHARED_DATA_TCP_PORT_PREFIX)
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

fn parse_item(raw: &Value) -> EmbyItem {
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
    EmbyItem {
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
        overview: decode_entities(raw["Overview"].as_str().unwrap_or("")),
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

#[cfg(test)]
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
    agent: ureq::Agent,
}
