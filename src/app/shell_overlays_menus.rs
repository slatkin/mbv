use super::super::components::{
    ComponentId, ContextMenuComponent, FeedsManageComponent, LibraryRoutesComponent,
    MultiselectComponent, OverlayId, PopupId, SelectionModalComponent, ShellRequest,
};
use super::super::shell::Model;
use crate::app::types_context_menu::{ContextMenu, ContextMenuAnchor, ContextMenuEntry};
use crate::app::PanelFocus;
use ratatui::layout::Rect;

impl Model {
    // --- Context menu -------------------------------------------------------

    fn context_menu_id() -> ComponentId {
        ComponentId::Overlay(OverlayId::ContextMenu)
    }

    /// Compute the context menu's painted rect from the current `AppLayout`
    /// and the menu's anchor/entries. Replaces the old `layout.context_menu_rect`
    /// global written during `App::render` (task 5.3c); the component now owns
    /// its rect and hit test.
    fn context_menu_rect(&self, anchor: ContextMenuAnchor, entries: &[ContextMenuEntry]) -> Rect {
        let layout = &self.app.layout;
        let size = ContextMenu::rendered_size(entries);
        let (panel_rect, anchor_rect): (Rect, Option<Rect>) = match &anchor {
            ContextMenuAnchor::SelectedItem(focus) => {
                let (panel, selected) = match focus {
                    PanelFocus::Library => (layout.main.left_area, layout.main.selected_item_rect),
                    PanelFocus::Queue => {
                        (layout.main.queue_area, layout.main.queue_selected_item_rect)
                    }
                };
                (panel, selected)
            }
            ContextMenuAnchor::Pointer { .. } => {
                let panel = match self.app.effective_panel_focus() {
                    PanelFocus::Library if layout.main.is_wide_tv_active() => {
                        let pos = match &anchor {
                            ContextMenuAnchor::Pointer { x, y } => (*x, *y).into(),
                            ContextMenuAnchor::SelectedItem(_) => unreachable!(),
                        };
                        if layout.main.tv_wide_left_area.contains(pos) {
                            layout.main.tv_wide_left_area
                        } else {
                            layout.main.tv_wide_right_area
                        }
                    }
                    PanelFocus::Library => layout.main.left_area,
                    PanelFocus::Queue => layout.main.queue_area,
                };
                (panel, None)
            }
        };
        let pointer = match &anchor {
            ContextMenuAnchor::Pointer { x, y } => Some((*x, *y)),
            _ => None,
        };
        let (x, y) = ContextMenu::place(panel_rect, size, anchor_rect.as_ref(), pointer);
        Rect {
            x,
            y,
            width: size.0,
            height: size.1,
        }
    }

    /// Render the ContextMenu overlay if mounted. Placement is recomputed from
    /// `AppLayout` each frame (so it follows the fresh layout after a resize),
    /// then passed to the component via downcast (task 5.3c).
    pub(in crate::app) fn render_context_menu_overlay(&mut self, f: &mut ratatui::Frame) {
        let id = Self::context_menu_id();
        if !self.application.mounted(&id) {
            return;
        }
        // Read the menu's anchor/entries immutably, compute the rect, then
        // borrow mutably only to set it (avoids aliasing `self`).
        let (anchor, entries) = {
            let comp = self.application.get_component(&id);
            let Some(menu) = comp
                .and_then(|component| component.as_any().downcast_ref::<ContextMenuComponent>())
            else {
                return;
            };
            (menu.anchor(), menu.entries().to_vec())
        };
        let rect = self.context_menu_rect(anchor, &entries);
        if let Some(comp) = self.application.get_component_mut(&id) {
            if let Some(menu) = comp.as_any_mut().downcast_mut::<ContextMenuComponent>() {
                menu.set_rect(rect);
            }
        }
        self.application.view(&id, f, f.area());
    }

    /// Shell-owned key handling for the Context menu (task 5.3c): the component
    /// owns cursor/selection rendering; the shell owns action dispatch.
    pub(in crate::app) fn handle_context_menu_key(&mut self, key: crossterm::event::KeyEvent) {
        let id = Self::context_menu_id();
        if !self.application.mounted(&id) {
            return;
        }
        match key.code {
            crossterm::event::KeyCode::Up | crossterm::event::KeyCode::Down => {
                if let Some(comp) = self.application.get_component_mut(&id) {
                    if let Some(menu) = comp.as_any_mut().downcast_mut::<ContextMenuComponent>() {
                        menu.move_cursor(key.code == crossterm::event::KeyCode::Down);
                    }
                }
            }
            crossterm::event::KeyCode::Enter => {
                let action = self
                    .application
                    .get_component(&id)
                    .and_then(|component| component.as_any().downcast_ref::<ContextMenuComponent>())
                    .and_then(|menu| menu.action_at(menu.cursor()));
                self.dismiss_context_menu();
                self.app.execute_context_action(action);
            }
            crossterm::event::KeyCode::Esc => self.dismiss_context_menu(),
            _ => {}
        }
    }

    /// Activate the context-menu entry at the component-owned cursor (mouse
    /// click on a selectable row, or hover-resolved selection).
    pub(in crate::app) fn handle_context_menu_select(&mut self, idx: usize) {
        let id = Self::context_menu_id();
        if !self.application.mounted(&id) {
            return;
        }
        let action = self
            .application
            .get_component(&id)
            .and_then(|component| component.as_any().downcast_ref::<ContextMenuComponent>())
            .and_then(|menu| menu.action_at(idx));
        self.dismiss_context_menu();
        self.app.execute_context_action(action);
    }

    fn dismiss_context_menu(&mut self) {
        let id = Self::context_menu_id();
        if self.application.mounted(&id) {
            let _ = self.application.umount(&id);
        }
    }

    // --- Selection modal ----------------------------------------------------

    fn selection_modal_id() -> ComponentId {
        ComponentId::Overlay(OverlayId::SelectionModal)
    }

    /// Route typed Selection modal requests to the existing source-specific
    /// App actions after reading the component-owned snapshot.
    pub(in crate::app) fn handle_selection_modal_request(&mut self, request: ShellRequest) {
        let id = Self::selection_modal_id();
        match request {
            ShellRequest::DismissSelectionModal => self.app.close_selection_modal(),
            ShellRequest::SelectionModalFilterSelected | ShellRequest::SelectionModalRefresh => {
                let Some((source, selected)) = self
                    .application
                    .get_component(&id)
                    .and_then(|component| {
                        component.as_any().downcast_ref::<SelectionModalComponent>()
                    })
                    .and_then(|selection| {
                        Some((
                            selection.source()?.clone(),
                            selection.filter_selected().unwrap_or(0),
                        ))
                    })
                else {
                    return;
                };
                match source {
                    super::super::types_selection_modal::SelectionModalSource::Series {
                        series_id,
                    } => self
                        .app
                        .select_series_selection_modal_season(series_id, selected),
                    super::super::types_selection_modal::SelectionModalSource::Podcast {
                        library_item_id,
                    } => self
                        .app
                        .select_podcast_selection_modal_filter(library_item_id, selected),
                    _ => {}
                }
            }
            ShellRequest::SelectionModalActivate(item_id) => {
                let Some(source) = self
                    .application
                    .get_component(&id)
                    .and_then(|component| {
                        component.as_any().downcast_ref::<SelectionModalComponent>()
                    })
                    .and_then(SelectionModalComponent::source)
                    .cloned()
                else {
                    return;
                };
                self.app.activate_selection_modal_item(source, item_id);
            }
            _ => {}
        }
    }

    /// Render the mounted Selection modal. The component owns its snapshot and
    /// records the returned geometry for its own mouse hit-testing.
    pub(in crate::app) fn render_selection_modal_overlay(&mut self, f: &mut ratatui::Frame) {
        let id = Self::selection_modal_id();
        if !self.application.mounted(&id) {
            return;
        }
        self.application.view(&id, f, f.area());
    }

    // --- Settings Multiselect popup ----------------------------------------

    fn multiselect_id() -> ComponentId {
        ComponentId::Popup(PopupId::Multiselect)
    }

    pub(in crate::app) fn sync_multiselect(&mut self) {
        let id = Self::multiselect_id();
        let mounted = self.application.mounted(&id);
        if self.app.multiselect_popup.is_some() && !mounted {
            self.application
                .mount(id.clone(), Box::new(MultiselectComponent::new()), vec![])
                .expect("mount Multiselect");
            self.application.active(&id).expect("activate Multiselect");
        } else if self.app.multiselect_popup.is_none() && mounted {
            let _ = self.application.umount(&id);
        }
        if let Some(popup) = self.app.multiselect_popup.as_ref() {
            if let Some(comp) = self.application.get_component_mut(&id) {
                if let Some(multiselect) = comp.as_any_mut().downcast_mut::<MultiselectComponent>()
                {
                    multiselect.set_content(popup);
                }
            }
        }
    }

    pub(in crate::app) fn handle_multiselect_commit(&mut self) {
        let id = Self::multiselect_id();
        let Some((kind, items)) = self
            .application
            .get_component_mut(&id)
            .and_then(|component| {
                component
                    .as_any_mut()
                    .downcast_mut::<MultiselectComponent>()
                    .and_then(|component| component.commit_snapshot())
            })
        else {
            return;
        };
        if self
            .app
            .multiselect_popup
            .as_ref()
            .is_some_and(|popup| popup.kind == kind)
        {
            if let Some(popup) = self.app.multiselect_popup.as_mut() {
                popup.items = items;
            }
            self.app.close_multiselect_popup();
        }
    }

    pub(in crate::app) fn render_multiselect_popup(&mut self, f: &mut ratatui::Frame) {
        let id = Self::multiselect_id();
        if self.application.mounted(&id) {
            self.application.view(&id, f, f.area());
        }
    }

    // --- Settings Library-routes popup -------------------------------------

    fn library_routes_id() -> ComponentId {
        ComponentId::Popup(PopupId::LibraryRoutes)
    }

    pub(in crate::app) fn sync_library_routes(&mut self) {
        let id = Self::library_routes_id();
        let mounted = self.application.mounted(&id);
        if self.app.library_routes_popup.is_some() && !mounted {
            self.application
                .mount(id.clone(), Box::new(LibraryRoutesComponent::new()), vec![])
                .expect("mount LibraryRoutes");
            self.application
                .active(&id)
                .expect("activate LibraryRoutes");
        } else if self.app.library_routes_popup.is_none() && mounted {
            let _ = self.application.umount(&id);
        }
        if let Some(popup) = self.app.library_routes_popup.as_ref() {
            if let Some(comp) = self.application.get_component_mut(&id) {
                if let Some(routes) = comp.as_any_mut().downcast_mut::<LibraryRoutesComponent>() {
                    routes.set_content(popup);
                }
            }
        }
    }

    fn sync_library_routes_to_app(&mut self) {
        let id = Self::library_routes_id();
        let Some((stage, cursor)) = self
            .application
            .get_component_mut(&id)
            .and_then(|component| {
                component
                    .as_any_mut()
                    .downcast_mut::<LibraryRoutesComponent>()
                    .and_then(|routes| routes.snapshot())
            })
        else {
            return;
        };
        if let Some(popup) = self.app.library_routes_popup.as_mut() {
            popup.stage = stage;
            popup.cursor = cursor;
        }
    }

    pub(in crate::app) fn handle_library_routes_request(&mut self, request: ShellRequest) {
        self.sync_library_routes_to_app();
        match request {
            ShellRequest::LibraryRoutesEnter => self.app.handle_library_routes_enter(),
            ShellRequest::LibraryRoutesEsc => self.app.handle_library_routes_esc(),
            _ => {}
        }
    }

    pub(in crate::app) fn render_library_routes_popup(&mut self, f: &mut ratatui::Frame) {
        let id = Self::library_routes_id();
        if self.application.mounted(&id) {
            self.application.view(&id, f, f.area());
        }
    }

    // --- Settings Feed-management popup ------------------------------------

    fn feeds_manage_id() -> ComponentId {
        ComponentId::Popup(PopupId::FeedManage)
    }

    pub(in crate::app) fn sync_feeds_manage(&mut self) {
        let id = Self::feeds_manage_id();
        let mounted = self.application.mounted(&id);
        if self.app.feeds_manage_popup.is_some() && !mounted {
            self.application
                .mount(id.clone(), Box::new(FeedsManageComponent::new()), vec![])
                .expect("mount FeedManage");
            self.application.active(&id).expect("activate FeedManage");
        } else if self.app.feeds_manage_popup.is_none() && mounted {
            let _ = self.application.umount(&id);
        }
        let Some(popup) = self.app.feeds_manage_popup.as_ref() else {
            return;
        };
        let feeds = self.app.config.lock().unwrap().feeds.clone();
        if let Some(comp) = self.application.get_component_mut(&id) {
            if let Some(feeds_manage) = comp.as_any_mut().downcast_mut::<FeedsManageComponent>() {
                feeds_manage.set_content(popup, feeds);
            }
        }
    }

    fn sync_feeds_manage_to_app(&mut self) {
        let id = Self::feeds_manage_id();
        let Some((stage, cursor)) = self
            .application
            .get_component_mut(&id)
            .and_then(|component| {
                component
                    .as_any_mut()
                    .downcast_mut::<FeedsManageComponent>()
                    .and_then(|feeds_manage| feeds_manage.snapshot())
            })
        else {
            return;
        };
        if let Some(popup) = self.app.feeds_manage_popup.as_mut() {
            popup.stage = stage;
            popup.cursor = cursor;
        }
    }

    pub(in crate::app) fn handle_feeds_manage_request(&mut self, key: crossterm::event::KeyEvent) {
        self.sync_feeds_manage_to_app();
        let _ = self.app.handle_key_feeds_manage(key);
    }

    pub(in crate::app) fn render_feeds_manage_popup(&mut self, f: &mut ratatui::Frame) {
        let id = Self::feeds_manage_id();
        if self.application.mounted(&id) {
            self.application.view(&id, f, f.area());
        }
    }
}
