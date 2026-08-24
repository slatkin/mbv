use super::super::components::{
    ComponentId, HelpComponent, OverlayId, SearchSidebarComponent, SessionsComponent,
};
use super::super::shell::Model;
use super::super::SidebarId;

impl Model {
    // --- Help sidebar -------------------------------------------------------

    /// Mount the Help overlay and make it the active component. Closes the
    /// non-blocking overlays (settings/sessions/playlists) first, matching the
    /// legacy F1 arms in each of their handlers.
    pub(in crate::app) fn mount_help(&mut self) {
        self.app.close_sidebar(SidebarId::Settings);
        self.app.close_sidebar(SidebarId::Sessions);
        self.app.close_sidebar(SidebarId::Playlists);
        self.application
            .mount(
                ComponentId::Overlay(OverlayId::Help),
                Box::new(HelpComponent::new()),
                vec![],
            )
            .expect("mount Help");
        self.application
            .active(&ComponentId::Overlay(OverlayId::Help))
            .expect("activate Help");
    }

    /// Unmount the Help overlay; TuiRealm's LIFO focus stack auto-restores
    /// focus to `LegacyInput`.
    pub(in crate::app) fn umount_help(&mut self) {
        let _ = self
            .application
            .umount(&ComponentId::Overlay(OverlayId::Help));
    }

    /// Render the Help overlay if mounted, after the legacy `App::render`.
    /// Sets the component's destination and panel area via downcast so its
    /// `view()` paints over the legacy frame (design D5/D9).
    pub(in crate::app) fn render_help_overlay(&mut self, f: &mut ratatui::Frame) {
        let help_id = ComponentId::Overlay(OverlayId::Help);
        if !self.application.mounted(&help_id) {
            return;
        }
        if let Some(comp) = self.application.get_component_mut(&help_id) {
            if let Some(help) = comp.as_any_mut().downcast_mut::<HelpComponent>() {
                let panel_area = (self.app.layout.main.panel_area.width > 0)
                    .then_some(self.app.layout.main.panel_area);
                help.set_panel_area(panel_area);
                help.set_destination(self.app.effective_panel_focus(), self.app.tab);
            }
        }
        self.application.view(&help_id, f, f.area());
    }
    // --- Search sidebar -----------------------------------------------------
    //
    // The Search sidebar is a non-blocking overlay mounted when
    // `App::open_sidebar(Search)` transition. The component owns the
    // sidebar state (query, cursor, scroll, type_filter, loading, results)
    // and the 300 ms debounce (driven by `UserEvent::Clock`); the shell owns
    // the Emby client and spawns the search thread (design D4/D5).

    fn search_id() -> ComponentId {
        ComponentId::Overlay(OverlayId::Search)
    }

    /// Sync the Search component mount state with the Search sidebar state.
    pub(in crate::app) fn sync_search_sidebar(&mut self) {
        let id = Self::search_id();
        let mounted = self.application.mounted(&id);
        if self.app.is_sidebar_open(SidebarId::Search) && !mounted {
            self.application
                .mount(id.clone(), Box::new(SearchSidebarComponent::new()), vec![])
                .expect("mount Search");
            self.application.active(&id).expect("activate Search");
        } else if !self.app.is_sidebar_open(SidebarId::Search) && mounted {
            let _ = self.application.umount(&id);
        }
    }

    /// Render the Search overlay if mounted. Sets the panel area via
    /// downcast so the component's `view()` paints over the legacy frame.
    pub(in crate::app) fn render_search_overlay(&mut self, f: &mut ratatui::Frame) {
        let id = Self::search_id();
        if !self.application.mounted(&id) {
            return;
        }
        if let Some(comp) = self.application.get_component_mut(&id) {
            if let Some(search) = comp.as_any_mut().downcast_mut::<SearchSidebarComponent>() {
                let panel_area = (self.app.layout.main.panel_area.width > 0)
                    .then_some(self.app.layout.main.panel_area);
                search.set_panel_area(panel_area);
            }
        }
        self.application.view(&id, f, f.area());
    }

    /// Drain search results from `search_rx` into the `SearchSidebarComponent`
    /// via downcast. The shell owns the channel; the component owns the state.
    pub(in crate::app) fn drain_search_results(&mut self) -> bool {
        let id = Self::search_id();
        let mut received = 0;
        while let Ok((query, result)) = self.app.search_rx.try_recv() {
            received += 1;
            if let Some(comp) = self.application.get_component_mut(&id) {
                if let Some(search) = comp.as_any_mut().downcast_mut::<SearchSidebarComponent>() {
                    search.apply_drain(&query, result);
                }
            }
        }
        received > 0
    }

    // --- Sessions sidebar ---------------------------------------------------

    fn sessions_id() -> ComponentId {
        ComponentId::Overlay(OverlayId::Sessions)
    }

    /// Mount or unmount the Sessions sidebar from the shell-owned presence
    /// flag. The component owns its cursor and scroll once mounted.
    pub(in crate::app) fn sync_sessions(&mut self) {
        let id = Self::sessions_id();
        let mounted = self.application.mounted(&id);
        if self.app.is_sidebar_open(SidebarId::Sessions) && !mounted {
            self.application
                .mount(id.clone(), Box::new(SessionsComponent::new()), vec![])
                .expect("mount Sessions");
            self.application.active(&id).expect("activate Sessions");
        } else if !self.app.is_sidebar_open(SidebarId::Sessions) && mounted {
            let _ = self.application.umount(&id);
        }
    }

    /// Render Sessions from shell-owned runtime data. Cursor, scroll, and hit
    /// geometry remain private to the mounted component.
    pub(in crate::app) fn render_sessions_overlay(&mut self, f: &mut ratatui::Frame) {
        let id = Self::sessions_id();
        if !self.application.mounted(&id) {
            return;
        }
        let panel_area =
            (self.app.layout.main.panel_area.width > 0).then_some(self.app.layout.main.panel_area);
        let connected_session_id = self.app.connected_session_id.as_deref();
        let tracking = self.app.remote_tracker.is_some();
        let cast_attachment_id = self
            .app
            .cast_attachment
            .as_ref()
            .map(|attachment| attachment.receiver_id.as_str());
        if let Some(comp) = self.application.get_component_mut(&id) {
            if let Some(sessions) = comp.as_any_mut().downcast_mut::<SessionsComponent>() {
                sessions.set_content(
                    &self.app.panel_targets,
                    self.app.sessions_loading,
                    connected_session_id,
                    tracking,
                    cast_attachment_id,
                    self.app.can_disconnect_remote(),
                    panel_area,
                );
            }
        }
        self.application.view(&id, f, f.area());
    }
}
