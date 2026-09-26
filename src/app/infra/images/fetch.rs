use super::{
    audiobookshelf_book_cover_cache_key, audiobookshelf_cover_cache_key, App, ImageFetchReq,
    ImageSource, Instant, LevelFillAction, LevelFillState, LibEvent, MAX_IMAGE_FETCHES,
    NAV_IMAGE_FETCH_IDLE_DELAY, PAGE_SIZE,
};

mod card_images;
mod level_artists;
mod level_warmup;

#[cfg(test)]
mod batch_tests;

impl App {
    /// Proactively fetches the full track list for `album_id` so the view's
    /// inline album detail pane (#145) can render it without the user
    /// drilling in first. A simple one-shot fetch (no throttle queue) —
    /// only one album is ever highlighted at a time, so there is no fan-out
    /// to bound.
    pub(in crate::app) fn fetch_album_tracks(&mut self, album_id: String) {
        if self.album_tracks_loading.contains(&album_id)
            || self.album_tracks_cache.contains_key(&album_id)
        {
            return;
        }
        self.album_tracks_loading.insert(album_id.clone());
        let Some(client) = self.emby_snapshot() else {
            self.album_tracks_loading.remove(&album_id);
            return;
        };
        let tx = self.lib_tx.clone();
        std::thread::spawn(move || {
            let tracks = client
                .get_items_sorted(
                    &album_id,
                    None,
                    false,
                    0,
                    PAGE_SIZE,
                    "ParentIndexNumber,IndexNumber",
                    "Ascending",
                )
                .map(|(items, _total)| items)
                .unwrap_or_default();
            let _ = tx.send(LibEvent::AlbumTracksFetched { album_id, tracks });
        });
    }

    /// Proactively fetches TV series detail (seasons + episodes) so the
    /// Inline series detail pane can render without the user
    /// drilling in first.
    pub(in crate::app) fn fetch_series_detail(&mut self, series_id: &str) {
        if series_id.is_empty() {
            return;
        }
        if self.series_detail_loading.contains(series_id)
            || self.series_detail_cache.contains_key(series_id)
        {
            return;
        }
        self.series_detail_loading.insert(series_id.to_string());
        let Some(client) = self.emby_snapshot() else {
            self.series_detail_loading.remove(series_id);
            return;
        };
        let tx = self.lib_tx.clone();
        let sid = series_id.to_string();
        std::thread::spawn(move || {
            let (seasons, episodes) = client
                .get_items_sorted(&sid, None, false, 0, PAGE_SIZE, "SortName", "Ascending")
                .map(|(items, _total)| items)
                .map(|seasons| (seasons, std::collections::HashMap::new()))
                .unwrap_or_default();
            let _ = tx.send(LibEvent::SeriesDetailFetched {
                series_id: sid,
                seasons,
                episodes,
            });
        });
    }

    /// Fetches one season only after the complete ordered Series detail is in
    /// the cache. The detail event handler calls this for every uncached pill.
    pub(in crate::app) fn fetch_series_season_episodes(
        &mut self,
        series_id: String,
        season_id: String,
    ) {
        let key = (series_id.clone(), season_id.clone());
        let Some(detail) = self.series_detail_cache.get(&series_id) else {
            if !series_id.is_empty() && !season_id.is_empty() {
                self.pending_series_season_expansions.insert(key);
                self.fetch_series_detail(&series_id);
            }
            return;
        };
        if !detail.seasons.iter().any(|season| season.id == season_id)
            || detail.episodes.contains_key(&season_id)
            || self.series_season_loading.contains(&key)
        {
            self.pending_series_season_expansions.remove(&key);
            return;
        }
        let Some(client) = self.emby_snapshot() else {
            return;
        };
        self.pending_series_season_expansions.remove(&key);
        self.series_detail_loading.insert(series_id.clone());
        self.series_season_loading.insert(key);
        let tx = self.lib_tx.clone();
        std::thread::spawn(move || {
            let episodes = client
                .get_items_sorted(
                    &season_id,
                    None,
                    false,
                    0,
                    PAGE_SIZE,
                    "IndexNumber",
                    "Ascending",
                )
                .map(|(items, _)| items)
                .unwrap_or_default();
            let _ = tx.send(LibEvent::SeriesSeasonEpisodesFetched {
                series_id,
                season_id,
                episodes,
            });
        });
    }
}
