//! The GUI Library's Audiobooks projection (add-gui-library-panel design
//! D12, row 9.1): the flat book list in the TUI's sort order — author sort
//! key, then title — with the A-Z rail over the author sort key, the core's
//! selected book, and the book Hero: portrait cover art, the TUI's facts
//! (title, author, duration, `Finished`/`N%`), and the chapters as the
//! Hero's Workspace, with `AudioFile` `Part N` rows when the book holds no
//! chapters. The projection reads `App` data only, and its row, label, and
//! now-playing rules are the TUI's own (`mbv_ui_model::audiobookshelf_browse::book_rows`,
//! `book_content::chapter_rows`, `hero_content_abs_book`), so the two
//! Clients cannot drift.

use mbv_emby_model::saturating_i64_from_f64;
use mbv_emby_model::seconds_to_ticks;
use mbv_ui_model::audiobookshelf_browse::BookRow;
use mbv_ui_model::audiobookshelf_browse::books::audiobookshelf_book_item;
use mbv_ui_model::library_hero::ArtworkSource;
use mbv_ui_model::library_hero::producers::hero_content_abs_book;
use mbv_ui_model::ui_util::fmt_duration_gutter;

use super::GuiLibraryArtwork;
use super::GuiListSort;
use super::GuiListSortFrame;
use super::GuiSortTarget;
use super::artwork::{GuiRowArt, GuiRowMediaKind, catalog_book_row_source, row_art};
use super::home::{GuiArtShape, artwork_shape};
use super::list_sort::gui_title_sort_key;
use crate::gui::GuiCore;
use mbv_client_core::App;
use mbv_ui_model::meta::{MetaKind, MetaSegment};

/// The active destination's Audiobooks library index (row 9.1): the
/// destination whose book list the GUI Library panel paints. `None` for every
/// other destination, which presents no Audiobooks section.
#[must_use]
pub(crate) fn audiobooks_destination(app: &App) -> Option<usize> {
    match super::tabs::active_destination(app) {
        Some(super::GuiDestinationKind::Audiobooks { lib_idx }) => Some(lib_idx),
        _ => None,
    }
}

/// One row of the book list (row 9.1). The identity is the book's stable
/// `library_item_id` a select or play command sends, never a row position.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiAudiobooksRow {
    /// The book's stable `library_item_id`.
    pub id: String,
    /// The book's display title, the TUI row's primary.
    pub title: String,
    /// The author display credit, the TUI row's secondary.
    pub subtitle: Option<String>,
    /// The book's total duration, the TUI row's duration gutter.
    pub duration: String,
    /// The row's rail letter (design D6): the first letter of the key the
    /// list's sort compares — the title under Title, the author sort key
    /// under Author — or `#` for a digit- or symbol-led key. `None` under a
    /// date sort, which shows no rail.
    pub sort_letter: Option<char>,
    /// The row's art (gui-delineated-rows design D4).
    pub art: GuiRowArt,
    /// The media kind the row shows when it has no image (design D4).
    pub media: GuiRowMediaKind,
}

/// The book Hero's facts (row 9.1): the shared `hero_content_abs_book`
/// producer's facts — the same meta rows the TUI's hero paints — the
/// portrait cover artwork, and the chapters as the Hero's Workspace. The
/// producer carries no narrator, series, or description, and the design
/// adds none.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiAudiobooksHero {
    /// The hero's title.
    pub title: String,
    /// The meta rows in display order: the author, the duration, and the
    /// `Finished` or `N%` progress row.
    pub meta_rows: Vec<Vec<MetaSegment>>,
    /// Which meta row paints the duration role, when one does.
    pub duration_row: Option<usize>,
    /// Which meta row paints the progress role, when one does.
    pub progress_row: Option<usize>,
    /// The cleaned description, when the catalog record carries one: the
    /// overview the TUI's Hero paints, which the GUI dropped before D15.
    pub overview: Option<String>,
    /// The artwork box the Hero reserves: the book artwork policy's
    /// portrait.
    pub art_shape: GuiArtShape,
    /// The Hero's cover artwork, fetched through the Service-scoped book
    /// cover chain. `None` when the book declares no cover or no server is
    /// configured; the window content paints its own placeholder there.
    pub art: Option<GuiLibraryArtwork>,
    /// The book's chapter rows; `None` while the book's detail has not
    /// landed, which the projection's `workspace_loading` names.
    pub workspace: Option<GuiAudiobooksWorkspace>,
}

/// The book Hero's Workspace (row 9.1): the book's chapter rows — or its
/// `AudioFile` `Part N` rows when the book holds no chapters — in the TUI's
/// `visible_rows` order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiAudiobooksWorkspace {
    /// The chapter rows in the TUI's `visible_rows` order.
    pub rows: Vec<GuiAudiobooksChapterRow>,
}

/// One chapter row of the book Hero's Workspace (row 9.1): the TUI's
/// chapter-row projection (`book_content::chapter_rows`). The chapter value
/// is the stable Service discriminator the TUI's chapter activation
/// addresses — the chapter number on a chapter row, the audio-part index on
/// a `Part N` row — never a display position. The current-chapter mark is
/// not here (design D5): the rows carry the chapter's span instead, and the
/// window marks the row whose span holds the playhead position while the
/// Hero's book is the playing book.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiAudiobooksChapterRow {
    /// The row's stable Service discriminator.
    pub chapter: usize,
    /// The row's label: the chapter's title, or `Part N`.
    pub title: String,
    /// The row's duration, the TUI's duration gutter; empty when unknown.
    pub duration: String,
    /// The chapter's start on the book's merged timeline, in ticks. An
    /// audio-part row carries no span — an empty one never holds the
    /// position — as the TUI's part rows never light.
    pub start_ticks: i64,
    /// The chapter's end on the book's merged timeline, in ticks.
    pub end_ticks: i64,
}

/// The Audiobooks projection the GUI Library panel paints (row 9.1): the
/// flat book list, the core's selected book, and its Hero. No pill rides
/// here — the letter rail replaces the TUI's eight surname buckets, as it
/// does for Shows and the plain Emby libraries.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GuiAudiobooksProjection {
    /// Whether the book catalog has not landed yet: the initial load's page
    /// fetches are still running. A replacement batch's pages keep the
    /// published catalog serving, as the TUI's list does.
    pub loading: bool,
    /// Whether the catalog fetch failed, the browse state's error. The
    /// published catalog keeps serving when it can.
    pub failed: bool,
    /// The catalog's books in the list's sort order: the catalog's own order
    /// (author sort key, then title) under Author, the title's under Title,
    /// and the date added's under Newest and Oldest.
    pub rows: Vec<GuiAudiobooksRow>,
    /// The list's sort control (design D7): the target a press names, its
    /// active sort, and the choices the book library offers.
    pub sort: Option<GuiListSortFrame>,
    /// The core's selected book (row 9.1): the pick the select command last
    /// named while the catalog still carries it, else the catalog's first
    /// book — the Hero default. `None` while the catalog holds no book.
    pub selected: Option<String>,
    /// The selected book's Hero facts; `None` while no book is selected.
    pub hero: Option<GuiAudiobooksHero>,
    /// Whether the selected book's detail fetch has not landed yet: the
    /// Workspace's loading state.
    pub workspace_loading: bool,
    /// The Audiobooks book playing at the playhead (design D5, D3 rule 4):
    /// its stable `library_item_id`, the same fact the playhead marks sample.
    /// The window marks the chapter row whose span holds the position while
    /// the Hero's book is this one. `None` while no book plays or the active
    /// slot is pending.
    pub playing_book_id: Option<String>,
}

impl GuiCore {
    /// The Audiobooks projection the GUI Library panel paints (row 9.1):
    /// the flat book list, the selected book, and its Hero. A `lib_idx`
    /// that names no book library projects the empty projection, as the
    /// frame's gate excludes that destination anyway.
    #[must_use]
    pub fn gui_audiobooks_projection(&mut self, lib_idx: usize) -> GuiAudiobooksProjection {
        if !super::audiobooks::abs_book_library(&self.app, lib_idx) {
            return GuiAudiobooksProjection::default();
        }
        let Some(state) = self.app.audiobookshelf_book_browse_state(lib_idx) else {
            return GuiAudiobooksProjection {
                loading: true,
                ..GuiAudiobooksProjection::default()
            };
        };
        let loading = !state.loading_pages.is_empty();
        let failed = state.error.is_some();
        let sort_frame = super::list_sort::list_sort_frame(
            &self.app,
            &self.gui,
            GuiSortTarget::Audiobooks(lib_idx),
        );
        let sort = sort_frame.active;
        let mut books: Vec<&mbv_audiobookshelf::AudiobookshelfBook> = state.books.iter().collect();
        // Under Author the catalog's own order stands: it is already the
        // author sort key, then the title (design D6). Under Title the title
        // leads and the author sort key follows it.
        match sort {
            GuiListSort::Author => {}
            GuiListSort::Title => books.sort_by_cached_key(|book| {
                (
                    gui_title_sort_key(&book.title),
                    gui_title_sort_key(&book.author_sort_key),
                    book.library_item_id.clone(),
                )
            }),
            GuiListSort::Newest | GuiListSort::Oldest => {
                books.sort_by(|left, right| {
                    sort.compare_date(book_added_secs(left), book_added_secs(right))
                        .then_with(|| left.library_item_id.cmp(&right.library_item_id))
                });
            }
        }
        let rows: Vec<GuiAudiobooksRow> = books
            .into_iter()
            .map(|book| book_row(&self.app, book, sort))
            .collect();
        let selected = super::audiobooks::abs_effective_pick(&self.app, &self.gui, lib_idx)
            .filter(|pick| rows.iter().any(|row| &row.id == pick));
        let workspace_loading = selected
            .as_deref()
            .is_some_and(|pick| state.detail_loading_ids.contains_key(pick));
        let hero = selected
            .as_deref()
            .and_then(|pick| self.abs_hero(lib_idx, pick));
        GuiAudiobooksProjection {
            loading,
            failed,
            rows,
            sort: Some(sort_frame),
            selected,
            hero,
            workspace_loading,
            playing_book_id: crate::gui::gui_library::marks::playing_book_id(&self.app),
        }
    }

    /// The selected book's Hero (row 9.1): the TUI's own facts — the shared
    /// book producer's title, duration, and progress rows, with the
    /// catalog-only `Read by <narrator>` and published-year rows and the
    /// description overview restored, exactly as `book_content`'s
    /// `resolved_hero_data` builds them (design D15) — the cover fetched
    /// through the Service-scoped book cover chain, and the Workspace the
    /// book's detail holds.
    fn abs_hero(&mut self, lib_idx: usize, book_id: &str) -> Option<GuiAudiobooksHero> {
        let state = self.app.audiobookshelf_book_browse_state(lib_idx)?;
        let book = audiobookshelf_book_item(state, book_id)?;
        // The catalog's own record beside the queue item: the narrator, the
        // published year, and the description ride only there.
        let catalog = state
            .books
            .iter()
            .find(|candidate| candidate.library_item_id == book_id);
        let narrator = catalog
            .and_then(|book| book.narrator.clone())
            .filter(|value| !value.is_empty());
        let year = catalog
            .and_then(|book| book.published_year.clone())
            .filter(|value| !value.is_empty());
        let description = catalog.and_then(|book| book.description.clone());
        // The shared book producer's facts, with the catalog-only rows the
        // TUI's Hero restores: `Read by <narrator>`, the published year, and
        // the description overview.
        let mut content = hero_content_abs_book(&book);
        if let Some(narrator) = narrator {
            content
                .facts
                .meta_rows
                .push(vec![MetaSegment::plain(format!("Read by {narrator}"))]);
        }
        if let Some(year) = year {
            content
                .facts
                .meta_rows
                .push(vec![MetaSegment::new(year, MetaKind::Date)]);
        }
        content.overview = description
            .as_deref()
            .map(mbv_ui_model::ui_util::clean_overview)
            .filter(|overview| !overview.is_empty());
        let art = match &content.facts.artwork.source {
            Some(ArtworkSource::AudiobookshelfCover {
                library_item_id,
                book: true,
            }) => self.abs_hero_artwork(library_item_id),
            Some(_) | None => None,
        };
        let workspace = self.abs_workspace(lib_idx, book_id);
        Some(GuiAudiobooksHero {
            title: content.facts.title.clone(),
            meta_rows: content.facts.meta_rows.clone(),
            duration_row: content.facts.duration_row,
            progress_row: content.facts.progress_row,
            overview: content.overview.clone(),
            art_shape: artwork_shape(super::artwork::hero_shape(
                &self.app,
                content.facts.artwork.shape,
                art.as_ref(),
            )),
            art,
            workspace,
        })
    }

    /// The book Hero's cover artwork (row 9.1): the Service-scoped
    /// hero-scoped book-cover cache key, fetched here and encoded from the
    /// image cache once per key — the same resolution the other projections'
    /// hero art runs. `None` when no Audiobookshelf server is configured.
    fn abs_hero_artwork(&mut self, library_item_id: &str) -> Option<GuiLibraryArtwork> {
        let cache_key = self.app.audiobookshelf_book_cover_key(library_item_id)?;
        Some(super::artwork::resolve_gui_art(&self.app, &cache_key))
    }

    /// The book Hero's Workspace (row 9.1): the book's chapter rows in the
    /// TUI's `visible_rows` order, the `Part N` fallback over the audio
    /// files when the book holds no chapters. `None` while the book's
    /// detail has not landed.
    fn abs_workspace(&self, lib_idx: usize, book_id: &str) -> Option<GuiAudiobooksWorkspace> {
        let state = self.app.audiobookshelf_book_browse_state(lib_idx)?;
        let rows = state
            .visible_rows(book_id)
            .into_iter()
            .map(chapter_row)
            .collect();
        Some(GuiAudiobooksWorkspace { rows })
    }
}

/// One row of the book list (row 9.1): the TUI book row's labels — the
/// title primary, the author credit secondary, the duration gutter — with
/// the rail letter over the key the list's sort compares (design D6), and
/// the row art the book artwork policy's cover names (gui-delineated-rows
/// design D5), read from the cache alone.
fn book_row(
    app: &App,
    book: &mbv_audiobookshelf::AudiobookshelfBook,
    sort: GuiListSort,
) -> GuiAudiobooksRow {
    GuiAudiobooksRow {
        id: book.library_item_id.clone(),
        title: book.title.clone(),
        subtitle: book
            .author_display
            .clone()
            .filter(|author| !author.is_empty()),
        duration: gutter(book.duration_seconds),
        sort_letter: book_row_letter(book, sort),
        art: row_art(app, catalog_book_row_source(book).as_ref()),
        media: GuiRowMediaKind::Audio,
    }
}

/// The rail letter a book row carries (design D6): the first letter of the
/// key the list's sort compares — the title under Title, the author sort key
/// under Author, whose surname the TUI's buckets group by — and no letter
/// under a date sort.
fn book_row_letter(
    book: &mbv_audiobookshelf::AudiobookshelfBook,
    sort: GuiListSort,
) -> Option<char> {
    match sort {
        GuiListSort::Author => sort.rail_letter(&book.author_sort_key),
        GuiListSort::Title | GuiListSort::Newest | GuiListSort::Oldest => {
            sort.rail_letter(&book.title)
        }
    }
}

/// The book's added instant in seconds, the date sorts' key (design D6, D9):
/// the catalog's `addedAt` milliseconds, so a book with no added instant
/// sorts last in both date sorts.
fn book_added_secs(book: &mbv_audiobookshelf::AudiobookshelfBook) -> Option<u64> {
    book.added_at_ms.map(|ms| ms / 1_000)
}

/// One chapter row (row 9.1): the TUI chapter-row projection's labels — the
/// chapter title or the `Part N` fallback — with the duration gutter and the
/// chapter's span on the book's merged timeline (design D5). An audio-part
/// row carries no span, as the TUI's rows never light on one.
fn chapter_row(row: BookRow) -> GuiAudiobooksChapterRow {
    match row {
        BookRow::Chapter {
            id,
            start,
            end,
            title,
        } => GuiAudiobooksChapterRow {
            chapter: id,
            title,
            // The chapter's own span, the TUI chapter row's duration: `end`
            // is the chapter's absolute end on the merged timeline.
            duration: gutter(end - start),
            start_ticks: seconds_to_ticks(start),
            end_ticks: seconds_to_ticks(end),
        },
        BookRow::AudioFile { index, duration } => GuiAudiobooksChapterRow {
            chapter: index,
            title: format!("Part {index}"),
            duration: gutter(duration),
            start_ticks: 0,
            end_ticks: 0,
        },
    }
}

/// The TUI's duration gutter for `seconds`: truncated to whole seconds, the
/// same rule the TUI's book and chapter rows apply.
fn gutter(seconds: f64) -> String {
    if seconds > 0.0 {
        fmt_duration_gutter(saturating_i64_from_f64(seconds).max(0))
    } else {
        String::new()
    }
}
