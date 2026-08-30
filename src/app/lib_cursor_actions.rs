use super::{
    App, SelectionModalFilter, SelectionModalListState, SelectionModalRow, SelectionModalSource,
};
use crate::app::images::NAV_IMAGE_FETCH_IDLE_DELAY;
use crate::app::types_selection_modal::SelectionModalItem;
use crate::app::ui_util::fmt_duration_approx;
use mbv_core::api::{EmbyItem, TICKS_PER_SECOND};
use std::time::Instant;

pub(super) fn series_season_pill_labels(detail: &super::SeriesDetail) -> Vec<String> {
    detail
        .seasons
        .iter()
        .enumerate()
        .map(|(index, season)| {
            let number = if season.index_number > 0 {
                season.index_number as usize
            } else {
                index + 1
            };
            format!("{number:02}")
        })
        .collect()
}

pub(super) fn series_modal_state_for_season(
    detail: &super::SeriesDetail,
    season_index: usize,
) -> SelectionModalListState {
    let Some(season) = detail.seasons.get(season_index) else {
        return SelectionModalListState::Empty;
    };
    let Some(episodes) = detail.episodes.get(&season.id) else {
        return SelectionModalListState::Loading;
    };
    let rows = std::iter::once(SelectionModalRow::Header(season.display_name()))
        .chain(episodes.iter().enumerate().map(|(index, episode)| {
            let number = if episode.index_number > 0 {
                episode.index_number as usize
            } else {
                index + 1
            };
            let meta = if episode.runtime_ticks > 0 {
                fmt_duration_approx(episode.runtime_ticks / TICKS_PER_SECOND)
            } else {
                String::new()
            };
            SelectionModalRow::Item(SelectionModalItem {
                name: format!("{number}. {}", episode.name),
                meta,
                id: episode.id.clone(),
            })
        }))
        .collect();
    SelectionModalListState::ready(rows)
}

impl App {
    /// Number of columns the currently-rendered library list uses: 1 for
    /// every single-column renderer (season grids, feed home-video group
    /// views) and the pane-derived count for the plain, letter-grouped,
    /// and grouped-album list renderers. Grouped album views (album-folder
    /// listings and the music-group view) render through
    /// `render_power_grouped_album_rows`, which packs `cols` albums per
    /// row, so they stride by the pane-derived count like the other
    /// column-aware renderers. Search results always render through the
    /// plain (column-aware) renderer, so they use the pane-derived count
    /// even inside a music library at the album-folder level.
    pub(super) fn current_library_columns(&self, lib_idx: usize) -> usize {
        use crate::app::library_column_width::library_column_count;
        if self.layout.main.is_wide_movies_active() {
            // The wide Movies right rail always renders the list as one
            // column (right-panel-arrangements spec), regardless of how
            // wide the rail gets.
            return 1;
        }
        if self.layout.main.is_wide_tv_active() {
            return 1;
        }
        if self.is_viewing_season_grid(lib_idx) || self.is_feed_home_video_group_view(lib_idx) {
            return 1;
        }
        library_column_count(self.layout.main.left_area.width)
    }

    /// Moves the library cursor vertically by `item_rows` display rows
    /// (1 for up/down, one viewport for page keys). Flat lists stride by
    /// `cols` items per row; letter-grouped lists move through the laid-out
    /// row map, since independent bucket packing means item index no longer
    /// maps to row by division. Grouped album views stride by `cols` too;
    /// the single-column views (feed home-video groups, season grids)
    /// receive `item_rows` exactly as they do today.
    pub(super) fn move_lib_cursor_rows(&mut self, lib_idx: usize, item_rows: i64) {
        // Defensive bounds check: the dispatch front door normalizes a stale
        // destination first, but async Service removal can invalidate the
        // matched index between normalization and this call. No-op (never
        // substitute library zero) on a miss.
        if lib_idx >= self.libs.len() {
            return;
        }

        // Letter-grouped lists: resolve the target item through the last
        // frame's laid-out item rows. The grouped-album view also publishes
        // `left_sorted_indices` but resolves movement through its own
        // column-aware cursor (see `album_cursor.rs`), so it is excluded.
        if !self.is_viewing_album_folders(lib_idx)
            && !self.layout.main.left_sorted_indices.is_empty()
        {
            if let Some(delta) = self.letter_vertical_delta(lib_idx, item_rows) {
                self.move_lib_cursor(lib_idx, delta);
                return;
            }
        }

        let cols = self.current_library_columns(lib_idx);
        self.move_lib_cursor(lib_idx, item_rows * cols as i64);
    }

    /// Computes the flat (sorted-order) delta that lands the cursor on the
    /// item `item_rows` rows up (negative) or down (positive) from its
    /// current display row, per the last frame's laid-out item rows.
    /// Headers/spacers/fillers do not participate: the target is the
    /// `item_rows`-th *item row* away, keeping the cursor's column (a
    /// ragged target row falls back to its last item; moving past the end
    /// clamps to the last item). Returns `None` when the layout is stale
    /// (cursor not found), letting the caller fall back to flat arithmetic.
    fn letter_vertical_delta(&self, lib_idx: usize, item_rows: i64) -> Option<i64> {
        let sorted = &self.layout.main.left_sorted_indices;
        let all_rows = &self.layout.main.left_item_rows;
        if all_rows.is_empty() || sorted.is_empty() {
            return None;
        }
        let item_row_list: Vec<&Vec<usize>> = all_rows.iter().filter(|r| !r.is_empty()).collect();
        if item_row_list.is_empty() {
            return None;
        }
        let cursor = self.libs[lib_idx].nav_stack.last()?.resting().cursor();
        let (cur_row, cur_col) = item_row_list
            .iter()
            .enumerate()
            .find_map(|(r, row)| row.iter().position(|&i| i == cursor).map(|col| (r, col)))?;
        let row_count = item_row_list.len();
        let target_row = if item_rows < 0 {
            cur_row.saturating_sub(item_rows.unsigned_abs() as usize)
        } else {
            cur_row
                .saturating_add(item_rows as usize)
                .min(row_count.saturating_sub(1))
        };
        let target = item_row_list[target_row]
            .get(cur_col)
            .copied()
            .or_else(|| item_row_list[target_row].last().copied())?;

        // Single pass over `sorted` for both positions instead of two
        // separate `.position()` scans -- this runs on every j/k/Up/Down
        // keypress in letter-grouped view, so halving the work (and
        // early-exiting once both are found) matters on large libraries.
        let mut cur_pos = None;
        let mut target_pos = None;
        for (pos, &idx) in sorted.iter().enumerate() {
            if idx == cursor {
                cur_pos = Some(pos);
            }
            if idx == target {
                target_pos = Some(pos);
            }
            if cur_pos.is_some() && target_pos.is_some() {
                break;
            }
        }
        Some(target_pos? as i64 - cur_pos? as i64)
    }

    pub(super) fn move_lib_cursor(&mut self, lib_idx: usize, delta: i64) {
        if lib_idx >= self.libs.len() {
            return;
        }
        self.move_lib_cursor_inner(lib_idx, delta);
    }

    fn move_lib_cursor_inner(&mut self, lib_idx: usize, delta: i64) {
        // Defensive bounds check; see `move_lib_cursor_rows` for the stale
        // index contract. Never substitute library zero on a miss.
        let now = Instant::now();
        let idle = now.duration_since(self.last_nav_at) >= NAV_IMAGE_FETCH_IDLE_DELAY;
        self.last_nav_at = now;
        self.mark_library_navigation(now);

        if self.is_feed_home_video_group_view(lib_idx) {
            if let Some(state) = self.libs[lib_idx].feed_home_video.as_mut() {
                let n = state.selected_len();
                if n > 0 {
                    state.video_cursor = super::ui_util::move_cursor(state.video_cursor, delta, n);
                    self.save_default_library_position(lib_idx);
                }
            }
            return;
        }

        // With letter-grouped display, navigate in sorted display order so
        // the cursor follows what the user sees (articles stripped) rather than raw item order.
        if !self.layout.main.left_sorted_indices.is_empty() {
            let needs_sorted = self.libs[lib_idx].nav_stack.last().is_some();
            if needs_sorted {
                let current = self.libs[lib_idx]
                    .nav_stack
                    .last()
                    .unwrap()
                    .resting()
                    .cursor();
                let sorted_n = self.layout.main.left_sorted_indices.len();
                let pos = self
                    .layout
                    .main
                    .left_sorted_indices
                    .iter()
                    .position(|&i| i == current)
                    .unwrap_or(0);
                let new_pos = super::ui_util::move_cursor(pos, delta, sorted_n);
                let new_cursor = self.layout.main.left_sorted_indices[new_pos];
                if let Some(lvl) = self.libs[lib_idx].nav_stack.last_mut() {
                    lvl.set_resting_cursor(new_cursor);
                }
                self.save_default_library_position(lib_idx);
                if idle {
                    self.maybe_fetch_next_page(lib_idx, new_cursor);
                }
                return;
            }
        }

        let lib = &mut self.libs[lib_idx];
        if let Some(lvl) = lib.nav_stack.last_mut() {
            let n = lvl.items.len();
            if n > 0 {
                let next = super::ui_util::move_cursor(lvl.resting().cursor(), delta, n);
                lvl.set_resting_cursor(next);
                self.save_default_library_position(lib_idx);
                if idle {
                    self.maybe_fetch_next_page(lib_idx, next);
                }
            }
        }
    }

    pub(super) fn apply_lib_cursor_index(&mut self, lib_idx: usize, index: usize) {
        if lib_idx >= self.libs.len() {
            return;
        }

        let now = Instant::now();
        let idle = now.duration_since(self.last_nav_at) >= NAV_IMAGE_FETCH_IDLE_DELAY;
        self.last_nav_at = now;
        self.mark_library_navigation(now);

        if self.is_feed_home_video_group_view(lib_idx) {
            if let Some(state) = self.libs[lib_idx].feed_home_video.as_mut() {
                if state.selected_len() > 0 {
                    state.video_cursor = index;
                    self.save_default_library_position(lib_idx);
                }
            }
            return;
        }

        if let Some(level) = self.libs[lib_idx].nav_stack.last_mut() {
            level.set_resting_cursor(index);
            self.save_default_library_position(lib_idx);
        }
        if idle {
            self.maybe_fetch_next_page(lib_idx, index);
        }
    }

    pub(super) fn jump_lib_cursor(&mut self, lib_idx: usize, to_end: bool) {
        // Defensive bounds check; see `move_lib_cursor_rows` for the stale
        // index contract. Never substitute library zero on a miss.
        if lib_idx >= self.libs.len() {
            return;
        }

        if self.is_feed_home_video_group_view(lib_idx) {
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
            let needs_sorted = !self.layout.main.left_sorted_indices.is_empty();
            if needs_sorted {
                let n = self.layout.main.left_sorted_indices.len();
                let new_cursor =
                    self.layout.main.left_sorted_indices[if to_end { n - 1 } else { 0 }];
                if let Some(lvl) = self.libs[lib_idx].nav_stack.last_mut() {
                    lvl.set_resting_cursor(new_cursor);
                }
                self.save_default_library_position(lib_idx);
                self.maybe_fetch_next_page(lib_idx, new_cursor);
                return;
            }
        }

        let lib = &mut self.libs[lib_idx];
        if let Some(lvl) = lib.nav_stack.last_mut() {
            let n = lvl.items.len();
            if n > 0 {
                let target = if to_end { n - 1 } else { 0 };
                lvl.set_resting_cursor(target);
                self.save_default_library_position(lib_idx);
                self.maybe_fetch_next_page(lib_idx, target);
            }
        }
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
        let lvl = match lib.nav_stack.last() {
            Some(l) => l,
            None => return false,
        };
        lvl.items
            .first()
            .map(|i| i.item_type == "Season")
            .unwrap_or(false)
    }
    /// Ensures the series detail is fetched for the wide TV component.
    pub(super) fn enter_series_selection(&mut self, _lib_idx: usize, item: &EmbyItem) {
        if item.item_type != "Series" || item.id.is_empty() {
            return;
        }
        // Ensure the series detail (seasons + episodes) is fetched.
        self.fetch_series_detail(item.id.clone());
    }

    /// Opens the Series constituent-list modal (design.md Decision 7): one
    /// flat scrollable list with a non-selectable `Header` row per season
    /// and selectable episode `Item` rows beneath it. Ensures the series
    /// detail is fetched, mirroring `enter_series_selection`; if it hasn't
    /// landed in `series_detail_cache` yet, opens with a loading placeholder
    /// instead of episode rows.
    pub(super) fn open_series_selection_modal(&mut self, item: &EmbyItem) {
        let season_index = 0;
        if self.series_detail_cache.contains_key(&item.id) {
            let season_id = self
                .series_detail_cache
                .get(&item.id)
                .and_then(|detail| detail.seasons.get(season_index))
                .map(|season| season.id.clone());
            if let Some(season_id) = season_id {
                if !self
                    .series_detail_cache
                    .get(&item.id)
                    .is_some_and(|detail| detail.episodes.contains_key(&season_id))
                {
                    self.fetch_series_season_episodes(item.id.clone(), season_id);
                }
            }
        } else {
            self.fetch_series_detail(item.id.clone());
        }
        let (state, filter) = match self.series_detail_cache.get(&item.id) {
            Some(detail) => (
                series_modal_state_for_season(detail, season_index),
                Some(SelectionModalFilter {
                    labels: series_season_pill_labels(detail),
                    selected: season_index.min(detail.seasons.len().saturating_sub(1)),
                }),
            ),
            None => (SelectionModalListState::Loading, None),
        };
        self.open_selection_modal(
            SelectionModalSource::Series {
                series_id: item.id.clone(),
            },
            item.display_name(),
            state,
            filter,
        );
    }

    pub(super) fn select_series_selection_modal_season(
        &mut self,
        series_id: String,
        season_index: usize,
    ) {
        let Some(detail) = self.series_detail_cache.get(&series_id).cloned() else {
            return;
        };
        if detail.seasons.is_empty() {
            self.refresh_selection_modal(
                SelectionModalSource::Series { series_id },
                SelectionModalListState::Empty,
                None,
            );
            return;
        }
        if season_index >= detail.seasons.len() {
            return;
        }
        let season_id = detail.seasons[season_index].id.clone();
        if !detail.episodes.contains_key(&season_id) {
            self.fetch_series_season_episodes(series_id.clone(), season_id);
        }
        let state = series_modal_state_for_season(&detail, season_index);
        self.refresh_selection_modal(SelectionModalSource::Series { series_id }, state, None);
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
