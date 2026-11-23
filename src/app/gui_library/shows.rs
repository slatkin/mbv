// The Shows destination's actions (add-gui-library-panel design D14;
// gui-collection-dropdown design D8, D9): the three-choice view selection, the
// per-library New fetch, and the select, play, play-next, and add commands. The
// projection lives in `shows_projection.rs`, as the other destinations split
// their data path from their actions. All shows the series tree; New and
// Upcoming show the merged flat lists the Home sections used to carry.
//
// The per-library state lives in the core's GUI store, so every step is a free
// function over the `App` the GUI core owns, with the `GuiCore` entries as
// one-line calls (design D4), so the launch, the router's row work, and the
// shared library event drain reach them through the `App` they already hold.

use mbv_emby_model::EmbyItem;
use mbv_queue::QueueItem;
use mbv_ui_model::library_hero::producers::hero_content_queue;

use super::request::GuiSectionRequest;
use super::shows_view::GuiShowsView;
use crate::app::App;
use crate::app::state::events::LibEvent;
use crate::gui::GuiCore;

/// The GUI Shows New list's fetch completion (design D9): one Emby TV
/// library's latest episodes, sent through the shared library event channel.
/// Only the GUI Client's launch starts one, so the TUI never receives it.
#[derive(Debug)]
pub(in crate::app) struct GuiShowsNewEvent {
    pub(in crate::app) library_id: String,
    pub(in crate::app) result: Result<Vec<EmbyItem>, mbv_emby::EmbyError>,
}

impl GuiCore {
    /// Install one Shows view (design D8).
    pub(in crate::app::gui_library) fn set_gui_shows_view(
        &mut self,
        lib_idx: usize,
        view: GuiShowsView,
    ) {
        set_shows_view(&mut self.app, lib_idx, view);
    }

    /// Select a Shows row (design D12, D8, D9).
    pub(in crate::app::gui_library) fn select_gui_shows_row(&mut self, lib_idx: usize, row: &str) {
        select_shows_row(&mut self.app, lib_idx, row);
    }
}

/// The Shows view of one library (design D8): a library with no stored choice
/// shows All, the default. It reads only the core's Shows store, so it is a
/// core function the projection, the row art, and the launch reach through the
/// `App` they hold.
pub(in crate::app) fn shows_view(app: &App, lib_idx: usize) -> GuiShowsView {
    app.gui
        .shows_views
        .get(&lib_idx)
        .copied()
        .unwrap_or_default()
}

/// Install one Shows view (design D8): a press replaces the active view, and
/// the loaders for the newly shown list run through the existing per-library
/// sources. A stale library index changes nothing.
pub(in crate::app::gui_library) fn set_shows_view(
    app: &mut App,
    lib_idx: usize,
    view: GuiShowsView,
) {
    if !super::tv::tv_library(app, lib_idx) {
        super::log_ignored_library_command(
            "showsSetView",
            format_args!("the destination is no Shows library"),
        );
        return;
    }
    if shows_view(app, lib_idx) == view {
        return;
    }
    app.gui.shows_views.insert(lib_idx, view);
    match view {
        GuiShowsView::All | GuiShowsView::Upcoming => {
            app.maybe_spawn_tv_upcoming_snapshot(lib_idx);
        }
        GuiShowsView::New => {
            let library_id = app.libs[lib_idx].library.id.clone();
            spawn_shows_new(app, library_id);
        }
    }
}

/// Request the two TV lists' data at GUI launch (design D9): each Emby TV
/// library's New episodes through one worker fetch, and its eager Upcoming
/// snapshot through the once-per-session guard the TV tab already uses. Both
/// requests are idempotent, so a repeated catalog landing re-requests nothing.
pub(in crate::app) fn spawn_shows_sections(app: &mut App) {
    let tv_libraries: Vec<(usize, String)> = (0..app.libs.len())
        .filter(|lib_idx| app.libs[*lib_idx].library.collection_type == "tvshows")
        .map(|lib_idx| (lib_idx, app.libs[lib_idx].library.id.clone()))
        .collect();
    for (lib_idx, library_id) in tv_libraries {
        spawn_shows_new(app, library_id);
        app.maybe_spawn_tv_upcoming_snapshot(lib_idx);
    }
}

/// Start one TV library's New fetch (design D9): the same
/// `get_latest_episodes(parent_id, 30)` call the TUI's Latest level makes, on
/// a worker thread, landing in `shows_new` through its own event. It never
/// touches `nav_stack`, so the TUI's levels are the TUI's.
fn spawn_shows_new(app: &mut App, library_id: String) {
    if app.gui.shows_new.contains_key(&library_id) {
        return;
    }
    let Some(client) = app.emby_snapshot() else {
        return;
    };
    // The requesting mark blocks a second fetch; the completion replaces it
    // with landed rows, empty when the fetch fails.
    app.gui
        .shows_new
        .insert(library_id.clone(), GuiSectionRequest::Requesting);
    let tx = app.channels.lib_tx.clone();
    std::thread::spawn(move || {
        let result = client.get_latest_episodes(&library_id, 30);
        let _ = tx.send(LibEvent::GuiShowsNew(GuiShowsNewEvent {
            library_id,
            result,
        }));
    });
}

/// Land one TV library's New fetch (design D9): success and failure both land
/// the request, so an empty or failed list paints the empty state and no later
/// launch step or frame starts the same fetch again.
pub(in crate::app) fn handle_shows_new(app: &mut App, event: GuiShowsNewEvent) {
    let GuiShowsNewEvent { library_id, result } = event;
    match result {
        Ok(items) => {
            app.gui
                .shows_new
                .insert(library_id, GuiSectionRequest::landed(items));
        }
        Err(error) => {
            tracing::info!(
                name: "gui_library.shows_new.failed",
                target: "gui_library",
                library_id,
                error = %error,
                "the library's latest episodes failed to load"
            );
            app.gui
                .shows_new
                .insert(library_id, GuiSectionRequest::landed(Vec::new()));
        }
    }
}

/// The Emby item a New row names (design D9): the merged per-library episodes
/// hold the stable item id, so a select names the same item the projection
/// painted.
pub(in crate::app::gui_library) fn shows_new_item(app: &App, item_id: &str) -> Option<EmbyItem> {
    app.gui
        .shows_new
        .values()
        .flat_map(GuiSectionRequest::items)
        .find(|item| item.id == item_id)
        .cloned()
}

/// The single flat row's Hero facts (design D9): the shared Hero producer the
/// Recently added rows use, so a New row's Hero cannot drift from the same
/// item shown elsewhere. No Workspace rides a flat episode Hero.
pub(in crate::app::gui_library) fn shows_flat_hero(
    app: &mut App,
    item: &EmbyItem,
) -> super::GuiShowsHero {
    let item = QueueItem::Emby(Box::new(item.clone()));
    let content = hero_content_queue(&item);
    let art = super::artwork::emby_hero_art(app, content.facts.artwork.source.as_ref());
    let art_shape = super::home::artwork_shape(super::artwork::hero_shape(
        app,
        content.facts.artwork.shape,
        art.as_ref(),
    ));
    super::GuiShowsHero {
        title: content.facts.title.clone(),
        parent: None,
        meta_rows: content.facts.meta_rows.clone(),
        duration_row: content.facts.duration_row,
        progress_row: content.facts.progress_row,
        overview: content.overview.clone(),
        art_shape,
        art,
        workspace: None,
    }
}

/// Select a Shows row (design D12, D8, D9): in All the series the Hero shows,
/// in New the flat episode, and in Upcoming nothing — an Upcoming row is
/// display-only and the Hero keeps its item. A stale frame's row changes
/// nothing.
pub(in crate::app::gui_library) fn select_shows_row(app: &mut App, lib_idx: usize, row: &str) {
    if !super::tv::tv_library(app, lib_idx) {
        super::log_ignored_library_command(
            "showsSelect",
            format_args!("the destination is no Shows library"),
        );
        return;
    }
    match shows_view(app, lib_idx) {
        GuiShowsView::All => {
            let unchanged = app
                .gui
                .tv_shows
                .get(&lib_idx)
                .and_then(|selection| selection.series_id.as_deref())
                == Some(row);
            if !unchanged {
                // A pick move is library navigation: it closes the image warm
                // gate, so fast arrow traffic warms no neighbour art.
                app.stamp_navigation();
            }
            super::tv::select_tv_series(app, lib_idx, row);
        }
        GuiShowsView::New => {
            if shows_new_item(app, row).is_some() {
                app.stamp_navigation();
                app.gui.shows_flat_selection = Some(row.to_owned());
            } else {
                super::log_stale_library_item("showsSelect", row);
            }
        }
        GuiShowsView::Upcoming => {}
    }
}

/// Play the Shows episode a command names (design D12): the shared library play
/// path, whose admission and route checks are the Owner's queue's own. An
/// Upcoming row plays nothing and a stale id plays nothing.
pub(in crate::app::gui_library) fn play_shows_item(app: &mut App, lib_idx: usize, item_id: &str) {
    match super::shows_projection::shows_item(app, lib_idx, item_id) {
        Some(item) => app.play_or_activate_lib_item(lib_idx, item),
        None => super::log_stale_library_item("showsPlay", item_id),
    }
}

/// Play the Shows episode next (design D12): the shared play-next admission, so
/// the episode enters directly after the active slot.
pub(in crate::app::gui_library) fn play_next_shows_item(
    app: &mut App,
    lib_idx: usize,
    item_id: &str,
) {
    match super::shows_projection::shows_item(app, lib_idx, item_id) {
        Some(item) => app.play_lib_item_next(lib_idx, item),
        None => super::log_stale_library_item("showsPlayNext", item_id),
    }
}

/// Add the Shows episode to the queue (design D12): the shared add path.
pub(in crate::app::gui_library) fn enqueue_shows_item(
    app: &mut App,
    lib_idx: usize,
    item_id: &str,
) {
    match super::shows_projection::shows_item(app, lib_idx, item_id) {
        Some(item) => app.enqueue_lib_item(lib_idx, item),
        None => super::log_stale_library_item("showsQueue", item_id),
    }
}
