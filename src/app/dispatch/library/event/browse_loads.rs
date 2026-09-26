use crate::app::state::types::browse::BrowseResting;
use crate::app::{App, BrowseLevel};
use mbv_core::api::EmbyItem;

impl App {
    pub(super) fn retain_grouped_music_level_items(&self, lib_idx: usize, level: &mut BrowseLevel) {
        crate::app::dispatch::library::browse::retain_grouped_music_level_items(
            level,
            self.is_grouped_music_library(lib_idx),
        );
    }

    pub(super) fn handle_lib_loaded(
        &mut self,
        lib_idx: usize,
        parent_id: &str,
        mut level: BrowseLevel,
    ) {
        // Filtering belongs at the event boundary so every level-row producer
        // (including refresh and restore) applies the same server-row
        // accounting invariant.
        self.retain_grouped_music_level_items(lib_idx, &mut level);
        // The drain's own parent id tells a root load apart from a deeper
        // level's load for the pending Series landing retry below.
        let loaded_parent_id = parent_id.to_string();
        self.handle_loaded_level(lib_idx, parent_id, level);
        if let Some(mode) = self.libs[lib_idx].tv_content_mode.clone() {
            if let Some(level) = self.libs[lib_idx].nav_stack.last_mut() {
                level.tv_content_mode = Some(mode);
            }
        }
        self.maybe_capture_library_total_and_apply_default_pill(lib_idx);
        self.maybe_auto_push_tv_season_level(lib_idx);
        self.maybe_auto_push_music_group_level(lib_idx);
        self.maybe_aggregate_feed_after_loaded(lib_idx);
        self.maybe_fetch_next_page(
            lib_idx,
            self.libs[lib_idx]
                .nav_stack
                .last()
                .map_or(0, |l| l.resting().cursor()),
        );
        self.spawn_all_items_prefetch(lib_idx);
        // A pending Series landing retries once this library's ROOT level has
        // drained (U2 correction: ensure-then-land); a deeper level's load
        // re-arms and waits.
        self.retry_pending_series_landing(lib_idx, &loaded_parent_id);
    }

    /// On the FIRST unfiltered load of a library's top browse level, this
    /// captures the library's TRUE total (`LibraryTab.library_total`) --
    /// `get_user_views` doesn't carry child counts, so this fetch's
    /// `total_count` is the only place that number comes from. If the
    /// library qualifies for the letter-range pill row
    /// (`LIBRARY_PILL_THRESHOLD`) and no pill was already restored from a
    /// saved session, this applies the library's default first-range pill and
    /// issues one scoped refresh to replace the level's items with that range -- see
    /// plan §5. A no-op for every subsequent load of the same level
    /// (`library_total` is already `Some`), for music/feed/podcast
    /// libraries, and for non-root levels.
    pub(in crate::app) fn maybe_capture_library_total_and_apply_default_pill(
        &mut self,
        lib_idx: usize,
    ) {
        let Some(lib) = self.libs.get(lib_idx) else {
            return;
        };
        if lib.library_total.is_some() || lib.library.collection_type == "music" {
            return;
        }
        if lib.nav_stack.len() != 1 {
            return;
        }
        let Some(level) = lib.nav_stack.first() else {
            return;
        };
        if level.loading || level.letter_filter.is_some() {
            return;
        }
        let total = level.total_count;
        let parent_id = level.parent_id.clone();
        let item_types = level.item_types.clone();
        let unplayed_only = level.unplayed_only;
        let sort_by = level.sort_by.clone();
        let sort_order = level.sort_order.clone();
        let is_tv = lib.library.collection_type == "tvshows";
        let filter_kind = crate::app::render::LetterFilterKind::from_collection_type(
            lib.library.collection_type.as_str(),
        );
        if let Some(lib) = self.libs.get_mut(lib_idx) {
            lib.library_total = Some(total);
        }
        if is_tv {
            let mode = crate::app::render::resolve_tv_content_mode(total, None);
            let large = total > crate::app::render::LIBRARY_PILL_THRESHOLD;
            if let Some(lib) = self.libs.get_mut(lib_idx) {
                lib.tv_content_mode = Some(mode.clone());
                if let Some(level) = lib.nav_stack.last_mut() {
                    level.tv_content_mode = Some(mode);
                }
            }
            if large {
                if let Some(last) = self.libs[lib_idx].nav_stack.last_mut() {
                    last.loading = true;
                    last.items.clear();
                    last.item_types = Some("Episode".into());
                }
                self.spawn_tv_latest(lib_idx, parent_id, self.libs[lib_idx].library.name.clone());
            }
            return;
        }
        if total <= crate::app::render::LIBRARY_PILL_THRESHOLD {
            return;
        }
        let filter = crate::app::render::LetterFilter::default_filter_for_kind(filter_kind);
        if let Some(last) = self.libs[lib_idx].nav_stack.last_mut() {
            last.loading = true;
            last.letter_filter = Some(filter.clone());
        }
        self.spawn_refresh(
            lib_idx,
            parent_id,
            item_types,
            unplayed_only,
            sort_by,
            sort_order,
            0,
            Some(&filter),
        );
    }

    pub(super) fn handle_lib_page_appended(
        &mut self,
        lib_idx: usize,
        parent_id: &str,
        items: Vec<EmbyItem>,
        total_count: usize,
    ) {
        let fetched_rows = items.len();
        let mut items = Some(items);
        if let Some(items) = items.as_mut() {
            crate::app::dispatch::library::browse::retain_grouped_music_items(
                items,
                self.is_grouped_music_library(lib_idx),
            );
        }
        self.update_current_browse_level(lib_idx, parent_id, true, |last| {
            last.items.extend(items.take().unwrap());
            last.fetched_rows += fetched_rows;
            last.total_count = total_count;
            last.loading = false;
        });
        self.normalize_current_browse_level_items(lib_idx);
        self.start_or_supersede_music_grouping(lib_idx);
        self.maybe_aggregate_feed_after_page_append(lib_idx, parent_id);
        self.maybe_fetch_next_page(
            lib_idx,
            self.libs[lib_idx]
                .nav_stack
                .last()
                .map_or(0, |l| l.resting().cursor()),
        );
    }

    pub(super) fn handle_lib_refreshed(
        &mut self,
        lib_idx: usize,
        parent_id: &str,
        item_types: Option<&str>,
        unplayed_only: bool,
        items: Vec<EmbyItem>,
        total_count: usize,
    ) {
        let is_feed_video_refresh = self.is_feed_home_video_library(lib_idx)
            && item_types == Some("Video")
            && unplayed_only;
        if !is_feed_video_refresh {
            let mut items = Some(items);
            let updated = self.update_current_browse_level(lib_idx, parent_id, false, |last| {
                last.items = items.take().unwrap();
                last.fetched_rows = last.items.len();
                last.total_count = total_count;
                last.loading = false;
            });
            if updated {
                let grouped_music = self.is_grouped_music_library(lib_idx);
                if let Some(level) = self
                    .libs
                    .get_mut(lib_idx)
                    .and_then(|lib| lib.nav_stack.last_mut())
                {
                    crate::app::dispatch::library::browse::retain_grouped_music_level_items(
                        level,
                        grouped_music,
                    );
                }
            }
        }
        self.normalize_current_browse_level_items(lib_idx);
        self.start_or_supersede_music_grouping(lib_idx);
        self.maybe_refresh_feed_groups_after_refresh(lib_idx);
        self.spawn_all_items_prefetch(lib_idx);
        // A group switch refreshes the album level in place (no `Loaded`
        // event, which is where an ordinary first navigation kicks off
        // completion pagination); re-arm it here so the new group's level
        // still loads to completion unconditionally.
        if self.is_music_group_view(lib_idx) {
            self.maybe_fetch_next_page(lib_idx, 0);
        }
    }

    pub(super) fn handle_restored_library_position(
        &mut self,
        lib_idx: usize,
        requested_position: &crate::config::LibraryPosition,
        position: crate::config::LibraryPosition,
        nav_stack: Vec<BrowseLevel>,
    ) {
        if self.saved_library_position(lib_idx).as_ref() != Some(requested_position) {
            return;
        }
        let nav_stack =
            self.restore_library_position_levels(lib_idx, requested_position, nav_stack);
        let position = self.rebuild_restored_library_position(
            lib_idx,
            position,
            requested_position,
            &nav_stack,
        );

        // A restore an armed pending Series landing is waiting on is never
        // stale: the landing spawned it and cannot retry until it applies,
        // and the landing is initiated from another tab (queue "Go to
        // Library") whose tab switch happens only on completion.
        let serves_pending_landing = self
            .pending_series_landing
            .as_ref()
            .is_some_and(|pending| pending.lib_idx == lib_idx);
        if self.active_library_position_scope_for(lib_idx).is_none() && !serves_pending_landing {
            return;
        }
        if let Some(lib) = self.libs.get_mut(lib_idx) {
            lib.apply_library_position(&position, nav_stack);
        }
        self.finish_restored_library_position(lib_idx);
        // Deliberately no `spawn_all_items_prefetch` call here (unlike
        // `handle_lib_loaded`'s sibling call, which is safe): this method
        // fires for every library restored at app *startup*, all
        // concurrently. Eagerly fetching+parsing a whole library's worth of
        // full-field items (People, MediaStreams, ...) here piles CPU-bound
        // JSON parsing on top of N other libraries' simultaneous restore
        // fetches and visibly stalls first paint of the default library
        // (#260). `all_items` is a pure cache for instant fuzzy-search open
        // via the unified search modal. The modal reads it lazily
        // (see `AllItemsPrefetched` handling), so nothing here requires
        // it to be warm. If you're tempted to add
        // this back, don't: benchmark against a library with 500+ items
        // first and check `~/.local/state/mbv/mbv.log` for `parent=<id>`
        // `http=`/`parse=` timings from `get_items_sorted`.
    }

    fn restore_library_position_levels(
        &self,
        lib_idx: usize,
        requested_position: &crate::config::LibraryPosition,
        mut nav_stack: Vec<BrowseLevel>,
    ) -> Vec<BrowseLevel> {
        for (index, level) in nav_stack.iter_mut().enumerate() {
            self.retain_grouped_music_level_items(lib_idx, level);
            if let Some(saved_level) = requested_position.levels.get(index) {
                let cursor = saved_level
                    .focused_item_id
                    .as_ref()
                    .and_then(|id| level.items.iter().position(|item| &item.id == id))
                    .unwrap_or_else(|| {
                        saved_level
                            .cursor_index
                            .min(level.items.len().saturating_sub(1))
                    });
                level.resting = BrowseResting::new(
                    cursor,
                    BrowseLevel::scroll_for_cursor(cursor, self.lib_page_size()),
                );
            }
        }
        // A saved child below a newly empty folder is no longer a reachable
        // path. The worker may have fetched it before the boundary filter ran,
        // so stop at the deepest retained parent.
        let mut valid_levels = nav_stack.len().min(1);
        while valid_levels < nav_stack.len() {
            let parent_id = nav_stack[valid_levels].parent_id.as_str();
            if nav_stack[valid_levels - 1]
                .items
                .iter()
                .any(|item| item.id == parent_id)
            {
                valid_levels += 1;
            } else {
                break;
            }
        }
        nav_stack.truncate(valid_levels);
        nav_stack
    }

    fn rebuild_restored_library_position(
        &self,
        lib_idx: usize,
        mut position: crate::config::LibraryPosition,
        requested_position: &crate::config::LibraryPosition,
        nav_stack: &[BrowseLevel],
    ) -> crate::config::LibraryPosition {
        // Rebuild the saved position from the filtered levels so a dropped
        // empty folder cannot leave a stale focused id or server-row count.
        let library_total = position
            .levels
            .first()
            .and_then(|level| level.library_total);
        position.levels = nav_stack
            .iter()
            .map(BrowseLevel::to_position_level)
            .collect();
        if let Some(root) = position.levels.first_mut() {
            root.library_total = library_total;
            root.tv_content_mode =
                (self.libs[lib_idx].library.collection_type == "tvshows").then(|| {
                    crate::app::render::resolve_tv_content_mode(
                        library_total.unwrap_or_default(),
                        requested_position
                            .levels
                            .first()
                            .and_then(|level| level.tv_content_mode.as_ref()),
                    )
                });
            if let Some(mbv_core::config::TvContentMode::Range(index)) = &root.tv_content_mode {
                root.letter_filter_index = Some(*index);
            } else if root.tv_content_mode.is_some() {
                root.letter_filter_index = None;
            }
        }
        position
    }

    fn finish_restored_library_position(&mut self, lib_idx: usize) {
        // Positions saved before the letter-pill feature existed carry no
        // `library_total`, so without this call `should_show_letter_pills`
        // would stay false forever for those libraries. This is a no-op for
        // saves that already have `library_total` set (see the function's
        // own early-return checks).
        self.maybe_capture_library_total_and_apply_default_pill(lib_idx);
        self.maybe_refresh_feed_groups_after_refresh(lib_idx);
        let restored = self
            .libs
            .get(lib_idx)
            .map(crate::app::state::types::library_tab::LibraryTab::library_position_snapshot);
        if restored.as_ref() != self.saved_library_position(lib_idx).as_ref() {
            if let Some(restored) = restored {
                self.replace_saved_library_position(lib_idx, restored);
            }
        }
        // A pending Series landing retries against the restored root corpus.
        // `arm` only pays the whole-library prefetch for a user-initiated
        // pending landing, so a startup restore stays in the no-prefetch
        // regime the note below protects.
        if let Some(parent_id) = self
            .libs
            .get(lib_idx)
            .and_then(|lib| lib.nav_stack.first())
            .map(|lvl| lvl.parent_id.clone())
        {
            self.retry_pending_series_landing(lib_idx, &parent_id);
        }
    }
}
