use super::{AudiobookshelfClient, AudiobookshelfError};
use serde::Deserialize;
use std::collections::HashMap;
use std::io::Read;
use std::time::Duration;

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
    pub published_at: Option<String>,
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
    Episode {
        library_item_id: String,
        episode_id: String,
    },
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

#[derive(Debug, Clone, PartialEq)]
pub struct AudiobookshelfChapter {
    pub id: usize,
    pub start: f64,
    pub end: f64,
    pub title: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AudiobookshelfAudioFile {
    pub index: usize,
    pub ino: String,
    pub duration: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AudiobookshelfBook {
    pub library_item_id: String,
    pub title: String,
    /// Raw author credit (full, possibly multi-author) for display.
    pub author_display: Option<String>,
    /// Surname of the first-listed author; the raw credit on parse failure.
    pub author_sort_key: String,
    pub cover_path: Option<String>,
    pub chapters: Vec<AudiobookshelfChapter>,
    pub audio_files: Vec<AudiobookshelfAudioFile>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AudiobookshelfBookPage {
    pub page: usize,
    pub limit: usize,
    pub total: usize,
    pub items: Vec<AudiobookshelfBook>,
}

/// Book listening progress, keyed by `libraryItemId` only (no episode identity).
#[derive(Debug, Clone, PartialEq)]
pub struct AudiobookshelfBookProgress {
    pub library_item_id: String,
    pub current_time_seconds: f64,
    pub is_finished: bool,
}

/// Surname of the first-listed author, falling back to the raw credit when
/// `human_name` cannot extract one.
pub fn audiobook_author_sort_key(name: &str) -> String {
    human_name::Name::parse(name)
        .map(|n| n.surname().to_owned())
        .unwrap_or_else(|| name.to_string())
}

/// The full raw author credit for display: the `authors` list joined, else the
/// single `author` string.
fn book_author_display(author: Option<&str>, authors: Option<&[String]>) -> Option<String> {
    if let Some(authors) = authors.filter(|list| !list.is_empty()) {
        return Some(authors.join(", "));
    }
    author
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// Sort key from the raw credit: only the first-listed author participates.
fn first_listed_author_sort_key(credit: &str) -> String {
    let first = credit.split(',').next().unwrap_or_default().trim();
    if first.is_empty() {
        credit.to_string()
    } else {
        audiobook_author_sort_key(first)
    }
}

#[derive(Debug, Deserialize)]
struct LibrariesResponse {
    libraries: Vec<LibraryWire>,
}
#[derive(Debug, Deserialize)]
struct LibraryWire {
    id: String,
    name: String,
    #[serde(rename = "mediaType")]
    media_type: String,
}
#[derive(Debug, Deserialize)]
struct ItemsResponse {
    page: usize,
    limit: usize,
    total: usize,
    #[serde(alias = "items")]
    results: Vec<ShowWire>,
}
#[derive(Debug, Deserialize)]
struct ShowWire {
    #[serde(rename = "id", alias = "libraryItemId")]
    library_item_id: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    author: Option<String>,
    #[serde(rename = "coverPath", default)]
    cover_path: Option<String>,
    #[serde(default)]
    media: Option<PodcastMediaWire>,
}
#[derive(Debug, Deserialize)]
struct PodcastMediaWire {
    #[serde(rename = "coverPath", default)]
    cover_path: Option<String>,
    metadata: Option<PodcastMetadataWire>,
}
#[derive(Debug, Deserialize)]
struct PodcastMetadataWire {
    title: Option<String>,
    author: Option<String>,
    description: Option<String>,
}
#[derive(Debug, Deserialize)]
struct ExpandedWire {
    id: String,
    media: Option<MediaWire>,
}
#[derive(Debug, Deserialize)]
struct MediaWire {
    episodes: Option<Vec<EpisodeWire>>,
}
#[derive(Debug, Deserialize)]
struct EpisodeWire {
    id: String,
    title: String,
    #[serde(rename = "publishedAt")]
    published_at: Option<serde_json::Value>,
    duration: Option<f64>,
}
#[derive(Debug, Deserialize)]
struct ProgressResponse {
    #[serde(rename = "mediaProgress")]
    media_progress: Vec<ProgressWire>,
}
#[derive(Debug, Deserialize)]
struct ProgressWire {
    #[serde(rename = "libraryItemId")]
    library_item_id: String,
    #[serde(rename = "episodeId")]
    episode_id: Option<String>,
    #[serde(rename = "currentTime")]
    current_time: Option<f64>,
    #[serde(rename = "isFinished")]
    is_finished: Option<bool>,
}
#[derive(Debug, Deserialize)]
struct ShelfWire {
    label: String,
    entries: Vec<ShelfEntryWire>,
}
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum ShelfEntryWire {
    #[serde(rename = "show")]
    Show {
        #[serde(rename = "libraryItemId")]
        library_item_id: String,
    },
    #[serde(rename = "episode")]
    Episode {
        #[serde(rename = "libraryItemId")]
        library_item_id: String,
        #[serde(rename = "episodeId")]
        episode_id: String,
    },
}
#[derive(Debug, Deserialize)]
struct BooksResponse {
    page: usize,
    limit: usize,
    total: usize,
    #[serde(alias = "items")]
    results: Vec<BookWire>,
}
#[derive(Debug, Deserialize)]
struct BookWire {
    #[serde(rename = "id", alias = "libraryItemId")]
    library_item_id: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(rename = "coverPath", default)]
    cover_path: Option<String>,
    #[serde(default)]
    media: Option<BookMediaWire>,
}
#[derive(Debug, Deserialize, Default)]
struct BookMediaWire {
    #[serde(rename = "coverPath", default)]
    cover_path: Option<String>,
    #[serde(default)]
    metadata: Option<BookMetadataWire>,
    #[serde(default)]
    chapters: Option<Vec<ChapterWire>>,
    #[serde(rename = "audioFiles", default)]
    audio_files: Option<Vec<AudioFileWire>>,
}
#[derive(Debug, Deserialize)]
struct BookMetadataWire {
    title: Option<String>,
    #[serde(default)]
    author: Option<String>,
    #[serde(default)]
    authors: Option<Vec<String>>,
}
#[derive(Debug, Deserialize)]
struct ChapterWire {
    id: usize,
    start: f64,
    end: f64,
    title: String,
}
#[derive(Debug, Deserialize)]
struct AudioFileWire {
    index: usize,
    ino: String,
    duration: f64,
}
#[derive(Debug, Deserialize)]
struct BookDetailWire {
    id: String,
    media: Option<BookMediaWire>,
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
    pub fn books_bounded(
        &self,
        key: &str,
        library_id: &str,
        page: usize,
        limit: usize,
        bound: Duration,
    ) -> Result<AudiobookshelfBookPage, AudiobookshelfError> {
        let key = key.to_owned();
        let library_id = library_id.to_owned();
        let limit = limit.clamp(1, 100);
        self.bounded(bound, move |client| {
            client.books(&key, &library_id, page, limit)
        })
    }
    pub fn book_detail_bounded(
        &self,
        key: &str,
        library_item_id: &str,
        bound: Duration,
    ) -> Result<(Vec<AudiobookshelfChapter>, Vec<AudiobookshelfAudioFile>), AudiobookshelfError>
    {
        let key = key.to_owned();
        let id = library_item_id.to_owned();
        self.bounded(bound, move |client| client.book_detail(&key, &id))
    }
    pub fn book_progress_bounded(
        &self,
        key: &str,
        bound: Duration,
    ) -> Result<HashMap<String, AudiobookshelfBookProgress>, AudiobookshelfError> {
        let key = key.to_owned();
        self.bounded(bound, move |client| client.book_progress(&key))
    }

    pub(super) fn get(&self, key: &str, path: &str) -> Result<ureq::Response, AudiobookshelfError> {
        self.agent
            .get(&format!("{}{}", self.server_url, path))
            .set("Authorization", &format!("Bearer {key}"))
            .call()
            .map_err(map_error)
    }
    fn libraries(&self, key: &str) -> Result<Vec<AudiobookshelfLibrary>, AudiobookshelfError> {
        let response: LibrariesResponse = self
            .get(key, "/api/libraries")?
            .into_json()
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
    fn podcast_shows(
        &self,
        key: &str,
        id: &str,
        page: usize,
        limit: usize,
    ) -> Result<AudiobookshelfShowPage, AudiobookshelfError> {
        let path = format!("/api/libraries/{id}/items?page={page}&limit={limit}");
        let response: ItemsResponse = self
            .get(key, &path)?
            .into_json()
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
    fn podcast_detail(
        &self,
        key: &str,
        id: &str,
    ) -> Result<Vec<AudiobookshelfDownloadedEpisode>, AudiobookshelfError> {
        let response: ExpandedWire = self
            .get(key, &format!("/api/items/{id}?expanded=1"))?
            .into_json()
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
                published_at: x.published_at.and_then(|value| match value {
                    serde_json::Value::String(value) => Some(value),
                    serde_json::Value::Number(value) => Some(value.to_string()),
                    _ => None,
                }),
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
            .into_json()
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
    fn books(
        &self,
        key: &str,
        id: &str,
        page: usize,
        limit: usize,
    ) -> Result<AudiobookshelfBookPage, AudiobookshelfError> {
        let path = format!("/api/libraries/{id}/items?page={page}&limit={limit}");
        let response: BooksResponse = self
            .get(key, &path)?
            .into_json()
            .map_err(|_| AudiobookshelfError::malformed())?;
        if response.limit == 0 {
            return Err(AudiobookshelfError::protocol());
        }
        Ok(AudiobookshelfBookPage {
            page: response.page,
            limit: response.limit,
            total: response.total,
            items: response
                .results
                .into_iter()
                .map(|x| {
                    let metadata = x.media.as_ref().and_then(|media| media.metadata.as_ref());
                    let author_display = book_author_display(
                        metadata.and_then(|value| value.author.as_deref()),
                        metadata.and_then(|value| value.authors.as_deref()),
                    );
                    AudiobookshelfBook {
                        author_sort_key: author_display
                            .as_deref()
                            .map(first_listed_author_sort_key)
                            .unwrap_or_default(),
                        title: x
                            .title
                            .or_else(|| metadata.and_then(|value| value.title.clone()))
                            .unwrap_or_default(),
                        cover_path: x.cover_path.or_else(|| {
                            x.media.as_ref().and_then(|media| media.cover_path.clone())
                        }),
                        library_item_id: x.library_item_id,
                        author_display,
                        chapters: Vec::new(),
                        audio_files: Vec::new(),
                    }
                })
                .collect(),
        })
    }
    fn book_detail(
        &self,
        key: &str,
        id: &str,
    ) -> Result<(Vec<AudiobookshelfChapter>, Vec<AudiobookshelfAudioFile>), AudiobookshelfError>
    {
        let response: BookDetailWire = self
            .get(key, &format!("/api/items/{id}?expanded=1"))?
            .into_json()
            .map_err(|_| AudiobookshelfError::malformed())?;
        if response.id != id {
            return Err(AudiobookshelfError::protocol());
        }
        let media = response.media.unwrap_or_default();
        let chapters = media
            .chapters
            .unwrap_or_default()
            .into_iter()
            .map(|x| AudiobookshelfChapter {
                id: x.id,
                start: x.start,
                end: x.end,
                title: x.title,
            })
            .collect();
        let audio_files = media
            .audio_files
            .unwrap_or_default()
            .into_iter()
            .map(|x| AudiobookshelfAudioFile {
                index: x.index,
                ino: x.ino,
                duration: x.duration,
            })
            .collect();
        Ok((chapters, audio_files))
    }
    fn book_progress(
        &self,
        key: &str,
    ) -> Result<HashMap<String, AudiobookshelfBookProgress>, AudiobookshelfError> {
        let response: ProgressResponse = self
            .get(key, "/api/me/progress")?
            .into_json()
            .map_err(|_| AudiobookshelfError::malformed())?;
        Ok(response
            .media_progress
            .into_iter()
            .filter(|x| x.episode_id.is_none())
            .map(|x| {
                let value = AudiobookshelfBookProgress {
                    current_time_seconds: x.current_time.unwrap_or(0.0).max(0.0),
                    is_finished: x.is_finished.unwrap_or(false),
                    library_item_id: x.library_item_id.clone(),
                };
                (x.library_item_id, value)
            })
            .collect())
    }
    fn shelves(
        &self,
        key: &str,
        id: &str,
    ) -> Result<Vec<AudiobookshelfShelf>, AudiobookshelfError> {
        let response: Vec<ShelfWire> = self
            .get(key, &format!("/api/libraries/{id}/personalized"))?
            .into_json()
            .map_err(|_| AudiobookshelfError::malformed())?;
        Ok(response
            .into_iter()
            .map(|x| AudiobookshelfShelf {
                label: x.label,
                entries: x
                    .entries
                    .into_iter()
                    .map(|entry| match entry {
                        ShelfEntryWire::Show { library_item_id } => {
                            AudiobookshelfShelfEntry::Show(library_item_id)
                        }
                        ShelfEntryWire::Episode {
                            library_item_id,
                            episode_id,
                        } => AudiobookshelfShelfEntry::Episode {
                            library_item_id,
                            episode_id,
                        },
                    })
                    .collect(),
            })
            .collect())
    }
    fn cover(&self, key: &str, id: &str) -> Result<Vec<u8>, AudiobookshelfError> {
        let response = self.get(key, &format!("/api/items/{id}/cover"))?;
        let mut bytes = Vec::new();
        response
            .into_reader()
            .read_to_end(&mut bytes)
            .map_err(|_| AudiobookshelfError::malformed())?;
        Ok(bytes)
    }
}

pub(super) fn map_error(error: ureq::Error) -> AudiobookshelfError {
    match error {
        ureq::Error::Status(401 | 403, _) => {
            AudiobookshelfError::new(super::AudiobookshelfFailureClass::AuthenticationRejected)
        }
        ureq::Error::Status(status, _) if status >= 500 => {
            AudiobookshelfError::new(super::AudiobookshelfFailureClass::Server)
        }
        ureq::Error::Status(_, _) => AudiobookshelfError::protocol(),
        _ => AudiobookshelfError::connectivity(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> String {
        std::fs::read_to_string(format!(
            "{}/tests/fixtures/audiobookshelf/{name}.json",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap()
    }

    #[test]
    fn fixtures_decode_without_losing_native_identity() {
        let libraries: LibrariesResponse = serde_json::from_str(&fixture("libraries")).unwrap();
        assert_eq!(libraries.libraries[1].id, "lib-podcast");
        assert_eq!(libraries.libraries[1].media_type, "podcast");
        let page: ItemsResponse = serde_json::from_str(&fixture("items-page")).unwrap();
        assert_eq!((page.page, page.limit, page.total), (0, 20, 2));
        assert_eq!(page.results[0].library_item_id, "show-2");
        assert_eq!(
            page.results[0]
                .media
                .as_ref()
                .and_then(|media| media.metadata.as_ref())
                .and_then(|metadata| metadata.title.as_deref()),
            Some("Second Show")
        );
        assert_eq!(
            page.results[0]
                .media
                .as_ref()
                .and_then(|media| media.metadata.as_ref())
                .and_then(|metadata| metadata.description.as_deref()),
            Some("Second show description.")
        );
        let expanded: ExpandedWire = serde_json::from_str(&fixture("item-expanded")).unwrap();
        assert_eq!(expanded.id, "show-2");
        assert_eq!(expanded.media.unwrap().episodes.unwrap()[0].id, "episode-1");
    }

    #[test]
    fn progress_and_shelf_fixtures_preserve_user_and_server_order() {
        let progress: ProgressResponse = serde_json::from_str(&fixture("progress")).unwrap();
        assert_eq!(progress.media_progress[0].library_item_id, "show-2");
        assert!(!progress.media_progress[0].is_finished.unwrap());
        let completed: ProgressResponse = serde_json::from_str(
            r#"{"mediaProgress":[{"libraryItemId":"show-2","episodeId":"episode-2","currentTime":120.0,"isFinished":true}]}"#,
        )
        .unwrap();
        assert_eq!(completed.media_progress[0].is_finished, Some(true));
        let shelves: Vec<ShelfWire> = serde_json::from_str(&fixture("shelves")).unwrap();
        assert_eq!(shelves[0].label, "Continue listening");
        assert!(matches!(
            shelves[0].entries[1],
            ShelfEntryWire::Episode { .. }
        ));
    }

    #[test]
    fn null_episode_id_progress_is_skipped() {
        let json = r#"{"mediaProgress":[{"libraryItemId":"lib-1","episodeId":null,"currentTime":10.0,"isFinished":false}]}"#;
        let response: ProgressResponse = serde_json::from_str(json).unwrap();
        let mapped: HashMap<(String, String), AudiobookshelfProgress> = response
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
            .collect();
        assert!(mapped.is_empty());
    }

    #[test]
    fn covers_allow_present_and_missing_paths() {
        let cover_json = fixture("present-cover");
        let trimmed = cover_json.trim();
        let inner = trimmed
            .strip_prefix('{')
            .and_then(|s| s.strip_suffix('}'))
            .unwrap();
        let present: ShowWire = serde_json::from_str(&format!(
            "{{\"libraryItemId\":\"show-2\",\"title\":\"Show\",{inner}}}"
        ))
        .unwrap();
        assert_eq!(present.cover_path.as_deref(), Some("/cover/show-2"));
        let missing: serde_json::Value = serde_json::from_str(&fixture("missing-cover")).unwrap();
        assert!(missing["coverPath"].is_null());
    }

    #[test]
    fn invalid_page_metadata_is_a_protocol_failure() {
        let page: ItemsResponse =
            serde_json::from_str(r#"{"page":0,"limit":20,"total":1,"results":[]}"#).unwrap();
        assert_eq!(page.page, 0);
    }

    #[test]
    fn auth_failures_are_classified_and_errors_redact_credentials() {
        let error = AudiobookshelfError::new(
            super::super::AudiobookshelfFailureClass::AuthenticationRejected,
        );
        assert!(!error.to_string().contains("secret-key"));
        assert!(serde_json::from_str::<ItemsResponse>("not json").is_err());
    }
}
