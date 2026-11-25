use super::{App, MAX_IMAGE_FETCHES, NAV_IDLE_DELAY, PAGE_SIZE};
use crate::app::state::app_struct::{LevelFillAction, LevelFillState};
use crate::app::{LibEvent, MusicEvent, SeriesEvent};
use mbv_images::{ImageFetchReq, ImageSource};
use mbv_images::{audiobookshelf_book_cover_cache_key, audiobookshelf_cover_cache_key};
use std::time::Instant;

mod card_images;
mod completions;
mod emby;
mod level_artists;
mod level_warmup;
pub(in crate::app) mod transport;

#[cfg(test)]
mod batch_tests;

#[cfg(test)]
mod emby_tests;

impl App {
    /// Proactively fetches the full track list for `album_id` so the view's
    /// inline album detail pane (#145) can render it without the user
    /// drilling in first. A simple one-shot fetch (no throttle queue) —
    /// only one album is ever highlighted at a time, so there is no fan-out
    /// to bound.
    pub fn fetch_album_tracks(&mut self, album_id: String) {
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
        let tx = self.channels.lib_tx.clone();
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
            let _ = tx.send(LibEvent::Music(MusicEvent::AlbumTracksFetched {
                album_id,
                tracks,
            }));
        });
    }

    /// Refetches `album_id`'s track list while keeping the cached list in
    /// place (design D9): the view paints the tracks it already holds until
    /// the landing replaces them, so a refresh never blanks the pane. An
    /// album the cache lacks takes the first-load path, and an album whose
    /// fetch already runs is skipped. A failed refresh sends
    /// [`MusicEvent::AlbumTracksRefreshFailed`] instead of the empty list a
    /// first load falls back to, so the cache never stores a failure.
    pub fn refetch_album_tracks(&mut self, album_id: String) {
        if !self.album_tracks_cache.contains_key(&album_id) {
            self.fetch_album_tracks(album_id);
            return;
        }
        if self.album_tracks_loading.contains(&album_id) {
            return;
        }
        self.album_tracks_loading.insert(album_id.clone());
        let Some(client) = self.emby_snapshot() else {
            self.album_tracks_loading.remove(&album_id);
            return;
        };
        let tx = self.channels.lib_tx.clone();
        std::thread::spawn(move || {
            let event = match client.get_items_sorted(
                &album_id,
                None,
                false,
                0,
                PAGE_SIZE,
                "ParentIndexNumber,IndexNumber",
                "Ascending",
            ) {
                Ok((tracks, _total)) => MusicEvent::AlbumTracksFetched { album_id, tracks },
                Err(_) => MusicEvent::AlbumTracksRefreshFailed { album_id },
            };
            let _ = tx.send(LibEvent::Music(event));
        });
    }

    /// Proactively fetches TV series detail (seasons + episodes) so the
    /// Inline series detail pane can render without the user
    /// drilling in first.
    pub fn fetch_series_detail(&mut self, series_id: &str) {
        if series_id.is_empty() {
            return;
        }
        // A plain fetch supersedes a quiet in-view load of the same id (design
        // D6): its landing starts the first season as any pick expects.
        self.series_detail_quiet.remove(series_id);
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
        let tx = self.channels.lib_tx.clone();
        let sid = series_id.to_string();
        std::thread::spawn(move || {
            let (seasons, episodes) = client
                .get_items_sorted(&sid, None, false, 0, PAGE_SIZE, "SortName", "Ascending")
                .map(|(items, _total)| items)
                .map(|seasons| (seasons, std::collections::HashMap::new()))
                .unwrap_or_default();
            let _ = tx.send(LibEvent::Series(SeriesEvent::DetailFetched {
                series_id: sid,
                seasons,
                episodes,
            }));
        });
    }

    /// Fetches `series_id`'s detail as a quiet in-view load (design D6): the
    /// landing caches the detail but starts no first-season episode fetch, so
    /// an open row in view that keeps its seasons closed costs no episode
    /// request. Only a fetch this call started gets the mark, so a call that
    /// joins a plain fetch already in flight leaves that fetch's landing plain;
    /// a call that joins a quiet fetch keeps the mark that fetch set, because
    /// it starts no fetch of its own. A plain `fetch_series_detail` of the same
    /// id drops the mark, and the landing then loads the first season as a pick
    /// expects.
    pub fn fetch_series_detail_quiet(&mut self, series_id: &str) {
        let joined = self.series_detail_loading.contains(series_id);
        let quiet_pending = self.series_detail_quiet.contains(series_id);
        self.fetch_series_detail(series_id);
        if (joined && quiet_pending) || (!joined && self.series_detail_loading.contains(series_id))
        {
            self.series_detail_quiet.insert(series_id.to_string());
        }
    }

    /// Fetches one season only after the complete ordered Series detail is
    /// in the cache. The detail event handler arms the first season and any
    /// pending expansions; a season-pill click or a keyboard season move
    /// arms the selected pill the same way.
    pub fn fetch_series_season_episodes(&mut self, series_id: String, season_id: String) {
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
        let tx = self.channels.lib_tx.clone();
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
            let _ = tx.send(LibEvent::Series(SeriesEvent::SeasonEpisodesFetched {
                series_id,
                season_id,
                episodes,
            }));
        });
    }
}
