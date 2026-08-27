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

    pub fn mirror_tv_workspace_cursor(&mut self, lib_idx: usize) {
        let Some(id) = self.tv_workspace_id.as_ref() else {
            return;
        };
        let Some(item_id) = self
            .application
            .get_component(id)
            .and_then(|component| component.as_any().downcast_ref::<TvWorkspaceComponent>())
            .and_then(TvWorkspaceComponent::selected_item_id)
            .map(str::to_owned)
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
    use crate::app::tests::make_item;
    use tuirealm::component::AppComponent;
    use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};

    #[test]
    fn typed_tv_requests_keep_component_cursor_authoritative() {
        let _guard = crate::config::TestStateDirGuard::new();
        let mut model = Model::new(make_movie_app());
        let mut component = TvWorkspaceComponent::new();
        component.set_content(TvWideRenderCtx::new(
            LibraryListRenderCtx::from_items(
                vec![
                    make_item("Series A", "Series"),
                    make_item("Series B", "Series"),
                ],
                0,
                0,
            ),
            None,
            None,
            0,
            None,
            true,
            false,
        ));

        let down = component.on(&Event::Keyboard(KeyEvent {
            code: Key::Down,
            modifiers: KeyModifiers::NONE,
        }));
        assert_eq!(component.cursor(), 1);
        let Some(Msg::Shell(request)) = down else {
            panic!("TV Down must produce a typed shell request");
        };
        assert!(matches!(request, ShellRequest::TvMoveRows { rows: 1 }));
        model.handle_tv_request(request);
        assert_eq!(model.app.libs[0].nav_stack[0].cursor, 0);

        let end = component.on(&Event::Keyboard(KeyEvent {
            code: Key::End,
            modifiers: KeyModifiers::NONE,
        }));
        assert_eq!(component.cursor(), 1);
        let Some(Msg::Shell(request)) = end else {
            panic!("TV End must produce a typed shell request");
        };
        assert!(matches!(
            request,
            ShellRequest::TvJumpCursor { to_end: true }
        ));
        model.handle_tv_request(request);
        assert_eq!(model.app.libs[0].nav_stack[0].cursor, 0);
    }
}
