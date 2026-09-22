use crate::app::render::{feed_age_group, FeedAgeGroup};
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

/// The podcast tab's state pills, in the painted pill-bar order (spec: the
/// state pills `All` / `Unplayed` / `Played` precede the show pills).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum AudiobookshelfEpisodeFilter {
    #[default]
    All,
    Unplayed,
    Played,
}

impl AudiobookshelfEpisodeFilter {
    pub(super) const ALL: [Self; 3] = [Self::All, Self::Unplayed, Self::Played];

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Played => "Played",
            Self::Unplayed => "Unplayed",
        }
    }
}

/// The podcast tab's pill selection, stored by value and never by a painted
/// position (design D3): the show list grows and re-sorts as pages land
/// (`append_page` sorts by title), so an index would silently rebind the
/// active view. The remembered pill across tab switches is the same value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::app) enum PillSelection {
    /// A state pill: every fetched show's episodes, filtered by play state.
    State(AudiobookshelfEpisodeFilter),
    /// A show pill: that show's episodes regardless of play state, by the
    /// show's provider-native `library_item_id`.
    Show(String),
}

#[derive(Debug, Clone)]
pub(super) struct AudiobookshelfBrowseState {
    pub library: AudiobookshelfLibrary,
    /// The paged show list; it feeds the podcast pill bar (one pill per
    /// show). Page arrivals append at most once per show and sort by title.
    pub shows: Vec<AudiobookshelfShow>,
    pub total: usize,
    pub next_page: usize,
    pub loading_pages: HashSet<usize>,
    pub selected_id: Option<String>,
    pub error: Option<String>,
    /// Per-show downloaded-episode cache filled by the per-show episode
    /// fan-out: one entry per fetched show, keyed by that show's
    /// `library_item_id`. A re-arrival replaces the entry (append at most
    /// once); the flat episode views concatenate the entries in show order.
    pub detail_cache: HashMap<String, Vec<AudiobookshelfDownloadedEpisode>>,
    /// Show ids with an episode fetch in flight, holding the request serial
    /// each fetch was issued under (the state's monotonically increasing
    /// `next_detail_request`). A response retires — and may write the cache
    /// from — its own serial only, so an orphaned or superseded response can
    /// neither retire a newer request's mark nor overwrite a newer cache
    /// entry.
    pub detail_loading_ids: HashMap<String, u64>,
    /// The serial issued to the most recent per-show episode fetch.
    pub next_detail_request: u64,
    /// The committed show-pill scope for the lazy episode fan-out (design
    /// D5): `Some(id)` = a show pill is active, so that show's episodes are
    /// the view's only requirement; `None` = a state pill (or the tab's
    /// default) is active, so every listed show is required. Written by the
    /// shell from the component's resolved pill movement, read by the
    /// fan-out scheduler — never a mirror of the owner's painted selection.
    pub committed_show_pill: Option<String>,
    /// The tab's selected episode, by `(library_item_id, episode_id)`
    /// identity — the same identity as the progress map. A refresh that
    /// removes the episode from the views clears it.
    pub selected_episode: Option<(String, String)>,
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
            detail_cache: HashMap::new(),
            detail_loading_ids: HashMap::new(),
            next_detail_request: 0,
            committed_show_pill: None,
            selected_episode: None,
            progress: HashMap::new(),
        }
    }

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
        self.selected_id = self
            .shows
            .get(cursor)
            .map(|show| show.library_item_id.clone());
    }

    pub fn cache_detail(&mut self, id: String, episodes: Vec<AudiobookshelfDownloadedEpisode>) {
        self.detail_cache.insert(id, episodes);
    }

    /// Clears the per-show episode cache, the in-flight fetch marks, and the
    /// selected episode for a refresh; the episode views reload from the
    /// per-show fan-out (the show list itself is cleared by the caller).
    pub fn clear_episodes(&mut self) {
        self.detail_cache.clear();
        self.detail_loading_ids.clear();
        self.selected_episode = None;
    }

    /// The fetched episode with exactly this `(library_item_id, episode_id)`
    /// identity, from the per-show cache — regardless of which show's fetch
    /// placed it or which pill view is active.
    pub fn episode_by_identity(
        &self,
        library_item_id: &str,
        episode_id: &str,
    ) -> Option<&AudiobookshelfDownloadedEpisode> {
        self.detail_cache
            .get(library_item_id)?
            .iter()
            .find(|episode| episode.episode_id == episode_id)
    }

    /// The flat episode view: every fetched show's downloaded episodes,
    /// concatenated in show (pill-bar) order so the view is deterministic.
    /// The active pill scopes and the state filter narrows this view in the
    /// owner; here it is the unfiltered union.
    pub fn visible_episodes(
        &self,
        filter: AudiobookshelfEpisodeFilter,
    ) -> Vec<&AudiobookshelfDownloadedEpisode> {
        let mut visible = Vec::new();
        for show in &self.shows {
            if let Some(episodes) = self.detail_cache.get(&show.library_item_id) {
                visible.extend(self.visible_episodes_from(episodes, filter));
            }
        }
        // Each per-show run is already newest-first; one stable merge pass
        // over the whole union keeps undated episodes last.
        visible.sort_by(|left, right| {
            compare_publication_dates(left.published_at, right.published_at)
        });
        visible
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
            compare_publication_dates(left.published_at, right.published_at)
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
    /// Position in the fixed label/range table, independent of the populated
    /// buckets vec's position after empty ranges are omitted.
    pub index: usize,
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
                index: i,
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
    /// `chapter_focused` and `selected_bucket` are component-only and never
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
                    id: chapter.id,
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
    /// `id` is the Service chapter number: the stable row discriminator that
    /// survives a refresh re-composing the visible rows (design.md D4).
    Chapter {
        id: usize,
        start: f64,
        end: f64,
        title: String,
    },
    AudioFile {
        index: usize,
        duration: f64,
    },
}

fn compare_publication_dates(left: Option<u64>, right: Option<u64>) -> std::cmp::Ordering {
    match (left, right) {
        (None, None) => std::cmp::Ordering::Equal,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (Some(_), None) => std::cmp::Ordering::Less,
        (Some(left), Some(right)) => right.cmp(&left),
    }
}

/// One renderable row in the podcast tab's grouped flat episode list,
/// mirroring `FeedDisplayRow`: non-selectable age-group headings and spacers
/// around selectable `Entry` indices into the flat episode slice (grouping
/// never changes the indices or the stable `(library_item_id, episode_id)`
/// targeting). Consumed by the podcast owner (row 3.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::app) enum PodcastDisplayRow {
    Spacer,
    Heading(FeedAgeGroup),
    Entry(usize),
}

/// Groups the flat episode slice under the Feeds age groups. The slice is
/// sorted newest-first globally (undated episodes last) before the
/// consecutive-run merge, so an interleaved slice does not repeat headings;
/// a group with no episodes produces no heading.
pub(in crate::app) fn podcast_display_rows(
    episodes: &[AudiobookshelfDownloadedEpisode],
    now_secs: u64,
) -> Vec<PodcastDisplayRow> {
    let mut order: Vec<usize> = (0..episodes.len()).collect();
    order.sort_by(|&left, &right| {
        compare_publication_dates(episodes[left].published_at, episodes[right].published_at)
    });

    let mut rows = Vec::new();
    let mut last_group = None;
    for idx in order {
        let group = feed_age_group(episodes[idx].published_at, now_secs);
        if last_group != Some(group) {
            if last_group.is_some() {
                rows.push(PodcastDisplayRow::Spacer);
            }
            rows.push(PodcastDisplayRow::Heading(group));
            last_group = Some(group);
        }
        rows.push(PodcastDisplayRow::Entry(idx));
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use mbv_core::audiobookshelf::audiobook_author_sort_key;
    use mbv_core::config::AudiobookshelfBookBucket;

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
            description: None,
            published_at: None,
            duration_seconds: None,
        }
    }

    #[test]
    fn episode_cache_fills_progressively_per_show_and_dedupes() {
        let mut state = AudiobookshelfBrowseState::new(library());
        state.append_page(1, 20, 2, vec![show("a", "A"), show("b", "B")]);

        // Each show's fetch lands separately and joins the flat view without
        // disturbing the other shows' cached entries.
        state.cache_detail(
            "a".into(),
            vec![episode("a", "a-one"), episode("a", "a-two")],
        );
        assert_eq!(
            state
                .visible_episodes(AudiobookshelfEpisodeFilter::All)
                .into_iter()
                .map(|episode| episode.episode_id.as_str())
                .collect::<Vec<_>>(),
            ["a-one", "a-two"]
        );

        state.cache_detail("b".into(), vec![episode("b", "b-one")]);
        assert_eq!(
            state
                .visible_episodes(AudiobookshelfEpisodeFilter::All)
                .into_iter()
                .map(|episode| episode.episode_id.as_str())
                .collect::<Vec<_>>(),
            ["a-one", "a-two", "b-one"],
            "the flat view concatenates the cached shows in show order"
        );
        assert_eq!(
            state.detail_cache["a"],
            vec![episode("a", "a-one"), episode("a", "a-two")]
        );

        // A re-arrival replaces the show's entry: append at most once, never
        // a duplicate.
        state.cache_detail("a".into(), vec![episode("a", "a-one")]);
        assert_eq!(
            state
                .visible_episodes(AudiobookshelfEpisodeFilter::All)
                .into_iter()
                .map(|episode| episode.episode_id.as_str())
                .collect::<Vec<_>>(),
            ["a-one", "b-one"]
        );
    }

    #[test]
    fn cache_arrivals_keep_the_selected_episode_and_refresh_clears_it() {
        let mut state = AudiobookshelfBrowseState::new(library());
        state.append_page(1, 20, 2, vec![show("a", "A"), show("b", "B")]);
        state.selected_episode = Some(("a".into(), "a-one".into()));

        state.cache_detail("b".into(), vec![episode("b", "b-one")]);
        assert_eq!(
            state.selected_episode,
            Some(("a".into(), "a-one".into())),
            "a later show's cache arrival keeps the selected episode"
        );

        // Refresh: the cache reloads from the fan-out and the selected
        // episode identity goes with it.
        state.cache_detail("a".into(), vec![episode("a", "a-one")]);
        state.clear_episodes();
        assert!(state.detail_cache.is_empty());
        assert!(state.detail_loading_ids.is_empty());
        assert_eq!(state.selected_episode, None);
    }

    #[test]
    fn episode_by_identity_resolves_across_shows_and_requires_both_ids() {
        let mut state = AudiobookshelfBrowseState::new(library());
        state.append_page(1, 20, 2, vec![show("a", "A"), show("b", "B")]);
        state.cache_detail("a".into(), vec![episode("a", "shared")]);
        state.cache_detail("b".into(), vec![episode("b", "shared")]);

        assert_eq!(
            state
                .episode_by_identity("b", "shared")
                .map(|episode| episode.library_item_id.as_str()),
            Some("b"),
            "the same episode id on two shows stays isolated by show identity"
        );
        assert!(state.episode_by_identity("a", "missing").is_none());
        assert!(state.episode_by_identity("c", "shared").is_none());
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
        state.cache_detail(
            "a".into(),
            vec![
                episode("a", "finished"),
                episode("a", "partial"),
                episode("a", "missing"),
            ],
        );
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
        state.append_page(0, 20, 2, vec![show("a", "A"), show("b", "B")]);
        state.cache_detail(
            "a".into(),
            vec![
                episode_with_date("a", "old", Some(1_767_225_600)),
                episode_with_date("a", "new", Some(1_786_492_800)),
            ],
        );
        state.cache_detail("b".into(), vec![episode_with_date("b", "undated", None)]);

        assert_eq!(
            state
                .visible_episodes(AudiobookshelfEpisodeFilter::All)
                .into_iter()
                .map(|episode| episode.episode_id.as_str())
                .collect::<Vec<_>>(),
            ["new", "old", "undated"],
            "the flat union sorts newest-first globally, undated episodes last"
        );
    }

    #[test]
    fn display_rows_insert_non_selectable_groups_without_changing_indices() {
        const DAY: u64 = 24 * 60 * 60;
        let now = 30 * DAY;
        // Deliberately interleaved and unsorted: the builder sorts the flat
        // slice newest-first globally before merging consecutive runs, and a
        // group with no episodes produces no heading.
        let episodes = vec![
            episode_with_date("a", "month", Some(now - 30 * DAY)),
            episode_with_date("a", "new", Some(now)),
            episode_with_date("a", "undated", None),
            episode_with_date("a", "recent", Some(now - 2 * DAY)),
        ];

        assert_eq!(
            podcast_display_rows(&episodes, now),
            vec![
                PodcastDisplayRow::Heading(FeedAgeGroup::New),
                PodcastDisplayRow::Entry(1),
                PodcastDisplayRow::Spacer,
                PodcastDisplayRow::Heading(FeedAgeGroup::Recent),
                PodcastDisplayRow::Entry(3),
                PodcastDisplayRow::Spacer,
                PodcastDisplayRow::Heading(FeedAgeGroup::OlderThanMonth),
                PodcastDisplayRow::Entry(0),
                PodcastDisplayRow::Spacer,
                PodcastDisplayRow::Heading(FeedAgeGroup::Unknown),
                PodcastDisplayRow::Entry(2),
            ],
            "undated episodes sort last and group as `Unknown date`; empty groups are omitted"
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
    fn surname_bucket_table_matches_mbv_core_bucket_indices() {
        for (index, (&label, &upper)) in SURNAME_BUCKET_LABELS
            .iter()
            .zip(SURNAME_BUCKET_UPPER.iter())
            .enumerate()
        {
            let bucket = AudiobookshelfBookBucket::from_bucket_index(index)
                .expect("every app-side surname bucket has a core identity");
            let (expected_label, expected_upper) = match bucket {
                AudiobookshelfBookBucket::AToC => ("A\u{2013}C", 'c'),
                AudiobookshelfBookBucket::DToF => ("D\u{2013}F", 'f'),
                AudiobookshelfBookBucket::GToI => ("G\u{2013}I", 'i'),
                AudiobookshelfBookBucket::JToL => ("J\u{2013}L", 'l'),
                AudiobookshelfBookBucket::MToO => ("M\u{2013}O", 'o'),
                AudiobookshelfBookBucket::PToR => ("P\u{2013}R", 'r'),
                AudiobookshelfBookBucket::SToU => ("S\u{2013}U", 'u'),
                AudiobookshelfBookBucket::VToZ => ("V\u{2013}Z", 'z'),
            };
            assert_eq!((label, upper), (expected_label, expected_upper));
        }
        assert_eq!(
            AudiobookshelfBookBucket::from_bucket_index(SURNAME_BUCKET_LABELS.len()),
            None
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
        published_at: Option<u64>,
    ) -> AudiobookshelfDownloadedEpisode {
        AudiobookshelfDownloadedEpisode {
            library_item_id: show.into(),
            episode_id: id.into(),
            title: id.into(),
            description: None,
            published_at,
            duration_seconds: None,
        }
    }
}
