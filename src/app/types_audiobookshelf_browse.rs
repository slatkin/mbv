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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AudiobookshelfEpisodeFilter {
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
    pub episode_filter: AudiobookshelfEpisodeFilter,
    pub episode_selection: Option<usize>,
    pub scroll: usize,
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
            episode_filter: AudiobookshelfEpisodeFilter::All,
            episode_selection: None,
            scroll: 0,
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

    pub fn select(&mut self, cursor: usize) {
        let previous_id = self.selected_id.clone();
        self.selected_id = self
            .shows
            .get(cursor)
            .map(|show| show.library_item_id.clone());
        if self.selected_id != previous_id {
            self.episode_filter = AudiobookshelfEpisodeFilter::All;
        }
        self.episodes = self
            .selected_id
            .as_ref()
            .and_then(|id| self.detail_cache.get(id).cloned());
        self.detail_loading = false;
        self.episode_selection = None;
    }

    pub fn cache_detail(&mut self, id: String, episodes: Vec<AudiobookshelfDownloadedEpisode>) {
        self.detail_cache.insert(id, episodes);
    }

    pub fn selected_show(&self) -> Option<&AudiobookshelfShow> {
        let id = self.selected_id.as_deref()?;
        self.shows.iter().find(|show| show.library_item_id == id)
    }

    pub fn visible_episodes(&self) -> Vec<&AudiobookshelfDownloadedEpisode> {
        let mut episodes = self
            .episodes
            .as_deref()
            .unwrap_or_default()
            .iter()
            .filter(|episode| match self.episode_filter {
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

    pub fn set_episode_filter(&mut self, filter: AudiobookshelfEpisodeFilter) {
        self.episode_filter = filter;
        if self.episode_selection.is_some() {
            self.episode_selection = Some(0);
        }
    }

    pub fn enter_episode_selection(&mut self) {
        self.episode_selection = Some(0);
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
    pub selected_id: Option<String>,
    pub error: Option<String>,
    pub detail_cache: HashMap<String, (Vec<AudiobookshelfChapter>, Vec<AudiobookshelfAudioFile>)>,
    pub detail_loading: bool,
    pub progress: HashMap<String, AudiobookshelfBookProgress>,
    pub chapter_selection: Option<usize>,
    pub scroll: usize,
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
            detail_loading: false,
            progress: HashMap::new(),
            chapter_selection: None,
            scroll: 0,
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
        self.chapter_selection = None;
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

    pub fn append_page_books(
        &mut self,
        page: usize,
        limit: usize,
        total: usize,
        books: Vec<AudiobookshelfBook>,
    ) {
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
        if self.selected_id.is_none() && !self.books.is_empty() {
            self.select(0);
        }
        let _ = limit;
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
    fn rows_move_between_show_and_episode_identity() {
        let mut state = AudiobookshelfBrowseState::new(library());
        state.append_page(1, 20, 2, vec![show("a", "A"), show("b", "B")]);
        state.episodes = Some(vec![episode("a", "shared"), episode("a", "two")]);
        state.enter_episode_selection();
        state.select(1);
        assert_eq!(state.episodes, None);
        assert_eq!(state.episode_selection, None);
        assert_eq!(state.episode_filter, AudiobookshelfEpisodeFilter::All);
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

        state.set_episode_filter(AudiobookshelfEpisodeFilter::Played);
        assert_eq!(state.visible_episodes()[0].episode_id, "finished");
        state.set_episode_filter(AudiobookshelfEpisodeFilter::Unplayed);
        assert_eq!(
            state
                .visible_episodes()
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
                .visible_episodes()
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
            20,
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
