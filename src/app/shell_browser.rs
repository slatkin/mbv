use super::components::{BrowserComponent, BrowserKey, BrowserKind, ComponentId};
use super::shell::Model;
use super::{PanelFocus, TabSelection};
use mbv_core::config::ServiceKind;

impl Model {
    fn emby_browser_component_id(&self) -> Option<ComponentId> {
        let TabSelection::EmbyLibrary(index) = self.app.tab else {
            return None;
        };
        let library = self.app.libs.get(index)?;
        if self.app.is_podcast_library(index) || self.app.is_feed_home_video_group_view(index) {
            return None;
        }
        let kind = BrowserKind::from_collection_type(&library.library.collection_type);
        if !matches!(
            kind,
            BrowserKind::Generic | BrowserKind::Movies | BrowserKind::HomeVideos
        ) {
            return None;
        }
        Some(ComponentId::Browser(BrowserKey {
            service: ServiceKind::Emby,
            library_id: library.library.id.clone(),
            kind,
        }))
    }

    pub(super) fn sync_emby_browser(&mut self) {
        let next_id = self.emby_browser_component_id();
        if self.emby_browser_id != next_id {
            if let Some(id) = self.emby_browser_id.take() {
                let _ = self.application.umount(&id);
            }
            if let Some(id) = next_id.clone() {
                self.application
                    .mount(id.clone(), Box::new(BrowserComponent::new()), vec![])
                    .expect("mount Emby browser");
                self.application.active(&id).expect("activate Emby browser");
                self.emby_browser_id = Some(id);
            }
        }

        let Some(id) = self.emby_browser_id.as_ref() else {
            return;
        };
        let TabSelection::EmbyLibrary(index) = self.app.tab else {
            return;
        };
        let context = self.app.library_list_render_ctx(index, false);
        let focused = matches!(self.app.effective_panel_focus(), PanelFocus::Library);
        if let Some(comp) = self.application.get_component_mut(id) {
            if let Some(browser) = comp.as_any_mut().downcast_mut::<BrowserComponent>() {
                browser.set_content(context, focused);
            }
        }
    }

    pub(super) fn render_emby_browser_component(&mut self, frame: &mut ratatui::Frame) {
        let Some(id) = self.emby_browser_id.as_ref() else {
            return;
        };
        let area = self.app.layout.main.left_area;
        if area.width == 0 || area.height == 0 {
            return;
        }
        self.application.view(id, frame, area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::components::{BrowserComponent, LegacyTerminalEvent, Msg};
    use crate::app::render::make_movie_app;
    use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};

    #[test]
    fn shell_mounts_and_syncs_the_generic_emby_browser() {
        let mut model = Model::new(make_movie_app());
        model.sync_emby_browser();
        let id = model.emby_browser_id.clone().expect("browser mounted");
        let message = {
            model
                .application
                .get_component_mut(&id)
                .unwrap()
                .on(&Event::Keyboard(KeyEvent {
                    code: Key::Down,
                    modifiers: KeyModifiers::NONE,
                }))
        };
        let Some(Msg::Legacy(LegacyTerminalEvent::Key(key))) = message else {
            panic!("browser movement should forward to the legacy handler");
        };
        assert!(!model.app.handle_key(key));
        model.sync_emby_browser();
        assert_eq!(model.app.libs[0].nav_stack[0].cursor, 1);
        assert!(model
            .application
            .get_component(&id)
            .unwrap()
            .as_any()
            .downcast_ref::<BrowserComponent>()
            .is_some());
    }
}
