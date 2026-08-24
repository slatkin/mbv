//! Home sync/render methods for the shell `Model` (design D2/D9, task 3.4).
//!
//! `HomeComponent` is mounted for the whole session (never unmounted, never
//! made TuiRealm-`active`): keyboard/mouse input for Home stays on the
//! legacy `App::handle_key`/`handle_mouse` path (see `components::home`'s
//! module doc for why), so the component only needs to render. Every tick
//! the shell mirrors Home content into the component, then paints the
//! component's `view()` over the legacy frame. Cursor/section/scroll state
//! stays in the component and is not pushed back in from `App`.

use super::components::{ComponentId, HomeComponent};
use super::shell::Model;
use super::{PanelFocus, TabSelection};
use mbv_core::playback_queue::QueueItem;

impl Model {
    /// Mount `HomeComponent` for the session. Called once from `Model::new`;
    /// never unmounted (Home is always available, matching `App.tab`
    /// defaulting to `TabSelection::Home`).
    pub(super) fn mount_home(&mut self) {
        self.application
            .mount(ComponentId::Home, Box::new(HomeComponent::new()), vec![])
            .expect("mount Home");
    }

    /// Mirror `App.home`'s content and runtime render flags into the mounted
    /// `HomeComponent`. Its cursor, section, and scroll are component-local.
    pub(super) fn sync_home(&mut self) {
        let continue_items: Vec<QueueItem> = self
            .app
            .home
            .continue_items
            .iter()
            .cloned()
            .map(|item| QueueItem::Emby(Box::new(item)))
            .collect();
        let latest = self
            .app
            .home
            .latest
            .iter()
            .map(|(title, source, items, _cursor)| (title.clone(), source.clone(), items.clone()))
            .collect();
        let focused = !matches!(self.app.effective_panel_focus(), PanelFocus::Queue);
        let use_nerd_fonts = self.app.use_nerd_fonts;
        if let Some(comp) = self.application.get_component_mut(&ComponentId::Home) {
            if let Some(home) = comp.as_any_mut().downcast_mut::<HomeComponent>() {
                home.set_content(continue_items, latest, self.app.home_loading);
                home.set_focused(focused);
                home.set_use_nerd_fonts(use_nerd_fonts);
            }
        }
    }

    /// Paint `HomeComponent`'s `view()` over the legacy frame when Home is
    /// the active destination, then paint the cover image it computed but
    /// couldn't paint itself (no image-cache authority in the component).
    pub(super) fn render_home_component(&mut self, f: &mut ratatui::Frame) {
        if !matches!(self.app.tab, TabSelection::Home) {
            return;
        }
        let area = self.app.layout.main.home_area;
        if area.width == 0 || area.height == 0 {
            return;
        }
        self.application.view(&ComponentId::Home, f, area);
        let image_paint = self
            .application
            .get_component_mut(&ComponentId::Home)
            .and_then(|comp| comp.as_any_mut().downcast_mut::<HomeComponent>())
            .and_then(|home| home.take_image_paint());
        self.app.paint_home_image(f, image_paint);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::components::msg::{LegacyTerminalEvent, Msg};
    use crate::app::tests::make_app_stub;
    use tuirealm::component::AppComponent;
    use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};

    #[test]
    fn shell_sync_keeps_home_component_cursor_local() {
        let mut model = Model::new(make_app_stub());
        model.app.home.continue_items = vec![
            crate::app::tests::make_item("one", "Movie"),
            crate::app::tests::make_item("two", "Movie"),
        ];
        model.sync_home();

        let message = {
            let component = model
                .application
                .get_component_mut(&ComponentId::Home)
                .expect("Home component mounted")
                .as_any_mut()
                .downcast_mut::<HomeComponent>()
                .expect("Home component type");
            component.on(&Event::Keyboard(KeyEvent {
                code: Key::Down,
                modifiers: KeyModifiers::NONE,
            }))
        };
        assert_eq!(message, Some(Msg::Legacy(LegacyTerminalEvent::NoOp)));

        model.sync_home();

        let component = model
            .application
            .get_component(&ComponentId::Home)
            .expect("Home component mounted")
            .as_any()
            .downcast_ref::<HomeComponent>()
            .expect("Home component type");
        assert_eq!(component.cursor(), 1);
    }
}
