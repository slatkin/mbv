use super::components::{BrowserKey, BrowserKind, ComponentId, ShellRequest, TvWorkspaceComponent};
use super::render::TvWideRenderCtx;
use super::shell::Model;
use super::{PanelFocus, TabSelection};
use mbv_core::config::ServiceKind;

impl Model {
    pub(super) fn handle_tv_request(&mut self, request: ShellRequest) {
        let Some(lib_idx) = self.app.tab.emby_library_index() else {
            return;
        };
        match request {
            ShellRequest::TvEpisodeActivate => {
                let Some((series_id, season_cursor, episode_cursor)) =
                    self.tv_episode_activation_selection()
                else {
                    return;
                };
                self.app
                    .play_tv_episode(&series_id, season_cursor, episode_cursor);
            }
            // The component owns the cursor; mirror its selected item only at
            // this writer seam so App effects resolve the same target.
            ShellRequest::TvMoveRows { .. }
            | ShellRequest::TvMoveColumn { .. }
            | ShellRequest::TvJumpCursor { .. }
            | ShellRequest::TvActivate
            | ShellRequest::TvBack
            | ShellRequest::TvCycleLetterPill { .. } => {
                self.mirror_tv_workspace_cursor(lib_idx);
                match request {
                    ShellRequest::TvActivate => {
                        self.app.activate_selected_series(lib_idx);
                    }
                    ShellRequest::TvBack => self.app.go_back(lib_idx),
                    ShellRequest::TvCycleLetterPill { delta } => {
                        self.app.cycle_letter_pill(lib_idx, delta)
                    }
                    _ => {}
                }
                self.push_tv_workspace_content();
                self.mirror_tv_workspace_cursor(lib_idx);
            }
            // Episode and season cursors are component-local until the later
            // episode action slice; retain typed routing without touching App.
            ShellRequest::TvEpisodeMove { .. } | ShellRequest::TvSeasonMove { .. } => {}
            _ => {}
        }
    }

    fn tv_episode_activation_selection(&self) -> Option<(String, usize, usize)> {
        let id = self.tv_workspace_id.as_ref()?;
        self.application
            .get_component(id)
            .and_then(|component| component.as_any().downcast_ref::<TvWorkspaceComponent>())
            .and_then(TvWorkspaceComponent::episode_activation_selection)
    }

    pub fn mirror_tv_workspace_cursor(&mut self, lib_idx: usize) {
        let Some(id) = self.tv_workspace_id.as_ref() else {
            return;
        };
        let Some(item_id) = self
            .application
            .get_component(id)
            .and_then(|component| component.as_any().downcast_ref::<TvWorkspaceComponent>())
            .and_then(TvWorkspaceComponent::selected_item_id)
        else {
            return;
        };
        if let Some(level) = self.app.libs[lib_idx].nav_stack.last_mut() {
            if let Some(cursor) = level.items.iter().position(|item| item.id == item_id) {
                level.cursor = cursor;
            }
        }
    }

    fn tv_workspace_component_id(&self) -> Option<ComponentId> {
        let TabSelection::EmbyLibrary(index) = self.app.tab else {
            return None;
        };
        let library = self.app.libs.get(index)?;
        if library.library.collection_type != "tvshows" || !self.app.layout.main.is_wide_tv_active()
        {
            return None;
        }
        Some(ComponentId::Browser(BrowserKey {
            service: ServiceKind::Emby,
            library_id: library.library.id.clone(),
            kind: BrowserKind::TvShows,
        }))
    }

    pub(super) fn sync_tv_workspace(&mut self) {
        let next_id = self.tv_workspace_component_id();
        if self.tv_workspace_id != next_id {
            if let Some(id) = self.tv_workspace_id.take() {
                let _ = self.application.umount(&id);
            }
            if let Some(id) = next_id {
                self.application
                    .mount(id.clone(), Box::new(TvWorkspaceComponent::new()), vec![])
                    .expect("mount TV workspace");
                self.application.active(&id).expect("activate TV workspace");
                self.tv_workspace_id = Some(id);
                self.push_tv_workspace_content();
            }
        }
    }

    pub(super) fn push_tv_workspace_content(&mut self) {
        let Some(id) = self.tv_workspace_id.as_ref() else {
            return;
        };
        let TabSelection::EmbyLibrary(index) = self.app.tab else {
            return;
        };
        let Some(library) = self.app.libs.get(index) else {
            return;
        };
        if library.library.collection_type != "tvshows" || !self.app.layout.main.is_wide_tv_active()
        {
            return;
        }
        let list = self.app.library_list_render_ctx(index, false);
        let selected_series = list
            .selected_item()
            .cloned()
            .filter(|item| item.item_type == "Series");
        let series_detail = selected_series
            .as_ref()
            .and_then(|item| self.app.series_detail_cache.get(&item.id).cloned());
        let context = TvWideRenderCtx::new(
            list,
            selected_series,
            series_detail,
            0,
            None,
            matches!(self.app.effective_panel_focus(), PanelFocus::Library),
            self.app.should_show_letter_pills(index),
        );
        if let Some(comp) = self.application.get_component_mut(id) {
            if let Some(tv) = comp.as_any_mut().downcast_mut::<TvWorkspaceComponent>() {
                tv.set_content(context);
            }
        }
    }

    pub(super) fn render_tv_workspace_component(&mut self, frame: &mut ratatui::Frame) {
        let Some(id) = self.tv_workspace_id.as_ref() else {
            return;
        };
        let area = self.app.layout.main.tv_wide_area;
        if area.width == 0 || area.height == 0 {
            return;
        }
        self.application.view(id, frame, area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::components::{Msg, ShellRequest, TvWorkspaceComponent};
    use crate::app::render::make_movie_app;
    use ratatui::layout::Rect;
    use tuirealm::component::AppComponent;
    use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};

    fn mounted_tv_model() -> Model {
        let mut app = make_movie_app();
        app.libs[0].library.collection_type = "tvshows".into();
        for item in &mut app.libs[0].nav_stack[0].items {
            item.item_type = "Series".into();
        }
        app.layout.main.tv_wide_right_area = Rect::new(40, 0, 60, 20);
        let mut model = Model::new(app);
        model.sync_tv_workspace();
        model
    }

    #[test]
    fn push_tv_workspace_content_projects_selected_series_on_mount() {
        let model = mounted_tv_model();
        let id = model
            .tv_workspace_id
            .as_ref()
            .expect("TV workspace mounted");
        let component = model
            .application
            .get_component(id)
            .expect("TV workspace component mounted")
            .as_any()
            .downcast_ref::<TvWorkspaceComponent>()
            .expect("TV workspace component type");
        assert_eq!(component.selected_item_id(), Some("movie-focused".into()));
    }

    #[test]
    fn typed_tv_requests_keep_component_cursor_authoritative() {
        let mut model = mounted_tv_model();
        let id = model.tv_workspace_id.clone().expect("TV workspace mounted");
        let request = model
            .application
            .get_component_mut(&id)
            .expect("TV workspace component mounted")
            .as_any_mut()
            .downcast_mut::<TvWorkspaceComponent>()
            .expect("TV workspace component type")
            .on(&Event::Keyboard(KeyEvent {
                code: Key::Down,
                modifiers: KeyModifiers::NONE,
            }));
        let Some(Msg::Shell(request)) = request else {
            panic!("TV Down must produce a typed shell request");
        };
        assert!(matches!(request, ShellRequest::TvMoveRows { rows: 1 }));
        model.handle_tv_request(request);
        assert_eq!(model.app.libs[0].nav_stack[0].cursor, 1);
        let selected_id = model
            .application
            .get_component(&id)
            .and_then(|component| component.as_any().downcast_ref::<TvWorkspaceComponent>())
            .and_then(TvWorkspaceComponent::selected_item_id);
        assert_eq!(selected_id, Some("movie-second".into()));
    }

    #[test]
    fn tv_episode_activation_uses_component_cursors_and_cached_season_id() {
        let mut model = mounted_tv_model();
        let mut season_one = crate::app::tests::make_item("Season 1", "Season");
        season_one.id = "season-1".into();
        let mut season_two = crate::app::tests::make_item("Season 2", "Season");
        season_two.id = "season-2".into();
        let mut episode = crate::app::tests::make_item("Episode 2", "Episode");
        episode.id = "episode-2".into();
        episode.series_id = "movie-focused".into();
        let mut episodes = std::collections::HashMap::new();
        episodes.insert("season-2".into(), vec![episode]);
        model.app.series_detail_cache.insert(
            "movie-focused".into(),
            crate::app::SeriesDetail {
                seasons: vec![season_one, season_two],
                episodes,
            },
        );
        model.push_tv_workspace_content();
        let id = model.tv_workspace_id.clone().expect("TV workspace mounted");

        let enter_series = model
            .application
            .get_component_mut(&id)
            .expect("TV workspace component mounted")
            .as_any_mut()
            .downcast_mut::<TvWorkspaceComponent>()
            .expect("TV workspace component type")
            .on(&Event::Keyboard(KeyEvent {
                code: Key::Enter,
                modifiers: KeyModifiers::NONE,
            }));
        let Some(Msg::Shell(enter_series)) = enter_series else {
            panic!("series Enter must produce a typed request");
        };
        model.handle_tv_request(enter_series);

        let season = model
            .application
            .get_component_mut(&id)
            .expect("TV workspace component mounted")
            .as_any_mut()
            .downcast_mut::<TvWorkspaceComponent>()
            .expect("TV workspace component type")
            .on(&Event::Keyboard(KeyEvent {
                code: Key::Char(']'),
                modifiers: KeyModifiers::NONE,
            }));
        assert!(matches!(
            season,
            Some(Msg::Shell(ShellRequest::TvSeasonMove { delta: 1 }))
        ));
        // Make the App library cursor stale after the component has selected
        // the series; episode activation must not consult that cursor.
        model.app.libs[0].nav_stack[0].cursor = 1;

        let episode_request = model
            .application
            .get_component(&id)
            .expect("TV workspace component mounted")
            .as_any()
            .downcast_ref::<TvWorkspaceComponent>()
            .expect("TV workspace component type")
            .episode_activation_selection();
        assert_eq!(episode_request, Some(("movie-focused".into(), 1, 0)));
        model.handle_tv_request(ShellRequest::TvEpisodeActivate);
        assert!(model.app.play_tv_episode("movie-focused", 1, 0));
        assert!(!model.app.play_tv_episode("movie-focused", 0, 0));
        assert!(!model.app.play_tv_episode("movie-focused", 1, 1));
        assert!(!model.app.play_tv_episode("missing-series", 1, 0));
    }

    /// Build a two-level stack: a Series parent list whose cursor is parked
    /// off the child's parent, plus an empty Seasons child whose `parent_id`
    /// points back at parent item 0. Used to prove `go_back` restores the
    /// parent cursor by `parent_id`, never by the popped (mirrored) cursor.
    /// Build a two-level stack: a Series parent list (three rows, cursor
    /// parked off the child's parent) plus a Seasons child whose `parent_id`
    /// targets a discriminating nonzero parent row (index 2) and whose items
    /// contain the component's selected id so the mirror actually mutates
    /// this (last) level's cursor. Used to prove `go_back` restores the parent
    /// cursor by `parent_id`, never by the popped (mirrored) cursor.
    fn tv_two_level_model() -> Model {
        let mut model = mounted_tv_model();
        let mut third = crate::app::tests::make_item("Third Series", "Series");
        third.id = "movie-third".into();
        model.app.libs[0].nav_stack[0].items.push(third);
        model.app.libs[0].nav_stack[0].cursor = 0;
        let mut mirror_target = crate::app::tests::make_item("S", "Season");
        mirror_target.id = "movie-focused".into();
        model.app.libs[0].nav_stack.push(crate::app::BrowseLevel {
            parent_id: "movie-third".into(),
            title: "Seasons".into(),
            items: vec![
                crate::app::tests::make_item("Season A", "Season"),
                mirror_target,
            ],
            total_count: 2,
            cursor: 0,
            scroll: 0,
            item_types: Some("Season".into()),
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: false,
            all_items: None,
            letter_filter: None,
            music_grouping: None,
        });
        model
    }

    /// Build a three-level stack: Series parent -> Seasons child -> Episodes
    /// grandchild, so a single `go_back` from Episodes must auto-skip the
    /// Season level and still restore the Series cursor by `parent_id`.
    /// Build a three-level stack: Series parent (three rows) -> Seasons child
    /// -> Episodes grandchild, where the Season level's `parent_id` targets a
    /// discriminating nonzero parent row and the Episodes level's items
    /// contain the component's selected id so the mirror mutates it. A single
    /// `go_back` from Episodes must auto-skip the Season level and still
    /// restore the Series cursor by `parent_id`.
    fn tv_season_skip_model() -> Model {
        let mut model = mounted_tv_model();
        let mut third = crate::app::tests::make_item("Third Series", "Series");
        third.id = "movie-third".into();
        model.app.libs[0].nav_stack[0].items.push(third);
        model.app.libs[0].nav_stack[0].cursor = 0;
        let mut season = crate::app::tests::make_item("Season 1", "Season");
        season.id = "season-1".into();
        model.app.libs[0].nav_stack.push(crate::app::BrowseLevel {
            parent_id: "movie-third".into(),
            title: "Seasons".into(),
            items: vec![season],
            total_count: 1,
            cursor: 0,
            scroll: 0,
            item_types: Some("Season".into()),
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: false,
            all_items: None,
            letter_filter: None,
            music_grouping: None,
        });
        let mut mirror_target = crate::app::tests::make_item("E", "Episode");
        mirror_target.id = "movie-focused".into();
        model.app.libs[0].nav_stack.push(crate::app::BrowseLevel {
            parent_id: "season-1".into(),
            title: "Episodes".into(),
            items: vec![
                crate::app::tests::make_item("Episode 1", "Episode"),
                mirror_target,
            ],
            total_count: 2,
            cursor: 0,
            scroll: 0,
            item_types: Some("Episode".into()),
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: false,
            all_items: None,
            letter_filter: None,
            music_grouping: None,
        });
        model
    }

    #[test]
    fn activate_selected_series_resolves_mirrored_cursor_and_guards_series() {
        let mut model = mounted_tv_model();

        // Divergence: park App's cursor on "movie-second" (index 1) while the
        // mounted component still selects "movie-focused". Without the mirror
        // the resolution would follow the stale App cursor; with it,
        // activation follows the component's selected item.
        model.app.libs[0].nav_stack[0].cursor = 1;
        assert_eq!(
            model.app.selected_series_item(0).map(|i| i.id),
            Some("movie-second".into()),
            "stale App cursor resolves a different Series before the mirror"
        );
        model.mirror_tv_workspace_cursor(0);
        assert_eq!(
            model.app.selected_series_item(0).map(|i| i.id),
            Some("movie-focused".into()),
            "mirror realigns the cursor to the component's selected Series"
        );

        // Wide TV layout => enter_series_selection targets the component's
        // Series (asserted by the resolved target, not merely the bool).
        let wide_target = model.app.selected_series_item(0).expect("series");
        assert_eq!(wide_target.id, "movie-focused");
        assert!(model.app.activate_selected_series(0));

        // Narrow layout => open_series_selection_modal targets the same
        // Series, proven by the modal's Series source id.
        model.app.layout.main.tv_wide_right_area = ratatui::layout::Rect::default();
        model.mirror_tv_workspace_cursor(0);
        model.app.activate_selected_series(0);
        match model.app.pending_overlay.as_ref() {
            Some(crate::app::types_overlay::OverlayRequest::SelectionModal(modal)) => {
                if let crate::app::types_selection_modal::SelectionModalSource::Series {
                    series_id,
                } = &modal.source
                {
                    assert_eq!(
                        series_id.as_str(),
                        "movie-focused",
                        "narrow activation must target the component's selected Series"
                    );
                } else {
                    panic!("narrow activation must open a Series selection modal");
                }
            }
            _ => panic!("narrow layout must open the series selection modal"),
        }
        model.app.pending_overlay = None;

        // Guard 1: a non-tvshows collection_type rejects.
        model.app.libs[0].library.collection_type = "movies".into();
        assert!(!model.app.activate_selected_series(0));

        // Guard 2: a selected item that is not a Series rejects.
        model.app.libs[0].library.collection_type = "tvshows".into();
        model.app.libs[0].nav_stack[0].items[0].item_type = "Movie".into();
        assert!(!model.app.activate_selected_series(0));
    }

    #[test]
    fn go_back_ignores_popped_level_cursor_and_restores_by_parent_id() {
        // Without a prior mirror: the popped child cursor is deliberately
        // stale (99), yet go_back restores the parent cursor to row 2 (the
        // position of the child's parent_id "movie-third") -- by parent_id,
        // not the stale 99 and not a reset 0.
        let mut no_mirror = tv_two_level_model();
        no_mirror.app.libs[0].nav_stack.last_mut().unwrap().cursor = 99;
        no_mirror.app.go_back(0);
        assert_eq!(no_mirror.app.libs[0].nav_stack.len(), 1);
        assert_eq!(no_mirror.app.libs[0].nav_stack[0].cursor, 2);

        // With a prior mirror call that actually mutates the popped child
        // cursor (to 1, the position of the component's selected id within the
        // child items): the restored parent cursor is still 2, identical and
        // not the mutated child cursor.
        let mut with_mirror = tv_two_level_model();
        with_mirror.app.libs[0].nav_stack.last_mut().unwrap().cursor = 99;
        with_mirror.mirror_tv_workspace_cursor(0);
        assert_eq!(
            with_mirror.app.libs[0].nav_stack.last().unwrap().cursor,
            1,
            "mirror must mutate the popped child cursor to the component selection"
        );
        with_mirror.app.go_back(0);
        assert_eq!(with_mirror.app.libs[0].nav_stack.len(), 1);
        assert_eq!(with_mirror.app.libs[0].nav_stack[0].cursor, 2);

        // Season auto-skip: from the Episodes level, one go_back skips the
        // Season level and restores the Series cursor by parent_id (row 2),
        // despite a stale popped cursor (42).
        let mut skip = tv_season_skip_model();
        skip.app.libs[0].nav_stack.last_mut().unwrap().cursor = 42;
        skip.app.go_back(0);
        assert_eq!(skip.app.libs[0].nav_stack.len(), 1);
        assert_eq!(skip.app.libs[0].nav_stack[0].cursor, 2);
    }

    #[test]
    fn cycle_letter_pill_derives_from_filter_not_cursor() {
        // A tvshows library large enough to surface letter pills, at its top
        // browse level with pill bucket 0 (A–C) selected.
        let mut model = mounted_tv_model();
        model.app.libs[0].library_total = Some(1000);
        model.app.libs[0].nav_stack[0].letter_filter =
            Some(crate::app::render::LetterFilter::for_index(0).unwrap());

        // Stale cursor: cycle_letter_pill must ignore `level.cursor` and
        // advance the filter 0 -> 1 purely from `letter_filter`.
        model.app.libs[0].nav_stack[0].cursor = 7;
        model.app.cycle_letter_pill(0, 1);
        let after_stale = model.app.libs[0].nav_stack[0].letter_filter.clone();
        assert_eq!(after_stale.as_ref().map(|f| f.index), Some(1));
        assert!(model.app.libs[0].nav_stack[0].loading);
        // select_letter_pill intentionally resets the level cursor to 0,
        // regardless of its prior (stale) value.
        assert_eq!(
            model.app.libs[0].nav_stack[0].cursor, 0,
            "select_letter_pill resets the level cursor regardless of its prior value"
        );

        // Fresh cursor at a different value: identical filter result, proving
        // the cycle never consults `level.cursor`.
        let mut fresh = mounted_tv_model();
        fresh.app.libs[0].library_total = Some(1000);
        fresh.app.libs[0].nav_stack[0].letter_filter =
            Some(crate::app::render::LetterFilter::for_index(0).unwrap());
        fresh.app.libs[0].nav_stack[0].cursor = 0;
        fresh.app.cycle_letter_pill(0, 1);
        assert_eq!(
            fresh.app.libs[0].nav_stack[0].letter_filter, after_stale,
            "cycle_letter_pill result must not depend on level.cursor"
        );
    }
}
