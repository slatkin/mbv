use super::{AudiobookshelfClient, AudiobookshelfError};
use serde::Deserialize;
use std::collections::HashMap;
use std::time::Duration;

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

/// Surname of the first-listed author: the final title-cased whitespace token,
/// falling back to the raw credit when nothing can be extracted.
pub fn audiobook_author_sort_key(name: &str) -> String {
    let Some(token) = name.split_whitespace().next_back() else {
        return name.to_string();
    };
    let first = token.chars().next().unwrap_or(' ');
    first.to_uppercase().collect::<String>() + &token[first.len_utf8()..]
}

/// The full raw author credit for display: the `authors` list joined, else the
/// single `author` string.
pub(super) fn book_author_display(
    author: Option<&str>,
    authors: Option<&[String]>,
) -> Option<String> {
    if let Some(authors) = authors.filter(|list| !list.is_empty()) {
        return Some(authors.join(", "));
    }
    author
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// Sort key from the raw credit: only the first-listed author participates.
pub(super) fn first_listed_author_sort_key(credit: &str) -> String {
    let first = credit.split(',').next().unwrap_or_default().trim();
    if first.is_empty() {
        credit.to_string()
    } else {
        audiobook_author_sort_key(first)
    }
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

    fn books(
        &self,
        key: &str,
        id: &str,
        page: usize,
        limit: usize,
    ) -> Result<AudiobookshelfBookPage, AudiobookshelfError> {
        let path = format!(
            "/api/libraries/{}/items?page={page}&limit={limit}",
            crate::encode_path_segment(id)
        );
        let response: BooksResponse = self
            .get(key, &path)?
            .body_mut()
            .read_json()
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
        let response: super::ProgressResponse = self
            .get(key, "/api/me/progress")?
            .body_mut()
            .read_json()
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
}
