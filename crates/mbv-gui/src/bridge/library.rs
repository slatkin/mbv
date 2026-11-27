//! The `library` command's wire form (add-gui-library-panel design D2): the
//! Library panel's window content sends one semantic request, and the neutral
//! [`crate::gui_core::GuiLibraryCommand`] this module builds from it is what the Client
//! core applies. Item identities are stable Service ids, never row positions,
//! so a command built from a stale paint addresses the item that paint showed
//! or nothing.

use crate::gui_core::{
    GuiCollectionTarget, GuiLibraryView, GuiListSort, GuiRowArtScope, GuiSortTarget,
};
use mbv_queue::QueueItemContentId;

/// One request from the Library panel's window content (design D2). The
/// variants and their field names are the wire vocabulary the window
/// sends; the translation into [`crate::gui_core::GuiLibraryCommand`] is the only place
/// the two shapes meet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuiLibraryCommand {
    /// Open the destination at `position`: the tab position the section's
    /// destination tabs carry, where 0 is Home.
    OpenDestination { position: usize },
    /// Check or uncheck one collection of `target` (gui-collection-dropdown
    /// design D5): a press on an unchecked collection checks it and clears
    /// All, a press on a checked one unchecks it, and the selection falls back
    /// to All when none stays checked. The key is the collection's stable
    /// Service id the collections frame carries.
    CollectionToggle {
        target: GuiCollectionTarget,
        key: String,
    },
    /// Select All for `target` (design D5): the All row's press.
    CollectionAll { target: GuiCollectionTarget },
    /// Set a flat list's sort (gui-flat-list-sort design D5): the target names
    /// the list, and the sort must be one of that target's choices. The core
    /// ignores a target that names no presented list and a sort outside its
    /// choices.
    ListSort {
        target: GuiSortTarget,
        sort: GuiListSort,
    },
    /// Select the destination's view (design D8; emby-favourites design D5):
    /// one of the destination's own choices. This is a view choice, not a
    /// collection command.
    LibrarySetView {
        lib_idx: usize,
        view: GuiLibraryView,
    },
    /// Show the item's detail in the Hero.
    ShowDetail { item_id: String },
    /// Open the album node: load its tracks when the cache does not hold
    /// them.
    OpenAlbum { album_id: String },
    /// Play the item.
    Play { item_id: String },
    /// Add the item to the queue.
    AddToQueue { item_id: String },
    /// Shuffle the artist whose albums are `albumIds`.
    ShuffleArtist { album_ids: Vec<String> },
    /// Show the artist's page in the Hero (gui-music-artist-page design D2):
    /// `lib_idx` is the Music library whose row was pressed, and `artist_key`
    /// is the artist row's stable identity.
    ShowArtist {
        lib_idx: usize,
        artist_key: crate::bridge::frame::library::GuiArtistKey,
    },
    /// Play the artist whose albums are `album_ids` (gui-music-artist-page
    /// design D2): the artist page's Play control.
    PlayArtist { album_ids: Vec<String> },
    /// Add the artist whose albums are `album_ids` to the queue
    /// (gui-music-artist-page design D2): the artist page's Enqueue
    /// control.
    AddArtistToQueue { album_ids: Vec<String> },
    /// Play the artist page's track `track_id` (gui-music-artist-page design
    /// D2): the page's album ids in their shown order name the sequence.
    PlayArtistTrack {
        album_ids: Vec<String>,
        track_id: String,
    },
    /// Run the row menu's entry at `index`.
    MenuEntry {
        row: crate::gui_core::GuiLibraryMenuRow,
        index: usize,
    },
    /// Carry the row's full menu in the section (the right-click item menu).
    RowMenu {
        row: crate::gui_core::GuiLibraryMenuRow,
    },
    /// Shuffle the row: the row's hover Shuffle control runs the row menu's
    /// own shuffle entry, so the two cannot disagree.
    Shuffle {
        row: crate::gui_core::GuiLibraryMenuRow,
    },
    /// Select a Home row (add-gui-library-panel design D12): the row the
    /// Hero shows. The core keeps the selection, so the window paints it.
    HomeSelect { row: GuiHomeRowId },
    /// Play a Home row (design D12).
    HomePlay { row: GuiHomeRowId },
    /// Enqueue a Home row (design D12).
    HomeQueue { row: GuiHomeRowId },
    /// Select a Shows row (design D12, D8): the series the Hero shows. The GUI
    /// Shows destination paints the series list alone.
    ShowsSelect { lib_idx: usize, row: String },
    /// Select the season whose episodes the Hero's Workspace shows (design
    /// D12).
    ShowsSelectSeason { lib_idx: usize, season_id: String },
    /// Play a Shows episode (design D12): Enter on the row, or the
    /// double-click a window sends with this same command.
    ShowsPlay { lib_idx: usize, item_id: String },
    /// Add a Shows episode to the queue (design D12).
    ShowsQueue { lib_idx: usize, item_id: String },
    /// Report the open Shows rows the All view paints, in paint order
    /// (gui-list-header-buttons design D5): the source id of each open series
    /// row and the `(series_id, source_id)` pair of each open season row. The
    /// core stores the report and starts the bounded in-view loads it misses.
    ShowsOpenInView {
        lib_idx: usize,
        series: Vec<String>,
        seasons: Vec<(String, String)>,
    },
    /// Refresh the active destination (gui-list-header-buttons design D7): the
    /// destination's own content reloads in place, and a tree list also
    /// reloads the children of its open rows in view. `album_ids` are the open
    /// Music album rows the window paints in view.
    Refresh { album_ids: Vec<String> },
    /// Select a plain Emby-library row (add-gui-library-panel design D12,
    /// row 8.1): the row the Hero shows. A row the active list no longer
    /// holds changes nothing.
    EmbySelect { lib_idx: usize, row: String },
    /// Play a plain Emby-library row (row 8.1): Enter on the row, or the
    /// double-click a window sends with this same command. A folder or a
    /// boxset plays through the TUI's folder-play rule, so no nested list
    /// opens.
    EmbyPlay { lib_idx: usize, item_id: String },
    /// Add a plain Emby-library row to the queue (row 8.1).
    EmbyQueue { lib_idx: usize, item_id: String },
    /// Select an Audiobooks book (add-gui-library-panel design D12, row
    /// 9.1): the book the Hero shows. A book the published catalog no longer
    /// carries changes nothing.
    AbsSelect { lib_idx: usize, row: String },
    /// Play an Audiobooks book (row 9.1): Enter on the book row, or the
    /// double-click a window sends with this same command. The queue is
    /// replaced with the book at its saved position.
    AbsPlay { lib_idx: usize, item_id: String },
    /// Play or seek from an Audiobooks chapter (row 9.1): Enter on a
    /// Workspace chapter row, or the double-click a window sends with
    /// this same command.
    AbsPlayChapter {
        lib_idx: usize,
        book_id: String,
        chapter: usize,
    },
    /// Add an Audiobooks book to the queue (row 9.1).
    AbsQueue { lib_idx: usize, item_id: String },
    /// Select the Podcasts episode the Hero shows (row 10.1): the row id the
    /// window carried — the episode's opaque identity.
    PodcastsSelect { lib_idx: usize, row: String },
    /// Play a Podcasts episode (row 10.1): Enter on the row, or the
    /// double-click a window sends with this same command.
    PodcastsPlay { lib_idx: usize, item_id: String },
    /// Add a Podcasts episode to the queue (row 10.1).
    PodcastsQueue { lib_idx: usize, item_id: String },
    /// Select the Feeds entry the Hero shows (row 11.1): the row id the window
    /// carried — the entry's opaque identity.
    FeedsSelect { row: String },
    /// Play a Feeds entry (row 11.1): Enter on the row, or the double-click a
    /// window sends with this same command.
    FeedsPlay { item_id: String },
    /// Add a Feeds entry to the queue (row 11.1).
    FeedsQueue { item_id: String },
    /// Send the Search field's query (add-gui-library-panel design D12, row
    /// 12.1): the field's typing sends one, and an empty query clears the
    /// results. Cross-destination, so no library index rides it.
    SearchQuery { query: String },
    /// Activate a Search result (row 12.1): Enter or a press on the row. The
    /// id and type pair is the row's stable Service identity.
    SearchActivate { id: String, item_type: String },
    /// Close the Search results (row 12.1): the Esc in the field.
    SearchClose,
    /// Report the stable ids of the rows `scope`'s list paints, plus its
    /// overscan (gui-delineated-rows design D7). The core stores the set and
    /// queues the row art fetches once navigation is idle.
    RowsInView {
        scope: GuiRowArtScopeFrame,
        ids: Vec<String>,
    },
    /// Open the URL in the user's browser (the Hero's `IMDb` link, TUI
    /// parity): the scheme gate runs in the core, because the url comes
    /// from the window — only an http or https URL reaches the opener, and
    /// any other scheme changes nothing.
    OpenUrl { url: String },
}

/// The Library list a `RowsInView` set names (gui-delineated-rows design D7),
/// mirroring `crate::gui_core::GuiRowArtScope`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuiRowArtScopeFrame {
    Home,
    Emby,
    Music,
    ShowsSeries,
    ShowsNew,
    ShowsUpcoming,
    Favourites,
    Podcasts,
    Audiobooks,
    Feeds,
}

impl From<GuiRowArtScopeFrame> for GuiRowArtScope {
    fn from(scope: GuiRowArtScopeFrame) -> Self {
        match scope {
            GuiRowArtScopeFrame::Home => Self::Home,
            GuiRowArtScopeFrame::Emby => Self::Emby,
            GuiRowArtScopeFrame::Music => Self::Music,
            GuiRowArtScopeFrame::ShowsSeries => Self::ShowsSeries,
            GuiRowArtScopeFrame::ShowsNew => Self::ShowsNew,
            GuiRowArtScopeFrame::ShowsUpcoming => Self::ShowsUpcoming,
            GuiRowArtScopeFrame::Favourites => Self::Favourites,
            GuiRowArtScopeFrame::Podcasts => Self::Podcasts,
            GuiRowArtScopeFrame::Audiobooks => Self::Audiobooks,
            GuiRowArtScopeFrame::Feeds => Self::Feeds,
        }
    }
}

/// One Home row's stable identity (design D12): the identity the Home
/// paths resolve, addressed by Service ids and never by a row position.
/// `Recent` carries the queue item's content id, whose own wire form is
/// itself (`provider` and `value`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuiHomeRowId {
    /// A `Continue watching` row's Emby item id.
    Continue(String),
    /// A `Recently added` row's queue-item content id.
    Recent(QueueItemContentId),
}

impl From<GuiLibraryCommand> for crate::gui_core::GuiLibraryCommand {
    fn from(command: GuiLibraryCommand) -> Self {
        match command {
            GuiLibraryCommand::OpenDestination { position } => Self::OpenDestination { position },
            GuiLibraryCommand::CollectionToggle { target, key } => {
                Self::CollectionToggle { target, key }
            }
            GuiLibraryCommand::CollectionAll { target } => Self::CollectionAll { target },
            GuiLibraryCommand::ListSort { target, sort } => Self::ListSort { target, sort },
            GuiLibraryCommand::LibrarySetView { lib_idx, view } => {
                Self::LibrarySetView { lib_idx, view }
            }
            GuiLibraryCommand::ShowDetail { item_id } => Self::ShowDetail { item_id },
            GuiLibraryCommand::OpenAlbum { album_id } => Self::OpenAlbum { album_id },
            GuiLibraryCommand::Play { item_id } => Self::Play { item_id },
            GuiLibraryCommand::AddToQueue { item_id } => Self::AddToQueue { item_id },
            GuiLibraryCommand::ShuffleArtist { album_ids } => Self::ShuffleArtist { album_ids },
            GuiLibraryCommand::ShowArtist {
                lib_idx,
                artist_key,
            } => Self::ShowArtist {
                lib_idx,
                artist_key: artist_key.into(),
            },
            GuiLibraryCommand::PlayArtist { album_ids } => Self::PlayArtist { album_ids },
            GuiLibraryCommand::AddArtistToQueue { album_ids } => {
                Self::AddArtistToQueue { album_ids }
            }
            GuiLibraryCommand::PlayArtistTrack {
                album_ids,
                track_id,
            } => Self::PlayArtistTrack {
                album_ids,
                track_id,
            },
            GuiLibraryCommand::MenuEntry { row, index } => Self::MenuEntry { row, index },
            GuiLibraryCommand::RowMenu { row } => Self::RowMenu { row },
            GuiLibraryCommand::Shuffle { row } => Self::Shuffle { row },
            GuiLibraryCommand::HomeSelect { row } => Self::HomeSelect { row: row.into() },
            GuiLibraryCommand::HomePlay { row } => Self::HomePlay { row: row.into() },
            GuiLibraryCommand::HomeQueue { row } => Self::HomeQueue { row: row.into() },
            GuiLibraryCommand::ShowsSelect { lib_idx, row } => Self::ShowsSelect { lib_idx, row },
            GuiLibraryCommand::ShowsSelectSeason { lib_idx, season_id } => {
                Self::ShowsSelectSeason { lib_idx, season_id }
            }
            GuiLibraryCommand::ShowsPlay { lib_idx, item_id } => {
                Self::ShowsPlay { lib_idx, item_id }
            }
            GuiLibraryCommand::ShowsQueue { lib_idx, item_id } => {
                Self::ShowsQueue { lib_idx, item_id }
            }
            GuiLibraryCommand::ShowsOpenInView {
                lib_idx,
                series,
                seasons,
            } => Self::ShowsOpenInView {
                lib_idx,
                series,
                seasons,
            },
            GuiLibraryCommand::Refresh { album_ids } => Self::Refresh { album_ids },
            GuiLibraryCommand::EmbySelect { lib_idx, row } => Self::EmbySelect { lib_idx, row },
            GuiLibraryCommand::EmbyPlay { lib_idx, item_id } => Self::EmbyPlay { lib_idx, item_id },
            GuiLibraryCommand::EmbyQueue { lib_idx, item_id } => {
                Self::EmbyQueue { lib_idx, item_id }
            }
            GuiLibraryCommand::AbsSelect { lib_idx, row } => Self::AbsSelect { lib_idx, row },
            GuiLibraryCommand::AbsPlay { lib_idx, item_id } => Self::AbsPlay { lib_idx, item_id },
            GuiLibraryCommand::AbsPlayChapter {
                lib_idx,
                book_id,
                chapter,
            } => Self::AbsPlayChapter {
                lib_idx,
                book_id,
                chapter,
            },
            GuiLibraryCommand::AbsQueue { lib_idx, item_id } => Self::AbsQueue { lib_idx, item_id },
            GuiLibraryCommand::PodcastsSelect { lib_idx, row } => {
                Self::PodcastsSelect { lib_idx, row }
            }
            GuiLibraryCommand::PodcastsPlay { lib_idx, item_id } => {
                Self::PodcastsPlay { lib_idx, item_id }
            }
            GuiLibraryCommand::PodcastsQueue { lib_idx, item_id } => {
                Self::PodcastsQueue { lib_idx, item_id }
            }
            GuiLibraryCommand::FeedsSelect { row } => Self::FeedsSelect { row },
            GuiLibraryCommand::FeedsPlay { item_id } => Self::FeedsPlay { item_id },
            GuiLibraryCommand::FeedsQueue { item_id } => Self::FeedsQueue { item_id },
            GuiLibraryCommand::SearchQuery { query } => Self::SearchQuery { query },
            GuiLibraryCommand::SearchActivate { id, item_type } => {
                Self::SearchActivate { id, item_type }
            }
            GuiLibraryCommand::SearchClose => Self::SearchClose,
            GuiLibraryCommand::RowsInView { scope, ids } => Self::RowsInView {
                scope: scope.into(),
                ids,
            },
            GuiLibraryCommand::OpenUrl { url } => Self::OpenUrl { url },
        }
    }
}

/// One row id's core identity, resolving the wire type the window sends.
/// The window mirrors the same shape on the way out, so a command built from
/// a carried row addresses exactly that row. The owned form is the ref
/// form's delegate, so the variant names are hand-written once.
impl From<&GuiHomeRowId> for crate::gui_core::GuiHomeRowId {
    fn from(row: &GuiHomeRowId) -> Self {
        match row {
            GuiHomeRowId::Continue(item_id) => Self::Continue(item_id.clone()),
            GuiHomeRowId::Recent(content_id) => Self::Recent(content_id.clone()),
        }
    }
}

impl From<GuiHomeRowId> for crate::gui_core::GuiHomeRowId {
    fn from(row: GuiHomeRowId) -> Self {
        Self::from(&row)
    }
}
