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
    use crate::app::render::{make_movie_app, LibraryListRenderCtx, TvWideRenderCtx};
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
}
