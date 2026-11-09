//! The GUI Library's Shows data path (add-gui-library-panel design D12, the
//! USER-APPROVED GUI Shows design): the per-library selection the core owns
//! — the active view mode, the selected series, the selected season — and
//! the open, set-mode, select-series, and select-season actions the later
//! core surface's commands apply. Every action runs the TUI's existing
//! level-building and fetch functions, so the two Clients cannot drift: the
//! mode load is `apply_tv_letter_mode` and its `spawn_tv_latest` /
//! `spawn_tv_upcoming` / `spawn_refresh` arms, and the series and season
//! loads are `fetch_series_detail` and `fetch_series_season_episodes`. The
//! TUI never reads any of it.

use mbv_queue::TvContentMode;

use super::App;

/// The GUI Library's Shows selection (design D12): per Shows library, the
/// active view mode, the selected series, and the selected season. The mode
/// defaults to the TUI rule's resolution (`resolve_tv_content_mode`), and
/// the season is never kept across a series change — the season pills start
/// on the first season. A series Hero exists only in the All series mode
/// (Latest and Upcoming are flat episode lists, and Upcoming paints no
/// Hero), so the series and season ids carry no meaning in those modes; the
/// flat modes' own selection lives in `flat_row`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GuiTvShowsSelection {
    /// The active view mode: the set-mode command's last value, else the
    /// resolved default. `None` before an open seeds it.
    pub mode: Option<TvContentMode>,
    /// The series the Hero shows, by stable Service id.
    pub series_id: Option<String>,
    /// The season whose episodes the Hero's Workspace shows, by stable
    /// Service id.
    pub season_id: Option<String>,
    /// The flat mode's selected row (design D12): the Latest episode's item
    /// id, or the Upcoming premiere's stable key the row carries. The All
    /// series mode's selection is `series_id`, so this field carries meaning
    /// only while a flat mode is active; a stale id falls back to the list's
    /// first row in the projection, as every selection's stale arm does.
    pub flat_row: Option<String>,
}

impl App {
    /// Whether `lib_idx` names an Emby TV (Shows) library: every GUI Shows
    /// method's destination guard.
    pub(in crate::app::gui_library) fn gui_tv_library(&self, lib_idx: usize) -> bool {
        self.libs
            .get(lib_idx)
            .is_some_and(|lib| lib.library.collection_type == "tvshows")
    }

    /// The Shows library's active view mode (design D12): the selection's
    /// stored mode, else the TUI rule's resolution over the library's own
    /// state — the resolution `resolve_tv_content_mode` computes, which the
    /// root load's default-pill capture also applies. `None` when
    /// `lib_idx` names no Shows library.
    #[must_use]
    pub fn gui_tv_mode(&self, lib_idx: usize) -> Option<TvContentMode> {
        let lib = self
            .libs
            .get(lib_idx)
            .filter(|lib| lib.library.collection_type == "tvshows")?;
        self.gui_tv_shows
            .get(&lib_idx)
            .and_then(|selection| selection.mode.clone())
            .or_else(|| {
                Some(mbv_ui_model::sort_filter::resolve_tv_content_mode(
                    lib.library_total.unwrap_or(0),
                    lib.tv_content_mode.as_ref(),
                ))
            })
    }

    /// Open the Shows library `lib_idx` (design D12): the root level of the
    /// active mode loads through the shared activation path — a missing
    /// stack starts the TUI's root load, whose completion captures the
    /// library total and resolves the default mode — and the selection's
    /// mode seeds from the library's resolved default while keeping a
    /// session's earlier set-mode choice. A `lib_idx` that names no Shows
    /// library is an ignored command.
    pub fn gui_tv_open(&mut self, lib_idx: usize) {
        if !self.gui_tv_library(lib_idx) {
            Self::log_ignored_library_command(
                "tvOpen",
                format_args!("the destination is no Shows library"),
            );
            return;
        }
        let lib = &self.libs[lib_idx];
        let resolved = mbv_ui_model::sort_filter::resolve_tv_content_mode(
            lib.library_total.unwrap_or(0),
            lib.tv_content_mode.as_ref(),
        );
        let entry = self.gui_tv_shows.entry(lib_idx).or_default();
        if entry.mode.is_none() {
            entry.mode = Some(resolved);
        }
        if self.libs[lib_idx].nav_stack.is_empty() {
            self.activate_library_position(lib_idx);
        }
    }

    /// Set the Shows library's active view mode (design D12): the mode the
    /// selection stores and the root level's load — the TUI's own
    /// `apply_tv_letter_mode`, which materializes the Latest rows, the
    /// Upcoming snapshot's level, or the All series list, and refetches
    /// nothing when the mode is already the selected one. A `lib_idx` that
    /// names no Shows library is an ignored command.
    pub fn gui_tv_set_mode(&mut self, lib_idx: usize, mode: &TvContentMode) {
        if !self.gui_tv_library(lib_idx) {
            Self::log_ignored_library_command(
                "tvSetMode",
                format_args!("the destination is no Shows library"),
            );
            return;
        }
        self.gui_tv_shows.entry(lib_idx).or_default().mode = Some(mode.clone());
        self.apply_tv_letter_mode(lib_idx, mode);
    }

    /// Select the series the Hero shows (design D12): the selection's
    /// series, the season reset (the pills are not kept across series), and
    /// the series-detail load — the trigger that today lives only in the
    /// presenting Client's workspace push (`push_tv_workspace_content`), so
    /// a Client that keeps no shell state starts it here. A stale frame's id
    /// — no row of the library's root list, or a row that is no Series — is
    /// an ignored command, so the flat modes select no Hero item.
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
