//! The GUI Library panel's Client-core half (add-gui-library-panel design D2,
//! D5): the semantic commands the window content sends, the public `App` entry
//! that applies each one, the Music collection store beside it, and the Music
//! projection the window content paints. The window content sends intent, and
//! every entry calls the existing `App` action for it. The TUI Client never
//! calls any of them.

pub(in crate::app) mod artist_lookup;
mod artwork;
mod audiobooks;
mod audiobooks_projection;
mod collection;
mod collection_command;
mod emby;
mod emby_projection;
pub(in crate::app) mod ensure;
mod entry_list;
mod feeds;
mod feeds_projection;
mod home;
mod item_actions;
mod item_menu;
mod launch;
mod list_sort;
mod marks;
mod podcasts;
mod podcasts_projection;
mod projection;
mod request;
mod rows_in_view;
mod search;
mod shows;
mod shows_projection;
mod shows_view;
mod store;
mod stores;
mod tabs;
mod tv;

pub use self::artwork::{
    GuiLibraryArtwork, GuiRowArt, GuiRowMediaKind, GuiRowSource, artist_rows_in_view_id,
};
pub use self::audiobooks::GuiAudiobooksSelection;
pub use self::audiobooks_projection::{
    GuiAudiobooksChapterRow, GuiAudiobooksHero, GuiAudiobooksProjection, GuiAudiobooksRow,
    GuiAudiobooksWorkspace,
};
pub use self::collection::{
    GuiCollectionEntry, GuiCollectionNoun, GuiCollectionSelection, GuiCollectionSet,
    GuiCollectionTarget, GuiCollections, GuiDateOrder, compare_gui_date_order,
};
use self::collection_command::CollectionOp;
use self::emby::GuiEmbyFeedCollectionSite;
pub use self::emby::GuiEmbyLibrarySelection;
pub(in crate::app) use self::emby::{GuiEmbyRootItems, GuiEmbyRootItemsEvent};
pub use self::emby_projection::{
    GuiEmbyHero, GuiEmbyHeroCredit, GuiEmbyProjection, GuiEmbyRow, GuiEmbyRowKind,
};
pub use self::entry_list::{GuiEntryListProjection, GuiEntryRow};
use self::feeds::GuiFeedsCollectionSite;
pub use self::feeds::GuiFeedsSelection;
pub use self::feeds_projection::{
    GuiFeedEntryId, GuiFeedsHero, GuiFeedsProjection, GuiFeedsRow, gui_feeds_platform,
};
pub use self::home::{
    GuiArtShape, GuiHomeHero, GuiHomeItemRow, GuiHomeProjection, GuiHomeRow, GuiHomeRowId,
    GuiHomeRowKind,
};
pub use self::item_menu::{GuiLibraryMenuItem, GuiLibraryMenuRow, GuiLibraryRowMenu};
pub use self::launch::GuiLaunchOpen;
pub use self::list_sort::{GuiListSort, GuiListSortFrame, GuiSortTarget};
pub use self::marks::GuiPlayheadMarks;
use self::podcasts::GuiPodcastsCollectionSite;
pub use self::podcasts::GuiPodcastsSelection;
pub use self::podcasts_projection::{
    GuiPodcastEpisodeId, GuiPodcastsHero, GuiPodcastsProjection, GuiPodcastsRow,
};
pub use self::projection::{
    GuiMusicAlbum, GuiMusicArtist, GuiMusicHero, GuiMusicProjection, GuiMusicTrack,
    GuiMusicTrackTotal,
};
pub(in crate::app) use self::request::GuiSectionRequest;
pub(in crate::app) use self::search::GuiSearchStore;
pub use self::search::{GuiSearchProjection, GuiSearchRow};
pub(in crate::app) use self::shows::GuiShowsNewEvent;
pub use self::shows_projection::{
    GuiShowsEpisodeRow, GuiShowsHero, GuiShowsProjection, GuiShowsRow, GuiShowsSeasonNode,
    GuiShowsSeasonPill, GuiShowsSeries, GuiShowsWorkspace,
};
pub use self::shows_view::GuiShowsView;
use self::store::GuiMusicCollectionSite;
pub(in crate::app) use self::store::{GuiGroupAlbums, GuiGroupAlbumsEvent};
pub use self::stores::GuiLibraryStores;
pub use self::tabs::{GuiDestinationKind, GuiLibraryTab, GuiLibraryTabs};
pub use self::tv::GuiTvShowsSelection;

use mbv_ui_model::settings::PanelFocus;

use crate::app::App;
use crate::gui::GuiCore;

/// The Library list whose rows a `RowsInView` set names (gui-delineated-rows
/// design D7): the destination list the ids belong to, so the core resolves
/// each id against its own data. It lives beside the command because the
/// window's `PointerList` is not visible to the core, and its Tracklist and
/// Workspace arms carry no thumbs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GuiRowArtScope {
    /// The Home flat list's item rows.
    Home,
    /// A plain Emby library's list rows.
    Emby,
    /// The Music list's artist rows.
    Music,
    /// The Shows All list's series rows.
    ShowsSeries,
    /// The Shows New list's flat episode rows.
    ShowsNew,
    /// The Shows Upcoming list's flat premiere rows.
    ShowsUpcoming,
    /// The Podcasts list's episode rows.
    Podcasts,
    /// The Audiobooks list's book rows.
    Audiobooks,
    /// The Feeds list's entry rows.
    Feeds,
}

/// One request from the GUI Library panel's window content (design D2): the
/// `library` command carries it into the Client core, and
/// [`GuiCore::apply_gui_library_command`] applies it. Item identities are stable
/// Service ids, never row positions, so a command built from a stale frame
/// addresses the item that frame painted or nothing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GuiLibraryCommand {
    /// Open a destination: `position` is the tab position the frame's
    /// destination tabs carry, where 0 is Home. A position that names no
    /// presented destination opens nothing (design D2): the command the
    /// frame's tabs carry can name only a painted tab, and a stale frame must
    /// not activate a destination the top bar does not carry.
    OpenDestination { position: usize },
    /// Check or uncheck one collection of `target` (design D5): a press on an
    /// unchecked collection checks it and clears All, a press on a checked one
    /// unchecks it, and the selection falls back to All when none stays
    /// checked. The Music destination installs the change through its one
    /// selection entry, so the Hero's album carries across it (design D11).
    CollectionToggle {
        target: GuiCollectionTarget,
        /// The collection's stable Service id.
        key: String,
    },
    /// Select All for `target` (design D5): the All row's press and the
    /// dropdown's Clear.
    CollectionAll { target: GuiCollectionTarget },
    /// Select a flat list's sort (design D5): `target` names the list, and
    /// `sort` is one of its choices. A target that names no presented flat
    /// list, a `ShowsNew` on a library that is not a TV library, and a sort
    /// outside the target's choices change nothing.
    ListSort {
        target: GuiSortTarget,
        sort: GuiListSort,
    },
    /// Select the Shows destination's single view (design D8): All, New, or
    /// Upcoming. This is a view choice, not a collection command.
    ShowsSetView { lib_idx: usize, view: GuiShowsView },
    /// Show the item's detail in the Hero (design D2).
    ShowDetail { item_id: String },
    /// Open an album node: load its tracks when the cache does not hold them
    /// (design D5).
    OpenAlbum { album_id: String },
    /// Play the item (design D8).
    Play { item_id: String },
    /// Play the item next (design D8): the shared play-next admission, so the
    /// item enters directly after the active slot.
    PlayNext { item_id: String },
    /// Add the item to the queue (design D8).
    AddToQueue { item_id: String },
    /// Shuffle the artist whose albums are `album_ids` (design D8).
    ShuffleArtist { album_ids: Vec<String> },
    /// Run the row menu's entry at `index` (design D8): the entry the frame
    /// painted for the row, run through the row's destination menu.
    MenuEntry {
        row: GuiLibraryMenuRow,
        index: usize,
    },
    /// Carry the row's full menu in the frame (the right-click item menu):
    /// the next frame's `rowMenu` section names the row and carries the
    /// TUI menu builder's entries for it, and a chosen entry runs through the
    /// same `MenuEntry` command as the Hero's menu carries.
    RowMenu { row: GuiLibraryMenuRow },
    /// Select a Home row (design D2, D12, the USER-APPROVED GUI Home design):
    /// the row the Hero shows. The core owns the selection, as the Music
    /// collection groups own their pick (`showDetail`), so the hero re-arms
    /// to its first item row when the frame's row has left the list.
    HomeSelect { row: home::GuiHomeRowId },
    /// Play a Home row (design D12): Enter on the row plays it through the
    /// Home flat list's play path, whose resume behaviour the row's
    /// Continue Watching membership decides.
    HomePlay { row: home::GuiHomeRowId },
    /// Enqueue a Home row (design D12): `n` on the row, carried as its own
    /// command for the window's Play next label, resolved through the
    /// same Home enqueue path as the add — the Owner owns the queue, and the
    /// Home rows have no library the play-next admission resolves against,
    /// so the row tail-appends like `a`.
    HomePlayNext { row: home::GuiHomeRowId },
    /// Enqueue a Home row (design D12): `a` on the row, through the Home
    /// flat list's enqueue path.
    HomeQueue { row: home::GuiHomeRowId },
    /// Select a Shows row (design D12, D8): the series the Hero shows, whose
    /// select runs the series-detail load. A row the series list no longer
    /// holds changes nothing.
    ShowsSelect { lib_idx: usize, row: String },
    /// Select the season whose episodes the Hero's Workspace shows (design
    /// D12): the season-episodes load starts when the cache does not hold
    /// them.
    ShowsSelectSeason { lib_idx: usize, season_id: String },
    /// Play a Shows episode (design D12): Enter on the row, or the
    /// double-click a window sends with this same command. The shared
    /// library play path runs, so the Owner owns the queue.
    ShowsPlay { lib_idx: usize, item_id: String },
    /// Play a Shows episode next (design D12): the shared play-next
    /// admission, so the episode enters directly after the active slot.
    ShowsPlayNext { lib_idx: usize, item_id: String },
    /// Add a Shows episode to the queue (design D12): the shared add path.
    ShowsQueue { lib_idx: usize, item_id: String },
    /// Select a plain Emby-library row (design D12, row 8.1): the row the
    /// Hero shows. A row the active list no longer holds changes nothing.
    EmbySelect { lib_idx: usize, row: String },
    /// Play a plain Emby-library row (design D12, row 8.1): Enter on the
    /// row, or the double-click a window sends with this same command.
    /// A folder or a boxset plays through the TUI's folder-play rule, so no
    /// nested list opens.
    EmbyPlay { lib_idx: usize, item_id: String },
    /// Play a plain Emby-library row next (design D12, row 8.1): the shared
    /// play-next admission, so the row enters directly after the active
    /// slot.
    EmbyPlayNext { lib_idx: usize, item_id: String },
    /// Add a plain Emby-library row to the queue (design D12, row 8.1): the
    /// shared add path.
    EmbyQueue { lib_idx: usize, item_id: String },
    /// Select an Audiobooks book (design D12, row 9.1): the book the Hero
    /// shows, whose select runs the TUI's own detail loader. A book the
    /// published catalog no longer carries changes nothing.
    AbsSelect { lib_idx: usize, row: String },
    /// Play an Audiobooks book (design D12, row 9.1): Enter on the book row,
    /// or the double-click a window sends with this same command. The
    /// queue is replaced with the book at its saved position, through the
    /// TUI's book play path and the Owner's Audiobookshelf admission.
    AbsPlay { lib_idx: usize, item_id: String },
    /// Play or seek from an Audiobooks chapter (design D12, row 9.1): Enter
    /// on a Workspace chapter row, or the double-click a window sends
    /// with this same command. The TUI's chapter-activation rule runs: a seek
    /// on the active book, a replace-and-start at the chapter's offset on
    /// another one.
    AbsPlayChapter {
        lib_idx: usize,
        book_id: String,
        chapter: usize,
    },
    /// Add an Audiobooks book to the queue (design D12, row 9.1): the TUI's
    /// enqueue path, through the Owner's Audiobookshelf admission.
    AbsQueue { lib_idx: usize, item_id: String },
    /// Select the Podcasts episode the Hero shows (design D12, row 10.1): a
    /// press on the row. The row id is the frame's opaque episode identity, so
    /// a stale frame's row changes nothing.
    PodcastsSelect { lib_idx: usize, row: String },
    /// Play a Podcasts episode (design D12, row 10.1): Enter on the row, or the
    /// double-click a window sends with this same command. The queue is
    /// replaced with the episode through the TUI's episode play path and the
    /// Owner's Audiobookshelf admission.
    PodcastsPlay { lib_idx: usize, item_id: String },
    /// Add a Podcasts episode to the queue (design D12, row 10.1): the TUI's
    /// enqueue path, through the Owner's Audiobookshelf admission.
    PodcastsQueue { lib_idx: usize, item_id: String },
    /// Select the Feeds entry the Hero shows (design D12, row 11.1): a press
    /// on the row. The row id is the frame's opaque entry identity, so a stale
    /// frame's row changes nothing.
    FeedsSelect { row: String },
    /// Play a Feeds entry (design D12, row 11.1): Enter on the row, or the
    /// double-click a window sends with this same command. The queue is
    /// replaced with the entry through the TUI's feed play path, whose
    /// no-playable-source guard flashes the TUI's toast.
    FeedsPlay { item_id: String },
    /// Add a Feeds entry to the queue (design D12, row 11.1): the TUI's
    /// enqueue path.
    FeedsQueue { item_id: String },
    /// Send the Search field's query (design D12, row 12.1): the TUI's own
    /// search worker answers it, and the Client's tick host drains the result
    /// into the store the frame carries. An empty query clears the results.
    SearchQuery { query: String },
    /// Activate a Search result (design D12, row 12.1): Enter or a press on
    /// the row. The id and type pair is the row's stable Service identity, so
    /// a stale frame's row navigates nowhere.
    SearchActivate { id: String, item_type: String },
    /// Close the Search results (design D12, row 12.1): Esc in the field.
    SearchClose,
    /// Report the stable ids of the rows `scope`'s list paints, plus its
    /// overscan (gui-delineated-rows design D7): the core stores the latest set
    /// per list and queues the row art fetches once navigation is idle. The
    /// ids are the frames' own row ids, never coordinates.
    RowsInView {
        scope: GuiRowArtScope,
        ids: Vec<String>,
    },
    /// Open the URL in the user's browser (the Hero's `IMDb` link, TUI
    /// parity): the scheme gate runs in the core, because the url comes
    /// from the window — only an http or https URL reaches the opener, and
    /// any other scheme changes nothing.
    OpenUrl { url: String },
}

/// The command's row identity for a stale-frame log: a play or enqueue
/// command names a Home row, so the ignored log carries the same identity
/// the frame painted.
fn home_row_command_id(command: &str, row: &GuiHomeRowId) -> String {
    format!("{command} {row:?}")
}

impl GuiCore {
    /// Apply one GUI Library command (design D2): each arm calls the existing
    /// `App` action for the intent, so the TUI's own paths do not change.
    /// Every command logs its receipt (`gui_library.command`), and one that
    /// the App's data cannot act on logs why it was ignored, so a live
    /// session's Library path is visible in the GUI log.
    pub fn apply_gui_library_command(&mut self, command: GuiLibraryCommand) {
        Self::log_library_command(&command);
        self.dispatch_gui_library_command(command);
    }

    /// The Library command's arms (design D2): the one dispatch the core's own
    /// call runs, so every variant has its own handling site here.
    fn dispatch_gui_library_command(&mut self, command: GuiLibraryCommand) {
        match command {
            GuiLibraryCommand::OpenDestination { position } => {
                self.open_gui_library_position(position);
            }
            GuiLibraryCommand::CollectionToggle { target, key } => {
                self.app
                    .apply_collection(&target, CollectionOp::Toggle { key });
            }
            GuiLibraryCommand::CollectionAll { target } => {
                self.app.apply_collection(&target, CollectionOp::All);
            }
            GuiLibraryCommand::ListSort { target, sort } => {
                self.app.apply_gui_list_sort(target, sort);
            }
            GuiLibraryCommand::ShowsSetView { lib_idx, view } => {
                self.app.set_gui_shows_view(lib_idx, view);
            }
            GuiLibraryCommand::ShowDetail { item_id } => {
                self.app.show_gui_library_detail(&item_id);
            }
            GuiLibraryCommand::OpenAlbum { album_id } => {
                self.app.open_gui_library_album(album_id);
            }
            GuiLibraryCommand::Play { item_id } => match self.app.gui_library_item(&item_id) {
                Some((lib_idx, item)) => self.app.play_gui_library_item(lib_idx, item),
                None => log_stale_library_item("play", &item_id),
            },
            GuiLibraryCommand::PlayNext { item_id } => match self.app.gui_library_item(&item_id) {
                Some((lib_idx, item)) => self.app.play_lib_item_next(lib_idx, item),
                None => log_stale_library_item("playNext", &item_id),
            },
            GuiLibraryCommand::AddToQueue { item_id } => {
                self.app.add_gui_library_item_to_queue(&item_id);
            }
            GuiLibraryCommand::ShuffleArtist { album_ids } => {
                self.app.shuffle_gui_artist(&album_ids);
            }
            // Row 3.6 runs the menu entry the frame carried through the shared
            // context-action executor.
            GuiLibraryCommand::MenuEntry { row, index } => {
                self.app.run_gui_library_menu_entry(&row, index);
            }
            GuiLibraryCommand::RowMenu { row } => {
                self.app.gui.library_row_menu_target = Some(row);
            }
            // The Home rows and every command after them are the second
            // pass's arms.
            _ => self.dispatch_gui_library_destination_command(command),
        }
    }

    /// The Library command's destination arms (design D2), from the Home rows
    /// down: the dispatch's second pass, so one function stays readable.
    fn dispatch_gui_library_destination_command(&mut self, command: GuiLibraryCommand) {
        match command {
            GuiLibraryCommand::HomeSelect { row } => self.select_gui_home_row(&row),
            GuiLibraryCommand::HomePlay { row } => self.play_gui_home_row(&row),
            GuiLibraryCommand::HomePlayNext { row } => {
                self.enqueue_gui_home_row(&row, "homePlayNext");
            }
            GuiLibraryCommand::HomeQueue { row } => self.enqueue_gui_home_row(&row, "homeQueue"),
            GuiLibraryCommand::ShowsSelect { lib_idx, row } => {
                self.app.select_gui_shows_row(lib_idx, &row);
            }
            GuiLibraryCommand::ShowsSelectSeason { lib_idx, season_id } => {
                self.app.gui_tv_select_season(lib_idx, &season_id);
            }
            GuiLibraryCommand::ShowsPlay { lib_idx, item_id } => {
                self.app.play_gui_shows_item(lib_idx, &item_id);
            }
            GuiLibraryCommand::ShowsPlayNext { lib_idx, item_id } => {
                self.app.play_next_gui_shows_item(lib_idx, &item_id);
            }
            GuiLibraryCommand::ShowsQueue { lib_idx, item_id } => {
                self.app.enqueue_gui_shows_item(lib_idx, &item_id);
            }
            GuiLibraryCommand::EmbySelect { lib_idx, row } => {
                self.app.gui_emby_select(lib_idx, &row);
            }
            GuiLibraryCommand::EmbyPlay { lib_idx, item_id } => {
                self.app.play_gui_emby_item(lib_idx, &item_id);
            }
            GuiLibraryCommand::EmbyPlayNext { lib_idx, item_id } => {
                self.app.play_next_gui_emby_item(lib_idx, &item_id);
            }
            GuiLibraryCommand::EmbyQueue { lib_idx, item_id } => {
                self.app.enqueue_gui_emby_item(lib_idx, &item_id);
            }
            GuiLibraryCommand::AbsSelect { lib_idx, row } => {
                self.app.gui_abs_select(lib_idx, &row);
            }
            // The Audiobookshelf destinations' commands (rows 9.1, 10.1): the
            // book list's play, chapter, and enqueue, and the podcast list's
            // select, play, and enqueue.
            GuiLibraryCommand::AbsPlay { lib_idx, item_id } => {
                self.app.play_gui_abs_book(lib_idx, &item_id);
            }
            GuiLibraryCommand::AbsPlayChapter {
                lib_idx,
                book_id,
                chapter,
            } => self.app.play_gui_abs_chapter(lib_idx, &book_id, chapter),
            GuiLibraryCommand::AbsQueue { lib_idx, item_id } => {
                self.app.enqueue_gui_abs_book(lib_idx, &item_id);
            }
            GuiLibraryCommand::PodcastsSelect { lib_idx, row } => {
                self.app.select_gui_podcast_row(lib_idx, &row);
            }
            GuiLibraryCommand::PodcastsPlay { lib_idx, item_id } => {
                self.app.play_gui_podcast_row(lib_idx, &item_id);
            }
            GuiLibraryCommand::PodcastsQueue { lib_idx, item_id } => {
                self.app.enqueue_gui_podcast_row(lib_idx, &item_id);
            }
            // The Feeds destination's own commands (row 11.1): the one feed
            // tab the Client holds, so no library index rides them.
            GuiLibraryCommand::FeedsSelect { row } => self.app.gui_feeds_select(&row),
            GuiLibraryCommand::FeedsPlay { item_id } => self.app.play_gui_feed_row(&item_id),
            GuiLibraryCommand::FeedsQueue { item_id } => self.app.enqueue_gui_feed_row(&item_id),
            // The Search field's commands (design D12, row 12.1): the field
            // is cross-destination, so no library index rides them.
            GuiLibraryCommand::SearchQuery { query } => self.app.gui_search_query(query),
            GuiLibraryCommand::SearchActivate { id, item_type } => {
                self.app.gui_search_activate(id, item_type);
            }
            GuiLibraryCommand::SearchClose => self.app.gui_search_close(),
            GuiLibraryCommand::RowsInView { scope, ids } => {
                self.app.apply_gui_rows_in_view(scope, ids);
            }
            GuiLibraryCommand::OpenUrl { url } => Self::open_gui_library_url(&url),
            // The first pass's arms, which this pass never receives: the
            // dispatch routes them before it hands the command over, so this
            // trailing arm is the routing mistake's own no-op.
            other => log_ignored_library_command(
                "dispatch",
                format_args!("{other:?} is no destination command of the second pass"),
            ),
        }
    }

    /// Log one GUI Library command's receipt (`gui_library.command`): every
    /// command reaches this line, so a live session's Library path is visible
    /// in the GUI log before any arm acts on it.
    fn log_library_command(command: &GuiLibraryCommand) {
        tracing::info!(
            name: "gui_library.command",
            target: "gui_library",
            command = ?command,
            "the GUI Library panel sent a command"
        );
    }

    /// Open the destination a tab position names (design D2): the GUI's one
    /// open step. The window's open-destination command and the keyboard
    /// router's destination steps (the tab keys and a digit) all take it, so a
    /// position resolves to the same destination state however it arrived. A
    /// position that names no presented destination opens nothing — the top
    /// bar paints no tab for it, so it is the absent digit key.
    pub(in crate::app) fn open_gui_library_position(&mut self, position: usize) {
        if self.app.presents_open_position(position) {
            self.app.set_library_tab(position);
            self.open_gui_active_destination();
        } else {
            log_ignored_library_command(
                "openDestination",
                format_args!("the position {position} names no presented destination"),
            );
        }
    }

    /// The open step the active destination's kind needs (design D12;
    /// gui-one-destination-ensure design D2, D4): the destination's user-action
    /// work, then its one load pass. The Shows destination forces its one view
    /// mode, the plain Emby library clears a failed root mark, and the
    /// Audiobooks, Podcasts, and Feeds destinations log an ignored command;
    /// every destination's loads are the pass's own steps, so an open starts no
    /// load of its own. Home and Music have no user-action work. The match names
    /// every destination kind and every absent case, so a new kind does not
    /// compile until it states its open step.
    fn open_gui_active_destination(&mut self) {
        match self.app.gui_active_destination() {
            // No presented destination: the open names nothing to open.
            None => {}
            // Home's rows ride the shared Home data and Music's loads are all
            // the pass's selection step (design D3), so neither has user-action
            // work and the pass alone is their open step.
            Some(GuiDestinationKind::Home | GuiDestinationKind::Music { .. }) => {
                self.app.ensure_gui_active_destination();
            }
            // Feeds' entries ride the shared startup fetch, so its pass is a
            // no-op and its open only reports an absent tab.
            Some(GuiDestinationKind::Feeds) => self.app.gui_feeds_open(),
            Some(GuiDestinationKind::Shows { lib_idx }) => self.app.gui_tv_open(lib_idx),
            Some(GuiDestinationKind::Library { lib_idx }) => self.app.gui_emby_open(lib_idx),
            Some(GuiDestinationKind::Audiobooks { lib_idx }) => self.app.gui_abs_open(lib_idx),
            Some(GuiDestinationKind::Podcasts { lib_idx }) => {
                self.app.gui_podcast_open(lib_idx);
            }
        }
    }

    /// Select a Home row (design D2, D12): the row the Hero shows. The
    /// selection is the core's, so the frame carries it back and the launch
    /// of a window-content reload keeps it.
    fn select_gui_home_row(&mut self, row: &GuiHomeRowId) {
        self.app.gui.home_selection = Some(row.clone());
    }

    /// Play a Home row (design D12): the Home flat list's play path, whose
    /// resume behaviour the row's Continue Watching membership decides. A
    /// stale frame's row, one whose item the App's Home data no longer
    /// holds, changes nothing, as every stale command's item arm does.
    fn play_gui_home_row(&mut self, row: &GuiHomeRowId) {
        let target = row.clone().into_home_item_target();
        let from_cw = row.is_continue();
        if let Some(item) = self.app.home_stable_target(&target) {
            self.app.home_play_target(item, from_cw);
        } else {
            log_stale_library_item("homePlay", &home_row_command_id("homePlay", row));
        }
    }

    /// Enqueue a Home row (design D12): the Home flat list's enqueue path,
    /// for both the explicit add and the play-next label — the Owner owns
    /// the queue, and the command's intent reaches it through the same
    /// resolved row, never a direct queue mutation. `name` is the command's
    /// intent, for the stale-frame log the item arms share.
    fn enqueue_gui_home_row(&mut self, row: &GuiHomeRowId, name: &str) {
        let target = row.clone().into_home_item_target();
        let logged = home_row_command_id(name, row);
        let from_cw = row.is_continue();
        if let Some(item) = self.app.home_stable_target(&target) {
            self.app.home_enqueue_target(item, from_cw);
        } else {
            log_stale_library_item(name, &logged);
        }
    }

    /// Open the URL `openUrl` carries (the Hero's `IMDb` link): the scheme
    /// gate runs first — the url comes from the window, so only an http or
    /// https URL reaches the opener, and any other scheme is logged and
    /// dropped. A failed spawn logs and changes nothing else.
    fn open_gui_library_url(url: &str) {
        if mbv_ui_model::ui_util::sanitize_url(url).is_none() {
            log_ignored_library_command(
                "openUrl",
                format_args!("the url's scheme is not http or https"),
            );
            return;
        }
        if let Err(error) = open_url(url) {
            tracing::warn!(
                name: "gui_library.command.open_url.failed",
                target: "gui_library",
                error = %error,
                "failed to open the link"
            );
        }
    }

    /// Set Panel focus for the GUI Library panel (design D4): the core
    /// thread's per-key Panel-focus adapter sets the App's stored focus
    /// through this entry, so the gates the App's key paths read agree with
    /// the core's `FocusStack`.
    pub fn set_gui_panel_focus(&mut self, focus: PanelFocus) {
        self.app.set_panel_focus(focus);
    }

    /// The Panel that holds focus (design D4, D9): the value the GUI Client's
    /// `FocusStack` and the frame's focus follow, and the one the launch
    /// location saves.
    #[must_use]
    pub fn gui_panel_focus(&self) -> PanelFocus {
        self.app.effective_panel_focus()
    }

    /// The id of the Emby item playing at the playhead (add-gui-library-panel,
    /// the Library's playing row): the Queue panel's own authority — the Owner
    /// status and the playback queue's item at the active index — read once
    /// for the frame's Library section, so the Library's playing row and the
    /// Queue's active slot cannot disagree. `None` while idle or while the
    /// active slot holds no Emby item.
    #[must_use]
    pub fn gui_playing_track_id(&self) -> Option<String> {
        playing_track_id(&self.app)
    }
}

/// The id of the Emby item playing at the playhead: the Owner status and the
/// queue's item at the active index, or `None` while idle or while the active
/// slot holds no Emby item.
fn playing_track_id(app: &App) -> Option<String> {
    let state = app.displayed_playback_state();
    if !state.active {
        return None;
    }
    match app.queue_item_at_playhead(state) {
        Some(mbv_queue::QueueItem::Emby(item)) => Some(item.id.clone()),
        Some(_) | None => None,
    }
}

/// Log one Library command's ignored receipt: the command's intent arrives,
/// the frame it rode is stale, or the position names no destination the GUI
/// presents.
fn log_ignored_library_command(command: &str, reason: impl std::fmt::Display) {
    tracing::info!(
        name: "gui_library.command.ignored",
        target: "gui_library",
        command,
        reason = %reason,
        "the Library command changed nothing"
    );
}

/// Log the item-named Library command whose item the App's Service data no
/// longer holds (design D2: a stale frame addresses the item it painted or
/// nothing).
fn log_stale_library_item(command: &str, item_id: &str) {
    log_ignored_library_command(
        command,
        format_args!("the item {item_id} is not in the Service data"),
    );
}

/// The Emby item id playing at the playhead, read from the `App` a projection
/// holds rather than a GUI core (`app::gui_library::marks` builds its playhead
/// marks from it), and from the GUI Client's Library section: the same read
/// the core's `GuiCore::gui_playing_track_id` makes.
pub fn playing_track_id_for_shell(app: &App) -> Option<String> {
    playing_track_id(app)
}

/// The GUI's one open step over an `App` the caller holds rather than a GUI
/// core: the tab step and the router's numbered destinations hold the shell's
/// `App`, so they enter through this wrapper, which runs the core's step.
pub(in crate::app) fn open_gui_library_position_for_app(app: &mut App, position: usize) {
    let mut core = GuiCore::new(std::mem::replace(app, crate::app::app_stub::handoff_app()));
    core.open_gui_library_position(position);
    *app = core.into_app();
}

/// The URL opener the `openUrl` command runs: the shared `open_url` spawn,
/// behind the test seam that observes the open without a real browser.
fn open_url(url: &str) -> std::io::Result<()> {
    #[cfg(test)]
    if let Some(open) = *crate::app::OPEN_URL_OVERRIDE.lock().unwrap() {
        open(url);
        return Ok(());
    }
    crate::app::open_url(url)
}
