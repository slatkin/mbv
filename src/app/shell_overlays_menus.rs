use super::super::components::audiobookshelf_book::AudiobookshelfBookComponent;
use super::super::components::msg::ContextMenuIntent;
use super::super::components::{
    ComponentId, ContextMenuComponent, HomeComponent, LibraryRoutesComponent, MultiselectComponent,
    OverlayId, PopupId, SelectionModalComponent, ShellRequest,
};
use super::super::shell::Model;
use crate::app::types_context_menu::{
    ContextMenu, ContextMenuAnchor, ContextMenuEntry, LibraryRoutePopup, LibraryRouteStage,
    MultiSelectKind, MultiSelectPopup,
};
use crate::app::{PanelFocus, TabSelection};
use ratatui::layout::Rect;

impl Model {
    // --- Context menu -------------------------------------------------------

    fn context_menu_id() -> ComponentId {
        ComponentId::Overlay(OverlayId::ContextMenu)
    }

    /// The mounted `HomeComponent`'s painted panel rect (list area) and
    /// selected-row rect, when Home is the active destination with the Library
    /// panel focused. Returns `None` otherwise, so the caller falls back to the
    /// legacy `AppLayout` geometry. The component gains this geometry from its
    /// own `view()` paint (`render_home_content`), so the menu placement tracks
    /// the component's real paint rather than any copied-back legacy geometry.
    fn home_menu_geometry(&self) -> Option<(Rect, Option<Rect>)> {
        if !matches!(self.app.tab, TabSelection::Home)
            || !matches!(self.app.effective_panel_focus(), PanelFocus::Library)
        {
            return None;
        }
        self.application
            .get_component(&ComponentId::Home)
            .and_then(|c| c.as_any().downcast_ref::<HomeComponent>())
            .map(HomeComponent::menu_placement_geometry)
    }

    /// Like `home_menu_geometry`, but for the mounted `AudiobookshelfBookComponent`
    /// (task 5.3d.13, render ownership). Returns the book surface's painted
    /// selected-item rect so the context menu anchors to what the component
    /// actually painted rather than the legacy `AppLayout` copy.
    fn book_menu_geometry(&self) -> Option<(Rect, Option<Rect>)> {
        let id = self.abs_book_id.clone()?;
        if !matches!(self.app.effective_panel_focus(), PanelFocus::Library) {
            return None;
        }
        self.application
            .get_component(&id)
            .and_then(|component| {
                component
                    .as_any()
                    .downcast_ref::<AudiobookshelfBookComponent>()
            })
            .map(|component| {
                (
                    self.app.layout.main.left_area,
                    component.geometry().selected_item_rect,
                )
            })
    }

    /// Compute the context menu's painted rect from the current anchor/entries
    /// and the owning surface's geometry. Replaces the old `layout.context_menu_rect`
    /// global written during `App::render` (task 5.3c); the component now owns
    /// its rect and hit test.
    fn context_menu_rect(&self, anchor: ContextMenuAnchor, entries: &[ContextMenuEntry]) -> Rect {
        let layout = &self.app.layout;
        let size = ContextMenu::rendered_size(entries);
        // When Home is the active destination with the Library panel focused,
        // the panel/selection placement geometry comes from the mounted
        // `HomeComponent`'s own painted geometry (its `list_area` claim rect
        // and `selected_item_rect`), not the legacy `AppLayout` copies, so the
        // menu tracks what the component actually painted (task 5.3d, Home
        // menu-placement geometry). Other destinations and Queue focus keep
        // using `AppLayout` as before.
        let home = self.home_menu_geometry();
        let (panel_rect, anchor_rect): (Rect, Option<Rect>) = match &anchor {
            ContextMenuAnchor::SelectedItem(focus) => {
                let (panel, selected) = match focus {
                    PanelFocus::Library => match home {
                        Some((panel, selected)) => (panel, selected),
                        None => match self.book_menu_geometry() {
                            Some((panel, selected)) => (panel, selected),
                            None => (layout.main.left_area, layout.main.selected_item_rect),
                        },
                    },
                    PanelFocus::Queue => {
                        (layout.main.queue_area, layout.main.queue_selected_item_rect)
                    }
                };
                (panel, selected)
            }
            ContextMenuAnchor::Pointer { .. } => {
                let panel = match self.app.effective_panel_focus() {
                    // Home is not a wide-TV destination; when it is active the
                    // pointer panel is the component's own list-area claim rect.
                    PanelFocus::Library if home.is_some() => home.unwrap().0,
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
    pub(in crate::app) fn handle_context_menu_intent(&mut self, intent: ContextMenuIntent) {
        let id = Self::context_menu_id();
        if !self.application.mounted(&id) {
            return;
        }
        match intent {
            ContextMenuIntent::MoveUp | ContextMenuIntent::MoveDown => {
                if let Some(comp) = self.application.get_component_mut(&id) {
                    if let Some(menu) = comp.as_any_mut().downcast_mut::<ContextMenuComponent>() {
                        menu.move_cursor(matches!(intent, ContextMenuIntent::MoveDown));
                    }
                }
            }
            ContextMenuIntent::Select => {
                let action = self
                    .application
                    .get_component(&id)
                    .and_then(|component| component.as_any().downcast_ref::<ContextMenuComponent>())
                    .and_then(|menu| menu.action_at(menu.cursor()));
                self.dismiss_context_menu();
                self.app.execute_context_action(action, self.home_cw_item());
            }
            ContextMenuIntent::Dismiss => self.dismiss_context_menu(),
        }
    }

    pub(in crate::app) fn handle_context_menu_key(&mut self, key: crossterm::event::KeyEvent) {
        let intent = match key.code {
            crossterm::event::KeyCode::Up => ContextMenuIntent::MoveUp,
            crossterm::event::KeyCode::Down => ContextMenuIntent::MoveDown,
            crossterm::event::KeyCode::Enter => ContextMenuIntent::Select,
            crossterm::event::KeyCode::Esc => ContextMenuIntent::Dismiss,
            _ => return,
        };
        self.handle_context_menu_intent(intent);
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
        self.app.execute_context_action(action, self.home_cw_item());
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
                    } => self.select_podcast_selection_modal_filter(library_item_id, selected),
                    // SelectionModalSource is closed (Series/Album/Podcast/Book);
                    // only Series and Podcast modals carry a filter, so Album and
                    // Book have no filter-selection effect.
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
            // unreachable: callers pass only DismissSelectionModal,
            // SelectionModalFilterSelected, SelectionModalRefresh, or
            // SelectionModalActivate (shell_messages.rs OR-group +
            // shell_overlays_modals.rs); every one has an arm above.
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
        self.commit_multiselect(kind, items);
    }

    fn commit_multiselect(&mut self, kind: MultiSelectKind, items: Vec<(String, String, bool)>) {
        if matches!(kind, MultiSelectKind::MyLanguages) {
            let selected: Vec<String> = items
                .iter()
                .filter(|(_, _, is_sel)| *is_sel)
                .map(|(_, name, _)| name.clone())
                .collect();
            {
                let mut c = self.app.config.lock().unwrap();
                if !selected.is_empty() {
                    if !c.subtitle_lang.is_empty() && !selected.contains(&c.subtitle_lang) {
                        c.subtitle_lang = String::new();
                    }
                    if !c.audio_lang.is_empty() && !selected.contains(&c.audio_lang) {
                        c.audio_lang = String::new();
                    }
                }
                c.my_languages = selected;
            }
            let cfg = self.app.config.lock().unwrap().clone();
            {
                let mut p = self.app.player.subtitle_prefs.lock().unwrap();
                p.subtitle_lang = cfg.subtitle_lang.clone();
                p.audio_lang = cfg.audio_lang.clone();
            }
            if let Err(e) = crate::config::save_config_settings(&cfg) {
                log::warn!(target: "config", "config save failed: {e}");
            }
            return;
        }

        let hidden: Vec<String> = items
            .iter()
            .filter(|(_, _, is_hidden)| *is_hidden)
            .map(|(lower, _, _)| lower.clone())
            .collect();
        {
            let mut c = self.app.config.lock().unwrap();
            match kind {
                MultiSelectKind::HiddenLibraries => c.hidden_libraries = hidden.clone(),
                MultiSelectKind::HiddenLatest => c.hidden_latest = hidden.clone(),
                MultiSelectKind::FeedViewLibraries => c.feed_view_libraries = hidden.clone(),
                MultiSelectKind::MyLanguages => unreachable!(),
            }
        }
        match kind {
            MultiSelectKind::HiddenLibraries => self.app.hidden_libraries = hidden,
            MultiSelectKind::HiddenLatest => self.app.hidden_latest = hidden,
            MultiSelectKind::FeedViewLibraries => {
                for lib in &mut self.app.libs {
                    lib.nav_stack.clear();
                }
            }
            MultiSelectKind::MyLanguages => unreachable!(),
        }
        let cfg = self.app.config.lock().unwrap().clone();
        if let Err(e) = crate::config::save_config_settings(&cfg) {
            log::warn!(target: "config", "config save failed: {e}");
        }
        if let Ok(content) = self.app.fetch_home() {
            // The commit runs the fetch synchronously (order-sensitive
            // side effects); the computed content is assigned to
            // Model-owned `home_content` directly — a shell-side caller
            // (task 5.3d).
            self.assign_home_content(content);
        }
    }

    pub(in crate::app) fn open_multiselect(&mut self, kind: MultiSelectKind) {
        let items: Vec<(String, String, bool)> = if matches!(kind, MultiSelectKind::MyLanguages) {
            const ALL_LANGS: &[&str] = &[
                "English",
                "French",
                "German",
                "Spanish",
                "Italian",
                "Portuguese",
                "Japanese",
                "Korean",
                "Chinese",
                "Russian",
                "Arabic",
                "Dutch",
                "Swedish",
                "Norwegian",
                "Danish",
                "Finnish",
                "Polish",
                "Czech",
                "Turkish",
            ];
            let my_langs = self.app.config.lock().unwrap().my_languages.clone();
            ALL_LANGS
                .iter()
                .map(|&name| {
                    let selected = my_langs.iter().any(|l| l == name);
                    (name.to_lowercase(), name.to_string(), selected)
                })
                .collect()
        } else {
            let Some(client) = self.app.emby_client() else {
                return;
            };
            let client = client.lock().unwrap();
            let all = match kind {
                MultiSelectKind::HiddenLibraries => client.get_views().unwrap_or_default(),
                MultiSelectKind::HiddenLatest => client.get_user_views().unwrap_or_default(),
                MultiSelectKind::FeedViewLibraries => client.get_views().unwrap_or_default(),
                MultiSelectKind::MyLanguages => unreachable!(),
            };
            let config = self.app.config.lock().unwrap();
            let selected_list = match kind {
                MultiSelectKind::HiddenLibraries => &config.hidden_libraries,
                MultiSelectKind::HiddenLatest => &config.hidden_latest,
                MultiSelectKind::FeedViewLibraries => &config.feed_view_libraries,
                MultiSelectKind::MyLanguages => unreachable!(),
            };
            all.iter()
                .filter(|v| v.collection_type != "playlists")
                .map(|v| {
                    let lower = v.name.to_lowercase();
                    let is_hidden = selected_list.contains(&lower);
                    (lower, v.name.clone(), is_hidden)
                })
                .collect()
        };
        let id = Self::multiselect_id();
        if !self.application.mounted(&id) {
            self.application
                .mount(id.clone(), Box::new(MultiselectComponent::new()), vec![])
                .expect("mount Multiselect");
            self.application.active(&id).expect("activate Multiselect");
        }
        if let Some(comp) = self.application.get_component_mut(&id) {
            if let Some(multiselect) = comp.as_any_mut().downcast_mut::<MultiselectComponent>() {
                multiselect.set_content(&MultiSelectPopup {
                    kind,
                    items,
                    cursor: 0,
                });
            }
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

    pub(in crate::app) fn handle_library_routes_request(&mut self, request: ShellRequest) {
        match request {
            ShellRequest::LibraryRoutesEnter => self.handle_library_routes_enter(),
            ShellRequest::LibraryRoutesEsc => self.handle_library_routes_esc(),
            // unreachable: shell_messages.rs routes only LibraryRoutesEnter /
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
        if let Some(comp) = self.application.get_component_mut(&id) {
            if let Some(routes) = comp.as_any_mut().downcast_mut::<LibraryRoutesComponent>() {
                routes.set_content(popup);
            }
        }
    }

    pub(in crate::app) fn open_library_routes(&mut self) {
        log::info!(target: "library_route", "F2 route picker opened");
        let Some(client) = self.app.emby_client() else {
            return;
        };
        let client = client.lock().unwrap();
        let all = match client.get_views() {
            Ok(all) => {
                log::info!(target: "library_route", "F2 library fetch succeeded count={}", all.len());
                all
            }
            Err(e) => {
                log::warn!(target: "library_route", "F2 library fetch failed: {e}");
                drop(client);
                self.app.flash(
                    format!("⚠ Library routes couldn't load libraries ({e})"),
                    crate::app::notify_actions::ToastSeverity::Error,
                );
                return;
            }
        };
        let routes = self.app.config.lock().unwrap().library_routes.clone();
        let items: Vec<(String, String, Option<String>)> = all
            .iter()
            .filter(|v| v.collection_type != "playlists")
            .map(|v| {
                let lower = v.name.to_lowercase();
                let assigned = routes.get(&lower).cloned();
                (lower, v.name.clone(), assigned)
            })
            .collect();
        drop(client);
        let id = Self::library_routes_id();
        if !self.application.mounted(&id) {
            self.application
                .mount(id.clone(), Box::new(LibraryRoutesComponent::new()), vec![])
                .expect("mount LibraryRoutes");
            self.application
                .active(&id)
                .expect("activate LibraryRoutes");
        }
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
                if let Some((lower, display, _)) = items.get(self.library_routes_cursor()) {
                    let lower = lower.clone();
                    let display = display.clone();
                    self.enter_device_stage(lower, display);
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
        self.with_library_routes(|c| c.cursor())
            .unwrap_or_else(|| 0)
    }

    pub(in crate::app) fn enter_device_stage(
        &mut self,
        library_lower: String,
        library_display: String,
    ) {
        let sessions = match self.app.fetch_sessions_blocking() {
            Ok(sessions) => {
                log::info!(target: "library_route", "F2 session fetch succeeded count={}", sessions.len());
                sessions
            }
            Err(e) => {
                log::warn!(target: "library_route", "F2 session fetch failed library={library_lower:?}: {e}");
                self.app.flash(
                    format!("⚠ Library routes couldn't load devices ({e})"),
                    crate::app::notify_actions::ToastSeverity::Error,
                );
                return;
            }
        };
        let Some(client) = self.app.emby_client() else {
            return;
        };
        let local_device_name = client.lock().unwrap().device_name.clone();
        let mut devices: Vec<(String, Option<mbv_core::remote_player::DaemonEndpoint>)> = sessions
            .iter()
            .filter(|s| s.client.eq_ignore_ascii_case("mbv"))
            .filter(|s| !s.device_name.eq_ignore_ascii_case(&local_device_name))
            .map(|s| {
                let endpoint = self.app.session_direct_endpoint(s);
                if let Some(endpoint) = &endpoint {
                    log::info!(target: "library_route", "F2 endpoint eligible device={:?} endpoint={endpoint}", s.device_name);
                } else {
                    log::info!(target: "library_route", "F2 endpoint rejected device={:?} reason=no resolvable direct-connect endpoint", s.device_name);
                }
                (s.device_name.clone(), endpoint)
            })
            .collect();
        devices.sort_by(|a, b| a.0.cmp(&b.0));
        devices.dedup_by(|a, b| a.0.eq_ignore_ascii_case(&b.0));
        log::info!(target: "library_route", "F2 candidate count={} library={library_lower:?}", devices.len());

        let current_endpoint = self
            .app
            .config
            .lock()
            .unwrap()
            .library_routes
            .get(&library_lower)
            .and_then(|raw| mbv_core::remote_player::DaemonEndpoint::parse(raw).ok());
        let cursor = current_endpoint
            .and_then(|current| {
                devices
                    .iter()
                    .position(|(_, ep)| ep.as_ref() == Some(&current))
            })
            .map(|idx| idx + 1)
            .unwrap_or(0);

        self.set_library_routes_content(&LibraryRoutePopup {
            stage: LibraryRouteStage::PickDevice {
                library_lower,
                library_display,
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

        if cursor > 0 {
            if let Some((name, None)) = devices.get(cursor - 1) {
                self.app.flash(
                    format!(
                        "{name} is not currently routable (no resolvable direct-connect endpoint)"
                    ),
                    crate::app::notify_actions::ToastSeverity::Neutral,
                );
                return;
            }
        }

        {
            let mut c = self.app.config.lock().unwrap();
            if cursor == 0 {
                c.library_routes.remove(&library_lower);
                log::info!(target: "library_route", "F2 route removed library={library_lower:?}");
            } else if let Some((_, Some(endpoint))) = devices.get(cursor - 1) {
                c.library_routes
                    .insert(library_lower.clone(), endpoint.to_string());
                log::info!(target: "library_route", "F2 endpoint persisted library={library_lower:?} endpoint={endpoint}");
            }
        }
        let cfg = self.app.config.lock().unwrap().clone();
        self.app.library_routes = cfg.library_routes.clone();
        log::info!(target: "library_route", "runtime route table synchronized count={}", self.app.library_routes.len());
        let save_result = crate::app::render::save_route_config(&cfg);
        if !self.finish_route_config_save(save_result) {
            return;
        }
        self.app.persist_roaming_settings();

        let Some(client) = self.app.emby_client() else {
            return;
        };
        let refresh_result = { client.lock().unwrap().get_views() };
        let all = match refresh_result {
            Ok(all) => all,
            Err(e) => {
                log::warn!(target: "library_route", "F2 post-save library refresh failed: {e}");
                self.app.flash(
                    format!("⚠ Library route saved but couldn't refresh libraries ({e})"),
                    crate::app::notify_actions::ToastSeverity::Error,
                );
                return;
            }
        };
        let routes = cfg.library_routes.clone();
        let items: Vec<(String, String, Option<String>)> = all
            .iter()
            .filter(|v| v.collection_type != "playlists")
            .map(|v| {
                let lower = v.name.to_lowercase();
                let assigned = routes.get(&lower).cloned();
                (lower, v.name.clone(), assigned)
            })
            .collect();
        let restored_cursor = items
            .iter()
            .position(|(lower, _, _)| *lower == library_lower)
            .unwrap_or(0);
        self.set_library_routes_content(&LibraryRoutePopup {
            stage: LibraryRouteStage::PickLibrary { items },
            cursor: restored_cursor,
        });
    }

    pub(in crate::app) fn finish_route_config_save(&mut self, result: Result<(), String>) -> bool {
        match result {
            Ok(()) => {
                log::info!(target: "library_route", "config save succeeded");
                true
            }
            Err(e) => {
                log::warn!(target: "library_route", "config save failed: {e}");
                self.app.flash(
                    format!("⚠ Library route changed but config save failed ({e})"),
                    crate::app::notify_actions::ToastSeverity::Error,
                );
                false
            }
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

    pub(in crate::app) fn handle_feeds_manage_request(&mut self, key: crossterm::event::KeyEvent) {
        self.handle_feeds_manage_key(key);
    }

    pub(in crate::app) fn render_feeds_manage_popup(&mut self, f: &mut ratatui::Frame) {
        let id = Self::feeds_manage_id();
        if self.application.mounted(&id) {
            self.application.view(&id, f, f.area());
        }
    }

    fn dismiss_popup(&mut self, id: &ComponentId) {
        if self.application.mounted(id) {
            let _ = self.application.umount(id);
        }
    }
}
