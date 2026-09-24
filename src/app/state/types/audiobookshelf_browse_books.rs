//! Book-tab browse state: the author-surname-grouped book list, the
//! selected book's chapter/audio-file detail, and book progress.

use mbv_core::audiobookshelf::{
    AudiobookshelfAudioFile, AudiobookshelfBook, AudiobookshelfBookProgress, AudiobookshelfChapter,
    AudiobookshelfLibrary,
};
use std::collections::{HashMap, HashSet};

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
pub(in crate::app) struct SurnameBucket {
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
pub(in crate::app) fn build_surname_buckets(books: &[AudiobookshelfBook]) -> Vec<SurnameBucket> {
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
pub(in crate::app) struct AudiobookshelfBookBrowseState {
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
pub(in crate::app) enum BookRow {
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
