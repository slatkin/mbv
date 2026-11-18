//! The GUI Library's Shows data path (add-gui-library-panel design D12, the
//! USER-APPROVED GUI Shows design; gui-collection-dropdown design D8): the
//! per-library selection the core owns — the selected series and season — and
//! the open, select-series, and select-season actions the core's commands
//! apply. The GUI Shows destination has one view mode, the series list, so the
//! open forces the TUI's `TvContentMode::All` load and no mode command exists.
//! Every action runs the TUI's existing level-building and fetch functions, so
//! the two Clients cannot drift: the series and season loads are
//! `fetch_series_detail` and `fetch_series_season_episodes`. The TUI never
//! reads any of it.

use mbv_queue::TvContentMode;

use super::App;

/// The GUI Library's Shows selection (design D12, D8): per Shows library, the
/// selected series and the selected season. The season is never kept across a
/// series change — the season pills start on the first season. The GUI paints
/// the All series list and no other view mode, so the selection holds no mode
/// and no flat row.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GuiTvShowsSelection {
    /// The series the Hero shows, by stable Service id.
    pub series_id: Option<String>,
    /// The season whose episodes the Hero's Workspace shows, by stable
    /// Service id.
    pub season_id: Option<String>,
}

impl App {
    /// Whether `lib_idx` names an Emby TV (Shows) library: every GUI Shows
    /// method's destination guard.
    pub(in crate::app::gui_library) fn gui_tv_library(&self, lib_idx: usize) -> bool {
        self.libs
            .get(lib_idx)
            .is_some_and(|lib| lib.library.collection_type == "tvshows")
    }

    /// Open the Shows library `lib_idx` (design D12, D8): the series list —
    /// the GUI's one view mode — loads through the shared activation path and
    /// the TUI's own mode load, so a library whose TUI default or saved
    /// position is another mode lands the series list here. A `lib_idx` that
    /// names no Shows library is an ignored command.
    pub fn gui_tv_open(&mut self, lib_idx: usize) {
        if !self.gui_tv_library(lib_idx) {
            Self::log_ignored_library_command(
                "tvOpen",
                format_args!("the destination is no Shows library"),
            );
            return;
        }
        if self.libs[lib_idx].nav_stack.is_empty() {
            self.activate_library_position(lib_idx);
        }
        self.apply_tv_letter_mode(lib_idx, &TvContentMode::All);
    }

    /// Select the series the Hero shows (design D12): the selection's
    /// series, the season reset (the pills are not kept across series), and
    /// the series-detail load — the trigger that today lives only in the
    /// presenting Client's workspace push (`push_tv_workspace_content`), so
    /// a Client that keeps no shell state starts it here. A stale frame's id
    /// — no row of the library's root list, or a row that is no Series — is
    /// an ignored command.
    pub fn gui_tv_select_series(&mut self, lib_idx: usize, series_id: &str) {
        if !self.gui_tv_library(lib_idx) {
            Self::log_ignored_library_command(
                "tvSelectSeries",
                format_args!("the destination is no Shows library"),
            );
            return;
        }
        let in_root = self.libs[lib_idx].nav_stack.last().is_some_and(|level| {
            level
                .items
                .iter()
                .any(|item| item.id == series_id && item.item_type == "Series")
        });
        if !in_root {
            Self::log_stale_library_item("tvSelectSeries", series_id);
            return;
        }
        let entry = self.gui_tv_shows.entry(lib_idx).or_default();
        if entry.series_id.as_deref() != Some(series_id) {
            entry.season_id = None;
        }
        entry.series_id = Some(series_id.to_string());
        self.fetch_series_detail(series_id);
    }

    /// Select the season whose episodes the Hero's Workspace shows (design
    /// D12): the selection's season and the season-episodes load —
    /// `fetch_series_season_episodes`, which fetches the series detail
    /// first when the cache lacks it and deduplicates what is in flight or
    /// cached. A season command with no selected series is an ignored
    /// command, as is a `lib_idx` that names no Shows library.
    pub fn gui_tv_select_season(&mut self, lib_idx: usize, season_id: &str) {
        if !self.gui_tv_library(lib_idx) {
            Self::log_ignored_library_command(
                "tvSelectSeason",
                format_args!("the destination is no Shows library"),
            );
            return;
        }
        let entry = self.gui_tv_shows.entry(lib_idx).or_default();
        let Some(series_id) = entry.series_id.clone() else {
            Self::log_ignored_library_command(
                "tvSelectSeason",
                format_args!("no series is selected"),
            );
            return;
        };
        entry.season_id = Some(season_id.to_string());
        self.fetch_series_season_episodes(series_id, season_id.to_string());
    }
}
