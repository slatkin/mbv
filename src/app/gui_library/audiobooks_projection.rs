//! The GUI Library's Audiobooks projection (add-gui-library-panel design
//! D12, row 9.1): the flat book list in the TUI's sort order — author sort
//! key, then title — with the A-Z rail over the author sort key, the core's
//! selected book, and the book Hero: portrait cover art, the TUI's facts
//! (title, author, duration, `Finished`/`N%`), and the chapters as the
//! Hero's Workspace, with `AudioFile` `Part N` rows when the book holds no
//! chapters. The projection reads `App` data only, and its row, label, and
//! now-playing rules are the TUI's own (`mbv_render::book_rows`,
//! `book_content::chapter_rows`, `hero_content_abs_book`), so the two
//! Clients cannot drift.

use mbv_components::library_panel::content::ArtworkSource;
use mbv_components::library_panel::hero::hero_content_abs_book;
use mbv_emby_model::saturating_i64_from_f64;
use mbv_ui_model::audiobookshelf_browse::BookRow;
use mbv_ui_model::audiobookshelf_browse::books::audiobookshelf_book_item;
use mbv_ui_model::now_playing::WorkspaceNowPlaying;
use mbv_ui_model::ui_util::fmt_duration_gutter;

use super::GuiLibraryArtwork;
use super::home::{GuiArtShape, artwork_shape};
use super::projection::sort_letter;
use crate::app::App;
use crate::app::shell::now_playing::derive_workspace_now_playing;

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
    /// The row's rail letter (design D12): the first letter of the author
    /// sort key — the surname the list sorts by — or `#` for a digit- or
    /// symbol-led surname.
    pub sort_letter: char,
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
    pub meta_rows: Vec<String>,
    /// Which meta row paints the duration role, when one does.
    pub duration_row: Option<usize>,
    /// Which meta row paints the progress role, when one does.
    pub progress_row: Option<usize>,
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
/// a `Part N` row — never a display position.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiAudiobooksChapterRow {
    /// The row's stable Service discriminator.
    pub chapter: usize,
    /// The row's label: the chapter's title, or `Part N`.
    pub title: String,
    /// The row's duration, the TUI's duration gutter; empty when unknown.
    pub duration: String,
    /// Whether this chapter is playing: the Owner's active item is this
    /// book and the playback position sits in this chapter, derived exactly
    /// as the TUI's `WorkspaceNowPlaying::Chapter` projection derives it.
    /// An audio-part row never carries it, as the TUI's rows never light.
    pub current: bool,
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
    /// The catalog's books in the TUI's order: author sort key, then title.
    pub rows: Vec<GuiAudiobooksRow>,
    /// The core's selected book (row 9.1): the pick the select command last
    /// named while the catalog still carries it, else the catalog's first
    /// book — the Hero default. `None` while the catalog holds no book.
    pub selected: Option<String>,
    /// The selected book's Hero facts; `None` while no book is selected.
    pub hero: Option<GuiAudiobooksHero>,
    /// Whether the selected book's detail fetch has not landed yet: the
    /// Workspace's loading state.
    pub workspace_loading: bool,
}

impl App {
    /// The active destination's Audiobooks library index (row 9.1): the
    /// destination whose book list the GUI Library panel paints. `None` for
    /// every other destination, which presents no Audiobooks section.
    #[must_use]
    pub fn gui_audiobooks_destination(&self) -> Option<usize> {
        match self.gui_active_destination() {
            Some(super::GuiDestinationKind::Audiobooks { lib_idx }) => Some(lib_idx),
            _ => None,
        }
    }

    /// The Audiobooks projection the GUI Library panel paints (row 9.1):
    /// the flat book list, the selected book, and its Hero. A `lib_idx`
    /// that names no book library projects the empty projection, as the
    /// frame's gate excludes that destination anyway.
    #[must_use]
    pub fn gui_audiobooks_projection(&mut self, lib_idx: usize) -> GuiAudiobooksProjection {
        if !self.gui_abs_book_library(lib_idx) {
            return GuiAudiobooksProjection::default();
        }
        let now_playing = derive_workspace_now_playing(self);
        let Some(state) = self.audiobookshelf_book_browse.get(lib_idx) else {
            return GuiAudiobooksProjection {
                loading: true,
                ..GuiAudiobooksProjection::default()
            };
        };
        let loading = !state.loading_pages.is_empty();
        let failed = state.error.is_some();
        let rows: Vec<GuiAudiobooksRow> = state.books.iter().map(book_row).collect();
        let selected = self
            .gui_abs_effective_pick(lib_idx)
            .filter(|pick| rows.iter().any(|row| &row.id == pick));
        let workspace_loading = selected
            .as_deref()
            .is_some_and(|pick| state.detail_loading_ids.contains_key(pick));
        let hero = selected
            .as_deref()
            .and_then(|pick| self.gui_abs_hero(lib_idx, pick, now_playing.as_ref()));
        GuiAudiobooksProjection {
            loading,
            failed,
            rows,
            selected,
            hero,
            workspace_loading,
        }
    }

    /// The selected book's Hero (row 9.1): the shared `hero_content_abs_book`
    /// producer's facts, the cover fetched through the Service-scoped book
    /// cover chain — the same fetch the TUI's hero projection issues — and
    /// the Workspace the book's detail holds.
    fn gui_abs_hero(
        &mut self,
        lib_idx: usize,
        book_id: &str,
        now_playing: Option<&WorkspaceNowPlaying>,
    ) -> Option<GuiAudiobooksHero> {
        let book = self
            .audiobookshelf_book_browse
            .get(lib_idx)
            .and_then(|state| audiobookshelf_book_item(state, book_id))?;
        let content = hero_content_abs_book(&book);
        let art = match &content.facts.artwork.source {
            Some(ArtworkSource::AudiobookshelfCover {
                library_item_id,
                book: true,
            }) => self.gui_abs_hero_artwork(library_item_id),
            Some(_) | None => None,
        };
        let workspace = self.gui_abs_workspace(lib_idx, book_id, now_playing);
        Some(GuiAudiobooksHero {
            title: content.facts.title.clone(),
            meta_rows: content.facts.meta_rows.clone(),
            duration_row: content.facts.duration_row,
            progress_row: content.facts.progress_row,
            art_shape: artwork_shape(content.facts.artwork.shape),
            art,
            workspace,
        })
    }

    /// The book Hero's cover artwork (row 9.1): the Service-scoped
    /// hero-scoped book-cover cache key, fetched here and encoded from the
    /// image cache once per key — the same resolution the other projections'
    /// hero art runs. `None` when no Audiobookshelf server is configured.
    fn gui_abs_hero_artwork(&mut self, library_item_id: &str) -> Option<GuiLibraryArtwork> {
        let cache_key = self.audiobookshelf_book_cover_key(library_item_id)?;
        Some(self.resolve_gui_home_art(&cache_key))
    }

    /// The book Hero's Workspace (row 9.1): the book's chapter rows in the
    /// TUI's `visible_rows` order, the `Part N` fallback over the audio
    /// files when the book holds no chapters. `None` while the book's
    /// detail has not landed.
    fn gui_abs_workspace(
        &self,
        lib_idx: usize,
        book_id: &str,
        now_playing: Option<&WorkspaceNowPlaying>,
    ) -> Option<GuiAudiobooksWorkspace> {
        let state = self.audiobookshelf_book_browse.get(lib_idx)?;
        let rows = state
            .visible_rows(book_id)
            .into_iter()
            .map(|row| chapter_row(row, book_id, now_playing))
            .collect();
        Some(GuiAudiobooksWorkspace { rows })
    }
}

/// One row of the book list (row 9.1): the TUI book row's labels — the
/// title primary, the author credit secondary, the duration gutter — with
/// the rail letter over the author sort key the list sorts by.
fn book_row(book: &mbv_audiobookshelf::AudiobookshelfBook) -> GuiAudiobooksRow {
    GuiAudiobooksRow {
        id: book.library_item_id.clone(),
        title: book.title.clone(),
        subtitle: book
            .author_display
            .clone()
            .filter(|author| !author.is_empty()),
        duration: gutter(book.duration_seconds),
        sort_letter: sort_letter(&book.author_sort_key),
    }
}

/// One chapter row (row 9.1): the TUI chapter-row projection's labels — the
/// chapter title or the `Part N` fallback — with the duration gutter and
/// the current-chapter mark. Only a chapter row can carry the mark, as the
/// TUI's rows never light on an audio part.
fn chapter_row(
    row: BookRow,
    book_id: &str,
    now_playing: Option<&WorkspaceNowPlaying>,
) -> GuiAudiobooksChapterRow {
    let current = matches!(
        now_playing,
        Some(WorkspaceNowPlaying::Chapter {
            library_item_id,
            chapter_id,
            ..
        }) if library_item_id == book_id
    );
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
            current: current
                && matches!(now_playing, Some(WorkspaceNowPlaying::Chapter { chapter_id: playing, .. }) if *playing == id),
        },
        BookRow::AudioFile { index, duration } => GuiAudiobooksChapterRow {
            chapter: index,
            title: format!("Part {index}"),
            duration: gutter(duration),
            current: false,
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
