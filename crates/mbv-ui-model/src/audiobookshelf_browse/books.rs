//! Book-tab browse state: the author-surname-grouped book list, the
//! selected book's chapter/audio-file detail, and book progress.

use mbv_audiobookshelf::{
    AudiobookshelfAudioFile, AudiobookshelfBook, AudiobookshelfBookProgress, AudiobookshelfChapter,
    AudiobookshelfLibrary,
};
use std::collections::{HashMap, HashSet};

use mbv_emby_model::{TICKS_PER_SECOND_F64, saturating_i64_from_f64};
use mbv_queue::{AudiobookshelfBookQueueItem, AudiobookshelfItem, QueueItem};

/// Resolve the selected book as a queue item without mutating playback state.
#[must_use]
pub fn audiobookshelf_book_queue_item(state: &AudiobookshelfBookBrowseState) -> Option<QueueItem> {
    let book_id = state.selected_id.as_deref()?;
    audiobookshelf_book_queue_item_for_id(state, book_id)
}

/// Resolve one book by `library_item_id` as a queue item without mutating
/// playback state (standard-media-context-menus task 5.2): context-menu
/// targets carry a book id, which may differ from the browse selection.
/// Queue-item construction lives here alone; the selected-book form resolves
/// on top of it.
#[must_use]
pub fn audiobookshelf_book_queue_item_for_id(
    state: &AudiobookshelfBookBrowseState,
    book_id: &str,
) -> Option<QueueItem> {
    Some(QueueItem::Audiobookshelf(AudiobookshelfItem::Book(
        audiobookshelf_book_item(state, book_id)?,
    )))
}

/// Resolve one book by `library_item_id` -- not only the browse selection --
/// as a queue-item payload without mutating playback state (chapter-row
/// activation targets a book row directly, which may not be the selection).
/// Duration is the sum of the book's audio-file durations (chapters are
/// offsets, not durations).
#[must_use]
pub fn audiobookshelf_book_item(
    state: &AudiobookshelfBookBrowseState,
    book_id: &str,
) -> Option<AudiobookshelfBookQueueItem> {
    let book = state
        .books
        .iter()
        .find(|book| book.library_item_id == book_id)?;
    if book.library_item_id.trim().is_empty() {
        return None;
    }
    let detail = state.detail_cache.get(&book.library_item_id);
    let duration_seconds = detail
        .map(|(_, files)| files.iter().map(|file| file.duration).sum())
        .filter(|duration| *duration > 0.0)
        .or_else(|| {
            detail.and_then(|(chapters, _)| {
                chapters
                    .iter()
                    .map(|chapter| chapter.end)
                    .max_by(f64::total_cmp)
            })
        });
    let progress = state.progress.get(&book.library_item_id);
    let to_ticks = |seconds: f64| {
        (seconds.is_finite() && seconds >= 0.0)
            .then(|| saturating_i64_from_f64((seconds * TICKS_PER_SECOND_F64).round()))
            .and_then(|ticks| u64::try_from(ticks).ok())
    };
    let is_finished = progress.is_some_and(|progress| progress.is_finished);
    Some(AudiobookshelfBookQueueItem {
        library_item_id: book.library_item_id.clone(),
        title: book.title.clone(),
        author: book.author_display.clone(),
        duration_ticks: duration_seconds.and_then(to_ticks),
        position_ticks: progress
            .and_then(|progress| {
                to_ticks(progress.current_time_seconds).and_then(|ticks| i64::try_from(ticks).ok())
            })
            .unwrap_or(0),
        played: is_finished,
        is_finished,
        cover_path: book.cover_path.clone(),
    })
}

/// Fixed alphabetical author-surname bucket labels for the book tab's
/// pill-filtered browsing (design "Alphabetical surname buckets..."):
/// mirrors `LETTER_FILTER_BUCKETS`' A-Z ranges without its non-alphabetic
/// "#" catch-all, since every surname bucket key (see `surname_bucket_key`)
/// always falls in `'a'..='z'`.
pub(super) const SURNAME_BUCKET_LABELS: [&str; 8] = [
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
pub(super) const SURNAME_BUCKET_UPPER: [char; 8] = ['c', 'f', 'i', 'l', 'o', 'r', 'u', 'z'];

/// One alphabetical author-surname range in the book browser's pill row:
/// its label and the `[start, end)` indices it covers in the
/// surname-sorted `books` list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SurnameBucket {
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
        .find(char::is_ascii_alphabetic)
        .map_or('a', |c| c.to_ascii_lowercase())
}

/// Partitions `books` (already sorted by `author_sort_key`) into the fixed
/// alphabetical ranges above, omitting any range with zero books rather than
/// producing an empty, selectable pill (book-browsing spec: "a range with no
/// books in the current library SHALL NOT produce an empty, selectable
/// bucket"). Pure: no app state, mirrors `music_grouping::build_grouped_album_catalog`'s
/// shape without sharing code — the grouping unit differs (fixed ranges, not
/// runs of identical artist).
#[must_use]
pub fn build_surname_buckets(books: &[AudiobookshelfBook]) -> Vec<SurnameBucket> {
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

/// The staged replacement catalog for one in-flight book refresh: the
/// book-shaped sibling of `ShowCatalogReplacement`. Pages collect here while
/// the published `books` list keeps serving the UI; only a completed
/// traversal publishes, and a failure discards the batch.
#[derive(Debug, Clone, Default)]
pub struct BookCatalogReplacement {
    /// The catalog request serial this batch was started under.
    pub request: u64,
    pub books: Vec<AudiobookshelfBook>,
    pub total: usize,
    pub next_page: usize,
    pub loading_pages: HashSet<usize>,
}

/// Book-shaped browse state: the author-surname-grouped book list, the
/// selected book's chapter/audio-file detail, and book progress keyed by
/// `library_item_id` only. Parallel to `AudiobookshelfBrowseState`; which one
/// a library tab uses is decided once by `AudiobookshelfBrowseKind`.
#[derive(Debug, Clone)]
pub struct AudiobookshelfBookBrowseState {
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
    /// Book ids with a detail request in flight, holding the request serial
    /// each fetch was issued under (the state's monotonically increasing
    /// `next_detail_request`). A response retires — and may write the cache
    /// from — its own serial only, so a superseded pre-refresh response can
    /// neither retire a newer request's mark nor overwrite a newer entry.
    pub detail_loading_ids: HashMap<String, u64>,
    /// The serial issued to the most recent book-detail fetch.
    pub next_detail_request: u64,
    pub detail_loading: bool,
    pub progress: HashMap<String, AudiobookshelfBookProgress>,
    /// Fixed alphabetical author-surname ranges over `books`, recomputed
    /// whenever `books` changes (see `append_page_books`).
    pub buckets: Vec<SurnameBucket>,
    /// The staged replacement catalog, `Some` only while a refresh batch is
    /// collecting; the published `books` list is untouched until
    /// `commit_catalog_replacement`.
    pub replacement: Option<BookCatalogReplacement>,
    /// The serial issued to the most recent catalog request chain.
    pub next_catalog_request: u64,
    /// The catalog request serial the state currently expects: `0` for the
    /// initial load, bumped by `begin_catalog_replacement`.
    pub catalog_request: u64,
}

impl AudiobookshelfBookBrowseState {
    #[must_use]
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
            detail_loading_ids: HashMap::new(),
            next_detail_request: 0,
            detail_loading: false,
            progress: HashMap::new(),
            buckets: Vec::new(),
            replacement: None,
            next_catalog_request: 0,
            catalog_request: 0,
        }
    }

    #[must_use]
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
            .is_some_and(|id| self.detail_loading_ids.contains_key(id));
    }

    #[must_use]
    pub fn selected_book(&self) -> Option<&AudiobookshelfBook> {
        let id = self.selected_id.as_deref()?;
        self.books.iter().find(|book| book.library_item_id == id)
    }

    /// The selected book's chapter rows, falling back to its `audioFiles`
    /// rows when `chapters[]` is empty (book-browsing spec: never an empty
    /// or broken list state).
    #[must_use]
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
                self.detail_loading = self.detail_loading_ids.contains_key(selected_id);
            }
        }
    }

    #[must_use]
    pub fn needs_page(&self) -> Option<usize> {
        (self.books.len() < self.total && self.loading_pages.is_empty()).then_some(self.next_page)
    }

    /// Starts a library-local replacement batch: the published catalog keeps
    /// serving the UI while pages stage separately. Returns the batch's
    /// request serial; the pre-refresh chain is superseded, and page 0 is
    /// marked in flight for the caller's immediate request.
    pub fn begin_catalog_replacement(&mut self) -> u64 {
        self.next_catalog_request += 1;
        self.catalog_request = self.next_catalog_request;
        let mut replacement = BookCatalogReplacement {
            request: self.catalog_request,
            ..BookCatalogReplacement::default()
        };
        replacement.loading_pages.insert(0);
        self.replacement = Some(replacement);
        // The pre-refresh chain's in-flight page marks are superseded: its
        // results are discarded at the event boundary, so a leaked mark would
        // strand the published catalog behind an unretirable `loading_pages`
        // entry if the replacement later aborts (#745).
        self.loading_pages.clear();
        self.catalog_request
    }

    /// Stages a replacement page when `request` is the current chain; a
    /// superseded request is ignored whole. Never touches the published
    /// catalog or its selection. Returns whether it staged.
    pub fn append_replacement_page(
        &mut self,
        request: u64,
        page: usize,
        total: usize,
        books: Vec<AudiobookshelfBook>,
    ) -> bool {
        if self.catalog_request != request {
            return false;
        }
        let Some(replacement) = self.replacement.as_mut() else {
            return false;
        };
        replacement.loading_pages.remove(&page);
        replacement.total = total;
        replacement.next_page = replacement.next_page.max(page + 1);
        for book in books {
            if !replacement
                .books
                .iter()
                .any(|existing| existing.library_item_id == book.library_item_id)
            {
                replacement.books.push(book);
            }
        }
        replacement.books.sort_by(|left, right| {
            left.author_sort_key
                .to_lowercase()
                .cmp(&right.author_sort_key.to_lowercase())
                .then_with(|| left.title.to_lowercase().cmp(&right.title.to_lowercase()))
        });
        true
    }

    /// The next page the active replacement needs, when `request` is current.
    #[must_use]
    pub fn replacement_needs_page(&self, request: u64) -> Option<usize> {
        if self.catalog_request != request {
            return None;
        }
        let replacement = self.replacement.as_ref()?;
        (replacement.books.len() < replacement.total && replacement.loading_pages.is_empty())
            .then_some(replacement.next_page)
    }

    /// Publishes the staged replacement once traversal completed: the
    /// published list becomes the authoritative result, so changed metadata is
    /// updated and deleted books are removed. The selection and surname
    /// buckets are reconciled against that complete result only. Returns
    /// whether a batch was published.
    pub fn commit_catalog_replacement(&mut self, request: u64) -> bool {
        if self.catalog_request != request {
            return false;
        }
        let Some(replacement) = self.replacement.take() else {
            return false;
        };
        self.books = replacement.books;
        self.total = replacement.total;
        self.next_page = replacement.next_page;
        self.loading_pages = replacement.loading_pages;
        self.books.sort_by(|left, right| {
            left.author_sort_key
                .to_lowercase()
                .cmp(&right.author_sort_key.to_lowercase())
                .then_with(|| left.title.to_lowercase().cmp(&right.title.to_lowercase()))
        });
        self.buckets = build_surname_buckets(&self.books);
        let selected_still_present = self
            .selected_id
            .as_deref()
            .is_some_and(|id| self.books.iter().any(|book| book.library_item_id == id));
        if !selected_still_present {
            self.selected_id = self.books.first().map(|book| book.library_item_id.clone());
        }
        self.detail_loading = self
            .selected_id
            .as_ref()
            .is_some_and(|id| self.detail_loading_ids.contains_key(id));
        true
    }

    /// Discards the staged replacement on failure; the published catalog and
    /// its browsing context are untouched. Returns whether a batch was
    /// discarded.
    pub fn abort_catalog_replacement(&mut self, request: u64) -> bool {
        if self.catalog_request != request {
            return false;
        }
        self.replacement.take().is_some()
    }
}

/// One renderable row beneath the book hero: a chapter (absolute seekable
/// range on the merged timeline) or an audio file (used when chapters are
/// absent). Both carry provider-native identity; neither is an episode shape.
#[derive(Debug, Clone, PartialEq)]
pub enum BookRow {
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
