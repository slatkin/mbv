use super::types_selection_modal::{SelectionModalFilter, SelectionModalSource};
use super::{App, SeriesDetail};

impl App {
    pub(super) fn handle_series_detail_fetched(&mut self, series_id: String, detail: SeriesDetail) {
        self.series_detail_cache.insert(series_id.clone(), detail);
        self.series_detail_loading.remove(&series_id);
        // Only the first season is needed up front: it's the default season
        // cursor/modal selection, and every other season is fetched lazily
        // on demand as the user navigates to it (`select_series_selection_modal_season`).
        let first_season_id = self
            .series_detail_cache
            .get(&series_id)
            .and_then(|detail| detail.seasons.first())
            .map(|season| season.id.clone());
        if let Some(season_id) = first_season_id {
            self.fetch_series_season_episodes(series_id.clone(), season_id);
            self.refresh_series_detail_loading(&series_id);
        }
        self.refresh_series_modal(&series_id);
    }

    pub(super) fn handle_series_season_episodes_fetched(
        &mut self,
        series_id: String,
        season_id: String,
        episodes: Vec<mbv_core::api::EmbyItem>,
    ) {
        let key = (series_id.clone(), season_id.clone());
        self.series_season_loading.remove(&key);
        let Some(detail) = self.series_detail_cache.get_mut(&series_id) else {
            self.refresh_series_detail_loading(&series_id);
            return;
        };
        if !detail.seasons.iter().any(|season| season.id == season_id) {
            self.refresh_series_detail_loading(&series_id);
            return;
        }
        if detail.episodes.contains_key(&season_id) {
            self.refresh_series_detail_loading(&series_id);
            return;
        }
        detail.episodes.insert(season_id.clone(), episodes);
        self.refresh_series_detail_loading(&series_id);
        self.refresh_series_modal(&series_id);
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

    fn refresh_series_modal(&mut self, series_id: &str) {
        let Some(detail) = self.series_detail_cache.get(series_id) else {
            return;
        };
        let source = SelectionModalSource::Series {
            series_id: series_id.to_owned(),
        };
        let selected = self
            .selection_modal
            .as_ref()
            .filter(|modal| modal.source == source)
            .and_then(|modal| modal.filter.as_ref().map(|filter| filter.selected))
            .unwrap_or(0)
            .min(detail.seasons.len().saturating_sub(1));
        if let Some(modal) = self
            .selection_modal
            .as_mut()
            .filter(|modal| modal.source == source)
        {
            modal.filter = Some(SelectionModalFilter {
                labels: super::lib_cursor_actions::series_season_pill_labels(detail),
                selected,
            });
        }
        self.refresh_selection_modal(
            source,
            super::lib_cursor_actions::series_modal_state_for_season(detail, selected),
        );
    }
}
