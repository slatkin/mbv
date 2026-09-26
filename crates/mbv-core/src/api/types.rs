pub use crate::config::Config;
use crate::id_types::{EmbySessionId, MediaSourceId};

pub const TICKS_PER_SECOND: i64 = 10_000_000;

/// Inclusive lower-bound percentage of known runtime at which a saved
/// position qualifies for resume. Exactly this percent qualifies.
pub const RESUME_THRESHOLD_PERCENT: i64 = 6;

/// Minimum video position, in ticks, worth recording as a `TrackCompleted`
/// occurrence's resume point — below this, the position is more likely
/// startup noise than a real watch, so the slot's prior stored position is
/// kept instead. Shared by the daemon's and the shell's own `TrackCompleted`
/// gates so the two processes cannot drift on the threshold value itself
/// (the surrounding gate logic is still duplicated by hand; see
/// `docs/invariants/06-queue-progress-application-sites.md`).
pub const MEANINGFUL_TRACK_COMPLETED_PROGRESS_TICKS: i64 = 30 * TICKS_PER_SECOND;

/// Shared resume-eligibility predicate used by both Emby items and feed
/// entries. A positive saved position with unknown runtime (`runtime_ticks
/// <= 0`) is always resumable. Zero and negative positions never qualify.
/// For a known runtime the position must be at least `RESUME_THRESHOLD_PERCENT`
/// (inclusive) of runtime. Uses `i128` multiplication to avoid overflow.
#[must_use]
pub fn should_resume(position_ticks: i64, runtime_ticks: i64) -> bool {
    if position_ticks <= 0 {
        return false;
    }
    if runtime_ticks > 0 {
        i128::from(position_ticks) * 100
            >= i128::from(runtime_ticks) * i128::from(RESUME_THRESHOLD_PERCENT)
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
        if let Some(ch) = decoded_char {
            result.push(ch);
            rest = &tail[semi_idx + 1..];
        } else {
            // Unrecognized entity: leave the leading '&' untouched and
            // keep scanning from just after it.
            result.push('&');
            rest = &tail[1..];
        }
    }
    result.push_str(rest);
    result
}

/// Convert a snippet of HTML (common in Audiobookshelf episode/podcast
/// descriptions) into plain terminal text: block tags become paragraph
/// breaks, links keep their visible text plus the URL as `text (URL)`,
/// and entities are decoded. Inline styling/formatting tags are dropped.
#[must_use]
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
                    result.extend((!result.is_empty() && !result.ends_with(' ')).then_some(' '));
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

// Task 5.3d (Emby browser effect decoupling): `PartialEq` is required so the
// TuiRealm shell `Msg`/`ShellRequest` enums (which are `#[derive(PartialEq)]`
// — the Application requires `Msg: PartialEq`) can carry the component-resolved
// item as the explicit owned target of a typed effect. Additive derive only;
// no semantics change.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EmbyPerson {
    pub name: String,
    pub role: String,
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EmbyLink {
    pub name: String,
    pub url: String,
}

/// One `ArtistItems` name/ID pair from a music album/item payload: an
/// album-artist display name and the stable artist item ID it maps to.
/// This pair is mbv's only source of stable artist identity; IDs from an
/// `/Artists` listing never enter this field.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EmbyArtistRef {
    pub name: String,
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EmbyItem {
    pub id: String,
    pub name: String,
    pub item_type: String,
    pub is_folder: bool,
    #[serde(default)]
    pub child_count: Option<u32>,
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
    /// `ArtistItems` name/ID pairs as returned on the payload, retained
    /// verbatim. `Default` so legacy serialized queue items (which predate
    /// the field) deserialize unchanged; serialized queue compatibility is
    /// unchanged.
    #[serde(default)]
    pub artist_items: Vec<EmbyArtistRef>,
    pub sort_name: String,
    pub production_year: u32,
    pub end_year: u32,
    pub overview: String,
    pub premiere_date: String,
    /// Full ISO 8601 (not truncated to a date), so destination Latest markers
    /// can compare it against the launch window at second precision.
    pub date_added: String,
    pub total_count: u32,
    pub container: String,
    pub video_info: String,
    pub audio_info: String,
    #[serde(default)]
    pub genres: Vec<String>,
    #[serde(default)]
    pub people: Vec<EmbyPerson>,
    #[serde(default)]
    pub external_urls: Vec<EmbyLink>,
    pub playlist_item_id: String,
    /// Declared image availability (task 5.3): the Emby default-DTO image
    /// tags the hero artwork policy reads to choose an artwork shape before
    /// any image is fetched. `Default` so older payloads deserialize
    /// unchanged.
    #[serde(default)]
    pub image_tags: EmbyImageTags,
}

/// Declared image availability for one item (task 5.3, design D5): the
/// provider metadata the artwork policy reads to decide availability before
/// any image is fetched, so the chosen shape never changes when the image
/// arrives. These are Emby's default item-DTO members (not `Fields`-gated
/// properties), so parsing them needs no extra `Fields` request.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EmbyImageTags {
    /// `ImageTags.Thumb`: a 16:9 landscape thumbnail is declared available.
    #[serde(default)]
    pub thumb: String,
    /// `ImageTags.Logo`: an optional semantic logo decoration is declared available.
    #[serde(default)]
    pub logo: String,
    /// `ImageTags.Primary`: a primary (poster/square) image is declared
    /// available.
    #[serde(default)]
    pub primary: String,
    /// `BackdropImageTags`: landscape backdrop tags.
    #[serde(default)]
    pub backdrops: Vec<String>,
    /// On episodes: the series' thumb tag (`ParentThumbImageTag`, falling
    /// back to `SeriesThumbImageTag`).
    #[serde(default)]
    pub series_thumb: String,
    /// On episodes: the series' backdrop tags (`ParentBackdropImageTags`).
    #[serde(default)]
    pub series_backdrops: Vec<String>,
}

impl EmbyItem {
    #[must_use]
    pub fn is_audio(&self) -> bool {
        self.media_type == "Audio" || self.item_type == "Audio"
    }

    /// Whether this is a music item: an Emby track, album or artist.
    ///
    /// Music is fire-and-forget in mbv — playback never resumes one
    /// (`resume_start_pos` gates on `!is_audio`), so its stored played/resume
    /// facts carry no meaning anywhere: a row derived from a music item is
    /// always ordinary, and only the row that is actually playing shows
    /// anything.
    #[must_use]
    pub fn is_music(&self) -> bool {
        self.item_type == "MusicAlbum" || self.item_type == "MusicArtist" || self.is_audio()
    }

    /// Resolves this item's stable artist item ID from its `ArtistItems`
    /// payload pairs. A pair is applicable only when its trimmed name
    /// case-insensitively equals the settled display artist, and only a
    /// single applicable pair resolves: zero matches, or several equal-name
    /// pairs, resolve nothing rather than picking an arbitrary first pair.
    /// Returns `None` for every absent, unmatched, or ambiguous case.
    #[must_use]
    pub fn matched_artist_item_id(&self, display_artist: &str) -> Option<&str> {
        let target = display_artist.trim().to_lowercase();
        if target.is_empty() {
            return None;
        }
        let mut matched: Option<&str> = None;
        for pair in &self.artist_items {
            if pair.id.is_empty() || pair.name.trim().to_lowercase() != target {
                continue;
            }
            if matched.is_some() {
                // Ambiguous: several pairs carry the display name with
                // distinct (or repeated) IDs. No arbitrary first pick.
                return None;
            }
            matched = Some(&pair.id);
        }
        matched
    }

    #[must_use]
    pub fn is_video(&self) -> bool {
        self.media_type == "Video"
    }

    #[must_use]
    #[expect(
        clippy::cast_precision_loss,
        reason = "playback position ticks → seconds through f64; no lossless integer-path conversion exists (approved, issue #804)"
    )]
    pub fn resume_seconds(&self) -> f64 {
        self.playback_position_ticks as f64 / TICKS_PER_SECOND as f64
    }

    #[must_use]
    pub fn should_resume(&self) -> bool {
        should_resume(self.playback_position_ticks, self.runtime_ticks)
    }

    #[must_use]
    #[expect(
        clippy::cast_precision_loss,
        reason = "runtime ticks → seconds through f64; no lossless integer-path conversion exists (approved, issue #804)"
    )]
    pub fn runtime_seconds(&self) -> f64 {
        self.runtime_ticks as f64 / TICKS_PER_SECOND as f64
    }

    #[must_use]
    pub fn file_name(&self) -> &str {
        if self.path.is_empty() {
            return &self.name;
        }
        let p = std::path::Path::new(&self.path);
        p.file_name().and_then(|f| f.to_str()).unwrap_or(&self.name)
    }

    #[must_use]
    pub fn sort_key(&self) -> &str {
        if !self.path.is_empty() {
            self.file_name()
        } else if !self.sort_name.is_empty() {
            &self.sort_name
        } else {
            &self.name
        }
    }

    #[must_use]
    pub fn playback_label(&self) -> String {
        if self.item_type == "Audio" && !self.artist.is_empty() {
            format!("{} - {}", self.artist, self.name)
        } else {
            self.display_name()
        }
    }

    pub(super) fn folder(id: String, name: String, collection_type: String) -> Self {
        EmbyItem {
            id,
            name,
            item_type: "CollectionFolder".to_string(),
            is_folder: true,
            child_count: None,
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
            artist_items: Vec::new(),
            sort_name: String::new(),
            production_year: 0,
            end_year: 0,
            overview: String::new(),
            premiere_date: String::new(),
            date_added: String::new(),
            total_count: 0,
            container: String::new(),
            video_info: String::new(),
            audio_info: String::new(),
            genres: Vec::new(),
            people: Vec::new(),
            external_urls: Vec::new(),
            playlist_item_id: String::new(),
            image_tags: EmbyImageTags::default(),
        }
    }

    #[must_use]
    pub fn display_name(&self) -> String {
        if self.item_type == "Episode" && !self.series_name.is_empty() {
            format!("{} {}", self.series_name, self.name)
        } else {
            self.name.clone()
        }
    }

    /// The two-tone title parts for a list row: the series title and, for
    /// episode rows, the episode title that paints after it in the accent
    /// role. Non-episode items return the display name with no second part.
    #[must_use]
    pub fn display_name_parts(&self) -> (String, Option<String>) {
        if self.item_type == "Episode" && !self.series_name.is_empty() {
            (self.series_name.clone(), Some(self.name.clone()))
        } else {
            (self.name.clone(), None)
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
