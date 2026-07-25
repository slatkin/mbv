use super::{AlbumIndexState, App, LibEvent, PanelFocus};
use crate::app::images::NAV_IMAGE_FETCH_IDLE_DELAY;
use mbv_core::api::MediaItem;
use std::time::Instant;

impl App {
    pub(super) fn move_lib_cursor(&mut self, delta: i64) {
        let now = Instant::now();
        let idle = now.duration_since(self.last_nav_at) >= NAV_IMAGE_FETCH_IDLE_DELAY;
        self.last_nav_at = now;
        self.mark_power_library_navigation(now);
        let lib_idx = self.library_tab.saturating_sub(1);

        if matches!(self.panel_focus, PanelFocus::Library)
            && self.libs[lib_idx].search.is_none()
            && self.libs[lib_idx].album_track_focus.is_none()
            && self.move_power_music_group_display_cursor(lib_idx, delta)
        {
            self.save_default_library_position(lib_idx);
            if idle {
                self.maybe_fetch_next_page(lib_idx);
            }
            return;
        }

        if self.libs[lib_idx].search.is_none() && self.is_feed_home_video_group_view(lib_idx) {
            if let Some(state) = self.libs[lib_idx].feed_home_video.as_mut() {
                let n = state.selected_len();
                if n > 0 {
                    state.video_cursor =
                        (state.video_cursor as i64 + delta).clamp(0, n as i64 - 1) as usize;
                    self.save_default_library_position(lib_idx);
                }
            }
            return;
        }

        // With letter-grouped display, navigate in sorted display order so
        // the cursor follows what the user sees (articles stripped) rather than raw item order.
        if !self.layout.main.left_sorted_indices.is_empty() {
            let needs_sorted = self.libs[lib_idx].search.is_none()
                && self.libs[lib_idx].nav_stack.last().is_some();
            if needs_sorted {
                let current = self.libs[lib_idx].nav_stack.last().unwrap().cursor;
                let sorted_n = self.layout.main.left_sorted_indices.len();
                let pos = self
                    .layout
                    .main
                    .left_sorted_indices
                    .iter()
                    .position(|&i| i == current)
                    .unwrap_or(0);
                let new_pos = (pos as i64 + delta).clamp(0, sorted_n as i64 - 1) as usize;
                let new_cursor = self.layout.main.left_sorted_indices[new_pos];
                if let Some(lvl) = self.libs[lib_idx].nav_stack.last_mut() {
                    lvl.cursor = new_cursor;
                }
                self.save_default_library_position(lib_idx);
                if idle {
                    self.maybe_fetch_next_page(lib_idx);
                }
                return;
            }
        }

        let lib = &mut self.libs[lib_idx];
        if let Some(s) = &mut lib.search {
            let n = s.results.len();
            if n > 0 {
                s.cursor = (s.cursor as i64 + delta).clamp(0, n as i64 - 1) as usize;
            }
            return;
        }
        if let Some(lvl) = lib.nav_stack.last_mut() {
            let n = lvl.items.len();
            if n > 0 {
                lvl.cursor = (lvl.cursor as i64 + delta).clamp(0, n as i64 - 1) as usize;
                self.save_default_library_position(lib_idx);
            }
        }
        if idle {
            self.maybe_fetch_next_page(lib_idx);
        }
    }

    pub(super) fn jump_lib_cursor(&mut self, to_end: bool) {
        let lib_idx = self.library_tab.saturating_sub(1);

        if matches!(self.panel_focus, PanelFocus::Library)
            && self.libs[lib_idx].search.is_none()
            && self.libs[lib_idx].album_track_focus.is_none()
            && self.jump_power_music_group_display_cursor(lib_idx, to_end)
        {
            self.save_default_library_position(lib_idx);
            self.maybe_fetch_next_page(lib_idx);
            return;
        }

        if self.libs[lib_idx].search.is_none() && self.is_feed_home_video_group_view(lib_idx) {
            if let Some(state) = self.libs[lib_idx].feed_home_video.as_mut() {
                let n = state.selected_len();
                if n > 0 {
                    state.video_cursor = if to_end { n - 1 } else { 0 };
                    self.save_default_library_position(lib_idx);
                }
            }
            return;
        }

        // With letter-grouped display, Home/End jump to the first/last item
        // in sorted display order (article-stripped), not raw item order.
        if !self.layout.main.left_sorted_indices.is_empty() {
            let needs_sorted = self.libs[lib_idx].search.is_none()
                && !self.layout.main.left_sorted_indices.is_empty();
            if needs_sorted {
                let n = self.layout.main.left_sorted_indices.len();
                let new_cursor =
                    self.layout.main.left_sorted_indices[if to_end { n - 1 } else { 0 }];
                if let Some(lvl) = self.libs[lib_idx].nav_stack.last_mut() {
                    lvl.cursor = new_cursor;
                }
                self.save_default_library_position(lib_idx);
                self.maybe_fetch_next_page(lib_idx);
                return;
            }
        }

        let lib = &mut self.libs[lib_idx];
        if let Some(s) = &mut lib.search {
            let n = s.results.len();
            if n > 0 {
                s.cursor = if to_end { n - 1 } else { 0 };
            }
            return;
        }
        if let Some(lvl) = lib.nav_stack.last_mut() {
            let n = lvl.items.len();
            if n > 0 {
                lvl.cursor = if to_end { n - 1 } else { 0 };
                self.save_default_library_position(lib_idx);
            }
        }
        self.maybe_fetch_next_page(lib_idx);
    }

    pub(super) fn is_viewing_album_folders(&self, lib_idx: usize) -> bool {
        let lib = &self.libs[lib_idx];
        if lib.library.collection_type != "music" {
            return false;
        }
        if self.music_levels.is_empty() {
            return false;
        }
        let stack_len = lib.nav_stack.len();
        if stack_len < 1 {
            return false;
        }
        self.music_levels
            .get(stack_len - 1)
            .map(|s| s == "album")
            .unwrap_or(false)
    }

    pub(super) fn is_viewing_season_grid(&self, lib_idx: usize) -> bool {
        let lib = &self.libs[lib_idx];
        if lib.search.is_some() {
            return false;
        }
        let lvl = match lib.nav_stack.last() {
            Some(l) => l,
            None => return false,
        };
        lvl.items
            .first()
            .map(|i| i.item_type == "Season")
            .unwrap_or(false)
    }
    /// Activates series-selection mode for the given Series item.
    /// Ensures the series detail is fetched and sets `series_selection`
    /// to start at the first episode.
    pub(super) fn enter_series_selection(&mut self, lib_idx: usize, item: &MediaItem) {
        if item.item_type != "Series" || item.id.is_empty() {
            return;
        }
        // Ensure the series detail (seasons + episodes) is fetched.
        self.fetch_series_detail(item.id.clone());
        self.libs[lib_idx].series_selection = Some(0);
    }

    /// Returns the episodes for the current season in series-selection
    /// mode, or `None` if not in selection mode.
    pub(super) fn series_selection_episodes(&self, lib_idx: usize) -> Option<Vec<MediaItem>> {
        let _ep_idx = self.libs[lib_idx].series_selection?;
        let item = self.power_selected_series_item(lib_idx)?;
        let detail = self.series_detail_cache.get(&item.id)?;
        let season = detail
            .seasons
            .get(self.libs[lib_idx].series_season_cursor)?;
        detail.episodes.get(&season.id).cloned()
    }

    /// Switches to the previous (`delta == -1`) or next (`delta == 1`)
    /// season while in series-selection mode. Adjusts the season cursor
    /// and ensures episodes for the new season are fetched.
    pub(super) fn switch_series_selection_season(&mut self, lib_idx: usize, delta: i64) {
        let Some(item) = self.power_selected_series_item(lib_idx) else {
            return;
        };
        let Some(detail) = self.series_detail_cache.get(&item.id).cloned() else {
            return;
        };
        let n = detail.seasons.len();
        if n == 0 {
            return;
        }
        let cur = self.libs[lib_idx].series_season_cursor;
        let new_cur = (cur as i64 + delta).clamp(0, n as i64 - 1) as usize;
        if new_cur == cur {
            return;
        }
        let new_season = &detail.seasons[new_cur];
        // Ensure episodes for the new season are fetched.
        if !detail.episodes.contains_key(&new_season.id) {
            let series_id = item.id.clone();
            let season_id = new_season.id.clone();
            let client = self.client.lock().unwrap().clone();
            let tx = self.lib_tx.clone();
            std::thread::spawn(move || {
                let eps = client
                    .get_items_sorted(
                        &season_id,
                        None,
                        false,
                        0,
                        super::PAGE_SIZE,
                        "IndexNumber",
                        "Ascending",
                    )
                    .map(|(items, _total)| items)
                    .unwrap_or_default();
                let _ = tx.send(LibEvent::SeriesSeasonEpisodesFetched {
                    series_id,
                    season_id,
                    episodes: eps,
                });
            });
        }
        self.libs[lib_idx].series_season_cursor = new_cur;
        // Reset episode cursor to first episode.
        self.libs[lib_idx].series_selection = Some(0);
    }

    pub(super) fn is_home_video_view(&self, lib_idx: usize) -> bool {
        let lib = &self.libs[lib_idx];
        lib.library.collection_type == "homevideos"
    }

    pub(super) fn snap_grouped_album_cursor_to_display_order(&mut self, lib_idx: usize) {
        if !self.is_viewing_album_folders(lib_idx) {
            return;
        }
        // The grouped-by-artist album views (music.rs/list.rs) display albums
        // sorted by artist, not in the raw SortName-by-album-title order the
        // API returns them in — so the freshly-loaded default cursor (index 0
        // in raw order) can land on an arbitrary album instead of the first one
        // the user actually sees on screen. Snap it to the first album in (a
        // synchronous best-effort guess at) display order. Mirrors
        // `App::resolve_group_album_artist`'s fallback chain via
        // `initial_group_artist_sort_key`.
        if let Some(last) = self
            .libs
            .get_mut(lib_idx)
            .and_then(|lib| lib.nav_stack.last_mut())
        {
            if !last.items.is_empty() {
                let mut order: Vec<usize> = (0..last.items.len()).collect();
                order
                    .sort_by_key(|&i| super::render::initial_group_artist_sort_key(&last.items[i]));
                last.cursor = order[0];
            }
        }
    }

    pub(super) fn recursive_album_display_item(
        &self,
        lib_idx: usize,
        item_idx: usize,
        mut item: MediaItem,
    ) -> MediaItem {
        let Some(AlbumIndexState::Ready(entries)) = self
            .libs
            .get(lib_idx)
            .and_then(|lib| self.album_indexes.get(&lib.library.id))
        else {
            return item;
        };
        if let Some(entry) = entries
            .get(item_idx)
            .filter(|entry| entry.album.id == item.id)
        {
            item.name = entry.display_label.clone();
        }
        item
    }
}
