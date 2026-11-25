//! The GUI Library's one refresh dispatcher (gui-list-header-buttons design
//! D7): the header's refresh control sends one `Refresh` command, and this
//! module resolves the active destination to the refresh site that keeps the
//! rows it paints.
//!
//! A tree refresh reloads the children of the open parent rows in view (spec
//! "A tree refresh reloads the children of the open rows in view"): Music takes
//! the open album ids from the window, and Shows takes the open series and
//! seasons from the last in-view report, which the window keeps current on each
//! scroll. No refresh starts a load for an open row out of view.
//!
//! The reused paths keep their own failure reporting: a failed refresh keeps
//! the content the destination paints, and each path keeps its published
//! behaviour.

use crate::gui_core::GuiCore;
use crate::gui_core::gui_client_state::GuiClientState;
use mbv_client_core::App;

use super::GuiDestinationKind;
use super::shows_view::GuiShowsView;
use super::{emby, projection, shows, shows_open_loads, store};

impl GuiCore {
    /// Refresh the active destination, keeping the rows it paints (design D7):
    /// `album_ids` are the open Music album rows the window paints in view, so
    /// their tracks reload in place. A session that presents no destination is
    /// an ignored command.
    pub(crate) fn refresh_gui_destination(&mut self, album_ids: &[String]) {
        let Some(kind) = self.gui_active_destination() else {
            super::log_ignored_library_command("refresh", format_args!("no destination is active"));
            return;
        };
        match kind {
            GuiDestinationKind::Home => self.app.refresh_home_content(),
            GuiDestinationKind::Music { lib_idx } => {
                refresh_music(&mut self.app, &mut self.gui, lib_idx, album_ids);
            }
            GuiDestinationKind::Shows { lib_idx } => {
                refresh_shows(&mut self.app, &mut self.gui, lib_idx);
            }
            GuiDestinationKind::Library { lib_idx } => {
                if self.app.is_feed_home_video_library(lib_idx) {
                    // A feed-group library's rows come from the targets' own
                    // lists, so the refresh refetches the targets and never
                    // the hidden root listing (design D7a).
                    self.refresh_gui_feed_group(lib_idx);
                } else {
                    emby::refetch_emby_root_items(&mut self.app, &mut self.gui, lib_idx);
                }
            }
            // The Audiobookshelf destinations' root steps read the active
            // tab's browse state, and a collection command can name a library
            // the tab is not on, so both run under the driver's own guard.
            GuiDestinationKind::Audiobooks { lib_idx } => {
                if book_library_active(&self.app, lib_idx) {
                    self.app.audiobookshelf_book_refresh();
                } else {
                    log_inactive_audiobookshelf(lib_idx);
                }
            }
            GuiDestinationKind::Podcasts { lib_idx } => {
                if podcast_library_active(&self.app, lib_idx) {
                    // The GUI root step fetches the show list alone, so the
                    // refresh passes the same argument.
                    self.app.audiobookshelf_refresh(false);
                } else {
                    log_inactive_audiobookshelf(lib_idx);
                }
            }
            GuiDestinationKind::Feeds => self.app.refresh_feeds(),
        }
    }
}

/// Refresh the Music destination (design D7): each selected group's album list,
/// then the tracks of each open album row the window paints in view.
fn refresh_music(app: &mut App, gui: &mut GuiClientState, lib_idx: usize, album_ids: &[String]) {
    let groups: Vec<String> = {
        let set = projection::effective_music_set(app, gui, lib_idx);
        projection::selected_groups(app, gui, lib_idx, &set)
            .into_iter()
            .map(str::to_string)
            .collect()
    };
    for group_id in groups {
        store::refetch_group_albums(app, gui, lib_idx, group_id);
    }
    for album_id in album_ids {
        app.refetch_album_tracks(album_id.clone());
    }
}

/// Refresh the Shows destination in the view it paints (design D7): the root
/// level's in-place refresh and the open rows' stale marks in All, one list's
/// refetch in New, and the snapshot's refetch in Upcoming.
fn refresh_shows(app: &mut App, gui: &mut GuiClientState, lib_idx: usize) {
    match shows::shows_view(gui, lib_idx) {
        GuiShowsView::All => {
            app.refresh_lib(lib_idx);
            mark_open_shows_rows_stale(app, gui, lib_idx);
            shows_open_loads::pump_shows_open_loads(app, gui);
        }
        GuiShowsView::New => {
            if let Some(library_id) = app.emby_library_id(lib_idx).map(str::to_string) {
                shows::refetch_shows_new(app, gui, library_id);
            }
        }
        GuiShowsView::Upcoming => app.refetch_tv_upcoming_snapshot(lib_idx),
    }
}

/// Mark the open Shows rows the last in-view report names stale (design D7,
/// D9): their children reload in place, and a series the report does not name
/// starts no load. A report for another library describes a list this one does
/// not paint, so it marks nothing.
fn mark_open_shows_rows_stale(app: &mut App, gui: &GuiClientState, lib_idx: usize) {
    let Some(report) = gui
        .shows_open_in_view
        .as_ref()
        .filter(|report| report.lib_idx == lib_idx)
    else {
        return;
    };
    for series_id in &report.series {
        app.mark_series_detail_stale(series_id);
    }
    for (series_id, season_id) in &report.seasons {
        app.mark_series_season_stale(series_id, season_id);
    }
}

/// Whether `lib_idx` is an Audiobooks book library the active tab is on: the
/// guard the destination driver's Audiobooks arm holds.
fn book_library_active(app: &App, lib_idx: usize) -> bool {
    app.active_audiobookshelf_library_index() == Some(lib_idx)
        && super::audiobooks::abs_book_library(app, lib_idx)
}

/// Whether `lib_idx` is a Podcasts library the active tab is on: the guard the
/// destination driver's Podcasts arm holds.
fn podcast_library_active(app: &App, lib_idx: usize) -> bool {
    app.active_audiobookshelf_library_index() == Some(lib_idx)
        && super::podcasts::podcast_library(app, lib_idx)
}

/// Log a refresh of an Audiobookshelf library the active tab is not on: the
/// guard kept it from running.
fn log_inactive_audiobookshelf(lib_idx: usize) {
    super::log_ignored_library_command(
        "refresh",
        format_args!("the Audiobookshelf library {lib_idx} is not the active tab"),
    );
}
