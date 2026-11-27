// The Emby destinations' actions (add-gui-library-panel design D14;
// gui-collection-dropdown design D8, D9; emby-favourites design D5): the
// single-choice view selection, the per-library New fetch, and the select,
// play, play-next, and add commands. The projection lives in
// `shows_projection.rs`, as the other destinations split their data path from
// their actions. All shows each destination's own list; the Shows New and
// Upcoming views show the merged flat lists the Home sections used to carry.
//
// The per-library state lives in the core's GUI store, so every step is a free
// function over the `App` the GUI core owns, with the `GuiCore` entries as
// one-line calls (design D4), so the launch, the router's row work, and the
// shared library event drain reach them through the `App` they already hold.

use mbv_emby_model::EmbyItem;
use mbv_queue::QueueItem;
use mbv_ui_model::library_hero::producers::hero_content_queue;

use super::hero_facts::hero_facts_queue;
use super::library_view::{GuiLibraryView, library_view};
use super::request::GuiSectionRequest;
use crate::gui_core::GuiCore;
use crate::gui_core::gui_client_state::GuiClientState;
use mbv_client_core::App;
use mbv_client_core::state::library_fetch_events::LibraryFetchEvent;

impl GuiCore {
    /// Install one library view (design D5).
    pub(in crate::gui_core::gui_library) fn set_gui_library_view(
        &mut self,
        lib_idx: usize,
        view: GuiLibraryView,
    ) {
        set_library_view(&mut self.app, &mut self.gui, lib_idx, view);
    }

    /// Select a Shows row (design D12, D8, D9).
    pub(in crate::gui_core::gui_library) fn select_gui_shows_row(
        &mut self,
        lib_idx: usize,
        row: &str,
    ) {
        select_shows_row(&mut self.app, &mut self.gui, lib_idx, row);
    }
}

/// The Emby destination `lib_idx` names (design D5): the destination whose
/// view a command sets. `None` when the index names no Emby library, which
/// no view dropdown paints.
pub(in crate::gui_core::gui_library) fn emby_view_destination(
    app: &App,
    lib_idx: usize,
) -> Option<super::GuiDestinationKind> {
    let collection_type = app.emby_library_collection_type(lib_idx)?;
    Some(if app.is_grouped_music_library(lib_idx) {
        super::GuiDestinationKind::Music { lib_idx }
    } else if collection_type == "tvshows" {
        super::GuiDestinationKind::Shows { lib_idx }
    } else {
        super::GuiDestinationKind::Library { lib_idx }
    })
}

/// Install one library view (design D5): a press replaces the active view,
/// the choice stays inside the destination's own choices, and the loaders for
/// the newly shown list run through the existing per-library sources. A stale
/// library index, a library no Emby destination presents, and a view outside
/// the destination's choices change nothing.
pub(in crate::gui_core::gui_library) fn set_library_view(
    app: &mut App,
    gui: &mut GuiClientState,
    lib_idx: usize,
    view: GuiLibraryView,
) {
    let Some(destination) = emby_view_destination(app, lib_idx) else {
        super::log_ignored_library_command(
            "librarySetView",
            format_args!("the library index names no Emby library"),
        );
        return;
    };
    if !GuiLibraryView::choices(destination).contains(&view) {
        super::log_ignored_library_command(
            "librarySetView",
            format_args!("{view:?} is no choice of {destination:?}"),
        );
        return;
    }
    gui.library_views.insert(lib_idx, view);
    match view {
        GuiLibraryView::All | GuiLibraryView::Upcoming => {
            app.maybe_spawn_tv_upcoming_snapshot(lib_idx);
        }
        GuiLibraryView::New => {
            let Some(library_id) = app.emby_library_id(lib_idx).map(str::to_string) else {
                super::log_ignored_library_command(
                    "librarySetView",
                    format_args!("the library index names no tab"),
                );
                return;
            };
            spawn_shows_new(app, gui, library_id);
        }
        // Entering the Favourites view starts its fetch (emby-favourites
        // design D7): a first load when the store holds no list, an in-place
        // refresh when it does. Leaving the view restores the All list
        // untouched, because the All stores are never edited here.
        GuiLibraryView::Favourites => super::favourites::refetch_favourites(app, gui, lib_idx),
    }
}

/// Request the two TV lists' data at GUI launch (design D9): each Emby TV
/// library's New episodes through one worker fetch, and its eager Upcoming
/// snapshot through the once-per-session guard the TV tab already uses. Both
/// requests are idempotent, so a repeated catalog landing re-requests nothing.
pub(crate) fn spawn_shows_sections(app: &mut App, gui: &mut GuiClientState) {
    let tv_libraries: Vec<(usize, String)> = app
        .emby_library_identities()
        .into_iter()
        .filter(|(_, _, collection_type)| collection_type == "tvshows")
        .map(|(lib_idx, library_id, _)| (lib_idx, library_id))
        .collect();
    for (lib_idx, library_id) in tv_libraries {
        spawn_shows_new(app, gui, library_id);
        app.maybe_spawn_tv_upcoming_snapshot(lib_idx);
    }
}

/// Start one TV library's New fetch (design D9): the same
/// `get_latest_episodes(parent_id, 30)` call the TUI's Latest level makes, on
/// a worker thread, landing in `shows_new` through its own event. It never
/// touches `nav_stack`, so the TUI's levels are the TUI's.
fn spawn_shows_new(app: &mut App, gui: &mut GuiClientState, library_id: String) {
    if gui.shows_new.contains_key(&library_id) {
        return;
    }
    let Some(client) = app.emby_snapshot() else {
        return;
    };
    // The requesting mark blocks a second fetch; the completion replaces it
    // with landed rows, empty when the fetch fails.
    gui.shows_new
        .insert(library_id.clone(), GuiSectionRequest::Requesting);
    start_shows_new_fetch(app, client, library_id);
}

/// Refetch one TV library's New episodes over the `App` the GUI core owns
/// (gui-list-header-buttons design D8): with no landed list the refetch takes
/// the first-load path, and with a landed list it keeps that list in place and
/// starts the fetch, recording the library in the in-flight set so a second
/// refetch while that fetch runs starts nothing. No Emby client starts nothing
/// at all.
pub(crate) fn refetch_shows_new(app: &mut App, gui: &mut GuiClientState, library_id: String) {
    let Some(client) = app.emby_snapshot() else {
        // The no-client condition is not a fetch failure (design D8): the
        // refetch stores no mark and starts nothing.
        return;
    };
    let loaded = gui
        .shows_new
        .get(&library_id)
        .is_some_and(GuiSectionRequest::is_landed);
    if !loaded {
        spawn_shows_new(app, gui, library_id);
        return;
    }
    if !gui.shows_new_refreshing.insert(library_id.clone()) {
        return;
    }
    start_shows_new_fetch(app, client, library_id);
}

/// Start one TV library's New fetch worker (design D9, D8): the one spawn body
/// the first load and the refetch share, so both send the same result event,
/// whose landing replaces the store's value or reports a failed refresh.
fn start_shows_new_fetch(app: &App, client: mbv_emby::EmbyClient, library_id: String) {
    let tx = app.library_fetch_event_sender();
    std::thread::spawn(move || {
        let result = client.get_latest_episodes(&library_id, 30);
        tx.send(LibraryFetchEvent::ShowsNew { library_id, result });
    });
}

/// The Emby item a New row names (design D9): the merged per-library episodes
/// hold the stable item id, so a select names the same item the projection
/// painted.
pub(in crate::gui_core::gui_library) fn shows_new_item(
    gui: &GuiClientState,
    item_id: &str,
) -> Option<EmbyItem> {
    gui.shows_new
        .values()
        .flat_map(GuiSectionRequest::items)
        .find(|item| item.id == item_id)
        .cloned()
}

/// The single flat row's Hero facts (design D9; gui-hero-design-polish
/// design D1): the D1 queue builder's facts, and the split title's parent —
/// the item's series — so the header title rule (invariant 30) reads one
/// split. Shows New passes no feed name: the split reads the series alone.
/// No Workspace rides a flat episode Hero.
pub(in crate::gui_core::gui_library) fn shows_flat_hero(
    app: &mut App,
    item: &EmbyItem,
) -> super::GuiShowsHero {
    let item = QueueItem::Emby(Box::new(item.clone()));
    let content = hero_content_queue(&item);
    let parts = item.playback_title_parts(None);
    let art = super::artwork::emby_hero_art(app, content.facts.artwork.source.as_ref());
    let art_shape = super::home::artwork_shape(super::artwork::hero_shape(
        app,
        content.facts.artwork.shape,
        art.as_ref(),
    ));
    super::GuiShowsHero {
        title: content.facts.title.clone(),
        parent: parts.context.map(|context| context.text),
        facts: hero_facts_queue(&item),
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
pub(in crate::gui_core::gui_library) fn select_shows_row(
    app: &mut App,
    gui: &mut GuiClientState,
    lib_idx: usize,
    row: &str,
) {
    if !super::tv::tv_library(app, lib_idx) {
        super::log_ignored_library_command(
            "showsSelect",
            format_args!("the destination is no Shows library"),
        );
        return;
    }
    match library_view(gui, lib_idx) {
        GuiLibraryView::All => {
            let unchanged = gui
                .tv_shows
                .get(&lib_idx)
                .and_then(|selection| selection.series_id.as_deref())
                == Some(row);
            if !unchanged {
                // A pick move is library navigation: it closes the image warm
                // gate, so fast arrow traffic warms no neighbour art.
                app.stamp_navigation();
            }
            super::tv::select_tv_series(app, gui, lib_idx, row);
        }
        GuiLibraryView::New => {
            if shows_new_item(gui, row).is_some() {
                app.stamp_navigation();
                gui.shows_flat_selection = Some(row.to_owned());
            } else {
                super::log_stale_library_item("showsSelect", row);
            }
        }
        // A Favourites row's pick resolves through the favourites store and
        // lands in the list's own pick, so the Favourites Hero shows it and
        // the All selection is left as it was (emby-favourites design D6).
        GuiLibraryView::Favourites => {
            if !super::favourites::select_favourites_row(app, gui, lib_idx, row) {
                super::log_stale_library_item("showsSelect", row);
            }
        }
        // An Upcoming row is display-only.
        GuiLibraryView::Upcoming => {}
    }
}

/// Play the Shows episode a command names (design D12): the shared library play
/// path, whose admission and route checks are the Owner's queue's own. An
/// Upcoming row plays nothing and a stale id plays nothing.
pub(in crate::gui_core::gui_library) fn play_shows_item(
    app: &mut App,
    gui: &GuiClientState,
    lib_idx: usize,
    item_id: &str,
) {
    match super::shows_projection::shows_item(app, gui, lib_idx, item_id) {
        Some(item) => app.play_or_activate_lib_item(lib_idx, item),
        None => super::log_stale_library_item("showsPlay", item_id),
    }
}

/// Add the Shows episode to the queue (design D12): the shared add path.
pub(in crate::gui_core::gui_library) fn enqueue_shows_item(
    app: &mut App,
    gui: &GuiClientState,
    lib_idx: usize,
    item_id: &str,
) {
    match super::shows_projection::shows_item(app, gui, lib_idx, item_id) {
        Some(item) => app.enqueue_lib_item(lib_idx, item),
        None => super::log_stale_library_item("showsQueue", item_id),
    }
}
