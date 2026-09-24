use super::{AudiobookshelfClient, AudiobookshelfError};
use crate::playback_queue::AudiobookshelfQueueItem;
use serde::Deserialize;
use std::collections::HashMap;
use std::io::Read;
use std::time::Duration;

#[path = "catalog_books.rs"]
mod catalog_books;
pub use catalog_books::*;
// Re-export test helpers for sibling test modules
pub(super) use catalog_books::{book_author_display, first_listed_author_sort_key, AuthorWire, BooksResponse};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudiobookshelfLibrary {
    pub id: String,
    pub name: String,
    pub media_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudiobookshelfShow {
    pub library_item_id: String,
    pub title: String,
    pub author: Option<String>,
    pub description: Option<String>,
    pub cover_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AudiobookshelfDownloadedEpisode {
    pub library_item_id: String,
    pub episode_id: String,
    pub title: String,
    /// Episode description, converted to terminal text at the wire boundary
    /// (feeds the episode hero's overview).
    pub description: Option<String>,
    /// Publish instant as unix SECONDS, normalised once at the wire boundary
    /// (`published_at_secs`). `None` groups as `Unknown date`.
    pub published_at: Option<u64>,
    pub duration_seconds: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AudiobookshelfProgress {
    pub library_item_id: String,
    pub episode_id: String,
    pub current_time_seconds: f64,
    pub is_finished: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AudiobookshelfShelfEntry {
    Show(String),
    /// A fully-populated episode entry: the fields needed to build an
    /// `AudiobookshelfQueueItem` (for Home's per-library Latest pill) without
    /// a follow-up fetch. Entries whose `media` payload is absent or partial
    /// map to defaulted fields.
    Episode(AudiobookshelfQueueItem),
}

#[derive(Debug, Clone, PartialEq)]
pub struct AudiobookshelfShelf {
    pub label: String,
    pub entries: Vec<AudiobookshelfShelfEntry>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AudiobookshelfShowPage {
    pub page: usize,
    pub limit: usize,
    pub total: usize,
    pub items: Vec<AudiobookshelfShow>,
}

#[derive(Debug, Deserialize)]
pub(super) struct LibrariesResponse {
    pub(super) libraries: Vec<LibraryWire>,
}
#[derive(Debug, Deserialize)]
pub(super) struct LibraryWire {
    pub(super) id: String,
    pub(super) name: String,
    #[serde(rename = "mediaType")]
    pub(super) media_type: String,
}
#[derive(Debug, Deserialize)]
pub(super) struct ItemsResponse {
    pub(super) page: usize,
    pub(super) limit: usize,
    pub(super) total: usize,
    #[serde(alias = "items")]
    pub(super) results: Vec<ShowWire>,
}
#[derive(Debug, Deserialize)]
pub(super) struct ShowWire {
    #[serde(rename = "id", alias = "libraryItemId")]
    pub(super) library_item_id: String,
    #[serde(default)]
    pub(super) title: Option<String>,
    #[serde(default)]
    pub(super) author: Option<String>,
    #[serde(rename = "coverPath", default)]
    pub(super) cover_path: Option<String>,
    #[serde(default)]
    pub(super) media: Option<PodcastMediaWire>,
}
#[derive(Debug, Deserialize)]
pub(super) struct PodcastMediaWire {
    #[serde(rename = "coverPath", default)]
    pub(super) cover_path: Option<String>,
    pub(super) metadata: Option<PodcastMetadataWire>,
}
#[derive(Debug, Deserialize)]
pub(super) struct PodcastMetadataWire {
    pub(super) title: Option<String>,
    pub(super) author: Option<String>,
    pub(super) description: Option<String>,
}
#[derive(Debug, Deserialize)]
pub(super) struct ExpandedWire {
    pub(super) id: String,
    pub(super) media: Option<MediaWire>,
}
#[derive(Debug, Deserialize)]
pub(super) struct MediaWire {
    pub(super) episodes: Option<Vec<EpisodeWire>>,
}
#[derive(Debug, Deserialize)]
pub(super) struct EpisodeWire {
    pub(super) id: String,
    pub(super) title: String,
    #[serde(default)]
    pub(super) description: Option<String>,
    #[serde(rename = "publishedAt")]
    pub(super) published_at: Option<serde_json::Value>,
    pub(super) duration: Option<f64>,
}
#[derive(Debug, Deserialize)]
pub(super) struct ProgressResponse {
    #[serde(rename = "mediaProgress")]
    pub(super) media_progress: Vec<ProgressWire>,
}
#[derive(Debug, Deserialize)]
pub(super) struct ProgressWire {
    #[serde(rename = "libraryItemId")]
    pub(super) library_item_id: String,
    #[serde(rename = "episodeId")]
    pub(super) episode_id: Option<String>,
    #[serde(rename = "currentTime")]
    pub(super) current_time: Option<f64>,
    #[serde(rename = "isFinished")]
    pub(super) is_finished: Option<bool>,
}
/// One personalized shelf as ABS returns it (`GET /api/libraries/{id}/personalized`).
/// Shelf entries are full minified library items under `entities` — not bare IDs
/// — each carrying an embedded `media` (the podcast) and a top-level `recentEpisode`
/// (the episode to surface), verified against ABS 2.36.0.
#[derive(Debug, Clone, Deserialize)]
pub(super) struct ShelfWire {
    pub(super) label: String,
    #[serde(rename = "entities")]
    pub(super) entities: Vec<ShelfEntryWire>,
}
#[derive(Debug, Clone, Deserialize)]
pub(super) struct ShelfEntryWire {
    /// The library item id — the podcast/show id.
    pub(super) id: String,
    #[serde(default)]
    pub(super) media: Option<ShelfMediaWire>,
    #[serde(rename = "recentEpisode", default)]
    pub(super) recent_episode: Option<RecentEpisodeWire>,
}
#[derive(Debug, Clone, Deserialize)]
struct ShelfMediaWire {
    #[serde(default)]
    metadata: Option<ShelfEntryMetadataWire>,
    #[serde(rename = "coverPath", default)]
    cover_path: Option<String>,
}
#[derive(Debug, Clone, Deserialize)]
struct ShelfEntryMetadataWire {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    author: Option<String>,
}
#[derive(Debug, Clone, Deserialize)]
struct RecentEpisodeWire {
    /// The episode id, needed to play the episode.
    id: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(rename = "publishedAt", default)]
    published_at: Option<serde_json::Value>,
    #[serde(rename = "audioFile", default)]
    audio_file: Option<AudioFileDurationWire>,
}
#[derive(Debug, Clone, Deserialize)]
struct AudioFileDurationWire {
    #[serde(default)]
    duration: Option<f64>,
}

/// Maps a real ABS shelf library item to the public shape. Episode entries keep
/// the embedded podcast metadata and latest-episode fields so callers (Home's
/// per-library Latest pill) can build a `QueueItem` without a follow-up fetch.
/// An entry without a `recentEpisode` (a podcast with no published episode yet)
/// maps to a bare `Show` id rather than a playable episode.
pub(super) fn shelf_entry_from_wire(entry: ShelfEntryWire) -> AudiobookshelfShelfEntry {
    let Some(recent_episode) = entry.recent_episode else {
        return AudiobookshelfShelfEntry::Show(entry.id);
    };
    let metadata = entry.media.as_ref().and_then(|m| m.metadata.as_ref());
    let cover_path = entry
        .media
        .as_ref()
        .and_then(|m| m.cover_path.clone())
        .filter(|c| !c.is_empty());
    let duration_seconds = recent_episode.audio_file.as_ref().and_then(|a| a.duration);
    AudiobookshelfShelfEntry::Episode(AudiobookshelfQueueItem {
        library_item_id: entry.id,
        episode_id: recent_episode.id,
        title: recent_episode
            .title
            .clone()
            .or_else(|| metadata.and_then(|m| m.title.clone()))
            .unwrap_or_default(),
        show_title: metadata.and_then(|m| m.title.clone()),
        author: metadata.and_then(|m| m.author.clone()),
        description: recent_episode
            .description
            .as_deref()
            .map(crate::api::html_to_text),
        duration_ticks: duration_seconds
            .map(|seconds| (seconds * crate::api::TICKS_PER_SECOND as f64) as u64),
        position_ticks: 0,
        played: false,
        pub_date_secs: published_at_secs(recent_episode.published_at),
        is_finished: false,
        cover_path,
    })
}

/// Normalises an Audiobookshelf `publishedAt` wire value to unix seconds.
/// The ambiguity is resolved exactly once, here: Audiobookshelf 2.36 sends a
/// downloaded episode's `publishedAt` as an epoch-millisecond number
/// (`PodcastEpisode.toOldJSONExpanded`), while other wire shapes carry epoch
/// seconds as a number or numeric string, or ISO-8601 / RFC 2822 text. A
/// missing, unreadable, or zero (ABS's absent-date sentinel) value is `None`
/// (it groups as `Unknown date` rather than 1970-01-01).
pub(super) fn published_at_secs(value: Option<serde_json::Value>) -> Option<u64> {
    match value? {
        serde_json::Value::Number(number) => {
            let raw = number.as_u64().or_else(|| {
                let seconds = number.as_f64()?;
                (seconds >= 0.0 && seconds.fract() == 0.0).then_some(seconds as u64)
            })?;
            epoch_value_to_secs(raw)
        }
        serde_json::Value::String(text) => match text.trim().parse::<u64>() {
            Ok(raw) => epoch_value_to_secs(raw),
            Err(_) => parse_date_text(&text),
        },
        _ => None,
    }
}

/// Epoch values below 10^11 are seconds (that instant is year 5138); every
/// real epoch-millisecond value exceeds it. A zero epoch is ABS's absent-date
/// sentinel, not 1970-01-01, so it normalises to `None`.
fn epoch_value_to_secs(raw: u64) -> Option<u64> {
    if raw == 0 {
        return None;
    }
    Some(if raw >= 100_000_000_000 {
        raw / 1000
    } else {
        raw
    })
}

/// Parse ISO-8601 or RFC 2822 timestamp text into unix seconds UTC.
fn parse_date_text(text: &str) -> Option<u64> {
    use time::format_description::well_known::{Iso8601, Rfc2822};
    let t = text.trim();
    // ISO dates are Y-M-D based (at least two dashes); RFC 2822 dates are
    // month-name based. A plain `contains('T')` is not enough — "GMT"
    // zones and clock times contain "T"s too.
    let dt = if t.matches('-').count() >= 2 {
        // time's Iso8601 accepts "Z" but not lowercase "z"; normalize.
        let t = match t.strip_suffix('z') {
            Some(rest) => format!("{rest}Z"),
            None => t.to_owned(),
        };
        time::OffsetDateTime::parse(&t, &Iso8601::DEFAULT).ok()
    } else {
        time::OffsetDateTime::parse(t, &Rfc2822).ok()
    }?;
    if dt.year() < 1970 {
        return None;
    }
    Some(dt.unix_timestamp() as u64)
}
impl AudiobookshelfClient {
    /// Runs `f` against a cloned client on a bounded worker thread. All
    /// `*_bounded` wrappers below differ only in the args they capture and
    /// the method they call, so they share this dispatch.
    fn bounded<T>(
        &self,
        bound: Duration,
        f: impl FnOnce(Self) -> Result<T, AudiobookshelfError> + Send + 'static,
    ) -> Result<T, AudiobookshelfError>
    where
        T: Send + 'static,
    {
        let client = self.clone();
        crate::bounded::run_with_hard_bound(move || f(client), bound)
    }

    pub fn libraries_bounded(
        &self,
        key: &str,
        bound: Duration,
    ) -> Result<Vec<AudiobookshelfLibrary>, AudiobookshelfError> {
        let key = key.to_owned();
        self.bounded(bound, move |client| client.libraries(&key))
    }
    pub fn podcast_shows_bounded(
        &self,
        key: &str,
        library_id: &str,
        page: usize,
        limit: usize,
        bound: Duration,
    ) -> Result<AudiobookshelfShowPage, AudiobookshelfError> {
        let key = key.to_owned();
        let library_id = library_id.to_owned();
        let limit = limit.clamp(1, 100);
        self.bounded(bound, move |client| {
            client.podcast_shows(&key, &library_id, page, limit)
        })
    }
    pub fn podcast_detail_bounded(
        &self,
        key: &str,
        library_item_id: &str,
        bound: Duration,
    ) -> Result<Vec<AudiobookshelfDownloadedEpisode>, AudiobookshelfError> {
        let key = key.to_owned();
        let id = library_item_id.to_owned();
        self.bounded(bound, move |client| client.podcast_detail(&key, &id))
    }
    pub fn progress_bounded(
        &self,
        key: &str,
        bound: Duration,
    ) -> Result<HashMap<(String, String), AudiobookshelfProgress>, AudiobookshelfError> {
        let key = key.to_owned();
        self.bounded(bound, move |client| client.progress(&key))
    }
    pub fn shelves_bounded(
        &self,
        key: &str,
        library_id: &str,
        bound: Duration,
    ) -> Result<Vec<AudiobookshelfShelf>, AudiobookshelfError> {
        let key = key.to_owned();
        let id = library_id.to_owned();
        self.bounded(bound, move |client| client.shelves(&key, &id))
    }
    pub fn cover_bounded(
        &self,
        key: &str,
        library_item_id: &str,
        bound: Duration,
    ) -> Result<Vec<u8>, AudiobookshelfError> {
        let key = key.to_owned();
        let id = library_item_id.to_owned();
        self.bounded(bound, move |client| client.cover(&key, &id))
    }
    pub(super) fn get(
        &self,
        key: &str,
        path: &str,
    ) -> Result<ureq::http::Response<ureq::Body>, AudiobookshelfError> {
        self.agent
            .get(&format!("{}{}", self.server_url, path))
            .header("Authorization", &format!("Bearer {key}"))
            .call()
            .map_err(map_error)
    }
    fn libraries(&self, key: &str) -> Result<Vec<AudiobookshelfLibrary>, AudiobookshelfError> {
        let response: LibrariesResponse = self
            .get(key, "/api/libraries")?
            .body_mut()
            .read_json()
            .map_err(|_| AudiobookshelfError::malformed())?;
        Ok(response
            .libraries
            .into_iter()
            .map(|x| AudiobookshelfLibrary {
                id: x.id,
                name: x.name,
                media_type: x.media_type,
            })
            .collect())
    }
    pub(super) fn podcast_shows(
        &self,
        key: &str,
        id: &str,
        page: usize,
        limit: usize,
    ) -> Result<AudiobookshelfShowPage, AudiobookshelfError> {
        let path = format!(
            "/api/libraries/{}/items?page={page}&limit={limit}",
            crate::encode_path_segment(id)
        );
        let response: ItemsResponse = self
            .get(key, &path)?
            .body_mut()
            .read_json()
            .map_err(|_| AudiobookshelfError::malformed())?;
        if response.limit == 0 {
            return Err(AudiobookshelfError::protocol());
        }
        Ok(AudiobookshelfShowPage {
            page: response.page,
            limit: response.limit,
            total: response.total,
            items: response
                .results
                .into_iter()
                .map(|x| {
                    let metadata = x.media.as_ref().and_then(|media| media.metadata.as_ref());
                    AudiobookshelfShow {
                        library_item_id: x.library_item_id,
                        title: x
                            .title
                            .or_else(|| metadata.and_then(|value| value.title.clone()))
                            .unwrap_or_default(),
                        author: x
                            .author
                            .or_else(|| metadata.and_then(|value| value.author.clone())),
                        description: metadata.and_then(|value| value.description.clone()),
                        cover_path: x
                            .cover_path
                            .or_else(|| x.media.and_then(|media| media.cover_path)),
                    }
                })
                .collect(),
        })
    }
    pub(super) fn podcast_detail(
        &self,
        key: &str,
        id: &str,
    ) -> Result<Vec<AudiobookshelfDownloadedEpisode>, AudiobookshelfError> {
        let response: ExpandedWire = self
            .get(
                key,
                &format!("/api/items/{}?expanded=1", crate::encode_path_segment(id)),
            )?
            .body_mut()
            .read_json()
            .map_err(|_| AudiobookshelfError::malformed())?;
        if response.id != id {
            return Err(AudiobookshelfError::protocol());
        }
        Ok(response
            .media
            .and_then(|x| x.episodes)
            .unwrap_or_default()
            .into_iter()
            .map(|x| AudiobookshelfDownloadedEpisode {
                library_item_id: id.to_owned(),
                episode_id: x.id,
                title: x.title,
                description: x.description.as_deref().map(crate::api::html_to_text),
                published_at: published_at_secs(x.published_at),
                duration_seconds: x.duration,
            })
            .collect())
    }
    fn progress(
        &self,
        key: &str,
    ) -> Result<HashMap<(String, String), AudiobookshelfProgress>, AudiobookshelfError> {
        let response: ProgressResponse = self
            .get(key, "/api/me/progress")?
            .body_mut()
            .read_json()
            .map_err(|_| AudiobookshelfError::malformed())?;
        Ok(response
            .media_progress
            .into_iter()
            .filter_map(|x| {
                let episode_id = x.episode_id?;
                let value = AudiobookshelfProgress {
                    library_item_id: x.library_item_id.clone(),
                    episode_id: episode_id.clone(),
                    current_time_seconds: x.current_time.unwrap_or(0.0).max(0.0),
                    is_finished: x.is_finished.unwrap_or(false),
                };
                Some(((x.library_item_id, episode_id), value))
            })
            .collect())
    }
    fn shelves(
        &self,
        key: &str,
        id: &str,
    ) -> Result<Vec<AudiobookshelfShelf>, AudiobookshelfError> {
        let response: Vec<ShelfWire> = self
            .get(
                key,
                &format!(
                    "/api/libraries/{}/personalized",
                    crate::encode_path_segment(id)
                ),
            )?
            .body_mut()
            .read_json()
            .map_err(|_| AudiobookshelfError::malformed())?;
        Ok(response
            .into_iter()
            .map(|x| AudiobookshelfShelf {
                label: x.label,
                entries: x.entities.into_iter().map(shelf_entry_from_wire).collect(),
            })
            .collect())
    }
    fn cover(&self, key: &str, id: &str) -> Result<Vec<u8>, AudiobookshelfError> {
        let response = self.get(
            key,
            &format!("/api/items/{}/cover", crate::encode_path_segment(id)),
        )?;
        let mut bytes = Vec::new();
        response
            .into_body()
            .into_reader()
            .read_to_end(&mut bytes)
            .map_err(|_| AudiobookshelfError::malformed())?;
        Ok(bytes)
    }
}

pub(super) fn map_error(error: ureq::Error) -> AudiobookshelfError {
    match error {
        ureq::Error::StatusCode(401 | 403) => {
            AudiobookshelfError::new(super::AudiobookshelfFailureClass::AuthenticationRejected)
        }
        ureq::Error::StatusCode(status) if status >= 500 => {
            AudiobookshelfError::new(super::AudiobookshelfFailureClass::Server)
        }
        ureq::Error::StatusCode(_) => AudiobookshelfError::protocol(),
        _ => AudiobookshelfError::connectivity(),
    }
}

#[cfg(test)]
#[path = "tests/catalog.rs"]
mod tests;
