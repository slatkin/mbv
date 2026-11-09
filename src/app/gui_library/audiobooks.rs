//! The GUI Library's Audiobooks data path (add-gui-library-panel design D12,
//! row 9.1): the per-book-library selection the core owns, and the open,
//! select, and ensure actions the later core surface's commands apply. The
//! book catalog, its per-book detail, and the book progress already land
//! App-side — the catalog through the shared startup's fetch chain and the
//! lib events' own handlers, the progress through the Player event drain —
//! so this module only names the pick and triggers the loads the TUI's own
//! loaders run. The TUI never reads any of it.

use mbv_ui_model::audiobookshelf_browse::AudiobookshelfBrowseKind;

use crate::app::App;

/// The GUI Library's Audiobooks selection (row 9.1): per book library, the
/// book the Hero shows, by stable `library_item_id`. The TUI's own resting
/// selection is the browse state's `selected_id`, which this store never
/// writes, so the two Clients' selections stay independent; a stored book the
/// published catalog no longer carries falls back to the catalog's first book
/// in the projection, as every selection's stale arm does.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GuiAudiobooksSelection {
    /// The book the Hero shows. `None` before a select command names one.
    pub picked_book: Option<String>,
}

impl App {
    /// Whether `lib_idx` names an Audiobooks destination (row 9.1): every GUI
    /// Audiobooks method's destination guard — an Audiobookshelf library whose
    /// `media_type` resolves to the book browse kind. A podcast library stays
    /// unpresented until row 10.1, so it is no Audiobooks destination.
    pub(in crate::app::gui_library) fn gui_abs_book_library(&self, lib_idx: usize) -> bool {
        self.audiobookshelf_libraries
            .get(lib_idx)
            .is_some_and(|library| {
                AudiobookshelfBrowseKind::from_media_type(&library.media_type)
                    == AudiobookshelfBrowseKind::Book
            })
    }

    /// Open the Audiobooks destination `lib_idx` (row 9.1): a book catalog the
    /// App's data holds unloaded starts the TUI's own book catalog load — the
    /// refresh path the F5 press runs, whose replacement batch is
    /// single-flight and whose completion publishes the whole catalog. A
    /// catalog already loading, loaded, or owned by the startup's own fetch
    /// chain loads nothing, so no second chain ever walks beside the first.
    /// A `lib_idx` that names no book library is an ignored command.
    pub fn gui_abs_open(&mut self, lib_idx: usize) {
        if !self.gui_abs_book_library(lib_idx) {
            Self::log_ignored_library_command(
                "absOpen",
                format_args!("the destination is no Audiobooks library"),
            );
            return;
        }
        let unloaded = self
            .audiobookshelf_book_browse
            .get(lib_idx)
            .is_some_and(|state| {
                state.books.is_empty() && state.total == 0 && state.replacement.is_none()
            });
        if !unloaded || self.tab.audiobookshelf_index() != Some(lib_idx) {
            return;
        }
        self.audiobookshelf_book_refresh();
    }

    /// Select the book the Hero shows (row 9.1): the selection the core owns,
    /// so the frame carries it back, and the per-book detail and chapters load
    /// through the TUI's own loader. A book the published catalog does not
    /// carry is a stale frame's pick and changes nothing, as every
    /// item-named command's stale arm does; a catalog still landing admits
    /// the pick, because the list is what would name it stale.
    pub fn gui_abs_select(&mut self, lib_idx: usize, book_id: &str) {
        if !self.gui_abs_book_library(lib_idx) {
            Self::log_ignored_library_command(
                "absSelect",
                format_args!("the destination is no Audiobooks library"),
            );
            return;
        }
        if !self.gui_abs_book_known(lib_idx, book_id) {
            Self::log_stale_library_item("absSelect", book_id);
            return;
        }
        self.gui_abs_libs.entry(lib_idx).or_default().picked_book = Some(book_id.to_string());
        // The loader reads the active tab's own browse state, so the load
        // starts for the destination that is open; the frame path's ensure
        // covers the pick a restore or a late select left unloaded.
        if self.tab.audiobookshelf_index() == Some(lib_idx) {
            self.start_audiobookshelf_book_detail(book_id.to_string());
        }
    }

    /// The frame-path trigger the GUI's frame runs for the active Audiobooks
    /// destination (row 9.1), the Shows frame path's counterpart: the
    /// effective pick's detail load starts when the cache does not hold it
    /// and nothing is fetching it, so the default first book's Hero paints
    /// its chapters without a select. A `lib_idx` that names no book library
    /// is a no-op, so the frame path can call it unguarded for its
    /// destination.
    pub fn ensure_gui_abs_selection(&mut self, lib_idx: usize) {
        if !self.gui_abs_book_library(lib_idx) {
            return;
        }
        if let Some(book_id) = self.gui_abs_effective_pick(lib_idx)
            && self.tab.audiobookshelf_index() == Some(lib_idx)
        {
            self.start_audiobookshelf_book_detail(book_id);
        }
    }

    /// The book the Hero shows (row 9.1): the stored pick while the published
    /// catalog still carries it, else the catalog's first book — the Hero
    /// default, so the Hero paints as soon as the catalog lands. `None` while
    /// the catalog holds no book.
    pub(in crate::app) fn gui_abs_effective_pick(&self, lib_idx: usize) -> Option<String> {
        let state = self.audiobookshelf_book_browse.get(lib_idx)?;
        self.gui_abs_libs
            .get(&lib_idx)
            .and_then(|selection| selection.picked_book.as_deref())
            .filter(|picked| {
                state
                    .books
                    .iter()
                    .any(|book| &book.library_item_id == picked)
            })
            .or_else(|| {
                state
                    .books
                    .first()
                    .map(|book| book.library_item_id.as_str())
            })
            .map(str::to_string)
    }

    /// Whether the published catalog can still name `book_id` (row 9.1): a
    /// catalog that holds no book admits the pick — it is still landing, and
    /// the list is what would name the pick stale — and a loaded catalog
    /// admits only a book it carries.
    fn gui_abs_book_known(&self, lib_idx: usize, book_id: &str) -> bool {
        let Some(state) = self.audiobookshelf_book_browse.get(lib_idx) else {
            return false;
        };
        state.books.is_empty()
            || state
                .books
                .iter()
                .any(|book| book.library_item_id == book_id)
    }
}
