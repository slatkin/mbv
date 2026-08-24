use super::super::components::{
    ComponentId, ContextMenuComponent, FeedsManageComponent, LibraryRoutesComponent,
    MultiselectComponent, OverlayId, PopupId, SelectionModalComponent, ShellRequest,
};
use super::super::shell::Model;

impl Model {
    // --- Context menu -------------------------------------------------------

    fn context_menu_id() -> ComponentId {
        ComponentId::Overlay(OverlayId::ContextMenu)
    }

    /// Sync the ContextMenu component mount state with `App::context_menu`.
    pub(in crate::app) fn sync_context_menu(&mut self) {
        let id = Self::context_menu_id();
        let mounted = self.application.mounted(&id);
        if self.app.context_menu.is_some() && !mounted {
            self.dismiss_sidebars();
            self.application
                .mount(id.clone(), Box::new(ContextMenuComponent::new()), vec![])
                .expect("mount ContextMenu");
            self.application.active(&id).expect("activate ContextMenu");
        } else if self.app.context_menu.is_none() && mounted {
            let _ = self.application.umount(&id);
        }
    }

    /// Render the ContextMenu overlay if mounted. The placement rect is
    /// computed by `App::render_context_menu` (called from `App::render`),
    /// which writes `layout.context_menu_rect`; the shell reads that and
    /// passes it to the component via downcast.
    pub(in crate::app) fn render_context_menu_overlay(&mut self, f: &mut ratatui::Frame) {
        let id = Self::context_menu_id();
        if !self.application.mounted(&id) {
            return;
        }
        if let Some(comp) = self.application.get_component_mut(&id) {
            if let Some(menu) = comp.as_any_mut().downcast_mut::<ContextMenuComponent>() {
                if let Some(ref app_menu) = self.app.context_menu {
                    let rect = self.app.layout.context_menu_rect.unwrap_or_default();
                    menu.set_content(&app_menu.entries, app_menu.cursor, rect);
                }
            }
        }
        self.application.view(&id, f, f.area());
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
