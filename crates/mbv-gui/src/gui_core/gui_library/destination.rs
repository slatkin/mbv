//! The GUI Library command's destination arms (design D2): the routing pass's
//! second pass, from the Home rows down. It lives beside the pass that routes
//! it because the destination list, not one match, sets its size: the pass
//! names every variant of the routing pass, so a new variant is a compile
//! error in both, and it hands each group of destinations to the small pass
//! that group's arms own.

use super::GuiLibraryCommand;
use super::log_ignored_library_command;
use super::shows;
use crate::gui_core::GuiCore;

impl GuiCore {
    /// The Library command's destination arms (design D2), from the Home rows
    /// down: the dispatch's second pass, which hands its groups to the three
    /// passes below so one match stays readable.
    pub(super) fn dispatch_gui_library_destination_command(&mut self, command: GuiLibraryCommand) {
        match command {
            // The Home rows, the Shows tree, and the Refresh that reloads
            // either list: the two list destinations whose steps are the core's
            // own row steps.
            GuiLibraryCommand::HomeSelect { .. }
            | GuiLibraryCommand::HomePlay { .. }
            | GuiLibraryCommand::HomeQueue { .. }
            | GuiLibraryCommand::ShowsSelect { .. }
            | GuiLibraryCommand::ShowsSelectSeason { .. }
            | GuiLibraryCommand::ShowsPlay { .. }
            | GuiLibraryCommand::ShowsQueue { .. }
            | GuiLibraryCommand::ShowsOpenInView { .. }
            | GuiLibraryCommand::Refresh { .. } => self.dispatch_gui_home_shows_command(command),
            // The provider destinations: one pass per catalog's own steps.
            GuiLibraryCommand::EmbySelect { .. }
            | GuiLibraryCommand::EmbyPlay { .. }
            | GuiLibraryCommand::EmbyQueue { .. }
            | GuiLibraryCommand::AbsSelect { .. }
            | GuiLibraryCommand::AbsPlay { .. }
            | GuiLibraryCommand::AbsPlayChapter { .. }
            | GuiLibraryCommand::AbsQueue { .. }
            | GuiLibraryCommand::PodcastsSelect { .. }
            | GuiLibraryCommand::PodcastsPlay { .. }
            | GuiLibraryCommand::PodcastsQueue { .. }
            | GuiLibraryCommand::FeedsSelect { .. }
            | GuiLibraryCommand::FeedsPlay { .. }
            | GuiLibraryCommand::FeedsQueue { .. } => self.dispatch_gui_provider_command(command),
            // The commands the panel itself sends, which name no destination.
            GuiLibraryCommand::SearchQuery { .. }
            | GuiLibraryCommand::SearchActivate { .. }
            | GuiLibraryCommand::SearchClose
            | GuiLibraryCommand::RowsInView { .. }
            | GuiLibraryCommand::OpenUrl { .. } => self.dispatch_gui_panel_command(command),
            // The routing pass's arms, which this pass never receives: the
            // dispatch routes them before it hands the command over. Each is
            // named, so a new variant is a compile error in every pass, and
            // the one that reaches here is the routing mistake the log
            // reports rather than a silent drop.
            GuiLibraryCommand::OpenDestination { .. }
            | GuiLibraryCommand::CollectionToggle { .. }
            | GuiLibraryCommand::CollectionAll { .. }
            | GuiLibraryCommand::ListSort { .. }
            | GuiLibraryCommand::LibrarySetView { .. }
            | GuiLibraryCommand::ShowDetail { .. }
            | GuiLibraryCommand::SetFavourite { .. }
            | GuiLibraryCommand::OpenAlbum { .. }
            | GuiLibraryCommand::Play { .. }
            | GuiLibraryCommand::AddToQueue { .. }
            | GuiLibraryCommand::ShuffleArtist { .. }
            | GuiLibraryCommand::ShowArtist { .. }
            | GuiLibraryCommand::PlayArtist { .. }
            | GuiLibraryCommand::AddArtistToQueue { .. }
            | GuiLibraryCommand::PlayArtistTrack { .. }
            | GuiLibraryCommand::MenuEntry { .. }
            | GuiLibraryCommand::RowMenu { .. }
            | GuiLibraryCommand::Shuffle { .. } => log_ignored_library_command(
                "dispatch",
                format_args!("{command:?} is no destination command of the second pass"),
            ),
        }
    }

    /// The Home rows and the Shows tree (design D2, D12; gui-list-header-buttons
    /// design D5, D7): each arm is the core's own row step for the list, so the
    /// two flat destinations share one pass. The Refresh arm reloads the active
    /// destination, which is either list.
    fn dispatch_gui_home_shows_command(&mut self, command: GuiLibraryCommand) {
        match command {
            GuiLibraryCommand::HomeSelect { row } => self.select_gui_home_row(&row),
            GuiLibraryCommand::HomePlay { row } => self.play_gui_home_row(&row),
            GuiLibraryCommand::HomeQueue { row } => self.enqueue_gui_home_row(&row, "homeQueue"),
            GuiLibraryCommand::ShowsSelect { lib_idx, row } => {
                self.select_gui_shows_row(lib_idx, &row);
            }
            GuiLibraryCommand::ShowsSelectSeason { lib_idx, season_id } => {
                self.gui_tv_select_season(lib_idx, &season_id);
            }
            GuiLibraryCommand::ShowsPlay { lib_idx, item_id } => {
                shows::play_shows_item(&mut self.app, &self.gui, lib_idx, &item_id);
            }
            GuiLibraryCommand::ShowsQueue { lib_idx, item_id } => {
                shows::enqueue_shows_item(&mut self.app, &self.gui, lib_idx, &item_id);
            }
            GuiLibraryCommand::ShowsOpenInView {
                lib_idx,
                series,
                seasons,
            } => self.apply_gui_shows_open_in_view(lib_idx, series, seasons),
            GuiLibraryCommand::Refresh { album_ids } => {
                self.refresh_gui_destination(&album_ids);
            }
            // The other passes' arms, and the routing pass's own: the dispatch
            // hands each to the pass that owns it, or routes it before any pass
            // runs, so the one that reaches here is the routing mistake the log
            // reports rather than a silent drop.
            GuiLibraryCommand::EmbySelect { .. }
            | GuiLibraryCommand::EmbyPlay { .. }
            | GuiLibraryCommand::EmbyQueue { .. }
            | GuiLibraryCommand::AbsSelect { .. }
            | GuiLibraryCommand::AbsPlay { .. }
            | GuiLibraryCommand::AbsPlayChapter { .. }
            | GuiLibraryCommand::AbsQueue { .. }
            | GuiLibraryCommand::PodcastsSelect { .. }
            | GuiLibraryCommand::PodcastsPlay { .. }
            | GuiLibraryCommand::PodcastsQueue { .. }
            | GuiLibraryCommand::FeedsSelect { .. }
            | GuiLibraryCommand::FeedsPlay { .. }
            | GuiLibraryCommand::FeedsQueue { .. }
            | GuiLibraryCommand::SearchQuery { .. }
            | GuiLibraryCommand::SearchActivate { .. }
            | GuiLibraryCommand::SearchClose
            | GuiLibraryCommand::RowsInView { .. }
            | GuiLibraryCommand::OpenUrl { .. }
            | GuiLibraryCommand::OpenDestination { .. }
            | GuiLibraryCommand::CollectionToggle { .. }
            | GuiLibraryCommand::CollectionAll { .. }
            | GuiLibraryCommand::ListSort { .. }
            | GuiLibraryCommand::LibrarySetView { .. }
            | GuiLibraryCommand::ShowDetail { .. }
            | GuiLibraryCommand::SetFavourite { .. }
            | GuiLibraryCommand::OpenAlbum { .. }
            | GuiLibraryCommand::Play { .. }
            | GuiLibraryCommand::AddToQueue { .. }
            | GuiLibraryCommand::ShuffleArtist { .. }
            | GuiLibraryCommand::ShowArtist { .. }
            | GuiLibraryCommand::PlayArtist { .. }
            | GuiLibraryCommand::AddArtistToQueue { .. }
            | GuiLibraryCommand::PlayArtistTrack { .. }
            | GuiLibraryCommand::MenuEntry { .. }
            | GuiLibraryCommand::RowMenu { .. }
            | GuiLibraryCommand::Shuffle { .. } => log_ignored_library_command(
                "dispatch",
                format_args!("{command:?} is no Home or Shows command of the second pass"),
            ),
        }
    }

    /// The provider destinations' arms (rows 8.1, 9.1, 10.1, 11.1): the plain
    /// Emby library, the Audiobookshelf book list's play, chapter, and enqueue,
    /// the podcast list's select, play, and enqueue, and the one feed tab the
    /// Client holds.
    fn dispatch_gui_provider_command(&mut self, command: GuiLibraryCommand) {
        match command {
            GuiLibraryCommand::EmbySelect { lib_idx, row } => {
                self.gui_emby_select(lib_idx, &row);
            }
            GuiLibraryCommand::EmbyPlay { lib_idx, item_id } => {
                self.play_gui_emby_item(lib_idx, &item_id);
            }
            GuiLibraryCommand::EmbyQueue { lib_idx, item_id } => {
                self.enqueue_gui_emby_item(lib_idx, &item_id);
            }
            GuiLibraryCommand::AbsSelect { lib_idx, row } => {
                self.gui_abs_select(lib_idx, &row);
            }
            GuiLibraryCommand::AbsPlay { lib_idx, item_id } => {
                self.play_gui_abs_book(lib_idx, &item_id);
            }
            GuiLibraryCommand::AbsPlayChapter {
                lib_idx,
                book_id,
                chapter,
            } => self.play_gui_abs_chapter(lib_idx, &book_id, chapter),
            GuiLibraryCommand::AbsQueue { lib_idx, item_id } => {
                self.enqueue_gui_abs_book(lib_idx, &item_id);
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
            // The Feeds destination's own commands (row 11.1): the one feed tab
            // the Client holds, so no library index rides them.
            GuiLibraryCommand::FeedsSelect { row } => self.gui_feeds_select(&row),
            GuiLibraryCommand::FeedsPlay { item_id } => self.play_gui_feed_row(&item_id),
            GuiLibraryCommand::FeedsQueue { item_id } => self.enqueue_gui_feed_row(&item_id),
            // The other passes' arms, and the routing pass's own: the dispatch
            // hands each to the pass that owns it, or routes it before any pass
            // runs, so the one that reaches here is the routing mistake the log
            // reports rather than a silent drop.
            GuiLibraryCommand::HomeSelect { .. }
            | GuiLibraryCommand::HomePlay { .. }
            | GuiLibraryCommand::HomeQueue { .. }
            | GuiLibraryCommand::ShowsSelect { .. }
            | GuiLibraryCommand::ShowsSelectSeason { .. }
            | GuiLibraryCommand::ShowsPlay { .. }
            | GuiLibraryCommand::ShowsQueue { .. }
            | GuiLibraryCommand::ShowsOpenInView { .. }
            | GuiLibraryCommand::Refresh { .. }
            | GuiLibraryCommand::SearchQuery { .. }
            | GuiLibraryCommand::SearchActivate { .. }
            | GuiLibraryCommand::SearchClose
            | GuiLibraryCommand::RowsInView { .. }
            | GuiLibraryCommand::OpenUrl { .. }
            | GuiLibraryCommand::OpenDestination { .. }
            | GuiLibraryCommand::CollectionToggle { .. }
            | GuiLibraryCommand::CollectionAll { .. }
            | GuiLibraryCommand::ListSort { .. }
            | GuiLibraryCommand::LibrarySetView { .. }
            | GuiLibraryCommand::ShowDetail { .. }
            | GuiLibraryCommand::SetFavourite { .. }
            | GuiLibraryCommand::OpenAlbum { .. }
            | GuiLibraryCommand::Play { .. }
            | GuiLibraryCommand::AddToQueue { .. }
            | GuiLibraryCommand::ShuffleArtist { .. }
            | GuiLibraryCommand::ShowArtist { .. }
            | GuiLibraryCommand::PlayArtist { .. }
            | GuiLibraryCommand::AddArtistToQueue { .. }
            | GuiLibraryCommand::PlayArtistTrack { .. }
            | GuiLibraryCommand::MenuEntry { .. }
            | GuiLibraryCommand::RowMenu { .. }
            | GuiLibraryCommand::Shuffle { .. } => log_ignored_library_command(
                "dispatch",
                format_args!("{command:?} is no provider command of the second pass"),
            ),
        }
    }

    /// The commands the panel itself sends (design D12, row 12.1;
    /// gui-delineated-rows design D7; the Hero's `IMDb` link): they name no
    /// destination, so they ride one pass of their own.
    fn dispatch_gui_panel_command(&mut self, command: GuiLibraryCommand) {
        match command {
            GuiLibraryCommand::SearchQuery { query } => self.gui_search_query(query),
            GuiLibraryCommand::SearchActivate { id, item_type } => {
                self.gui_search_activate(id, item_type);
            }
            GuiLibraryCommand::SearchClose => self.gui_search_close(),
            GuiLibraryCommand::RowsInView { scope, ids } => {
                self.apply_gui_rows_in_view(scope, ids);
            }
            GuiLibraryCommand::OpenUrl { url } => Self::open_gui_library_url(&url),
            // The other passes' arms, and the routing pass's own: the dispatch
            // hands each to the pass that owns it, or routes it before any pass
            // runs, so the one that reaches here is the routing mistake the log
            // reports rather than a silent drop.
            GuiLibraryCommand::HomeSelect { .. }
            | GuiLibraryCommand::HomePlay { .. }
            | GuiLibraryCommand::HomeQueue { .. }
            | GuiLibraryCommand::ShowsSelect { .. }
            | GuiLibraryCommand::ShowsSelectSeason { .. }
            | GuiLibraryCommand::ShowsPlay { .. }
            | GuiLibraryCommand::ShowsQueue { .. }
            | GuiLibraryCommand::ShowsOpenInView { .. }
            | GuiLibraryCommand::Refresh { .. }
            | GuiLibraryCommand::EmbySelect { .. }
            | GuiLibraryCommand::EmbyPlay { .. }
            | GuiLibraryCommand::EmbyQueue { .. }
            | GuiLibraryCommand::AbsSelect { .. }
            | GuiLibraryCommand::AbsPlay { .. }
            | GuiLibraryCommand::AbsPlayChapter { .. }
            | GuiLibraryCommand::AbsQueue { .. }
            | GuiLibraryCommand::PodcastsSelect { .. }
            | GuiLibraryCommand::PodcastsPlay { .. }
            | GuiLibraryCommand::PodcastsQueue { .. }
            | GuiLibraryCommand::FeedsSelect { .. }
            | GuiLibraryCommand::FeedsPlay { .. }
            | GuiLibraryCommand::FeedsQueue { .. }
            | GuiLibraryCommand::OpenDestination { .. }
            | GuiLibraryCommand::CollectionToggle { .. }
            | GuiLibraryCommand::CollectionAll { .. }
            | GuiLibraryCommand::ListSort { .. }
            | GuiLibraryCommand::LibrarySetView { .. }
            | GuiLibraryCommand::ShowDetail { .. }
            | GuiLibraryCommand::SetFavourite { .. }
            | GuiLibraryCommand::OpenAlbum { .. }
            | GuiLibraryCommand::Play { .. }
            | GuiLibraryCommand::AddToQueue { .. }
            | GuiLibraryCommand::ShuffleArtist { .. }
            | GuiLibraryCommand::ShowArtist { .. }
            | GuiLibraryCommand::PlayArtist { .. }
            | GuiLibraryCommand::AddArtistToQueue { .. }
            | GuiLibraryCommand::PlayArtistTrack { .. }
            | GuiLibraryCommand::MenuEntry { .. }
            | GuiLibraryCommand::RowMenu { .. }
            | GuiLibraryCommand::Shuffle { .. } => log_ignored_library_command(
                "dispatch",
                format_args!("{command:?} is no panel command of the second pass"),
            ),
        }
    }
}
