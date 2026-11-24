//! The Shows in-view loader (gui-list-header-buttons design D5): the open
//! series and season rows a Shows list paints drive one bounded set of loads,
//! so a row that comes into view shows its children and a row the list scrolled
//! past never loads.
//!
//! The window knows which open rows it paints and the core knows what each row
//! still misses, so one command (`GuiLibraryCommand::ShowsOpenInView`) carries
//! the report and no coordinates. Each report replaces the last: one Shows list
//! paints at a time, so a row that leaves the view leaves the report with it and
//! its load never starts (spec "A row scrolled past").
//!
//! The loads the loader started and the pump that starts them are GUI-only, so
//! they live here; the report itself is stored in `GuiClientState`, and the
//! core reaches the pump through the store seam's
//! [`ClientLibraryEffect::SeriesContentChanged`](mbv_client_core::dispatch::library::stores::ClientLibraryEffect::SeriesContentChanged)
//! after a landing, so the core names no GUI type.

use crate::gui_core::GuiCore;
use crate::gui_core::gui_client_state::GuiClientState;
use mbv_client_core::App;

use super::GuiDestinationKind;
use super::tabs;

/// The loads this loader runs at one time (design D5): the rest wait for a
/// landing. The series cache cap stays at its own size, and the view never
/// shows more open series than the cap holds, so an in-view series is not
/// evicted by its neighbours.
pub(crate) const SHOWS_OPEN_LOAD_LIMIT: usize = 4;

/// The open Shows rows one report names (design D5), in paint order: the source
/// id of each open series row, and the `(series_id, source_id)` pair of each
/// open season row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct GuiShowsOpenInView {
    /// The Shows library the reporting list belongs to.
    pub(crate) lib_idx: usize,
    /// The open series rows' source ids, in paint order.
    pub(crate) series: Vec<String>,
    /// The open season rows' `(series_id, source_id)` pairs, in paint order.
    pub(crate) seasons: Vec<(String, String)>,
}

impl GuiCore {
    /// Store one in-view report and pump its loads (design D5): a report for a
    /// library that is not a Shows library is an ignored command, because no
    /// other destination paints a tree whose rows this loader could load.
    pub(crate) fn apply_gui_shows_open_in_view(
        &mut self,
        lib_idx: usize,
        series: Vec<String>,
        seasons: Vec<(String, String)>,
    ) {
        if self.app.emby_library_collection_type(lib_idx) != Some("tvshows") {
            super::log_ignored_library_command(
                "showsOpenInView",
                format_args!("the library {lib_idx} is not a Shows library"),
            );
            return;
        }
        self.gui.shows_open_in_view = Some(GuiShowsOpenInView {
            lib_idx,
            series,
            seasons,
        });
        self.pump_gui_shows_open_loads();
    }

    /// Start the loads the stored in-view report misses (design D5): the
    /// report handler runs it after each report, and the store seam runs it
    /// after each series landing.
    pub(crate) fn pump_gui_shows_open_loads(&mut self) {
        pump_shows_open_loads(&mut self.app, &mut self.gui);
    }
}

/// Start the loads the stored in-view report misses, up to
/// [`SHOWS_OPEN_LOAD_LIMIT`] at once (design D5): the free function the store
/// seam's `SeriesContentChanged` effect and the report handler both run.
///
/// Nothing starts while the active destination is not the report's Shows
/// library, because the report then describes a list the Client no longer
/// paints. A started load whose fetch the core no longer runs drops out first,
/// so a landing or a failed fetch frees its slot. The series rows start before
/// the season rows, in report order, and a season starts only once its series'
/// detail is cached.
pub(crate) fn pump_shows_open_loads(app: &mut App, gui: &mut GuiClientState) {
    let Some(report) = gui.shows_open_in_view.clone() else {
        return;
    };
    if !is_active_shows_library(app, report.lib_idx) {
        return;
    }
    gui.shows_open_series_loads
        .retain(|series_id| app.series_detail_loading(series_id));
    gui.shows_open_season_loads
        .retain(|(series_id, season_id)| app.series_season_loading(series_id, season_id));
    for series_id in &report.series {
        if in_flight_loads(gui) >= SHOWS_OPEN_LOAD_LIMIT {
            return;
        }
        if !series_missing(app, series_id) {
            continue;
        }
        app.fetch_series_detail_quiet(series_id);
        if app.series_detail_loading(series_id) {
            gui.shows_open_series_loads.insert(series_id.clone());
        }
    }
    for (series_id, season_id) in &report.seasons {
        if in_flight_loads(gui) >= SHOWS_OPEN_LOAD_LIMIT {
            return;
        }
        if !season_missing(app, series_id, season_id) {
            continue;
        }
        app.fetch_series_season_episodes(series_id.clone(), season_id.clone());
        if app.series_season_loading(series_id, season_id) {
            gui.shows_open_season_loads
                .insert((series_id.clone(), season_id.clone()));
        }
    }
}

/// Whether the active destination is the report's Shows library: the list the
/// report describes is the one the Client paints.
fn is_active_shows_library(app: &App, lib_idx: usize) -> bool {
    matches!(
        tabs::active_destination(app),
        Some(GuiDestinationKind::Shows { lib_idx: active }) if active == lib_idx
    )
}

/// The loads this loader has running: the ones whose fetch the core still
/// runs, as the pump's own drop pass has just left them.
fn in_flight_loads(gui: &GuiClientState) -> usize {
    gui.shows_open_series_loads.len() + gui.shows_open_season_loads.len()
}

/// Whether `series_id` misses its detail: no cached detail and no running
/// fetch. Group 5 widens this to stale entries.
fn series_missing(app: &App, series_id: &str) -> bool {
    app.series_detail(series_id).is_none() && !app.series_detail_loading(series_id)
}

/// Whether `(series_id, season_id)` misses its episodes: the series' detail is
/// cached, the detail holds no episode list for the season, and no fetch runs
/// for it.
fn season_missing(app: &App, series_id: &str, season_id: &str) -> bool {
    let Some(detail) = app.series_detail(series_id) else {
        return false;
    };
    !detail.episodes.contains_key(season_id) && !app.series_season_loading(series_id, season_id)
}
