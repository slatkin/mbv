use super::render::{effective_sort_str, LetterFilter, LetterFilterKind, LIBRARY_PILL_THRESHOLD};
use super::types_events::{PendingSeriesHandoff, PendingSeriesLanding};
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

    /// True when a miss against `lib_idx`'s current root corpus is not yet
    /// final: an unloaded library (no level), a level still loading or showing
    /// a partial page, or an active letter pill whose `all_items` is absent can
    /// all still grow to hold the pending series. A loaded, fully listed,
    /// unfiltered level (or one whose whole-library cache is already in hand)
    /// is complete, so a miss against it is the task 4.2 miss.
    fn series_corpus_can_grow(&self, lib_idx: usize) -> bool {
        self.libs
            .get(lib_idx)
            .and_then(|lib| lib.nav_stack.last())
            .is_none_or(|lvl| {
                lvl.all_items.is_none()
                    && (lvl.loading || lvl.letter_filter.is_some() || !lvl.is_fully_loaded())
            })
    }

    /// True when `lib_idx`'s root level is materialized (not loading) but its
    /// whole-library `all_items` cache is absent, so the pending series landing
    /// needs `spawn_all_items_prefetch` to grow the corpus.
    fn series_landing_needs_prefetch(&self, lib_idx: usize) -> bool {
        self.libs
            .get(lib_idx)
            .and_then(|lib| lib.nav_stack.last())
            .is_some_and(|lvl| !lvl.loading && lvl.all_items.is_none())
    }

    /// Arm the pending Series landing and start growing the target library's
    /// corpus (`ensure_lib_loaded_for` for an unloaded library, the
    /// whole-library prefetch when the root is loaded but `all_items` is
    /// absent). Returns false when the miss is already final (complete
    /// corpus), which keeps the caller's flash-and-keep-tab path.
    pub(super) fn arm_pending_series_landing(
        &mut self,
        lib_idx: usize,
        reveal: Box<EmbyItem>,
        switch_tab: bool,
        episode_id: Option<String>,
    ) -> bool {
        if !self.series_corpus_can_grow(lib_idx) {
            return false;
        }
        self.pending_series_landing = Some(PendingSeriesLanding {
            lib_idx,
            reveal,
            switch_tab,
            episode_id,
        });
        self.ensure_lib_loaded_for(lib_idx);
        if self.series_landing_needs_prefetch(lib_idx) {
            self.spawn_all_items_prefetch(lib_idx);
        }
        true
    }

    /// Retry a pending Series landing when the target library's corpus drains
    /// (`Loaded`, `AllItemsPrefetched`, or a restored position). A success
    /// lands per D4 (land, save position, switch); a miss that can still grow
    /// re-arms and asks for the next growth step; a miss against a complete
    /// corpus flashes and leaves the tab unchanged (task 4.2). A drain for any
    /// other library (or any level other than the library root, `parent_id`)
    /// leaves the pending untouched.
    pub(super) fn retry_pending_series_landing(&mut self, lib_idx: usize, parent_id: &str) {
        let Some(pending) = self.pending_series_landing.take() else {
            return;
        };
        let root_parent = self
            .libs
            .get(lib_idx)
            .and_then(|lib| lib.nav_stack.first())
            .map(|lvl| lvl.parent_id.as_str());
        if pending.lib_idx != lib_idx || root_parent != Some(parent_id) {
            self.pending_series_landing = Some(pending);
            return;
        }
        let reveal = pending.reveal;
        let episode_id = pending.episode_id;
        let name = reveal.name.clone();
        if self.activate_searched_series(lib_idx, &reveal) {
            self.save_default_library_position(lib_idx);
            if pending.switch_tab {
                self.set_library_tab(lib_idx + 1);
            }
            // The deferred landing completed on THIS drain: arm the same
            // hand-off the immediate arm arms (task 3.1), carrying the deep
            // selection (task 6.1).
            self.pending_series_handoff = Some(PendingSeriesHandoff {
                lib_idx,
                reveal,
                episode_id,
            });
        } else {
            let rearmed =
                self.arm_pending_series_landing(lib_idx, reveal, pending.switch_tab, episode_id);
            if !rearmed {
                self.flash_error(format!("Could not land on '{name}' in its library"));
            }
        }
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
        let grouped_music = self.is_grouped_music_library(lib_idx);
        let mut corpus = level
            .all_items
            .clone()
            .unwrap_or_else(|| level.items.clone());
        // The whole-library search corpus is intentionally unfiltered, so
        // apply the same grouped-music boundary rule before projecting it
        // back into the browse level.
        super::library_browse_actions::retain_grouped_music_items(&mut corpus, grouped_music);
        // The pill group the series sorts into (pills only exist at the
        // top level of pill-eligible libraries).
        let filter = if self.should_show_letter_pills(lib_idx) {
            let filter_kind = super::render::LetterFilterKind::from_collection_type(
                self.libs[lib_idx].library.collection_type.as_str(),
            );
            LetterFilter::for_sort_key_for_kind(effective_sort_str(item), filter_kind)
        } else {
            None
        };
        let mut filtered = corpus.clone();
        if let Some(filter) = &filter {
            filtered.retain(|candidate| {
                let key = effective_sort_str(candidate);
                filter.name_ge.is_none_or(|ge| key >= ge)
                    && filter.name_lt.is_none_or(|lt| key < lt)
            });
        }
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

    /// Navigate a flat TV episode row that carries no playable episode id to
    /// its owning series' Workspace. Emby's `GET /Shows/Upcoming` returns
    /// placeholders (`Type: Episode`, no `Id`, `series_id` set) for
    /// unaired/not-downloaded rows; the flat `Latest`/`Upcoming` level holds
    /// episodes only, so its corpus can never satisfy a Series landing.
    /// Switch the library to its series-bearing mode through the existing
    /// `select_letter_pill` level machinery (the letter range the series
    /// sorts into, or `All`), then arm the shared ensure-then-land path
    /// (`PendingSeriesLanding`): the series corpus drain lands the reveal and
    /// the shell's hand-off opens the Wide Workspace / Narrow Library Hero
    /// overlay. Returns false when `item` is not such a placeholder, so the
    /// caller keeps the ordinary play path.
    pub(super) fn open_series_for_unplayable_episode(
        &mut self,
        lib_idx: usize,
        item: &EmbyItem,
    ) -> bool {
        if item.item_type != "Episode" || !item.id.is_empty() || item.series_id.is_empty() {
            return false;
        }
        // The Series reveal carries the placeholder's series back-reference;
        // the episode-only identity must not leak into the Series item.
        let mut reveal = item.clone();
        reveal.item_type = "Series".into();
        reveal.id = item.series_id.clone();
        reveal.is_folder = true;
        reveal.series_id.clear();
        if !item.series_name.is_empty() {
            reveal.name = item.series_name.clone();
        }
        // The episode's `SortName` (often "Show S01E05") must not decide the
        // series' letter range; fall back to the article-stripped series name.
        reveal.sort_name.clear();
        if self.should_show_letter_pills(lib_idx) {
            let large = self.libs[lib_idx]
                .library_total
                .is_some_and(|total| total > LIBRARY_PILL_THRESHOLD);
            let filter_kind = LetterFilterKind::from_collection_type(
                self.libs[lib_idx].library.collection_type.as_str(),
            );
            // Range pill indices are offset by the two leading flat-mode
            // pills (`Latest`, `Upcoming`); the small-library row's `All` is
            // index 2.
            let pill_index = if large {
                LetterFilter::for_sort_key_for_kind(effective_sort_str(&reveal), filter_kind)
                    .map(|filter| filter.index + 2)
                    .unwrap_or(2)
            } else {
                2
            };
            self.select_letter_pill(lib_idx, pill_index);
        } else {
            // Unresolved total/mode (e.g. a restored flat position before the
            // first load): no pill row exists to switch, so force the
            // unfiltered series fetch the landing needs.
            let Some(level) = self.libs[lib_idx].nav_stack.last_mut() else {
                return false;
            };
            let parent_id = level.parent_id.clone();
            let unplayed_only = level.unplayed_only;
            let sort_by = level.sort_by.clone();
            let sort_order = level.sort_order.clone();
            level.tv_content_mode = Some(mbv_core::config::TvContentMode::All);
            level.letter_filter = None;
            level.all_items = None;
            level.items.clear();
            level.loading = true;
            level.item_types = Some("Series".into());
            level.set_resting_cursor(0);
            level.set_resting_scroll(0);
            self.libs[lib_idx].tv_content_mode = Some(mbv_core::config::TvContentMode::All);
            self.spawn_refresh(
                lib_idx,
                parent_id,
                Some("Series".into()),
                unplayed_only,
                sort_by,
                sort_order,
                0,
                None,
            );
        }
        // The placeholder never plays: report handled even if the landing is
        // somehow already unsatisfiable, so no empty-id item reaches playback.
        let _ = self.arm_pending_series_landing(lib_idx, Box::new(reveal), false, None);
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
        // A late completion must not replace a newer cached projection (for
        // example, a refresh that completed while this request was in flight).
        self.series_detail_cache
            .entry(series_id.clone())
            .or_insert(detail);
        self.series_detail_loading.remove(&series_id);
        let pending_seasons = self
            .pending_series_season_expansions
            .iter()
            .filter(|(pending_series, _)| pending_series == &series_id)
            .cloned()
            .collect::<Vec<_>>();
        for key in &pending_seasons {
            self.pending_series_season_expansions.remove(key);
        }
        let first_season_id = self
            .series_detail_cache
            .get(&series_id)
            .and_then(|detail| detail.seasons.first())
            .map(|season| season.id.clone());
        if let Some(season_id) = first_season_id {
            self.fetch_series_season_episodes(series_id.clone(), season_id);
        }
        for (_, season_id) in pending_seasons {
            self.fetch_series_season_episodes(series_id.clone(), season_id);
        }
        self.refresh_series_detail_loading(&series_id);
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
