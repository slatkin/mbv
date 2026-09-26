use crate::app::infra::ui_util::sort_episodes;
use crate::app::state::types::browse::BrowseResting;
use crate::app::{App, BrowseLevel};

impl App {
    pub(in crate::app) fn update_current_browse_level(
        &mut self,
        lib_idx: usize,
        parent_id: &str,
        require_loading: bool,
        mut update: impl FnMut(&mut BrowseLevel),
    ) -> bool {
        let Some(lib) = self.libs.get_mut(lib_idx) else {
            return false;
        };
        let Some(last) = lib.nav_stack.last_mut() else {
            return false;
        };
        if last.parent_id != parent_id || (require_loading && !last.loading) {
            return false;
        }
        update(last);
        true
    }

    pub(in crate::app) fn normalize_current_browse_level_items(&mut self, lib_idx: usize) {
        if let Some(last) = self
            .libs
            .get_mut(lib_idx)
            .and_then(|lib| lib.nav_stack.last_mut())
        {
            if last
                .items
                .first()
                .is_some_and(|item| item.item_type == "Episode")
            {
                sort_episodes(&mut last.items);
            }
        }
    }

    pub(in crate::app) fn handle_loaded_level(
        &mut self,
        lib_idx: usize,
        parent_id: String,
        level: BrowseLevel,
    ) {
        let mut level = Some(level);
        self.update_current_browse_level(lib_idx, &parent_id, true, |last| {
            *last = level.take().unwrap();
        });
        self.normalize_current_browse_level_items(lib_idx);
        self.snap_grouped_album_cursor_to_display_order(lib_idx);
        self.start_or_supersede_music_grouping(lib_idx);
    }

    pub(in crate::app) fn maybe_auto_push_tv_season_level(&mut self, lib_idx: usize) {
        // When a season list arrives for a TV library,
        // automatically push a loading placeholder and fetch the first season's
        // episodes so the user lands directly in the combined series view.
        let should_auto_push = self.tab.emby_library_index() == Some(lib_idx)
            && self.libs.get(lib_idx).is_some_and(|lib| {
                lib.library.collection_type == "tvshows"
                    && lib
                        .nav_stack
                        .last()
                        .is_some_and(|l| l.items.first().is_some_and(|i| i.item_type == "Season"))
            });

        if should_auto_push {
            let (season_id, season_name) = self
                .libs
                .get(lib_idx)
                .and_then(|lib| lib.nav_stack.last())
                .and_then(|l| l.items.get(l.resting().cursor()))
                .map(|s| (s.id.clone(), s.name.clone()))
                .unwrap_or_default();
            if !season_id.is_empty() {
                if let Some(lib) = self.libs.get_mut(lib_idx) {
                    lib.nav_stack.push(BrowseLevel {
                        fetched_rows: 0,
                        parent_id: season_id.clone(),
                        title: season_name.clone(),
                        items: vec![],
                        total_count: 0,
                        resting: BrowseResting::new(0, 0),
                        item_types: Some("Episode".into()),
                        unplayed_only: false,
                        sort_by: "SortName".into(),
                        sort_order: "Ascending".into(),
                        loading: true,
                        all_items: None,
                        letter_filter: None,
                        tv_content_mode: None,
                        music_grouping: None,
                    });
                }
                self.spawn_browse(
                    lib_idx,
                    season_id,
                    season_name,
                    Some("Episode".into()),
                    false,
                    "SortName".into(),
                    "Ascending".into(),
                );
            }
        }
    }
}
