//! Home sync/render methods for the shell `Model` (design D2/D9, task 3.4).
//!
//! `HomeComponent` is mounted for the whole session (never unmounted, never
//! made TuiRealm-`active`): keyboard/mouse input for Home stays on the
//! legacy `App::handle_key`/`handle_mouse` path (see `components::home`'s
//! module doc for why), so the component only needs to render. Every tick
//! the shell mirrors `App.home` into the component, then paints the
//! component's `view()` over the legacy frame -- the same "paint after"
//! pattern `shell_overlays.rs` uses for Search/Sessions/Help, but for an
//! inline (non-overlay) destination gated on `App.tab` instead of an open
//! flag.

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

    /// Mirror `App.home`'s content and legacy-input-driven cursor/section/
    /// scroll into the mounted `HomeComponent`, plus the runtime flags its
    /// render needs (focus, Nerd Font capability). Called every tick before
    /// rendering so the component's next `view()` matches what the legacy
    /// path just computed.
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
        let cursor = self.app.home.home_cursor;
        let section = self.app.home.section;
        let scroll = self.app.home.home_scroll;
        if let Some(comp) = self.application.get_component_mut(&ComponentId::Home) {
            if let Some(home) = comp.as_any_mut().downcast_mut::<HomeComponent>() {
                home.set_content(continue_items, latest, self.app.home_loading);
                home.sync_cursor_section_scroll(cursor, section, scroll);
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
