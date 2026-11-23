//! The GUI Library's Audiobooks data path (add-gui-library-panel design D12,
//! row 9.1): the per-book-library selection the core owns, and the open,
//! select, and ensure actions the later core surface's commands apply. The
//! book catalog, its per-book detail, and the book progress already land
//! App-side — the catalog through the shared startup's fetch chain and the
//! lib events' own handlers, the progress through the Player event drain —
//! so this module only names the pick and triggers the loads the TUI's own
//! loaders run. The TUI never reads any of it.
//!
//! Every step is a free function over the `App` the GUI core owns, with the
//! `GuiCore` entries as one-line calls (design D4), so the load sites, the
//! projections, and the Podcasts work reach them through the `App` they hold.

use mbv_queue::QueueItem;
use mbv_ui_model::audiobookshelf_browse::AudiobookshelfBrowseKind;
use mbv_ui_model::audiobookshelf_browse::books::audiobookshelf_book_queue_item_for_id;

use super::ensure::catalog_unloaded;
use super::entry_list::picked_or_first;
use super::tabs::GuiDestinationKind;
use crate::app::App;
use crate::app::dispatch::notify::ToastSeverity;
use crate::app::gui_client_state::GuiClientState;
use crate::gui::GuiCore;

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

impl GuiCore {
    /// Open the Audiobooks library `lib_idx` (row 9.1; gui-one-destination-ensure
    /// design D4).
    pub fn gui_abs_open(&mut self, lib_idx: usize) {
        open_abs(&mut self.app, &mut self.gui, lib_idx);
    }

    /// Select the book the Hero shows (row 9.1).
    pub fn gui_abs_select(&mut self, lib_idx: usize, book_id: &str) {
        select_abs(&mut self.app, &mut self.gui, lib_idx, book_id);
    }

    /// Play the book a command names (row 9.1).
    pub(in crate::app::gui_library) fn play_gui_abs_book(&mut self, lib_idx: usize, book_id: &str) {
        play_abs_book(&mut self.app, lib_idx, book_id);
    }

    /// Play or seek from the chapter a command names (row 9.1).
    pub(in crate::app::gui_library) fn play_gui_abs_chapter(
        &mut self,
        lib_idx: usize,
        book_id: &str,
        chapter_id: usize,
    ) {
        play_abs_chapter(&mut self.app, lib_idx, book_id, chapter_id);
    }

    /// Add the book a command names to the queue (row 9.1).
    pub(in crate::app::gui_library) fn enqueue_gui_abs_book(
        &mut self,
        lib_idx: usize,
        book_id: &str,
    ) {
        enqueue_abs_book(&mut self.app, lib_idx, book_id);
    }
}

/// Whether `lib_idx` names an Audiobooks destination (row 9.1): every GUI
/// Audiobooks step's destination guard — an Audiobookshelf library whose
/// `media_type` resolves to the book browse kind. A podcast library stays
/// unpresented until row 10.1, so it is no Audiobooks destination.
pub(in crate::app::gui_library) fn abs_book_library(app: &App, lib_idx: usize) -> bool {
    app.audiobookshelf_libraries
        .get(lib_idx)
        .is_some_and(|library| {
            AudiobookshelfBrowseKind::from_media_type(&library.media_type)
                == AudiobookshelfBrowseKind::Book
        })
}

/// Open the Audiobooks library `lib_idx` (row 9.1; gui-one-destination-ensure
/// design D4): the open clears a failed catalog mark, because a failed catalog
/// retries on an open and not on a repaint, and then runs the destination's
/// load pass. A `lib_idx` that names no Audiobooks library is an ignored
/// command.
pub(in crate::app::gui_library) fn open_abs(
    app: &mut App,
    gui: &mut GuiClientState,
    lib_idx: usize,
) {
    if !abs_book_library(app, lib_idx) {
        super::log_ignored_library_command(
            "absOpen",
            format_args!("the destination is no Audiobooks library"),
        );
        return;
    }
    clear_abs_catalog_failed(app, lib_idx);
    // The ordered load pass is the GUI core's, so the open step hands the
    // core the App it holds.
    let mut core = GuiCore::new(std::mem::replace(app, crate::app::handoff_app()));
    *core.gui_state_mut() = std::mem::take(gui);
    core.ensure_gui_destination(GuiDestinationKind::Audiobooks { lib_idx });
    *gui = std::mem::take(core.gui_state_mut());
    *app = core.into_app();
}

/// Select the book the Hero shows (row 9.1): the selection the core owns, so
/// the frame carries it back, and the per-book detail and chapters load
/// through the TUI's own loader. A book the published catalog does not carry
/// is a stale frame's pick and changes nothing, as every item-named command's
/// stale arm does; a catalog still landing admits the pick, because the list
/// is what would name it stale.
pub(in crate::app::gui_library) fn select_abs(
    app: &mut App,
    gui: &mut GuiClientState,
    lib_idx: usize,
    book_id: &str,
) {
    if !abs_book_library(app, lib_idx) {
        super::log_ignored_library_command(
            "absSelect",
            format_args!("the destination is no Audiobooks library"),
        );
        return;
    }
    if !abs_book_known(app, lib_idx, book_id) {
        super::log_stale_library_item("absSelect", book_id);
        return;
    }
    gui.abs_libs.entry(lib_idx).or_default().picked_book = Some(book_id.to_string());
    // The loader reads the active tab's own browse state, so the load starts
    // for the destination that is open; the frame path's ensure covers the
    // pick a restore or a late select left unloaded.
    if app.tab.audiobookshelf_index() == Some(lib_idx) {
        app.start_audiobookshelf_book_detail(book_id.to_string());
    }
}

/// Whether `lib_idx`'s book catalog is unloaded in a shape a catalog load may
/// start from ([`catalog_unloaded`]).
pub(in crate::app::gui_library) fn abs_catalog_needs_load(app: &App, lib_idx: usize) -> bool {
    app.audiobookshelf_book_browse
        .get(lib_idx)
        .is_some_and(|state| {
            catalog_unloaded(
                state.books.is_empty(),
                state.total,
                &state.loading_pages,
                state.replacement.is_none(),
                state.error.is_none(),
            )
        })
}

/// Clear the failed mark a user open retries from (design D4, Risks): the
/// loader's failure record is the browse state's `error`, which the frame
/// path's root step honours and an open clears.
pub(in crate::app::gui_library) fn clear_abs_catalog_failed(app: &mut App, lib_idx: usize) {
    if let Some(state) = app.audiobookshelf_book_browse.get_mut(lib_idx) {
        state.error = None;
    }
}

/// The book the Hero shows (row 9.1): the stored pick while the published
/// catalog still carries it, else the catalog's first book — the Hero default,
/// so the Hero paints as soon as the catalog lands. `None` while the catalog
/// holds no book. The selection arm is the shared `picked_or_first` rule every
/// destination's list applies (row 7C.4).
pub(in crate::app) fn abs_effective_pick(
    app: &App,
    gui: &GuiClientState,
    lib_idx: usize,
) -> Option<String> {
    let state = app.audiobookshelf_book_browse.get(lib_idx)?;
    picked_or_first(
        gui.abs_libs
            .get(&lib_idx)
            .and_then(|selection| selection.picked_book.clone()),
        |picked| {
            state
                .books
                .iter()
                .any(|book| book.library_item_id == picked)
        },
        || state.books.first().map(|book| book.library_item_id.clone()),
    )
}

/// Whether the published catalog can still name `book_id` (row 9.1): a
/// catalog that holds no book admits the pick — it is still landing, and the
/// list is what would name the pick stale — and a loaded catalog admits only a
/// book it carries.
fn abs_book_known(app: &App, lib_idx: usize, book_id: &str) -> bool {
    let Some(state) = app.audiobookshelf_book_browse.get(lib_idx) else {
        return false;
    };
    state.books.is_empty()
        || state
            .books
            .iter()
            .any(|book| book.library_item_id == book_id)
}

/// The book a play or enqueue command names, as the TUI's queue-item producer
/// builds it (row 9.1): the saved position and the finished state the browse
/// state's progress holds. `None` when the published catalog carries no book
/// of the id — a stale command plays nothing.
fn abs_book_queue_item(app: &App, lib_idx: usize, book_id: &str) -> Option<QueueItem> {
    let state = app.audiobookshelf_book_browse.get(lib_idx)?;
    audiobookshelf_book_queue_item_for_id(state, book_id)
}

/// Play the book a command names (row 9.1): the TUI's book play path — the
/// queue is replaced with the book, and the player honours its saved position
/// as the resume position — with the Owner's Audiobookshelf admission honoured.
/// A refusal flashes the TUI's owner-unavailable toast; a stale id plays
/// nothing.
pub(in crate::app::gui_library) fn play_abs_book(app: &mut App, lib_idx: usize, book_id: &str) {
    let Some(item) = abs_book_queue_item(app, lib_idx, book_id) else {
        super::log_stale_library_item("absPlay", book_id);
        return;
    };
    if !app.player.can_admit_audiobookshelf() {
        flash_abs_owner_unavailable(app);
        return;
    }
    let _ = app.submit_queue_item(item, true);
}

/// Play or seek from the chapter a command names (row 9.1): the TUI's
/// `ActivateChapter` rule (`activate_audiobookshelf_book_chapter`), scoped to
/// the destination the frame named. A stale id or chapter resolves through the
/// same rule's arms, whose refusal flashes its own toast.
pub(in crate::app::gui_library) fn play_abs_chapter(
    app: &mut App,
    lib_idx: usize,
    book_id: &str,
    chapter_id: usize,
) {
    if abs_book_queue_item(app, lib_idx, book_id).is_none() {
        super::log_stale_library_item("absPlayChapter", book_id);
        return;
    }
    app.activate_audiobookshelf_book_chapter(lib_idx, book_id, chapter_id);
}

/// Add the book a command names to the queue (row 9.1): the TUI's enqueue
/// path — one append at the tail — with the Owner's Audiobookshelf admission
/// honoured. A refusal flashes the TUI's owner-unavailable toast; a stale id
/// queues nothing.
pub(in crate::app::gui_library) fn enqueue_abs_book(app: &mut App, lib_idx: usize, book_id: &str) {
    let Some(item) = abs_book_queue_item(app, lib_idx, book_id) else {
        super::log_stale_library_item("absQueue", book_id);
        return;
    };
    if !app.player.can_admit_audiobookshelf() {
        flash_abs_owner_unavailable(app);
        return;
    }
    let _ = app.submit_queue_item(item, false);
}

/// The TUI's owner-unavailable toast (row 9.1, row 10.1): every play and
/// enqueue refusal of either Audiobookshelf destination flashes it, so the GUI
/// reads the same refusal the TUI paints.
pub(in crate::app::gui_library) fn flash_abs_owner_unavailable(app: &mut App) {
    app.flash(
        "Audiobookshelf playback owner is unavailable".into(),
        ToastSeverity::Error,
    );
}
