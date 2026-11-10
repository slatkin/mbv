//! The GUI Library panel's Client-core half (add-gui-library-panel design D2,
//! D5): the semantic commands the window content sends, the public `App` entry
//! that applies each one, the Music collection store beside it, and the Music
//! projection the window content paints. The window content sends intent, and
//! every entry calls the existing `App` action for it. The TUI Client never
//! calls any of them.

mod artwork;
mod audiobooks;
mod audiobooks_projection;
mod emby;
mod emby_projection;
mod feeds;
mod feeds_projection;
mod home;
mod item_menu;
mod launch;
mod podcasts;
mod podcasts_projection;
mod projection;
mod search;
mod shows;
mod shows_projection;
mod store;
mod tabs;
mod tv;

pub use self::artwork::GuiLibraryArtwork;
pub use self::audiobooks::GuiAudiobooksSelection;
pub use self::audiobooks_projection::{
    GuiAudiobooksChapterRow, GuiAudiobooksHero, GuiAudiobooksProjection, GuiAudiobooksRow,
    GuiAudiobooksWorkspace,
};
pub use self::emby::GuiEmbyLibrarySelection;
pub(in crate::app) use self::emby::{
    GuiEmbyLatestItems, GuiEmbyLatestItemsEvent, GuiEmbyRootItems, GuiEmbyRootItemsEvent,
};
pub use self::emby_projection::{
    GuiEmbyFeedGroup, GuiEmbyHero, GuiEmbyPill, GuiEmbyProjection, GuiEmbyRow, GuiEmbyRowKind,
};
pub use self::feeds::{GuiFeedPill, GuiFeedsSelection};
pub use self::feeds_projection::{
    GuiFeedEntryId, GuiFeedsHero, GuiFeedsPillOption, GuiFeedsProjection, GuiFeedsRow,
};
pub use self::home::{
    GuiArtShape, GuiHomeHero, GuiHomeItemRow, GuiHomeProjection, GuiHomeRow, GuiHomeRowId,
    GuiHomeRowKind, GuiHomeRowState,
};
pub use self::item_menu::{GuiLibraryMenuItem, GuiLibraryRowMenu};
pub use self::launch::GuiLaunchOpen;
pub use self::podcasts::{GuiPodcastPill, GuiPodcastsSelection};
pub use self::podcasts_projection::{
    GuiPodcastEpisodeId, GuiPodcastsHero, GuiPodcastsPillOption, GuiPodcastsProjection,
    GuiPodcastsRow,
};
pub use self::projection::{
    GuiMusicAlbum, GuiMusicArtist, GuiMusicHero, GuiMusicPill, GuiMusicProjection, GuiMusicTrack,
    GuiMusicTrackTotal,
};
pub(in crate::app) use self::search::GuiSearchStore;
pub use self::search::{GuiSearchProjection, GuiSearchRow};
pub use self::shows_projection::{
    GuiShowsEpisodeRow, GuiShowsHero, GuiShowsMode, GuiShowsPill, GuiShowsProjection, GuiShowsRow,
    GuiShowsSeasonNode, GuiShowsSeasonPill, GuiShowsSeries, GuiShowsWorkspace,
};
pub(in crate::app) use self::store::{GuiGroupAlbums, GuiGroupAlbumsEvent};
pub use self::tabs::{GuiDestinationKind, GuiLibraryTab, GuiLibraryTabs};
pub use self::tv::GuiTvShowsSelection;

use mbv_emby_model::EmbyItem;
use mbv_ui_model::settings::PanelFocus;
use mbv_ui_model::ui_util::is_playable;

use crate::app::App;

/// One request from the GUI Library panel's window content (design D2): the
/// `library` command carries it into the Client core, and
/// [`App::apply_gui_library_command`] applies it. Item identities are stable
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
    /// Select the one collection group of the Grouped Music library `lib_idx`
    /// (design D5, D6): one group is always selected, as in the TUI.
    SelectCollection { lib_idx: usize, group_id: String },
    /// Show the item's detail in the Hero (design D2).
    ShowDetail { item_id: String },
    /// Open an album node: load its tracks when the cache does not hold them
    /// (design D5).
    OpenAlbum { album_id: String },
    /// Take Panel focus for the Library panel (design D4).
    TakeFocus,
    /// Take Panel focus for the Queue panel (design D4): Tab across the panel
    /// boundary's far end. The mirror of [`GuiLibraryCommand::TakeFocus`],
    /// whose press path moves focus back the other way.
    TakeQueueFocus,
    /// Play the item (design D8).
    Play { item_id: String },
    /// Play the item next (design D8): the shared play-next admission, so the
    /// item enters directly after the active slot.
    PlayNext { item_id: String },
    /// Add the item to the queue (design D8).
    AddToQueue { item_id: String },
    /// Shuffle the artist whose albums are `album_ids` (design D8).
    ShuffleArtist { album_ids: Vec<String> },
    /// Run the item menu's entry at `index` (design D8): the entry the frame
    /// painted for the item, run through the shared context-action executor.
    MenuEntry { item_id: String, index: usize },
    /// Carry the item's full menu in the frame (the right-click item menu):
    /// the next frame's `rowMenu` section names the item and carries the
    /// TUI menu builder's entries for it, and a chosen entry runs through the
    /// same `MenuEntry` command as the Hero's menu carries.
    RowMenu { item_id: String },
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
    /// command for the front end's Play next label, resolved through the
    /// same Home enqueue path as the add — the Owner owns the queue, and the
    /// Home rows have no library the play-next admission resolves against,
    /// so the row tail-appends like `a`.
    HomePlayNext { row: home::GuiHomeRowId },
    /// Enqueue a Home row (design D12): `a` on the row, through the Home
    /// flat list's enqueue path.
    HomeQueue { row: home::GuiHomeRowId },
    /// Set the Shows destination's view mode (design D12, the USER-APPROVED
    /// GUI Shows design): the root level loads through the TUI's own mode
    /// load. A `lib_idx` that names no Shows library changes nothing.
    ShowsSetMode {
        lib_idx: usize,
        mode: shows_projection::GuiShowsMode,
    },
    /// Select a Shows row (design D12): the row the Hero shows — the All
    /// series mode's series (whose select runs the series-detail load), or a
    /// flat mode's episode or premiere row. A row the active mode's list no
    /// longer holds changes nothing.
    ShowsSelect { lib_idx: usize, row: String },
    /// Select the season whose episodes the Hero's Workspace shows (design
    /// D12): the season-episodes load starts when the cache does not hold
    /// them.
    ShowsSelectSeason { lib_idx: usize, season_id: String },
    /// Play a Shows episode (design D12): Enter on the row, or the
    /// double-click a front end sends with this same command. The shared
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
    /// Set the feed-view library's selected group (design D12, row 8.1):
    /// the TUI's own selection path, so the session cache, the stale-drop,
    /// and the lazy fetch behave as the TUI pill's press does. A `lib_idx`
    /// that names no feed-view library changes nothing.
    EmbySetFeedGroup {
        lib_idx: usize,
        group: emby_projection::GuiEmbyFeedGroup,
    },
    /// Play a plain Emby-library row (design D12, row 8.1): Enter on the
    /// row, or the double-click a front end sends with this same command.
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
    /// or the double-click a front end sends with this same command. The
    /// queue is replaced with the book at its saved position, through the
    /// TUI's book play path and the Owner's Audiobookshelf admission.
    AbsPlay { lib_idx: usize, item_id: String },
    /// Play or seek from an Audiobooks chapter (design D12, row 9.1): Enter
    /// on a Workspace chapter row, or the double-click a front end sends
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
    /// Set the Podcasts destination's pill (design D12, row 10.1): Latest, or
    /// one show by its stable `library_item_id`. The pill row is single-select,
    /// and the selected pill's own loads start through the TUI's own loaders.
    /// A show the published show list no longer carries changes nothing.
    PodcastsSetPill {
        lib_idx: usize,
        pill: podcasts::GuiPodcastPill,
    },
    /// Select the Podcasts episode the Hero shows (design D12, row 10.1): a
    /// press on the row. The row id is the frame's opaque episode identity, so
    /// a stale frame's row changes nothing.
    PodcastsSelect { lib_idx: usize, row: String },
    /// Play a Podcasts episode (design D12, row 10.1): Enter on the row, or the
    /// double-click a front end sends with this same command. The queue is
    /// replaced with the episode through the TUI's episode play path and the
    /// Owner's Audiobookshelf admission.
    PodcastsPlay { lib_idx: usize, item_id: String },
    /// Add a Podcasts episode to the queue (design D12, row 10.1): the TUI's
    /// enqueue path, through the Owner's Audiobookshelf admission.
    PodcastsQueue { lib_idx: usize, item_id: String },
    /// Set the Feeds destination's pill (design D12, row 11.1): Latest, or one
    /// subscription by its URL. The pill row is single-select, the state pills
    /// are dropped, and a subscription the tab no longer holds changes nothing.
    /// A Client holds one feed tab, so no library index rides here.
    FeedsSetPill { pill: feeds::GuiFeedPill },
    /// Select the Feeds entry the Hero shows (design D12, row 11.1): a press
    /// on the row. The row id is the frame's opaque entry identity, so a stale
    /// frame's row changes nothing.
    FeedsSelect { row: String },
    /// Play a Feeds entry (design D12, row 11.1): Enter on the row, or the
    /// double-click a front end sends with this same command. The queue is
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
}

/// The command's row identity for a stale-frame log: a play or enqueue
/// command names a Home row, so the ignored log carries the same identity
/// the frame painted.
fn home_row_command_id(command: &str, row: &GuiHomeRowId) -> String {
    format!("{command} {row:?}")
}

impl App {
    /// Apply one GUI Library command (design D2): each arm calls the existing
    /// `App` action for the intent, so the TUI's own paths do not change.
    /// Every command logs its receipt (`gui_library.command`), and one that
    /// the App's data cannot act on logs why it was ignored, so a live
    /// session's Library path is visible in the GUI log.
    pub fn apply_gui_library_command(&mut self, command: GuiLibraryCommand) {
        Self::log_library_command(&command);
        match command {
            GuiLibraryCommand::OpenDestination { position } => {
                if self.presents_open_position(position) {
                    self.set_library_tab(position);
                    self.open_gui_active_destination();
                } else {
                    Self::log_ignored_library_command(
                        "openDestination",
                        format_args!("the position {position} names no presented destination"),
                    );
                }
            }
            GuiLibraryCommand::SelectCollection { lib_idx, group_id } => {
                self.select_gui_library_collection(lib_idx, group_id);
            }
            GuiLibraryCommand::ShowDetail { item_id } => self.show_gui_library_detail(&item_id),
            GuiLibraryCommand::OpenAlbum { album_id } => self.open_gui_library_album(album_id),
            GuiLibraryCommand::TakeFocus => self.set_gui_panel_focus(PanelFocus::Library),
            GuiLibraryCommand::TakeQueueFocus => self.set_gui_panel_focus(PanelFocus::Queue),
            GuiLibraryCommand::Play { item_id } => match self.gui_library_item(&item_id) {
                Some((lib_idx, item)) => self.play_gui_library_item(lib_idx, item),
                None => Self::log_stale_library_item("play", &item_id),
            },
            GuiLibraryCommand::PlayNext { item_id } => match self.gui_library_item(&item_id) {
                Some((lib_idx, item)) => self.play_lib_item_next(lib_idx, item),
                None => Self::log_stale_library_item("playNext", &item_id),
            },
            GuiLibraryCommand::AddToQueue { item_id } => {
                self.add_gui_library_item_to_queue(&item_id);
            }
            GuiLibraryCommand::ShuffleArtist { album_ids } => {
                self.shuffle_gui_artist(&album_ids);
            }
            // Row 3.6 runs the menu entry the frame carried through the shared
            // context-action executor.
            GuiLibraryCommand::MenuEntry { item_id, index } => {
                self.run_gui_library_menu_entry(&item_id, index);
            }
            GuiLibraryCommand::RowMenu { item_id } => {
                self.gui_library_row_menu_target = Some(item_id);
            }
            GuiLibraryCommand::HomeSelect { row } => self.select_gui_home_row(&row),
            GuiLibraryCommand::HomePlay { row } => self.play_gui_home_row(&row),
            GuiLibraryCommand::HomePlayNext { row } => {
                self.enqueue_gui_home_row(&row, "homePlayNext");
            }
            GuiLibraryCommand::HomeQueue { row } => self.enqueue_gui_home_row(&row, "homeQueue"),
            GuiLibraryCommand::ShowsSetMode { lib_idx, mode } => {
                self.gui_tv_set_mode(lib_idx, &mbv_queue::TvContentMode::from(mode));
            }
            GuiLibraryCommand::ShowsSelect { lib_idx, row } => {
                self.select_gui_shows_row(lib_idx, &row);
            }
            GuiLibraryCommand::ShowsSelectSeason { lib_idx, season_id } => {
                self.gui_tv_select_season(lib_idx, &season_id);
            }
            GuiLibraryCommand::ShowsPlay { lib_idx, item_id } => {
                self.play_gui_shows_item(lib_idx, &item_id);
            }
            GuiLibraryCommand::ShowsPlayNext { lib_idx, item_id } => {
                self.play_next_gui_shows_item(lib_idx, &item_id);
            }
            GuiLibraryCommand::ShowsQueue { lib_idx, item_id } => {
                self.enqueue_gui_shows_item(lib_idx, &item_id);
            }
            GuiLibraryCommand::EmbySelect { lib_idx, row } => {
                self.gui_emby_select(lib_idx, &row);
            }
            GuiLibraryCommand::EmbySetFeedGroup { lib_idx, group } => {
                self.gui_emby_set_feed_group(lib_idx, &group);
            }
            GuiLibraryCommand::EmbyPlay { lib_idx, item_id } => {
                self.play_gui_emby_item(lib_idx, &item_id);
            }
            GuiLibraryCommand::EmbyPlayNext { lib_idx, item_id } => {
                self.play_next_gui_emby_item(lib_idx, &item_id);
            }
            GuiLibraryCommand::EmbyQueue { lib_idx, item_id } => {
                self.enqueue_gui_emby_item(lib_idx, &item_id);
            }
            GuiLibraryCommand::AbsSelect { lib_idx, row } => self.gui_abs_select(lib_idx, &row),
            // The Audiobookshelf destinations' groups (rows 9.1, 10.1): the
            // book list and the podcast list each send their own commands.
            command @ (GuiLibraryCommand::AbsPlay { .. }
            | GuiLibraryCommand::AbsPlayChapter { .. }
            | GuiLibraryCommand::AbsQueue { .. }
            | GuiLibraryCommand::PodcastsSetPill { .. }
            | GuiLibraryCommand::PodcastsSelect { .. }
            | GuiLibraryCommand::PodcastsPlay { .. }
            | GuiLibraryCommand::PodcastsQueue { .. }) => {
                self.apply_gui_audiobookshelf_command(command);
            }
            // The Feeds destination's own commands (row 11.1): the one feed
            // tab the Client holds, so no library index rides them.
            command @ (GuiLibraryCommand::FeedsSetPill { .. }
            | GuiLibraryCommand::FeedsSelect { .. }
            | GuiLibraryCommand::FeedsPlay { .. }
            | GuiLibraryCommand::FeedsQueue { .. }) => {
                self.apply_gui_feeds_command(command);
            }
            // The Search field's commands (design D12, row 12.1): the field
            // is cross-destination, so no library index rides them.
            command @ (GuiLibraryCommand::SearchQuery { .. }
            | GuiLibraryCommand::SearchActivate { .. }
            | GuiLibraryCommand::SearchClose) => self.apply_gui_search_command(command),
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

    /// Apply one of the Audiobookshelf destinations' commands (rows 9.1,
    /// 10.1): the book list's play, chapter, and enqueue, and the podcast
    /// list's pill, select, play, and enqueue. The caller routes only these
    /// variants here, so the trailing arm is the group's own no-op for a
    /// routing mistake — it logs and changes nothing rather than dropping the
    /// request silently.
    fn apply_gui_audiobookshelf_command(&mut self, command: GuiLibraryCommand) {
        match command {
            GuiLibraryCommand::AbsPlay { lib_idx, item_id } => {
                self.play_gui_abs_book(lib_idx, &item_id);
            }
            GuiLibraryCommand::AbsPlayChapter {
                lib_idx,
                book_id,
                chapter,
            } => {
                self.play_gui_abs_chapter(lib_idx, &book_id, chapter);
            }
            GuiLibraryCommand::AbsQueue { lib_idx, item_id } => {
                self.enqueue_gui_abs_book(lib_idx, &item_id);
            }
            GuiLibraryCommand::PodcastsSetPill { lib_idx, pill } => {
                self.gui_podcast_set_pill(lib_idx, pill);
            }
            GuiLibraryCommand::PodcastsSelect { lib_idx, row } => {
                self.select_gui_podcast_row(lib_idx, &row);
            }
            GuiLibraryCommand::PodcastsPlay { lib_idx, item_id } => {
                self.play_gui_podcast_row(lib_idx, &item_id);
            }
            GuiLibraryCommand::PodcastsQueue { lib_idx, item_id } => {
                self.enqueue_gui_podcast_row(lib_idx, &item_id);
            }
            other => Self::log_ignored_library_command(
                "audiobookshelf",
                format_args!("{other:?} is no Audiobookshelf destination command"),
            ),
        }
    }

    /// Apply one of the Feeds destination's commands (row 11.1): the pill, the
    /// select, the play, and the enqueue. The caller routes only these
    /// variants here, so the trailing arm is the group's own no-op for a
    /// routing mistake — it logs and changes nothing rather than dropping the
    /// request silently.
    fn apply_gui_feeds_command(&mut self, command: GuiLibraryCommand) {
        match command {
            GuiLibraryCommand::FeedsSetPill { pill } => self.gui_feeds_set_pill(pill),
            GuiLibraryCommand::FeedsSelect { row } => self.gui_feeds_select(&row),
            GuiLibraryCommand::FeedsPlay { item_id } => self.play_gui_feed_row(&item_id),
            GuiLibraryCommand::FeedsQueue { item_id } => self.enqueue_gui_feed_row(&item_id),
            other => Self::log_ignored_library_command(
                "feeds",
                format_args!("{other:?} is no Feeds destination command"),
            ),
        }
    }

    /// Apply one of the Search field's commands (design D12, row 12.1): the
    /// query the typing sends, the row an Enter or a press activates, and the
    /// close. The caller routes only these variants here, so the trailing arm
    /// is the group's own no-op for a routing mistake — it logs and changes
    /// nothing rather than dropping the request silently.
    fn apply_gui_search_command(&mut self, command: GuiLibraryCommand) {
        match command {
            GuiLibraryCommand::SearchQuery { query } => self.gui_search_query(query),
            GuiLibraryCommand::SearchActivate { id, item_type } => {
                self.gui_search_activate(id, item_type);
            }
            GuiLibraryCommand::SearchClose => self.gui_search_close(),
            other => Self::log_ignored_library_command(
                "search",
                format_args!("{other:?} is no Search command"),
            ),
        }
    }

    /// The open step the active destination's kind needs (design D12): the
    /// Shows destination seeds its mode selection with the TUI default
    /// rule's resolution and starts its root load when the stack holds no
    /// level, the plain Emby-library destination starts its root-list load
    /// (row 8.1), and the Feeds destination settles its stored pill (row
    /// 11.1, whose entries the shared startup fetch already fills). The other
    /// destinations' loads ride the tab settlement and the frame path.
    fn open_gui_active_destination(&mut self) {
        match self.gui_active_destination() {
            Some(GuiDestinationKind::Shows { lib_idx }) => self.gui_tv_open(lib_idx),
            Some(GuiDestinationKind::Library { lib_idx }) => self.gui_emby_open(lib_idx),
            Some(GuiDestinationKind::Audiobooks { lib_idx }) => self.gui_abs_open(lib_idx),
            Some(GuiDestinationKind::Podcasts { lib_idx }) => self.gui_podcast_open(lib_idx),
            Some(GuiDestinationKind::Feeds) => self.gui_feeds_open(),
            _ => {}
        }
    }

    /// Select a Home row (design D2, D12): the row the Hero shows. The
    /// selection is the core's, so the frame carries it back and the launch
    /// of a window-content reload keeps it.
    fn select_gui_home_row(&mut self, row: &GuiHomeRowId) {
        self.gui_home_selection = Some(row.clone());
    }

    /// Play a Home row (design D12): the Home flat list's play path, whose
    /// resume behaviour the row's Continue Watching membership decides. A
    /// stale frame's row, one whose item the App's Home data no longer
    /// holds, changes nothing, as every stale command's item arm does.
    fn play_gui_home_row(&mut self, row: &GuiHomeRowId) {
        let from_cw = row.is_continue();
        if let Some(item) = self.home_stable_target(&row.clone().into_home_item_target()) {
            self.home_play_target(item, from_cw);
        } else {
            Self::log_stale_library_item("homePlay", &home_row_command_id("homePlay", row));
        }
    }

    /// Enqueue a Home row (design D12): the Home flat list's enqueue path,
    /// for both the explicit add and the play-next label — the Owner owns
    /// the queue, and the command's intent reaches it through the same
    /// resolved row, never a direct queue mutation. `name` is the command's
    /// intent, for the stale-frame log the item arms share.
    fn enqueue_gui_home_row(&mut self, row: &GuiHomeRowId, name: &str) {
        let logged = home_row_command_id(name, row);
        let from_cw = row.is_continue();
        if let Some(item) = self.home_stable_target(&row.clone().into_home_item_target()) {
            self.home_enqueue_target(item, from_cw);
        } else {
            Self::log_stale_library_item(name, &logged);
        }
    }

    /// Log the Library command the active destination cannot serve: the
    /// command's intent arrives, the frame it rode is stale, or the position
    /// names no destination the GUI presents.
    fn log_ignored_library_command(command: &str, reason: impl std::fmt::Display) {
        tracing::info!(
            name: "gui_library.command.ignored",
            target: "gui_library",
            command,
            reason = %reason,
            "the Library command changed nothing"
        );
    }

    /// The command's row identity for a stale-frame log: a select, play, or
    /// enqueue command names a Home row, so the ignored log carries the same
    /// identity the frame painted.
    /// Log the item-named Library command whose item the App's Service data
    /// no longer holds (design D2: a stale frame addresses the item it
    /// painted or nothing).
    fn log_stale_library_item(command: &str, item_id: &str) {
        Self::log_ignored_library_command(
            command,
            format_args!("the item {item_id} is not in the Service data"),
        );
    }

    /// Set Panel focus for the GUI Library panel (design D4): the take-focus
    /// effect, sent by a press inside the panel and by a Tab across the panel
    /// boundary. The core thread sets its `FocusStack` with the same value, so
    /// the frame's focus and the App's key gates cannot disagree.
    pub fn set_gui_panel_focus(&mut self, focus: PanelFocus) {
        self.set_panel_focus(focus);
    }

    /// The Panel that holds focus (design D4, D9): the value the GUI Client's
    /// `FocusStack` and the frame's focus follow, and the one the launch
    /// location saves.
    #[must_use]
    pub fn gui_panel_focus(&self) -> PanelFocus {
        self.effective_panel_focus()
    }

    /// The id of the Emby item playing at the playhead (add-gui-library-panel,
    /// the Library's playing row): the Queue panel's own authority — the Owner
    /// status and the playback queue's item at the active index — read once
    /// for the frame's Library section, so the Library's playing row and the
    /// Queue's active slot cannot disagree. `None` while idle or while the
    /// active slot holds no Emby item.
    #[must_use]
    pub fn gui_playing_track_id(&self) -> Option<String> {
        let state = self.displayed_playback_state();
        if !state.active {
            return None;
        }
        match self.queue_item_at_playhead(state) {
            Some(mbv_queue::QueueItem::Emby(item)) => Some(item.id.clone()),
            Some(_) | None => None,
        }
    }

    /// Show `item_id` in the Hero (design D2, D13): the album the window content
    /// picked becomes the selected group's pick, so the pill reopens on it. An
    /// id the current Service data does not hold leaves the Hero's item as it
    /// was, because the frame that carried the id is stale.
    fn show_gui_library_detail(&mut self, item_id: &str) {
        let Some((lib_idx, item)) = self.gui_library_item(item_id) else {
            Self::log_stale_library_item("showDetail", item_id);
            return;
        };
        let Some(group_id) = self.gui_selected_group(lib_idx) else {
            return;
        };
        let key = (lib_idx, group_id);
        if self.gui_library_picks.get(&key) == Some(&item.id) {
            return;
        }
        self.gui_library_picks.insert(key, item.id);
        // A pick move is library navigation: it closes the image warm gate, so
        // fast arrow traffic warms no neighbour art (the TUI's own rule).
        self.stamp_navigation();
    }

    /// Open an album node (design D5, D7): the shared album-track fetch fills
    /// the cache the tracklist reads, and it is a no-op once the cache holds
    /// the album.
    fn open_gui_library_album(&mut self, album_id: String) {
        self.fetch_album_tracks(album_id);
    }

    /// Shuffle the artist whose albums are `album_ids` (design D8): the
    /// albums the window content's union holds, played through the shared
    /// album expansion with shuffle on, replacing the queue directly — the
    /// explicit Library shuffle behaves like the Library play, whose
    /// replace-confirm is skipped (the saved-playlist prompt still gates the
    /// replacement). Ids the store and the navigation state no longer hold
    /// are skipped.
    fn shuffle_gui_artist(&mut self, album_ids: &[String]) {
        let albums: Vec<EmbyItem> = album_ids
            .iter()
            .filter_map(|album_id| self.gui_library_item(album_id))
            .map(|(_lib_idx, album)| album)
            .collect();
        if albums.is_empty() {
            Self::log_ignored_library_command(
                "shuffleArtist",
                format_args!("the album ids name no album the Service data holds"),
            );
            return;
        }
        self.play_music_albums(albums, true, false);
    }

    /// Play the item (design D8), as the GUI Library's explicit play: the
    /// populated-queue replace-confirm is skipped — it is a TUI overlay the
    /// GUI Client never paints, so a play gated behind it was swallowed and
    /// the Hero's Play button did nothing over a populated queue. An album
    /// (folder) replaces the queue with its whole playable content through
    /// the shared folder path; a track replaces the queue with the album's
    /// cached playable tracks in disc/track order starting at the selected
    /// track when autoload is on (the library playback policy's autoload
    /// rule, through the same cache the add path reads), and otherwise takes
    /// the shared select-item path.
    fn play_gui_library_item(&mut self, lib_idx: usize, item: EmbyItem) {
        if item.is_folder {
            let ct = self.libs[lib_idx].library.collection_type.clone();
            self.play_folder_gated(&item.id, ct, false);
            return;
        }
        if !is_playable(&item) {
            return;
        }
        let autoload = self.config.lock().unwrap().autoload;
        if autoload
            && let Some(album_id) = self.gui_track_album_id(&item.id)
            && let Some((tracks, start_idx)) =
                self.resolve_playable_album_tracks(&album_id, &item.id)
        {
            self.play_items_routed(tracks, start_idx, mbv_queue::QueueSource::Album);
            return;
        }
        self.select_item(lib_idx, item);
    }

    /// Add the item to the queue (design D8), with the library playback
    /// policy's autoload rule applied to this one route: a track whose album
    /// the track cache holds queues that album's cached playable tracks in
    /// disc/track order starting at the selected track when autoload is on,
    /// and only the selected track when it is off. Every other item — an
    /// album, a folder, an item the cache does not hold — takes the shared
    /// add path unchanged. The TUI's own add paths keep their per-item
    /// appends; only the GUI Library's add applies the rule.
    fn add_gui_library_item_to_queue(&mut self, item_id: &str) {
        let Some((lib_idx, item)) = self.gui_library_item(item_id) else {
            Self::log_stale_library_item("addToQueue", item_id);
            return;
        };
        let autoload = self.config.lock().unwrap().autoload;
        if item.is_folder || !is_playable(&item) || !autoload {
            self.enqueue_lib_item(lib_idx, item);
            return;
        }
        let Some(album_id) = self.gui_track_album_id(&item.id) else {
            self.enqueue_lib_item(lib_idx, item);
            return;
        };
        let Some((tracks, start_idx)) = self.resolve_playable_album_tracks(&album_id, &item.id)
        else {
            self.enqueue_lib_item(lib_idx, item);
            return;
        };
        // The autoload block enters as one append in disc/track order, with
        // the admission checks and tail append the shared plural path runs.
        let tracks = tracks.into_iter().skip(start_idx).collect();
        self.enqueue_lib_items(lib_idx, tracks);
    }

    /// The album whose cached tracklist holds `track_id` (design D5): the
    /// tracklist a track row and the Hero's tracklist paint, keyed by the
    /// album the open-album command fetched it for. `None` when no cached
    /// tracklist holds the id — the add falls back to the single item.
    fn gui_track_album_id(&self, track_id: &str) -> Option<String> {
        self.album_tracks_cache
            .iter()
            .find_map(|(album_id, tracks)| {
                tracks
                    .iter()
                    .any(|track| track.id == track_id)
                    .then(|| album_id.clone())
            })
    }

    /// The item a command names, with the library it belongs to: the album
    /// lists the GUI store holds, then the plain Emby libraries' root lists,
    /// then the navigation levels the TUI also loads, then the picked album's
    /// cached tracks (design D5, row 8.1).
    fn gui_library_item(&self, item_id: &str) -> Option<(usize, EmbyItem)> {
        for (lib_idx, lib) in self.libs.iter().enumerate() {
            let stored = self
                .gui_stored_albums(lib_idx)
                .find(|album| album.id == item_id);
            let emby_root = || {
                self.gui_stored_emby_root_items(lib_idx)
                    .find(|item| item.id == item_id)
            };
            let browsed = || {
                lib.nav_stack
                    .iter()
                    .rev()
                    .find_map(|level| level.items.iter().find(|item| item.id == item_id))
            };
            if let Some(item) = stored.or_else(emby_root).or_else(browsed) {
                return Some((lib_idx, item.clone()));
            }
            // A series tree row names a season or an episode, which only the
            // cached series details hold (design D14).
            if let Some(item) = self.gui_shows_tree_item(item_id) {
                return Some((lib_idx, item));
            }
        }
        // A tracklist row names a track of the picked album, and only an
        // Emby library tab holds one.
        let track_lib = self.context_menu_lib_idx()?;
        self.album_tracks_cache
            .values()
            .flatten()
            .find(|track| track.id == item_id)
            .map(|track| (track_lib, track.clone()))
    }

    /// The season or episode a Shows tree row names (design D14): the cached
    /// series details hold both.
    fn gui_shows_tree_item(&self, item_id: &str) -> Option<EmbyItem> {
        self.series_detail_cache.values().find_map(|detail| {
            detail
                .seasons
                .iter()
                .chain(detail.episodes.values().flatten())
                .find(|item| item.id == item_id)
                .cloned()
        })
    }

    /// The albums the GUI store loaded for `lib_idx`'s selected groups.
    fn gui_stored_albums(&self, lib_idx: usize) -> impl Iterator<Item = &EmbyItem> {
        self.gui_music_albums
            .iter()
            .filter(move |((store_lib_idx, _group_id), _state)| *store_lib_idx == lib_idx)
            .filter_map(|(_key, state)| match state {
                GuiGroupAlbums::Loaded(albums) => Some(albums.iter()),
                GuiGroupAlbums::Loading | GuiGroupAlbums::Failed => None,
            })
            .flatten()
    }
}
