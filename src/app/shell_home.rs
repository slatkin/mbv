//! Home sync/render methods for the shell `Model` (design D2/D9, task 3.4).
//!
//! `HomeComponent` is mounted for the whole session (never unmounted, never
//! made TuiRealm-`active`): keyboard/mouse input for Home stays on the
//! legacy `App::handle_key`/`handle_mouse` path (see `components::home`'s
//! module doc for why), so the component only needs to render. Every tick
//! the shell mirrors Home content into the component, then paints the
//! component's `view()` over the legacy frame. Cursor/section/scroll state
//! stays in the component and is not pushed back in from `App`.

use super::components::{ComponentId, HomeComponent, ShellRequest};
use super::shell::Model;
use super::{PanelFocus, TabSelection};
use mbv_core::playback_queue::QueueItem;

impl Model {
    /// Route the Home typed effects (task 5.3d, Home typed-effect prep) to
    /// their `App` handlers with the component-provided target index.
    /// `HomeComponent` owns the cursor; the effect must act on the requested
    /// target directly — never by copying it into `App::home.home_cursor`
    /// and re-reading that field. `HomeToggleWatched` carries no index: it
    /// targets the Continue Watching column's own cursor (`continue_cursor`),
    /// matching the legacy `cw_toggle_watched` (preserved, not fixed).
    pub(super) fn handle_home_request(&mut self, request: ShellRequest) {
        match request {
            ShellRequest::HomePlay(cursor) => self.app.home_play(cursor),
            ShellRequest::HomeEnqueue(cursor) => self.app.home_enqueue(cursor),
            ShellRequest::HomeDelete(cursor) => self.app.home_delete(cursor),
            ShellRequest::HomeToggleWatched => self.app.cw_toggle_watched(),
            ShellRequest::HomeSectionSelected(section) => self.app.home_select_section(section),
            _ => {}
        }
    }

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
    use crate::app::tests::{make_app_stub, make_item, make_items};
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

    /// Task 5.3d, Home typed-effect prep: the shell routes each typed Home
    /// effect to its `App` handler with the component's target, and the effect
    /// acts on that target even when `App::home.home_cursor` points elsewhere.
    /// Enqueue is proven by the queued item's id; the section preference by
    /// the resulting `home.section`. The emby-gated effects prove their target
    /// by acting at all: absent a live Emby service they flash
    /// "Emby is unavailable", while a `home_cursor` parked on the latest
    /// folder (play) or past the CW range (delete/toggle) would skip silently.
    #[test]
    fn shell_home_effects_honor_component_target_not_app_home_cursor() {
        let _guard = crate::config::TestStateDirGuard::new();
        let mut model = Model::new(make_app_stub());
        // Three Continue Watching rows (ids id0..id2) plus one latest pill
        // holding a folder, so a home_cursor parked there makes `home_play`
        // return early via the folder guard — a clean "wrong target" signal.
        model.app.home.continue_items = make_items(3);
        let mut folder = make_item("folder", "CollectionFolder");
        folder.is_folder = true;
        model.app.home.latest = vec![(
            "Folder".into(),
            crate::app::types_playback::HomeLatestSource::Emby("lib".into()),
            vec![mbv_core::playback_queue::QueueItem::Emby(Box::new(folder))],
            0,
        )];
        let cw_len = model.app.home.continue_items.len();
        let folder_flat = cw_len; // flat index of the latest folder item

        // HomeEnqueue: the requested CW row (id2) is queued, not the row
        // under home_cursor (id0).
        model.app.home.home_cursor = 0;
        model.handle_home_request(ShellRequest::HomeEnqueue(2));
        let queued = model.app.player_tab.emby_items();
        assert_eq!(queued.len(), 1);
        assert_eq!(queued[0].id, "id2");

        // HomePlay: the requested CW row (id0) is resumed — the emby-gated
        // resume flashes "Emby is unavailable" — while a home_cursor parked
        // on the latest folder would return early (folder guard).
        model.app.status.clear();
        model.app.home.home_cursor = folder_flat;
        model.handle_home_request(ShellRequest::HomePlay(0));
        assert_eq!(
            model.app.status, "Emby is unavailable",
            "play must act on the CW target, not the home_cursor folder"
        );

        // HomeDelete: the requested CW row (in range) is removed — again the
        // emby-gated removal flashes — while a home_cursor past the CW range
        // would be skipped by the delete guard.
        model.app.status.clear();
        model.app.home.home_cursor = folder_flat; // >= cw_len
        model.handle_home_request(ShellRequest::HomeDelete(0));
        assert_eq!(
            model.app.status, "Emby is unavailable",
            "delete must act on the CW target, not skip on an out-of-range home_cursor"
        );

        // HomeToggleWatched: carries no index; it targets the Continue
        // Watching column's own cursor (continue_cursor row 1), not
        // home_cursor.
        model.app.status.clear();
        model.app.home.continue_cursor = 1;
        model.app.home.home_cursor = folder_flat;
        model.handle_home_request(ShellRequest::HomeToggleWatched);
        assert_eq!(
            model.app.status, "Emby is unavailable",
            "toggle must act on the continue_cursor target, not home_cursor"
        );

        // HomeSectionSelected: the requested pill index is persisted even
        // though home_cursor pointed elsewhere.
        model.app.home.home_cursor = 0;
        model.handle_home_request(ShellRequest::HomeSectionSelected(1));
        assert_eq!(
            model.app.home.section, 1,
            "section preference must be the requested target"
        );
    }
}
