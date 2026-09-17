use super::render::{effective_sort_str, LetterFilter};
use super::{App, SeriesDetail};
use mbv_core::api::EmbyItem;

impl App {
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

    /// Enter on an Inline Search Series result: navigate the library list to
    /// the series' natural place -- cursor on it, its letter-range pill
    /// marked when pills are shown -- using the search corpus already in
    /// hand, with no refetch. Returns false when the corpus doesn't hold the
    /// item; the caller keeps the ordinary folder activation.
    pub(super) fn activate_searched_series(&mut self, lib_idx: usize, item: &EmbyItem) -> bool {
        if item.item_type != "Series" || item.id.is_empty() {
            return false;
        }
        let Some(level) = self.libs[lib_idx].nav_stack.last() else {
            return false;
        };
        let corpus = level
            .all_items
            .clone()
            .unwrap_or_else(|| level.items.clone());
        // The pill group the series sorts into (pills only exist at the
        // top level of pill-eligible libraries).
        let filter = if self.should_show_letter_pills(lib_idx) {
            LetterFilter::for_sort_key(effective_sort_str(item))
        } else {
            None
        };
        let filtered: Vec<EmbyItem> = match &filter {
            Some(filter) => corpus
                .iter()
                .filter(|candidate| {
                    let key = effective_sort_str(candidate);
                    filter.name_ge.is_none_or(|ge| key >= ge)
                        && filter.name_lt.is_none_or(|lt| key < lt)
                })
                .cloned()
                .collect(),
            None => corpus.clone(),
        };
        let Some(cursor) = filtered.iter().position(|i| i.id == item.id) else {
            return false;
        };
        let Some(level) = self.libs[lib_idx].nav_stack.last_mut() else {
            return false;
        };
        level.letter_filter = filter;
        level.items = filtered;
        level.total_count = level.items.len();
        level.all_items = Some(corpus);
        level.set_resting_cursor(cursor);
        level.set_resting_scroll(0);
        level.loading = false;
        // Ensure the series detail (seasons + episodes) is fetched.
        self.fetch_series_detail(item.id.clone());
        self.save_default_library_position(lib_idx);
        true
    }

    /// Ensures the series detail is fetched for the wide TV component.
    pub(super) fn enter_series_selection(&mut self, _lib_idx: usize, item: &EmbyItem) {
        if item.item_type != "Series" || item.id.is_empty() {
            return;
        }
        // Ensure the series detail (seasons + episodes) is fetched.
        self.fetch_series_detail(item.id.clone());
    }

    pub(super) fn handle_series_detail_fetched(&mut self, series_id: String, detail: SeriesDetail) {
        self.series_detail_cache.insert(series_id.clone(), detail);
        self.series_detail_loading.remove(&series_id);
        let first_season_id = self
            .series_detail_cache
            .get(&series_id)
            .and_then(|detail| detail.seasons.first())
            .map(|season| season.id.clone());
        if let Some(season_id) = first_season_id {
            self.fetch_series_season_episodes(series_id.clone(), season_id);
            self.refresh_series_detail_loading(&series_id);
        }
    }

    pub(super) fn handle_series_season_episodes_fetched(
        &mut self,
        series_id: String,
        season_id: String,
        episodes: Vec<EmbyItem>,
    ) {
        let key = (series_id.clone(), season_id.clone());
        self.series_season_loading.remove(&key);
        let Some(detail) = self.series_detail_cache.get_mut(&series_id) else {
            self.refresh_series_detail_loading(&series_id);
            return;
        };
        if !detail.seasons.iter().any(|season| season.id == season_id)
            || detail.episodes.contains_key(&season_id)
        {
            self.refresh_series_detail_loading(&series_id);
            return;
        }
        detail.episodes.insert(season_id, episodes);
        self.refresh_series_detail_loading(&series_id);
    }

    fn refresh_series_detail_loading(&mut self, series_id: &str) {
        if self
            .series_season_loading
            .iter()
            .any(|(active_series, _)| active_series == series_id)
        {
            self.series_detail_loading.insert(series_id.to_owned());
        } else {
            self.series_detail_loading.remove(series_id);
        }
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
                last.set_resting_cursor(order[0]);
            }
        }
    }
}
