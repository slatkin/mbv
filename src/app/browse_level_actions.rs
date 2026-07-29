use super::ui_util::sort_episodes;
use super::{App, BrowseLevel};

impl App {
    pub(super) fn update_current_browse_level(
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

    pub(super) fn normalize_current_browse_level_items(&mut self, lib_idx: usize) {
        if let Some(last) = self
            .libs
            .get_mut(lib_idx)
            .and_then(|lib| lib.nav_stack.last_mut())
        {
            if last
                .items
                .first()
                .map(|item| item.item_type == "Episode")
                .unwrap_or(false)
            {
                sort_episodes(&mut last.items);
            }
        }
    }

    pub(super) fn handle_loaded_level(
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
    }

    pub(super) fn maybe_auto_push_power_tv_season_level(&mut self, lib_idx: usize) {
        // When a season list arrives for a TV library,
        // automatically push a loading placeholder and fetch the first season's
        // episodes so the user lands directly in the combined series view.
        let should_auto_push = self.library_tab == lib_idx + 1
            && self
                .libs
                .get(lib_idx)
                .map(|lib| {
                    lib.library.collection_type == "tvshows"
                        && lib
                            .nav_stack
                            .last()
                            .map(|l| {
                                l.items
                                    .first()
                                    .map(|i| i.item_type == "Season")
                                    .unwrap_or(false)
                            })
                            .unwrap_or(false)
                })
                .unwrap_or(false);

        if should_auto_push {
            let (season_id, season_name) = self
                .libs
                .get(lib_idx)
                .and_then(|lib| lib.nav_stack.last())
                .and_then(|l| l.items.get(l.cursor))
                .map(|s| (s.id.clone(), s.name.clone()))
                .unwrap_or_default();
            if !season_id.is_empty() {
                if let Some(lib) = self.libs.get_mut(lib_idx) {
                    lib.nav_stack.push(BrowseLevel {
                        parent_id: season_id.clone(),
                        title: season_name.clone(),
                        items: vec![],
                        total_count: 0,
                        cursor: 0,
                        item_types: Some("Episode".into()),
                        unplayed_only: false,
                        sort_by: "SortName".into(),
                        sort_order: "Ascending".into(),
                        loading: true,
                        scroll: 0,
                        all_items: None,
                        letter_filter: None,
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
