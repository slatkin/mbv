// The Shows destination's actions (add-gui-library-panel design D14;
// gui-collection-dropdown design D8, D9): the three-choice view selection, the
// per-library New fetch, and the select, play, play-next, and add commands. The
// projection lives in `shows_projection.rs`, as the other destinations split
// their data path from their actions. All shows the series tree; New and
// Upcoming show the merged flat lists the Home sections used to carry.

use mbv_components::library_panel::hero::hero_content_queue;
use mbv_emby_model::EmbyItem;
use mbv_queue::QueueItem;

use super::shows_view::GuiShowsView;
use crate::app::App;
use crate::app::state::events::LibEvent;

/// The GUI Shows New list's fetch completion (design D9): one Emby TV
/// library's latest episodes, sent through the shared library event channel.
/// Only the GUI Client's launch starts one, so the TUI never receives it.
#[derive(Debug)]
pub(in crate::app) struct GuiShowsNewEvent {
    pub(in crate::app) library_id: String,
    pub(in crate::app) result: Result<Vec<EmbyItem>, mbv_emby::EmbyError>,
}

impl App {
    /// The Shows view of one library (design D8): a library with no stored
    /// choice shows All, the default.
    pub(in crate::app) fn gui_shows_view(&self, lib_idx: usize) -> GuiShowsView {
        self.gui_shows_views
            .get(&lib_idx)
            .copied()
            .unwrap_or_default()
    }

    /// Install one Shows view (design D8): a press replaces the active view,
    /// and the loaders for the newly shown list run through the existing
    /// per-library sources. A stale library index changes nothing.
    pub(in crate::app::gui_library) fn set_gui_shows_view(
        &mut self,
        lib_idx: usize,
        view: GuiShowsView,
    ) {
        if !self.gui_tv_library(lib_idx) {
            Self::log_ignored_library_command(
                "showsSetView",
                format_args!("the destination is no Shows library"),
            );
            return;
        }
        if self.gui_shows_view(lib_idx) == view {
            return;
        }
        self.gui_shows_views.insert(lib_idx, view);
        match view {
            GuiShowsView::All | GuiShowsView::Upcoming => {
                self.maybe_spawn_tv_upcoming_snapshot(lib_idx);
            }
            GuiShowsView::New => {
                let library_id = self.libs[lib_idx].library.id.clone();
                self.spawn_gui_shows_new(library_id);
            }
        }
    }

    /// Request the two TV lists' data at GUI launch (design D9): each Emby TV
    /// library's New episodes through one worker fetch, and its eager Upcoming
    /// snapshot through the once-per-session guard the TV tab already uses.
    /// Both requests are idempotent, so a repeated catalog landing re-requests
    /// nothing.
    pub(in crate::app) fn spawn_gui_shows_sections(&mut self) {
        let tv_libraries: Vec<(usize, String)> = (0..self.libs.len())
            .filter(|lib_idx| self.libs[*lib_idx].library.collection_type == "tvshows")
            .map(|lib_idx| (lib_idx, self.libs[lib_idx].library.id.clone()))
            .collect();
        for (lib_idx, library_id) in tv_libraries {
            self.spawn_gui_shows_new(library_id);
            self.maybe_spawn_tv_upcoming_snapshot(lib_idx);
        }
    }

    /// Start one TV library's New fetch (design D9): the same
    /// `get_latest_episodes(parent_id, 30)` call the TUI's Latest level makes,
    /// on a worker thread, landing in `gui_shows_new` through its own event.
    /// It never touches `nav_stack`, so the TUI's levels are the TUI's.
    fn spawn_gui_shows_new(&mut self, library_id: String) {
        if self.gui_shows_new.contains_key(&library_id) {
            return;
        }
        let Some(client) = self.emby_snapshot() else {
            return;
        };
        // The empty entry marks the request, so a later launch step starts no
        // second fetch; the completion replaces it.
        self.gui_shows_new.insert(library_id.clone(), Vec::new());
        let tx = self.channels.lib_tx.clone();
        std::thread::spawn(move || {
            let result = client.get_latest_episodes(&library_id, 30);
            let _ = tx.send(LibEvent::GuiShowsNew(GuiShowsNewEvent {
                library_id,
                result,
            }));
        });
    }

    /// Land one TV library's New fetch (design D9): a loaded list replaces the
    /// request mark, and a failure leaves the library with no rows.
    pub(in crate::app) fn handle_gui_shows_new(&mut self, event: GuiShowsNewEvent) {
        let GuiShowsNewEvent { library_id, result } = event;
        match result {
            Ok(items) => {
                self.gui_shows_new.insert(library_id, items);
            }
            Err(error) => {
                tracing::info!(
                    name: "gui_library.shows_new.failed",
                    target: "gui_library",
                    library_id,
                    error = %error,
                    "the library's latest episodes failed to load"
                );
            }
        }
    }

    /// Select a Shows row (design D12, D8, D9): in All the series the Hero
    /// shows, in New the flat episode, and in Upcoming nothing — an Upcoming
    /// row is display-only and the Hero keeps its item. A stale frame's row
    /// changes nothing.
    pub(in crate::app::gui_library) fn select_gui_shows_row(&mut self, lib_idx: usize, row: &str) {
        if !self.gui_tv_library(lib_idx) {
            Self::log_ignored_library_command(
                "showsSelect",
                format_args!("the destination is no Shows library"),
            );
            return;
        }
        match self.gui_shows_view(lib_idx) {
            GuiShowsView::All => {
                let unchanged = self
                    .gui_tv_shows
                    .get(&lib_idx)
                    .and_then(|selection| selection.series_id.as_deref())
                    == Some(row);
                if !unchanged {
                    // A pick move is library navigation: it closes the image
                    // warm gate, so fast arrow traffic warms no neighbour art.
                    self.stamp_navigation();
                }
                self.gui_tv_select_series(lib_idx, row);
            }
            GuiShowsView::New => {
                if self.gui_shows_new_item(row).is_some() {
                    self.stamp_navigation();
                    self.gui_shows_flat_selection = Some(row.to_owned());
                } else {
                    Self::log_stale_library_item("showsSelect", row);
                }
            }
            GuiShowsView::Upcoming => {}
        }
    }

    /// The Emby item a New row names (design D9): the merged per-library
    /// episodes hold the stable item id, so a select names the same item the
    /// projection painted.
    pub(in crate::app::gui_library) fn gui_shows_new_item(
        &self,
        item_id: &str,
    ) -> Option<EmbyItem> {
        self.gui_shows_new
            .values()
            .flatten()
            .find(|item| item.id == item_id)
            .cloned()
    }

    /// The single flat row's Hero facts (design D9): the shared Hero producer
    /// the Recently added rows use, so a New row's Hero cannot drift from the
    /// same item shown elsewhere. No Workspace rides a flat episode Hero.
    pub(in crate::app::gui_library) fn gui_shows_flat_hero(
        &mut self,
        item: &EmbyItem,
    ) -> super::GuiShowsHero {
        let item = QueueItem::Emby(Box::new(item.clone()));
        let content = hero_content_queue(&item);
        let art = self.gui_emby_hero_art(content.facts.artwork.source.as_ref());
        let art_shape = super::home::artwork_shape(
            self.gui_hero_shape(content.facts.artwork.shape, art.as_ref()),
        );
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

    /// Play the Shows episode a command names (design D12): the shared
    /// library play path, whose admission and route checks are the Owner's
    /// queue's own. An Upcoming row plays nothing and a stale id plays nothing.
    pub(in crate::app::gui_library) fn play_gui_shows_item(
        &mut self,
        lib_idx: usize,
        item_id: &str,
    ) {
        match self.gui_shows_item(lib_idx, item_id) {
            Some(item) => self.play_or_activate_lib_item(lib_idx, item),
            None => Self::log_stale_library_item("showsPlay", item_id),
        }
    }

    /// Play the Shows episode next (design D12): the shared play-next
    /// admission, so the episode enters directly after the active slot.
    pub(in crate::app::gui_library) fn play_next_gui_shows_item(
        &mut self,
        lib_idx: usize,
        item_id: &str,
    ) {
        match self.gui_shows_item(lib_idx, item_id) {
            Some(item) => self.play_lib_item_next(lib_idx, item),
            None => Self::log_stale_library_item("showsPlayNext", item_id),
        }
    }

    /// Add the Shows episode to the queue (design D12): the shared add path.
    pub(in crate::app::gui_library) fn enqueue_gui_shows_item(
        &mut self,
        lib_idx: usize,
        item_id: &str,
    ) {
        match self.gui_shows_item(lib_idx, item_id) {
            Some(item) => self.enqueue_lib_item(lib_idx, item),
            None => Self::log_stale_library_item("showsQueue", item_id),
        }
    }
}
