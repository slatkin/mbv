use mbv_core::audiobookshelf::{
    AudiobookshelfAudioFile, AudiobookshelfBook, AudiobookshelfBookProgress, AudiobookshelfChapter,
    AudiobookshelfDownloadedEpisode, AudiobookshelfLibrary, AudiobookshelfProgress,
    AudiobookshelfShow,
};
use std::collections::{HashMap, HashSet};

/// The resolved browse kind for an Audiobookshelf library tab, derived once
/// from the library's `media_type` at the browse-dispatch seam. Downstream
/// renderers/input handlers branch on this value and never re-read
/// `media_type` per action (service-browse-dispatch capability).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AudiobookshelfBrowseKind {
    Podcast,
    Book,
}

impl AudiobookshelfBrowseKind {
    /// `media_type` values other than `"book"` resolve to Podcast, matching
    /// the pre-book behavior for the only two media types ABS exposes.
    pub(super) fn from_media_type(media_type: &str) -> Self {
        if media_type == "book" {
            Self::Book
        } else {
            Self::Podcast
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum AudiobookshelfEpisodeFilter {
    #[default]
    All,
    Played,
    Unplayed,
}

impl AudiobookshelfEpisodeFilter {
    pub(super) const ALL: [Self; 3] = [Self::All, Self::Played, Self::Unplayed];

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Played => "Played",
            Self::Unplayed => "Unplayed",
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct AudiobookshelfBrowseState {
    // Retained for upcoming library-local rendering milestones.
    #[allow(dead_code)]
    pub library: AudiobookshelfLibrary,
    pub shows: Vec<AudiobookshelfShow>,
    pub total: usize,
    pub next_page: usize,
    pub loading_pages: HashSet<usize>,
    pub selected_id: Option<String>,
    pub error: Option<String>,
    pub episodes: Option<Vec<AudiobookshelfDownloadedEpisode>>,
    pub detail_cache: HashMap<String, Vec<AudiobookshelfDownloadedEpisode>>,
    pub detail_loading: bool,
    pub progress: HashMap<(String, String), AudiobookshelfProgress>,
}

impl AudiobookshelfBrowseState {
    pub fn new(library: AudiobookshelfLibrary) -> Self {
        Self {
            library,
            shows: Vec::new(),
            total: 0,
            next_page: 0,
            loading_pages: HashSet::new(),
            selected_id: None,
            error: None,
            episodes: None,
            detail_cache: HashMap::new(),
            detail_loading: false,
            progress: HashMap::new(),
        }
    }

    // Retained for upcoming incremental-loading milestones.
    #[allow(dead_code)]
    pub fn cursor(&self) -> usize {
        self.selected_id
            .as_ref()
            .and_then(|id| {
                self.shows
                    .iter()
                    .position(|show| &show.library_item_id == id)
            })
            .unwrap_or(0)
    }

    /// Whether the last `select()` changed the selected show. The component
    /// owns the episode filter / episode-mode selection and consults this to
    /// reset them on an identity change (the reset moved off this content
    /// struct in split-browse-state-interaction-fields task 3.2).
    pub fn select_changed_identity(&self, cursor: usize) -> bool {
        self.shows.get(cursor).map(|show| &show.library_item_id) != self.selected_id.as_ref()
    }

    pub fn select(&mut self, cursor: usize) {
        self.selected_id = self
            .shows
            .get(cursor)
            .map(|show| show.library_item_id.clone());
        self.episodes = self
            .selected_id
            .as_ref()
            .and_then(|id| self.detail_cache.get(id).cloned());
        self.detail_loading = false;
    }

    pub fn cache_detail(&mut self, id: String, episodes: Vec<AudiobookshelfDownloadedEpisode>) {
        self.detail_cache.insert(id, episodes);
    }

    pub fn selected_show(&self) -> Option<&AudiobookshelfShow> {
        let id = self.selected_id.as_deref()?;
        self.shows.iter().find(|show| show.library_item_id == id)
    }

    pub fn visible_episodes(
        &self,
        filter: AudiobookshelfEpisodeFilter,
    ) -> Vec<&AudiobookshelfDownloadedEpisode> {
        self.visible_episodes_from(self.episodes.as_deref().unwrap_or_default(), filter)
    }

    pub fn visible_episodes_from<'a>(
        &self,
        source: &'a [AudiobookshelfDownloadedEpisode],
        filter: AudiobookshelfEpisodeFilter,
    ) -> Vec<&'a AudiobookshelfDownloadedEpisode> {
        let mut episodes = source
            .iter()
            .filter(|episode| match filter {
                AudiobookshelfEpisodeFilter::All => true,
                AudiobookshelfEpisodeFilter::Played => self
                    .progress
                    .get(&(episode.library_item_id.clone(), episode.episode_id.clone()))
                    .is_some_and(|progress| progress.is_finished),
                AudiobookshelfEpisodeFilter::Unplayed => !self
                    .progress
                    .get(&(episode.library_item_id.clone(), episode.episode_id.clone()))
                    .is_some_and(|progress| progress.is_finished),
            })
            .collect::<Vec<_>>();
        episodes.sort_by(|left, right| {
            compare_publication_dates(left.published_at.as_deref(), right.published_at.as_deref())
        });
        episodes
    }

    pub fn append_page(
        &mut self,
        page: usize,
        limit: usize,
        total: usize,
        shows: Vec<AudiobookshelfShow>,
    ) {
        self.loading_pages.remove(&page);
        self.total = total;
        self.next_page = self.next_page.max(page + 1);
        for show in shows {
            if !self
                .shows
                .iter()
                .any(|existing| existing.library_item_id == show.library_item_id)
            {
                self.shows.push(show);
            }
        }
        self.shows.sort_by_key(|show| show.title.to_lowercase());
        if self.selected_id.is_none() && !self.shows.is_empty() {
            self.select(0);
        }
        if let Some(selected_id) = self.selected_id.as_deref() {
            if self
                .shows
                .iter()
                .any(|show| show.library_item_id == selected_id)
                && self.cursor() >= self.shows.len()
            {
                self.select(self.shows.len().saturating_sub(1));
            } else if self.shows.len() >= self.total
                && !self
                    .shows
                    .iter()
                    .any(|show| show.library_item_id == selected_id)
            {
                self.select(0);
            }
        }
        let _ = limit;
    }

    pub fn needs_page(&self) -> Option<usize> {
        (self.shows.len() < self.total && self.loading_pages.is_empty()).then_some(self.next_page)
    }
}

/// Fixed alphabetical author-surname bucket labels for the book tab's
/// pill-filtered browsing (design "Alphabetical surname buckets..."):
/// mirrors `LETTER_FILTER_BUCKETS`' A-Z ranges without its non-alphabetic
/// "#" catch-all, since every surname bucket key (see `surname_bucket_key`)
/// always falls in `'a'..='z'`.
const SURNAME_BUCKET_LABELS: [&str; 8] = [
    "A\u{2013}C",
    "D\u{2013}F",
    "G\u{2013}I",
    "J\u{2013}L",
    "M\u{2013}O",
    "P\u{2013}R",
    "S\u{2013}U",
    "V\u{2013}Z",
];
/// Inclusive upper bound of each range in `SURNAME_BUCKET_LABELS`, in order.
const SURNAME_BUCKET_UPPER: [char; 8] = ['c', 'f', 'i', 'l', 'o', 'r', 'u', 'z'];

/// One alphabetical author-surname range in the book browser's pill row:
/// its label and the `[start, end)` indices it covers in the
/// surname-sorted `books` list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SurnameBucket {
    pub label: &'static str,
    pub start: usize,
    pub end: usize,
}

/// The bucket key character for a book: its `author_sort_key`'s first ASCII
/// letter, lowercased, falling back to `'a'` when the surname has none (a
/// digit- or symbol-led surname), so every book lands in a bucket rather
/// than being excluded (book-browsing spec: surname extraction fallback
/// keeps a book grouped and browsable).
fn surname_bucket_key(sort_key: &str) -> char {
    sort_key
        .chars()
        .find(|c| c.is_ascii_alphabetic())
        .map(|c| c.to_ascii_lowercase())
        .unwrap_or('a')
}

/// Partitions `books` (already sorted by `author_sort_key`) into the fixed
/// alphabetical ranges above, omitting any range with zero books rather than
/// producing an empty, selectable pill (book-browsing spec: "a range with no
/// books in the current library SHALL NOT produce an empty, selectable
/// bucket"). Pure: no app state, mirrors `music_grouping::build_grouped_album_catalog`'s
/// shape without sharing code — the grouping unit differs (fixed ranges, not
/// runs of identical artist).
pub(super) fn build_surname_buckets(books: &[AudiobookshelfBook]) -> Vec<SurnameBucket> {
    let mut buckets = Vec::with_capacity(SURNAME_BUCKET_LABELS.len());
    let mut start = 0;
    for (i, &upper) in SURNAME_BUCKET_UPPER.iter().enumerate() {
        let is_last = i + 1 == SURNAME_BUCKET_UPPER.len();
        let end = if is_last {
            books.len()
        } else {
            books.partition_point(|book| surname_bucket_key(&book.author_sort_key) <= upper)
        };
        if end > start {
            buckets.push(SurnameBucket {
                label: SURNAME_BUCKET_LABELS[i],
                start,
                end,
            });
        }
        start = end;
    }
    buckets
}

/// Same partitioning as `build_surname_buckets`, keyed by a podcast show's
/// title instead of a book's author surname -- shows have no separate sort
/// key, so the title itself (already the sort key `append_page` orders
/// `shows` by) is the bucket key. Kept as a separate function rather than a
/// generic one over both owning types: `AudiobookshelfShow` and
/// `AudiobookshelfBook` share no common trait today, and the loop body is
/// small enough that duplicating it is cheaper than introducing one.
pub(super) fn build_show_title_buckets(shows: &[AudiobookshelfShow]) -> Vec<SurnameBucket> {
    let mut buckets = Vec::with_capacity(SURNAME_BUCKET_LABELS.len());
    let mut start = 0;
    for (i, &upper) in SURNAME_BUCKET_UPPER.iter().enumerate() {
        let is_last = i + 1 == SURNAME_BUCKET_UPPER.len();
        let end = if is_last {
            shows.len()
        } else {
            shows.partition_point(|show| surname_bucket_key(&show.title) <= upper)
        };
        if end > start {
            buckets.push(SurnameBucket {
                label: SURNAME_BUCKET_LABELS[i],
                start,
                end,
            });
        }
        start = end;
    }
    buckets
}

/// Book-shaped browse state: the author-surname-grouped book list, the
/// selected book's chapter/audio-file detail, and book progress keyed by
/// `library_item_id` only. Parallel to `AudiobookshelfBrowseState`; which one
/// a library tab uses is decided once by `AudiobookshelfBrowseKind`.
#[derive(Debug, Clone)]
pub(super) struct AudiobookshelfBookBrowseState {
    pub library: AudiobookshelfLibrary,
    pub books: Vec<AudiobookshelfBook>,
    pub total: usize,
    pub next_page: usize,
    pub loading_pages: HashSet<usize>,
    /// The shell's *resting* selected book -- the last committed selection,
    /// written at the select/bucket/restore event and read by position save
    /// and detail-fetch routing. The component owns the live highlight;
    /// `chapter_selection` and `selected_bucket` are component-only and never
    /// projected (split-browse-state-interaction-fields D1/D2).
    pub selected_id: Option<String>,
    pub error: Option<String>,
    pub detail_cache: HashMap<String, (Vec<AudiobookshelfChapter>, Vec<AudiobookshelfAudioFile>)>,
    /// Book ids with a detail request in flight.
    pub detail_loading_ids: HashSet<String>,
    pub detail_loading: bool,
    pub progress: HashMap<String, AudiobookshelfBookProgress>,
    /// Fixed alphabetical author-surname ranges over `books`, recomputed
    /// whenever `books` changes (see `append_page_books`).
    pub buckets: Vec<SurnameBucket>,
}

impl AudiobookshelfBookBrowseState {
    pub fn new(library: AudiobookshelfLibrary) -> Self {
        Self {
            library,
            books: Vec::new(),
            total: 0,
            next_page: 0,
            loading_pages: HashSet::new(),
            selected_id: None,
            error: None,
            detail_cache: HashMap::new(),
            detail_loading_ids: HashSet::new(),
            detail_loading: false,
            progress: HashMap::new(),
            buckets: Vec::new(),
        }
    }

    pub fn cursor(&self) -> usize {
        self.selected_id
            .as_ref()
            .and_then(|id| {
                self.books
                    .iter()
                    .position(|book| &book.library_item_id == id)
            })
            .unwrap_or(0)
    }

    pub fn select(&mut self, cursor: usize) {
        self.selected_id = self
            .books
            .get(cursor)
            .map(|book| book.library_item_id.clone());
        self.detail_loading = self
            .selected_id
            .as_ref()
            .is_some_and(|id| self.detail_loading_ids.contains(id));
    }

    pub fn selected_book(&self) -> Option<&AudiobookshelfBook> {
        let id = self.selected_id.as_deref()?;
        self.books.iter().find(|book| book.library_item_id == id)
    }

    /// The selected book's chapter rows, falling back to its `audioFiles`
    /// rows when `chapters[]` is empty (book-browsing spec: never an empty
    /// or broken list state).
    pub fn visible_rows(&self, id: &str) -> Vec<BookRow> {
        let Some(detail) = self.detail_cache.get(id) else {
            return Vec::new();
        };
        let (chapters, audio_files) = detail;
        if !chapters.is_empty() {
            chapters
                .iter()
                .map(|chapter| BookRow::Chapter {
                    start: chapter.start,
                    end: chapter.end,
                    title: chapter.title.clone(),
                })
                .collect()
        } else if !audio_files.is_empty() {
            audio_files
                .iter()
                .map(|file| BookRow::AudioFile {
                    index: file.index,
                    duration: file.duration,
                })
                .collect()
        } else {
            Vec::new()
        }
    }

    pub fn append_page_books(&mut self, page: usize, total: usize, books: Vec<AudiobookshelfBook>) {
        self.loading_pages.remove(&page);
        self.total = total;
        self.next_page = self.next_page.max(page + 1);
        for book in books {
            if !self
                .books
                .iter()
                .any(|existing| existing.library_item_id == book.library_item_id)
            {
                self.books.push(book);
            }
        }
        // Author-surname grouping: stable by surname, then title.
        self.books.sort_by(|left, right| {
            left.author_sort_key
                .to_lowercase()
                .cmp(&right.author_sort_key.to_lowercase())
                .then_with(|| left.title.to_lowercase().cmp(&right.title.to_lowercase()))
        });

        // Recompute the alphabetical surname buckets against the
        // refreshed/paged-in list. The component re-anchors its
        // `selected_bucket` onto the still-selected book when it receives the
        // refreshed content (book-browsing spec: refresh/page loading
        // preserves the selected book regardless of its new bucket).
        self.buckets = build_surname_buckets(&self.books);

        if self.selected_id.is_none() && !self.books.is_empty() {
            self.select(0);
        } else if let Some(selected_id) = self.selected_id.as_deref() {
            if self
                .books
                .iter()
                .all(|book| book.library_item_id != selected_id)
                && self.books.len() >= self.total
            {
                self.select(0);
            } else {
                self.detail_loading = self.detail_loading_ids.contains(selected_id);
            }
        }
    }

    pub fn needs_page(&self) -> Option<usize> {
        (self.books.len() < self.total && self.loading_pages.is_empty()).then_some(self.next_page)
    }
}

/// One renderable row beneath the book hero: a chapter (absolute seekable
/// range on the merged timeline) or an audio file (used when chapters are
/// absent). Both carry provider-native identity; neither is an episode shape.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum BookRow {
    Chapter { start: f64, end: f64, title: String },
    AudioFile { index: usize, duration: f64 },
}

fn compare_publication_dates(left: Option<&str>, right: Option<&str>) -> std::cmp::Ordering {
    match (left, right) {
        (None, None) => std::cmp::Ordering::Equal,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (Some(_), None) => std::cmp::Ordering::Less,
        (Some(left), Some(right)) => match (left.parse::<f64>(), right.parse::<f64>()) {
            (Ok(left), Ok(right)) => right
                .partial_cmp(&left)
                .unwrap_or(std::cmp::Ordering::Equal),
            _ => right.cmp(left),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mbv_core::audiobookshelf::audiobook_author_sort_key;

    fn library() -> AudiobookshelfLibrary {
        AudiobookshelfLibrary {
            id: "library".into(),
            name: "Podcasts".into(),
            media_type: "podcast".into(),
        }
    }

    fn show(id: &str, title: &str) -> AudiobookshelfShow {
        AudiobookshelfShow {
            library_item_id: id.into(),
            title: title.into(),
            author: None,
            description: None,
            cover_path: None,
        }
    }

    fn episode(show: &str, id: &str) -> AudiobookshelfDownloadedEpisode {
        AudiobookshelfDownloadedEpisode {
            library_item_id: show.into(),
            episode_id: id.into(),
            title: id.into(),
            published_at: None,
            duration_seconds: None,
        }
    }

    #[test]
    fn select_drops_the_prior_shows_episodes_and_reports_identity_change() {
        let mut state = AudiobookshelfBrowseState::new(library());
        state.append_page(1, 20, 2, vec![show("a", "A"), show("b", "B")]);
        state.select(0);
        state.episodes = Some(vec![episode("a", "shared"), episode("a", "two")]);
        assert!(state.select_changed_identity(1));
        state.select(1);
        assert_eq!(state.episodes, None);
    }

    #[test]
    fn empty_episodes_and_missing_progress_are_unstarted() {
        let mut state = AudiobookshelfBrowseState::new(library());
        state.append_page(1, 20, 1, vec![show("a", "A")]);
        state.episodes = Some(Vec::new());
        assert_eq!(state.shows.len(), 1);
        assert!(!state.progress.contains_key(&("a".into(), "missing".into())));
    }

    #[test]
    fn same_episode_id_isolated_by_show_identity() {
        let mut state = AudiobookshelfBrowseState::new(library());
        state.progress.insert(
            ("a".into(), "shared".into()),
            AudiobookshelfProgress {
                library_item_id: "a".into(),
                episode_id: "shared".into(),
                current_time_seconds: 4.0,
                is_finished: false,
            },
        );
        assert!(!state.progress.keys().any(|(library_item_id, episode_id)| {
            library_item_id == "b" && episode_id == "shared"
        }));
    }

    #[test]
    fn filters_completed_progress_and_treats_partial_as_unplayed() {
        let mut state = AudiobookshelfBrowseState::new(library());
        state.append_page(0, 20, 1, vec![show("a", "A")]);
        state.episodes = Some(vec![
            episode("a", "finished"),
            episode("a", "partial"),
            episode("a", "missing"),
        ]);
        state.progress.insert(
            ("a".into(), "finished".into()),
            AudiobookshelfProgress {
                library_item_id: "a".into(),
                episode_id: "finished".into(),
                current_time_seconds: 1.0,
                is_finished: true,
            },
        );
        state.progress.insert(
            ("a".into(), "partial".into()),
            AudiobookshelfProgress {
                library_item_id: "a".into(),
                episode_id: "partial".into(),
                current_time_seconds: 1.0,
                is_finished: false,
            },
        );

        assert_eq!(
            state.visible_episodes(AudiobookshelfEpisodeFilter::Played)[0].episode_id,
            "finished"
        );
        assert_eq!(
            state
                .visible_episodes(AudiobookshelfEpisodeFilter::Unplayed)
                .into_iter()
                .map(|episode| episode.episode_id.as_str())
                .collect::<Vec<_>>(),
            ["partial", "missing"]
        );
    }

    #[test]
    fn visible_episodes_are_newest_first_with_undated_last() {
        let mut state = AudiobookshelfBrowseState::new(library());
        state.append_page(0, 20, 1, vec![show("a", "A")]);
        state.episodes = Some(vec![
            episode_with_date("a", "old", Some("2026-01-01")),
            episode_with_date("a", "undated", None),
            episode_with_date("a", "new", Some("2026-08-12")),
        ]);

        assert_eq!(
            state
                .visible_episodes(AudiobookshelfEpisodeFilter::All)
                .into_iter()
                .map(|episode| episode.episode_id.as_str())
                .collect::<Vec<_>>(),
            ["new", "old", "undated"]
        );
    }

    #[test]
    fn audiobookshelf_kind_resolves_once_by_media_type() {
        assert_eq!(
            AudiobookshelfBrowseKind::from_media_type("book"),
            AudiobookshelfBrowseKind::Book
        );
        assert_eq!(
            AudiobookshelfBrowseKind::from_media_type("podcast"),
            AudiobookshelfBrowseKind::Podcast
        );
        assert_eq!(
            AudiobookshelfBrowseKind::from_media_type("book"),
            AudiobookshelfBrowseKind::Book,
            "book resolves to Book every time — dispatch forks once and never re-reads media_type"
        );
    }

    #[test]
    fn book_pages_group_by_author_surname_only() {
        let mut state = AudiobookshelfBookBrowseState::new(library());
        state.append_page_books(
            0,
            3,
            vec![
                book("c", "Title C", "Zelda Author"),
                book("a", "Title A", "Alpha Author"),
                book("b", "Title B", "Beta Author"),
            ],
        );
        assert_eq!(
            state
                .books
                .iter()
                .map(|b| b.library_item_id.as_str())
                .collect::<Vec<_>>(),
            ["a", "b", "c"],
            "books group and sort by author surname, not title"
        );
    }

    #[test]
    fn visible_rows_fall_back_to_audio_files_when_chapters_empty() {
        let mut state = AudiobookshelfBookBrowseState::new(library());
        let id = "book-1";
        state.selected_id = Some(id.into());
        state.detail_cache.insert(
            id.into(),
            (
                Vec::new(),
                vec![
                    AudiobookshelfAudioFile {
                        index: 1,
                        ino: "f1".into(),
                        duration: 100.0,
                    },
                    AudiobookshelfAudioFile {
                        index: 2,
                        ino: "f2".into(),
                        duration: 200.0,
                    },
                ],
            ),
        );
        let rows = state.visible_rows(id);
        assert_eq!(rows.len(), 2);
        assert!(matches!(rows[0], BookRow::AudioFile { index: 1, .. }));
    }

    fn book(id: &str, title: &str, author: &str) -> AudiobookshelfBook {
        AudiobookshelfBook {
            library_item_id: id.into(),
            title: title.into(),
            author_display: Some(author.into()),
            author_sort_key: audiobook_author_sort_key(author),
            cover_path: None,
            duration_seconds: 0.0,
            narrator: None,
            published_year: None,
            genres: Vec::new(),
            description: None,
            series_name: None,
            chapters: Vec::new(),
            audio_files: Vec::new(),
        }
    }

    fn episode_with_date(
        show: &str,
        id: &str,
        published_at: Option<&str>,
    ) -> AudiobookshelfDownloadedEpisode {
        AudiobookshelfDownloadedEpisode {
            library_item_id: show.into(),
            episode_id: id.into(),
            title: id.into(),
            published_at: published_at.map(str::to_string),
            duration_seconds: None,
        }
    }
}
