use super::{
    App, SelectionModalFilter, SelectionModalListState, SelectionModalRow, SelectionModalSource,
};
use crate::app::types_selection_modal::SelectionModalItem;
use crate::app::ui_util::fmt_duration_approx;
use mbv_core::api::{EmbyItem, TICKS_PER_SECOND};

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
