use super::super::Model;
use mbv_components::library_panel::LibraryPanel;
use mbv_components::{
    ContextMenuComponent, LibraryRoutesComponent, MultiselectComponent, QueueComponent,
};
use mbv_ui_model::context_menu::{
    ContextMenu, ContextMenuAnchor, ContextMenuEntry, LibraryRoutePopup, LibraryRouteStage,
    MultiSelectKind, MultiSelectPopup, is_bulk_action,
};
use mbv_ui_model::settings::PanelFocus;
use mbv_ui_msg::ContextMenuIntent;
use mbv_ui_msg::{ComponentId, OverlayId, PopupId, ShellRequest};
use ratatui::layout::Rect;

impl Model {
    // --- Context menu -------------------------------------------------------

    fn context_menu_id() -> ComponentId {
        ComponentId::Overlay(OverlayId::ContextMenu)
    }

    /// Context-menu geometry for any migrated `LibraryPanel` owner, regardless
    /// of tab (task 5.11 generalized): the panel's own last-painted list
    /// geometry. The panel gains this geometry from its own `view()` paint,
    /// so the menu placement tracks the panel's real paint rather than any
    /// copied-back legacy geometry. Returns `None` when the panel never
    /// painted, preserving the legacy `AppLayout` fallback.
    fn library_menu_geometry(&self) -> Option<(Rect, Option<Rect>)> {
        if !matches!(self.app.effective_panel_focus(), PanelFocus::Library) {
            return None;
        }
        self.application
            .get_component(&ComponentId::Library)
            .and_then(|component| component.as_any().downcast_ref::<LibraryPanel>())
            .and_then(LibraryPanel::menu_geometry)
    }

    /// Like `library_menu_geometry`, but for the mounted `QueueComponent`
    /// (task 3.1, design D11): the queue panel answers the context-menu
    /// keyboard anchor from its own retained geometry, not a shell mirror.
    fn queue_menu_geometry(&self) -> Option<(Rect, Option<Rect>)> {
        self.application
            .get_component(&ComponentId::Queue)
            .and_then(|component| component.as_any().downcast_ref::<QueueComponent>())
            .map(|queue| (queue.content_area(), queue.selected_row_rect()))
    }

    /// Compute the context menu's painted rect from the current anchor/entries
    /// and the owning surface's geometry. Replaces the old `layout.context_menu_rect`
    /// global written during `App::render` (task 5.3c); the component now owns
    /// its rect and hit test.
    fn context_menu_rect(
        &self,
        anchor: ContextMenuAnchor,
        entries: &[ContextMenuEntry],
        bounds: Option<mbv_ui_model::geom::Rect>,
    ) -> Rect {
        let layout = &self.app.layout;
        let size = ContextMenu::rendered_size(entries);
        // An opener-named containing panel (the queue visual slot's chrome
        // menu) wins ahead of the destination-derived panel: the slot menu is
        // chrome, so panel focus must not decide its clamp. For
        // component-owned destinations, use the geometry exported by the
        // active control. Other destinations and Queue focus keep using
        // `AppLayout` as before. The opener-named bounds are already the
        // neutral presentation `Rect`; the render geometry converts at the
        // edge (`extract-client-core` task 1.2).
        let (panel_rect, anchor_rect): (Rect, Option<Rect>) = if let Some(bounds) = bounds {
            (
                Rect::new(bounds.x, bounds.y, bounds.width, bounds.height),
                None,
            )
        } else {
            match &anchor {
                ContextMenuAnchor::SelectedItem(focus) => {
                    let (panel, selected) = match focus {
                        PanelFocus::Library => match self.library_menu_geometry() {
                            Some((panel, selected)) => (panel, selected),
                            // No owning component publishes a selected-row
                            // anchor for this destination yet; the panel
                            // still places the menu, just without a row
                            // anchor (matches the legacy `AppLayout` mirror's
                            // behaviour, which never populated this field).
                            None => (layout.left_area, None),
                        },
                        PanelFocus::Queue => self.queue_menu_geometry().unwrap_or_default(),
                    };
                    (panel, selected)
                }
                ContextMenuAnchor::Pointer { .. } => {
                    let panel = match self.app.effective_panel_focus() {
                        PanelFocus::Library
                            if self.app.tab.emby_library_index().is_some_and(|lib_idx| {
                                self.app.wide_tv_show_area(lib_idx).is_some()
                            }) =>
                        {
                            // Task 8.2 deleted the `tv_wide_*` pane rects; the
                            // panel's paint-free library content area is the
                            // placement region for the whole Wide TV workspace.
                            self.app
                                .tab
                                .emby_library_index()
                                .and_then(|lib_idx| self.app.wide_tv_show_area(lib_idx))
                                .unwrap_or_default()
                        }
                        PanelFocus::Library => self
                            .library_menu_geometry()
                            .map_or(layout.left_area, |(panel, _)| panel),
                        PanelFocus::Queue => self
                            .queue_menu_geometry()
                            .map(|(panel, _)| panel)
                            .unwrap_or_default(),
                    };
                    (panel, None)
                }
            }
        };
        let pointer = match &anchor {
            ContextMenuAnchor::Pointer { x, y } => Some((*x, *y)),
            ContextMenuAnchor::SelectedItem(_) => None,
        };
        let (x, y) = ContextMenu::place(
            mbv_render::layout::rect_to_presentation(panel_rect),
            size,
            anchor_rect
                .as_ref()
                .map(|rect| mbv_render::layout::rect_to_presentation(*rect))
                .as_ref(),
            pointer,
        );
        Rect {
            x,
            y,
            width: size.0,
            height: size.1,
        }
    }

    /// Render the `ContextMenu` overlay if mounted. Placement is recomputed from
    /// `AppLayout` each frame (so it follows the fresh layout after a resize),
    /// then passed to the component via downcast (task 5.3c).
    pub(in crate::app) fn render_context_menu_overlay(&mut self, f: &mut ratatui::Frame) {
        let id = Self::context_menu_id();
        if !self.application.mounted(&id) {
            return;
        }
        // Read the menu's anchor/entries/bounds immutably, compute the rect,
        // then borrow mutably only to set it (avoids aliasing `self`).
        let (anchor, entries, bounds) = {
            let comp = self.application.get_component(&id);
            let Some(menu) = comp
                .and_then(|component| component.as_any().downcast_ref::<ContextMenuComponent>())
            else {
                return;
            };
            (menu.anchor(), menu.entries().to_vec(), menu.bounds())
        };
        let rect = self.context_menu_rect(anchor, &entries, bounds);
        if let Some(comp) = self.application.get_component_mut(&id)
            && let Some(menu) = comp.as_any_mut().downcast_mut::<ContextMenuComponent>()
        {
            menu.set_rect(rect);
        }
        self.application.view(&id, f, f.area());
    }

    /// Shell-owned key handling for the Context menu (task 5.3c): the component
    /// owns cursor/selection rendering; the shell owns action dispatch.
    pub(in crate::app) fn handle_context_menu_intent(&mut self, intent: ContextMenuIntent) {
        let id = Self::context_menu_id();
        if !self.application.mounted(&id) {
            return;
        }
        match intent {
            ContextMenuIntent::MoveUp | ContextMenuIntent::MoveDown => {
                if let Some(comp) = self.application.get_component_mut(&id)
                    && let Some(menu) = comp.as_any_mut().downcast_mut::<ContextMenuComponent>()
                {
                    menu.move_cursor(matches!(intent, ContextMenuIntent::MoveDown));
                }
            }
            ContextMenuIntent::Select => {
                let action = self
                    .application
                    .get_component(&id)
                    .and_then(|component| component.as_any().downcast_ref::<ContextMenuComponent>())
                    .and_then(|menu| menu.action_at(menu.cursor()));
                self.dismiss_context_menu();
                let is_bulk = is_bulk_action(action.as_ref());
                self.app
                    .execute_context_action(action, self.home_context_item.clone());
                if is_bulk {
                    let origin = self
                        .context_action_snapshot
                        .take()
                        .map(|snapshot| snapshot.origin)
                        .or_else(|| self.context_menu_origin.take());
                    if let Some(origin) = origin {
                        self.clear_multi_selection_from_origin(origin);
                    }
                }
            }
            ContextMenuIntent::Dismiss => self.dismiss_context_menu(),
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
        let is_bulk = is_bulk_action(action.as_ref());
        self.app
            .execute_context_action(action, self.home_context_item.clone());
        if is_bulk {
            let origin = self
                .context_action_snapshot
                .take()
                .map(|snapshot| snapshot.origin)
                .or_else(|| self.context_menu_origin.take());
            if let Some(origin) = origin {
                self.clear_multi_selection_from_origin(origin);
            }
        }
    }

    pub(crate) fn clear_multi_selection_from_origin(
        &mut self,
        origin: mbv_ui_model::media_list::SelectionOrigin,
    ) {
        self.visual_selection = None;
        match origin {
            mbv_ui_model::media_list::SelectionOrigin::Queue => {
                if let Some(comp) = self.application.get_component_mut(&ComponentId::Queue)
                    && let Some(queue) = comp.as_any_mut().downcast_mut::<QueueComponent>()
                {
                    queue.clear_selection();
                }
            }
            mbv_ui_model::media_list::SelectionOrigin::Library(origin) => {
                if let Some(comp) = self.application.get_component_mut(&ComponentId::Library)
                    && let Some(panel) = comp.as_any_mut().downcast_mut::<LibraryPanel>()
                {
                    panel.clear_selection_for_origin(&origin);
                }
            }
        }
    }

    fn dismiss_context_menu(&mut self) {
        let id = Self::context_menu_id();
        self.dismiss_focus_overlay(&id);
    }

    // --- Settings Multiselect popup ----------------------------------------

    fn multiselect_id() -> ComponentId {
        ComponentId::Popup(PopupId::Multiselect)
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
        self.dismiss_popup(&id);
        self.commit_multiselect(kind, &items);
    }

    fn commit_multiselect(&mut self, kind: MultiSelectKind, items: &[(String, String, bool)]) {
        if let Some(content) = self.app.commit_settings_choices(kind, items) {
            self.assign_home_content(content);
        }
    }

    pub(in crate::app) fn open_multiselect(&mut self, kind: MultiSelectKind) {
        let Some(items) = self.app.settings_choices(kind) else {
            return;
        };
        let id = Self::multiselect_id();
        if !self.application.mounted(&id) {
            self.application
                .mount(id.clone(), Box::new(MultiselectComponent::new()), vec![])
                .expect("mount Multiselect");
        }
        // Focus routes through the core focus stack (design D3): the push
        // takes focus over the Settings sidebar beneath it.
        self.push_focus_overlay(&id);
        if let Some(comp) = self.application.get_component_mut(&id)
            && let Some(multiselect) = comp.as_any_mut().downcast_mut::<MultiselectComponent>()
        {
            multiselect.set_content(&MultiSelectPopup {
                kind,
                items,
                cursor: 0,
            });
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

    pub(in crate::app) fn handle_library_routes_request(&mut self, request: &ShellRequest) {
        match request {
            ShellRequest::LibraryRoutesEnter => self.handle_library_routes_enter(),
            ShellRequest::LibraryRoutesEsc => self.handle_library_routes_esc(),
            // unreachable: shell/messages.rs routes only LibraryRoutesEnter /
            // LibraryRoutesEsc here; both have an arm above.
            _ => {}
        }
    }

    fn with_library_routes<T>(&self, f: impl FnOnce(&LibraryRoutesComponent) -> T) -> Option<T> {
        self.application
            .get_component(&Self::library_routes_id())
            .and_then(|component| component.as_any().downcast_ref::<LibraryRoutesComponent>())
            .map(f)
    }

    fn set_library_routes_content(&mut self, popup: &LibraryRoutePopup) {
        let id = Self::library_routes_id();
        if let Some(comp) = self.application.get_component_mut(&id)
            && let Some(routes) = comp.as_any_mut().downcast_mut::<LibraryRoutesComponent>()
        {
            routes.set_content(popup);
        }
    }

    pub(in crate::app) fn open_library_routes(&mut self) {
        let Some(items) = self.app.route_libraries() else {
            return;
        };
        let id = Self::library_routes_id();
        if !self.application.mounted(&id) {
            self.application
                .mount(id.clone(), Box::new(LibraryRoutesComponent::new()), vec![])
                .expect("mount LibraryRoutes");
        }
        self.push_focus_overlay(&id);
        self.set_library_routes_content(&LibraryRoutePopup {
            stage: LibraryRouteStage::PickLibrary { items },
            cursor: 0,
        });
    }

    pub(in crate::app) fn handle_library_routes_enter(&mut self) {
        let Some(stage) = self.with_library_routes(|c| c.stage().cloned()).flatten() else {
            return;
        };
        match stage {
            LibraryRouteStage::PickLibrary { items } => {
                if let Some((lower, _, _)) = items.get(self.library_routes_cursor()) {
                    let lower = lower.clone();
                    self.enter_device_stage(lower);
                }
            }
            LibraryRouteStage::PickDevice { .. } => {
                self.commit_device_selection();
            }
        }
    }

    fn handle_library_routes_esc(&mut self) {
        let id = Self::library_routes_id();
        let Some(stage) = self.with_library_routes(|c| c.stage().cloned()).flatten() else {
            return;
        };
        match stage {
            LibraryRouteStage::PickLibrary { .. } => self.dismiss_popup(&id),
            LibraryRouteStage::PickDevice { .. } => self.open_library_routes(),
        }
    }

    fn library_routes_cursor(&self) -> usize {
        self.with_library_routes(mbv_components::library_routes::LibraryRoutesComponent::cursor)
            .unwrap_or(0)
    }

    pub(in crate::app) fn enter_device_stage(&mut self, library_lower: String) {
        let Some((devices, cursor)) = self.app.route_devices(&library_lower) else {
            return;
        };
        self.set_library_routes_content(&LibraryRoutePopup {
            stage: LibraryRouteStage::PickDevice {
                library_lower,
                devices,
            },
            cursor,
        });
    }

    pub(in crate::app) fn commit_device_selection(&mut self) {
        let Some(LibraryRouteStage::PickDevice {
            library_lower,
            devices,
            ..
        }) = self.with_library_routes(|c| c.stage().cloned()).flatten()
        else {
            return;
        };
        let cursor = self.library_routes_cursor();
        // Cursor 0 is the automatic entry; a picked row carries its device
        // name and endpoint (a `None` endpoint is refused in
        // `assign_library_route`).
        let device = if cursor > 0 {
            devices.get(cursor - 1)
        } else {
            None
        };
        let Some((items, restored_cursor)) = self.app.assign_library_route(&library_lower, device)
        else {
            return;
        };
        self.set_library_routes_content(&LibraryRoutePopup {
            stage: LibraryRouteStage::PickLibrary { items },
            cursor: restored_cursor,
        });
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

    pub(in crate::app) fn render_feeds_manage_popup(&mut self, f: &mut ratatui::Frame) {
        let id = Self::feeds_manage_id();
        if self.application.mounted(&id) {
            self.application.view(&id, f, f.area());
        }
    }

    fn dismiss_popup(&mut self, id: &ComponentId) {
        self.dismiss_focus_overlay(id);
    }
}
