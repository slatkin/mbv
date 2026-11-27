//! The manual Emby item-state writes the context-menu entries and the shell
//! toggles run: the played-state mark, the favourite flag, and the Continue
//! Watching removal, each with its list ripple. The execute chains in the
//! parent module route here; the shared owner-queue played relay and the
//! action dispatch stay there.

use crate::app::dispatch::notify::ToastSeverity;
use crate::app::{App, LibEvent, ModelContentEvent};
use mbv_emby_model::EmbyItem;

impl App {
    /// The one Emby played-state propagation every manual Emby mark shares:
    /// the server clears the resume position on both a played and an unplayed
    /// mark, so the queue slot's pair is `(0, played)`.
    fn propagate_emby_played_to_queue(&mut self, item_id: &str, played: bool) {
        self.propagate_played_to_queue(vec![mbv_ctrl::ProgressUpdate {
            content_id: mbv_queue::QueueItemContentId::Emby(item_id.to_string()),
            position_ticks: 0,
            finished: played,
        }]);
    }

    pub(super) fn context_set_played(
        &mut self,
        item_id: &str,
        played: bool,
        lib_idx: Option<usize>,
    ) {
        let Some(client) = self.emby_client() else {
            self.flash("Emby is unavailable".into(), ToastSeverity::Warning);
            return;
        };
        let client = client.lock().unwrap();
        let result = if played {
            client.mark_played(item_id)
        } else {
            client.mark_unplayed(item_id)
        };
        drop(client);
        match result {
            Ok(()) => {
                self.propagate_emby_played_to_queue(item_id, played);
                if let Some(lib_idx) = lib_idx
                    && played
                {
                    self.remove_played_item_from_library(lib_idx, item_id);
                }
                if self.tab.is_home() {
                    match self.fetch_home() {
                        Ok(content) => {
                            // Delivered to the stored `home_content` via the
                            // lib_tx/ lib_rx drain (task 5.3d).
                            let _ = self.channels.lib_tx.send(LibEvent::ModelContent(
                                ModelContentEvent::HomeContentRefreshed(Box::new(content)),
                            ));
                        }
                        Err(e) => {
                            self.flash(format!("Couldn't refresh home: {e}"), ToastSeverity::Error);
                        }
                    }
                } else if let Some(lib_idx) = lib_idx {
                    self.refresh_lib(lib_idx);
                }
            }
            Err(e) => self.flash(
                format!("Couldn't update play status: {e}"),
                ToastSeverity::Error,
            ),
        }
    }

    /// The favourites write (emby-favourites design D3, D4): like
    /// `context_set_played`, a missing Emby client flashes "Emby is
    /// unavailable", an error flashes an error toast and changes nothing, and
    /// a success writes the server's returned `IsFavorite` — never the
    /// requested value — over every App-held copy of the item. No queue
    /// propagation and no list refetch run: design D4 flips the held copies
    /// in place, and queue items are out of scope (design D2).
    pub(super) fn context_set_favourite(&mut self, item_id: &str, favourite: bool) {
        let Some(client) = self.emby_client() else {
            self.flash("Emby is unavailable".into(), ToastSeverity::Warning);
            return;
        };
        let client = client.lock().unwrap();
        let result = client.set_favourite(item_id, favourite);
        drop(client);
        match result {
            Ok(actual) => self.flip_favourite_held_copies(item_id, actual),
            Err(e) => self.flash(
                format!("Couldn't update favourite: {e}"),
                ToastSeverity::Error,
            ),
        }
    }

    /// Set `is_favorite` on every App-held `EmbyItem` with `item_id`
    /// (emby-favourites design D4): the TUI browse levels, the Home Continue
    /// Watching content, the cached series details the Shows tree reads
    /// (`series_detail_item`), the album track cache, and the artist detail
    /// cache. Queue items stay untouched (design D2).
    fn flip_favourite_held_copies(&mut self, item_id: &str, favourite: bool) {
        let flip = |item: &mut EmbyItem| {
            if item.id == item_id {
                item.is_favorite = favourite;
            }
        };
        for lib in &mut self.libs {
            for level in &mut lib.nav_stack {
                for item in &mut level.items {
                    flip(item);
                }
            }
        }
        for item in &mut self.home_content.continue_items {
            flip(item);
        }
        for detail in self.series_detail_cache.values_mut() {
            for item in &mut detail.seasons {
                flip(item);
            }
            for episodes in detail.episodes.values_mut() {
                for item in episodes {
                    flip(item);
                }
            }
        }
        for tracks in self.album_tracks_cache.values_mut() {
            for item in tracks {
                flip(item);
            }
        }
        for entry in self.artist_detail_cache.values_mut() {
            for item in &mut entry.tracks {
                flip(item);
            }
        }
    }

    fn remove_played_item_from_library(&mut self, lib_idx: usize, item_id: &str) {
        // A feed-view group keeps its loaded selection rows when a video is
        // marked played (spec `feed-home-video-group-view`, "Marking played
        // keeps the row"); only unplayed-only levels of ordinary libraries
        // drop the row.
        if let Some(lvl) = self
            .libs
            .get_mut(lib_idx)
            .and_then(|lib| lib.nav_stack.last_mut())
            && lvl.unplayed_only
        {
            let id = item_id.to_string();
            let before = lvl.items.len();
            lvl.items.retain(|item| item.id != id);
            if lvl.items.len() != before {
                lvl.rows.retire();
            }
        }
    }

    ///
    /// # Panics
    ///
    /// Panics when the Emby client lock is poisoned.
    pub fn remove_from_continue_watching(&mut self, item: &EmbyItem) {
        // The shell resolved the target Continue Watching item at the Model
        // boundary from the stored `home_content` (task 5.3d) -- the App no
        // longer holds `home.continue_items`/`continue_cursor` to re-read.
        let Some(client) = self.emby_client() else {
            self.flash("Emby is unavailable".into(), ToastSeverity::Warning);
            return;
        };
        let client = client.lock().unwrap();
        let result = client.hide_from_resume(&item.id);
        drop(client);
        match result {
            Ok(()) => {
                match self.fetch_home() {
                    Ok(content) => {
                        // Delivered to the stored `home_content` via the
                        // lib_tx/ lib_rx drain (task 5.3d).
                        let _ = self.channels.lib_tx.send(LibEvent::ModelContent(
                            ModelContentEvent::HomeContentRefreshed(Box::new(content)),
                        ));
                    }
                    Err(e) => {
                        self.flash(format!("Couldn't refresh home: {e}"), ToastSeverity::Error);
                    }
                }
            }
            Err(e) => self.flash(
                format!("Couldn't remove from continue watching: {e}"),
                ToastSeverity::Error,
            ),
        }
    }

    ///
    /// # Panics
    ///
    /// Panics when the Emby client lock is poisoned.
    pub(in crate::app) fn toggle_watched_home_item(&mut self, item: &EmbyItem) {
        if item.is_folder || item.is_audio() {
            return;
        }
        let Some(client) = self.emby_client() else {
            self.flash("Emby is unavailable".into(), ToastSeverity::Warning);
            return;
        };
        let client = client.lock().unwrap();
        let result = if item.played {
            client.mark_unplayed(&item.id)
        } else {
            client.mark_played(&item.id)
        };
        drop(client);
        let new_played = !item.played;
        match result {
            Ok(()) => {
                self.propagate_emby_played_to_queue(&item.id, new_played);
                match self.fetch_home() {
                    Ok(content) => {
                        // Delivered to the stored `home_content` via the
                        // lib_tx/ lib_rx drain (task 5.3d).
                        let _ = self.channels.lib_tx.send(LibEvent::ModelContent(
                            ModelContentEvent::HomeContentRefreshed(Box::new(content)),
                        ));
                    }
                    Err(e) => {
                        self.flash(format!("Couldn't refresh home: {e}"), ToastSeverity::Error);
                    }
                }
            }
            Err(e) => self.flash(
                format!("Couldn't update play status: {e}"),
                ToastSeverity::Error,
            ),
        }
    }

    /// `toggle_watched`'s cursor-resolving wrapper has been deleted (task
    /// 4.3, R1): the live path is the item-taking
    /// `toggle_watched_item(lib_idx, item)` the shell routes
    /// `EmbyLibraryToggleWatched` through. Folder/audio guards, mark played/
    /// unplayed API behavior, unplayed-only removal, refresh,
    /// and unavailable-Service/error toasts are preserved exactly. The
    /// unplayed-only removal previously used `lvl.cursor` (the App cursor,
    /// which the legacy call always resolves to the toggled item); it now
    /// targets the supplied item's identity — identical in the legacy flow,
    /// and correct when the component-selected item differs from a parked
    /// App cursor. Feed-view groups keep their rows when a video is marked
    /// played (task 4.1, design D4).
    ///
    /// # Panics
    ///
    /// Panics when the Emby client lock is poisoned.
    pub fn toggle_watched_item(&mut self, lib_idx: usize, item: &EmbyItem) {
        if item.is_folder || item.is_audio() {
            return;
        }
        let Some(client) = self.emby_client() else {
            self.flash("Emby is unavailable".into(), ToastSeverity::Warning);
            return;
        };
        let client = client.lock().unwrap();
        let result = if item.played {
            client.mark_unplayed(&item.id)
        } else {
            client.mark_played(&item.id)
        };
        drop(client);
        match result {
            Ok(()) => {
                self.propagate_emby_played_to_queue(&item.id, !item.played);
                if !item.played {
                    self.remove_watched_item_from_library(lib_idx, &item.id);
                }
                self.refresh_lib(lib_idx);
            }
            Err(e) => self.flash(
                format!("Couldn't update play status: {e}"),
                ToastSeverity::Error,
            ),
        }
    }

    fn remove_watched_item_from_library(&mut self, lib_idx: usize, item_id: &str) {
        // Feed-view groups keep their loaded selection rows when a video is
        // marked watched (spec `feed-home-video-group-view`, "Marking played
        // keeps the row"); only unplayed-only levels of ordinary libraries
        // drop the row.
        if let Some(lvl) = self.libs[lib_idx].nav_stack.last_mut()
            && lvl.unplayed_only
            && let Some(pos) = lvl.items.iter().position(|item| item.id == item_id)
        {
            lvl.items.remove(pos);
            lvl.rows.retire();
        }
    }
}
