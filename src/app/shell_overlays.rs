//! Overlay sync/render methods for the shell `Model` (design D2/D9).
//!
//! Extracted from `shell.rs` to keep it under the 800-line cap. Each
//! converted surface has a `sync_*` (mount/unmount on App field
//! transitions), a `render_*_overlay` (set content via downcast then
//! `application.view()`), and the shell's run-loop Msg handlers call the
//! existing `App` handlers for cross-boundary work.

use super::components::{
    ComponentId, ConfirmComponent, ContextMenuComponent, DaemonLostComponent, FeedsManageComponent,
    HelpComponent, LibraryRoutesComponent, ModalId, MultiselectComponent, OverlayId,
    PlaybackPromptComponent, PopupId, RemoteReanchorComponent, SearchSidebarComponent,
    SelectionModalComponent, SessionsComponent, ShellRequest,
};
use super::shell::Model;

impl Model {
    // --- Playback prompt ----------------------------------------------------

    /// Sync the status-bar playback prompt with the shell-owned prompt state.
    pub(super) fn sync_playback_prompt(&mut self) {
        let id = ComponentId::PlaybackPrompt;
        let mounted = self.application.mounted(&id);
        let prompt_open =
            self.app.skip_intro_end_ticks.is_some() || self.app.next_up_item.is_some();
        if prompt_open && !mounted {
            self.application
                .mount(id.clone(), Box::new(PlaybackPromptComponent::new()), vec![])
                .expect("mount PlaybackPrompt");
            self.application
                .active(&id)
                .expect("activate PlaybackPrompt");
        } else if !prompt_open && mounted {
            let _ = self.application.umount(&id);
        }
        let visible = !self.app.status.is_empty()
            && (!self.app.system_notifications || self.app.notif_failed);
        let area = self.app.layout.playback.status_area;
        if let Some(comp) = self.application.get_component_mut(&id) {
            if let Some(prompt) = comp.as_any_mut().downcast_mut::<PlaybackPromptComponent>() {
                prompt.set_content(&self.app.status, visible, area);
            }
        }
    }

    /// Render the prompt after the legacy frame has established the status-bar
    /// geometry, before any blocking overlay paints its dimmed backdrop.
    pub(super) fn render_playback_prompt(&mut self, f: &mut ratatui::Frame) {
        let id = ComponentId::PlaybackPrompt;
        if !self.application.mounted(&id) {
            return;
        }
        let visible = !self.app.status.is_empty()
            && (!self.app.system_notifications || self.app.notif_failed);
        let area = self.app.layout.playback.status_area;
        if let Some(comp) = self.application.get_component_mut(&id) {
            if let Some(prompt) = comp.as_any_mut().downcast_mut::<PlaybackPromptComponent>() {
                prompt.set_content(&self.app.status, visible, area);
            }
        }
        self.application.view(&id, f, f.area());
    }

    /// True when a blocking overlay (context menu, selection modal,
    /// daemon-lost, confirm, remote-reanchor, save-playlist) is mounted —
    /// those swallow every key, including F1. Excludes Settings-child popups
    /// (multiselect/library-routes), which the `mount_help` path closes by
    /// closing settings.
    pub(super) fn is_blocking_overlay_open(&self) -> bool {
        self.application
            .mounted(&ComponentId::Overlay(OverlayId::ContextMenu))
            || self.app.selection_modal.is_some()
            || self
                .application
                .mounted(&ComponentId::Modal(ModalId::DaemonLost))
            || self
                .application
                .mounted(&ComponentId::Modal(ModalId::Confirm))
            || self
                .application
                .mounted(&ComponentId::Modal(ModalId::RemoteReanchor))
            || self.app.save_playlist_dialog.is_some()
    }

    // --- Help sidebar -------------------------------------------------------

    /// Mount the Help overlay and make it the active component. Closes the
    /// non-blocking overlays (settings/sessions/playlists) first, matching the
    /// legacy F1 arms in each of their handlers.
    pub(super) fn mount_help(&mut self) {
        self.app.show_settings = false;
        self.app.show_sessions = false;
        self.app.show_playlists = false;
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
    pub(super) fn umount_help(&mut self) {
        let _ = self
            .application
            .umount(&ComponentId::Overlay(OverlayId::Help));
    }

    /// Render the Help overlay if mounted, after the legacy `App::render`.
    /// Sets the component's destination and panel area via downcast so its
    /// `view()` paints over the legacy frame (design D5/D9).
    pub(super) fn render_help_overlay(&mut self, f: &mut ratatui::Frame) {
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

    // --- Confirm modal ------------------------------------------------------
    //
    // The Confirm modal is a blocking overlay mounted when `App::confirm_modal`
    // transitions from `None` to `Some`. The component owns rendering and
    // forwards keys to the shell's existing `handle_key_confirm_modal`; the
    // shell owns `ConfirmAction` dispatch (design D4/D9).

    fn confirm_id() -> ComponentId {
        ComponentId::Modal(ModalId::Confirm)
    }

    /// Sync the Confirm component mount state with `App::confirm_modal`.
    pub(super) fn sync_confirm_modal(&mut self) {
        let id = Self::confirm_id();
        let mounted = self.application.mounted(&id);
        if self.app.confirm_modal.is_some() && !mounted {
            self.application
                .mount(id.clone(), Box::new(ConfirmComponent::new()), vec![])
                .expect("mount Confirm");
            self.application.active(&id).expect("activate Confirm");
        } else if self.app.confirm_modal.is_none() && mounted {
            let _ = self.application.umount(&id);
        }
    }

    /// Render the Confirm overlay if mounted.
    pub(super) fn render_confirm_overlay(&mut self, f: &mut ratatui::Frame) {
        let id = Self::confirm_id();
        if !self.application.mounted(&id) {
            return;
        }
        if let Some(comp) = self.application.get_component_mut(&id) {
            if let Some(confirm) = comp.as_any_mut().downcast_mut::<ConfirmComponent>() {
                if let Some(ref modal) = self.app.confirm_modal {
                    confirm.set_content(&modal.title, &modal.message, &modal.hint);
                }
            }
        }
        self.application.view(&id, f, f.area());
    }

    // --- Daemon-lost modal --------------------------------------------------

    fn daemon_lost_id() -> ComponentId {
        ComponentId::Modal(ModalId::DaemonLost)
    }

    /// Sync the DaemonLost component mount state with `App::daemon_lost_modal`.
    pub(super) fn sync_daemon_lost_modal(&mut self) {
        let id = Self::daemon_lost_id();
        let mounted = self.application.mounted(&id);
        if self.app.daemon_lost_modal.is_some() && !mounted {
            self.application
                .mount(id.clone(), Box::new(DaemonLostComponent::new()), vec![])
                .expect("mount DaemonLost");
            self.application.active(&id).expect("activate DaemonLost");
        } else if self.app.daemon_lost_modal.is_none() && mounted {
            let _ = self.application.umount(&id);
        }
    }

    /// Render the DaemonLost overlay if mounted.
    pub(super) fn render_daemon_lost_overlay(&mut self, f: &mut ratatui::Frame) {
        let id = Self::daemon_lost_id();
        if !self.application.mounted(&id) {
            return;
        }
        if let Some(comp) = self.application.get_component_mut(&id) {
            if let Some(daemon_lost) = comp.as_any_mut().downcast_mut::<DaemonLostComponent>() {
                if let Some(ref modal) = self.app.daemon_lost_modal {
                    daemon_lost.set_content(
                        modal.last_playing_title.as_deref(),
                        &modal.daemon_log_path,
                        modal.restart_error.as_deref(),
                    );
                }
            }
        }
        self.application.view(&id, f, f.area());
    }

    // --- Remote-reanchor popup ----------------------------------------------

    fn remote_reanchor_id() -> ComponentId {
        ComponentId::Modal(ModalId::RemoteReanchor)
    }

    /// Sync the RemoteReanchor component mount state with
    /// `App::remote_reanchor_popup`.
    pub(super) fn sync_remote_reanchor_popup(&mut self) {
        let id = Self::remote_reanchor_id();
        let mounted = self.application.mounted(&id);
        if self.app.remote_reanchor_popup.is_some() && !mounted {
            self.application
                .mount(id.clone(), Box::new(RemoteReanchorComponent::new()), vec![])
                .expect("mount RemoteReanchor");
            self.application
                .active(&id)
                .expect("activate RemoteReanchor");
        } else if self.app.remote_reanchor_popup.is_none() && mounted {
            let _ = self.application.umount(&id);
        }
    }

    /// Render the RemoteReanchor overlay if mounted.
    pub(super) fn render_remote_reanchor_overlay(&mut self, f: &mut ratatui::Frame) {
        let id = Self::remote_reanchor_id();
        if !self.application.mounted(&id) {
            return;
        }
        if let Some(comp) = self.application.get_component_mut(&id) {
            if let Some(reanchor) = comp.as_any_mut().downcast_mut::<RemoteReanchorComponent>() {
                if let Some(ref popup) = self.app.remote_reanchor_popup {
                    reanchor.set_content(&popup.targets, popup.cursor);
                }
            }
        }
        self.application.view(&id, f, f.area());
    }

    // --- Context menu -------------------------------------------------------

    fn context_menu_id() -> ComponentId {
        ComponentId::Overlay(OverlayId::ContextMenu)
    }

    /// Sync the ContextMenu component mount state with `App::context_menu`.
    pub(super) fn sync_context_menu(&mut self) {
        let id = Self::context_menu_id();
        let mounted = self.application.mounted(&id);
        if self.app.context_menu.is_some() && !mounted {
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
    pub(super) fn render_context_menu_overlay(&mut self, f: &mut ratatui::Frame) {
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

    /// Sync the Selection modal mount state with the legacy App snapshot.
    pub(super) fn sync_selection_modal(&mut self) {
        let id = Self::selection_modal_id();
        let mounted = self.application.mounted(&id);
        if self.app.selection_modal.is_some() && !mounted {
            self.application
                .mount(id.clone(), Box::new(SelectionModalComponent::new()), vec![])
                .expect("mount SelectionModal");
            self.application
                .active(&id)
                .expect("activate SelectionModal");
        } else if self.app.selection_modal.is_none() && mounted {
            let _ = self.application.umount(&id);
        }
        if let Some(modal) = self.app.selection_modal.as_ref() {
            if let Some(comp) = self.application.get_component_mut(&id) {
                if let Some(selection) = comp.as_any_mut().downcast_mut::<SelectionModalComponent>()
                {
                    selection.set_content(modal);
                }
            }
        }
    }

    /// Route typed Selection modal requests to the existing source-specific
    /// App actions after validating the current modal snapshot.
    pub(super) fn handle_selection_modal_request(&mut self, request: ShellRequest) {
        match request {
            ShellRequest::DismissSelectionModal => self.app.close_selection_modal(),
            ShellRequest::SelectionModalFilterSelected(selected) => {
                let Some(modal) = self.app.selection_modal.as_ref() else {
                    return;
                };
                let valid = modal
                    .filter
                    .as_ref()
                    .is_some_and(|filter| selected < filter.labels.len());
                if !valid {
                    return;
                }
                match modal.source {
                    super::types_selection_modal::SelectionModalSource::Series { .. } => {
                        self.app.select_series_selection_modal_season(selected)
                    }
                    super::types_selection_modal::SelectionModalSource::Podcast { .. } => {
                        self.app.select_podcast_selection_modal_filter(selected)
                    }
                    _ => {}
                }
            }
            ShellRequest::SelectionModalActivate(item_id) => {
                let Some(item_id) = item_id else {
                    self.app.activate_selection_modal_item();
                    return;
                };
                let Some(row_index) = self.app.selection_modal.as_ref().and_then(|modal| {
                    modal
                        .state
                        .rows()
                        .iter()
                        .position(|row| row.item_id() == Some(item_id.as_str()))
                }) else {
                    return;
                };
                if let Some(modal) = self.app.selection_modal.as_mut() {
                    modal.cursor = row_index;
                }
                self.app.activate_selection_modal_item();
            }
            _ => {}
        }
    }

    /// Render the Selection modal from the shell-owned snapshot. Its component
    /// records the returned geometry for its own mouse hit-testing.
    pub(super) fn render_selection_modal_overlay(&mut self, f: &mut ratatui::Frame) {
        let id = Self::selection_modal_id();
        if !self.application.mounted(&id) {
            return;
        }
        if let Some(modal) = self.app.selection_modal.as_ref() {
            if let Some(comp) = self.application.get_component_mut(&id) {
                if let Some(selection) = comp.as_any_mut().downcast_mut::<SelectionModalComponent>()
                {
                    selection.set_content(modal);
                }
            }
        }
        self.application.view(&id, f, f.area());
    }

    // --- Settings Multiselect popup ----------------------------------------

    fn multiselect_id() -> ComponentId {
        ComponentId::Popup(PopupId::Multiselect)
    }

    pub(super) fn sync_multiselect(&mut self) {
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

    pub(super) fn handle_multiselect_commit(&mut self) {
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

    pub(super) fn render_multiselect_popup(&mut self, f: &mut ratatui::Frame) {
        let id = Self::multiselect_id();
        if self.application.mounted(&id) {
            self.application.view(&id, f, f.area());
        }
    }

    // --- Settings Library-routes popup -------------------------------------

    fn library_routes_id() -> ComponentId {
        ComponentId::Popup(PopupId::LibraryRoutes)
    }

    pub(super) fn sync_library_routes(&mut self) {
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

    pub(super) fn handle_library_routes_request(&mut self, request: ShellRequest) {
        self.sync_library_routes_to_app();
        match request {
            ShellRequest::LibraryRoutesEnter => self.app.handle_library_routes_enter(),
            ShellRequest::LibraryRoutesEsc => self.app.handle_library_routes_esc(),
            _ => {}
        }
    }

    pub(super) fn render_library_routes_popup(&mut self, f: &mut ratatui::Frame) {
        let id = Self::library_routes_id();
        if self.application.mounted(&id) {
            self.application.view(&id, f, f.area());
        }
    }

    // --- Settings Feed-management popup ------------------------------------

    fn feeds_manage_id() -> ComponentId {
        ComponentId::Popup(PopupId::FeedManage)
    }

    pub(super) fn sync_feeds_manage(&mut self) {
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

    pub(super) fn handle_feeds_manage_request(&mut self, key: crossterm::event::KeyEvent) {
        self.sync_feeds_manage_to_app();
        let _ = self.app.handle_key_feeds_manage(key);
    }

    pub(super) fn render_feeds_manage_popup(&mut self, f: &mut ratatui::Frame) {
        let id = Self::feeds_manage_id();
        if self.application.mounted(&id) {
            self.application.view(&id, f, f.area());
        }
    }

    // --- Search sidebar -----------------------------------------------------
    //
    // The Search sidebar is a non-blocking overlay mounted when
    // `App::search_sidebar_open` transitions to true. The component owns the
    // sidebar state (query, cursor, scroll, type_filter, loading, results)
    // and the 300 ms debounce (driven by `UserEvent::Clock`); the shell owns
    // the Emby client and spawns the search thread (design D4/D5).

    fn search_id() -> ComponentId {
        ComponentId::Overlay(OverlayId::Search)
    }

    /// Sync the Search component mount state with `App::search_sidebar_open`.
    pub(super) fn sync_search_sidebar(&mut self) {
        let id = Self::search_id();
        let mounted = self.application.mounted(&id);
        if self.app.search_sidebar_open && !mounted {
            self.application
                .mount(id.clone(), Box::new(SearchSidebarComponent::new()), vec![])
                .expect("mount Search");
            self.application.active(&id).expect("activate Search");
        } else if !self.app.search_sidebar_open && mounted {
            let _ = self.application.umount(&id);
        }
    }

    /// Render the Search overlay if mounted. Sets the panel area via
    /// downcast so the component's `view()` paints over the legacy frame.
    pub(super) fn render_search_overlay(&mut self, f: &mut ratatui::Frame) {
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
    pub(super) fn drain_search_results(&mut self) -> bool {
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
    pub(super) fn sync_sessions(&mut self) {
        let id = Self::sessions_id();
        let mounted = self.application.mounted(&id);
        if self.app.show_sessions && !mounted {
            self.application
                .mount(id.clone(), Box::new(SessionsComponent::new()), vec![])
                .expect("mount Sessions");
            self.application.active(&id).expect("activate Sessions");
        } else if !self.app.show_sessions && mounted {
            let _ = self.application.umount(&id);
        }
    }

    /// Render Sessions from shell-owned runtime data. Cursor, scroll, and hit
    /// geometry remain private to the mounted component.
    pub(super) fn render_sessions_overlay(&mut self, f: &mut ratatui::Frame) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::components::{
        Msg, MultiselectComponent, PlaybackPromptComponent, SelectionModalComponent,
    };
    use crate::app::tests::make_app_stub;
    use crate::app::types_context_menu::{LibraryRoutePopup, LibraryRouteStage};
    use crate::app::types_context_menu::{MultiSelectKind, MultiSelectPopup};
    use crate::app::types_feeds_manage::FeedsManagePopup;
    use crate::app::types_selection_modal::{
        SelectionModal, SelectionModalItem, SelectionModalListState, SelectionModalRow,
        SelectionModalSource,
    };
    use tuirealm::component::AppComponent;
    use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};

    #[test]
    fn playback_prompt_shell_sync_mounts_and_mirrors_status() {
        let mut model = Model::new(make_app_stub());
        model.app.skip_intro_end_ticks = Some(100);
        model.app.status = "Skip intro? (Y/n)".into();

        model.sync_playback_prompt();

        let id = ComponentId::PlaybackPrompt;
        assert!(model.application.mounted(&id));
        let component = model
            .application
            .get_component(&id)
            .expect("Playback prompt mounted")
            .as_any()
            .downcast_ref::<PlaybackPromptComponent>()
            .expect("Playback prompt type");
        assert_eq!(component.status(), "Skip intro? (Y/n)");

        model.app.skip_intro_end_ticks = None;
        model.sync_playback_prompt();
        assert!(!model.application.mounted(&id));
    }

    #[test]
    fn selection_modal_shell_syncs_and_routes_dismissal() {
        let mut model = Model::new(make_app_stub());
        model.app.selection_modal = Some(SelectionModal {
            source: SelectionModalSource::Album {
                album_id: "album-1".into(),
            },
            title: "Tracks".into(),
            state: SelectionModalListState::Ready(vec![SelectionModalRow::Item(
                SelectionModalItem {
                    name: "Track".into(),
                    meta: String::new(),
                    id: "track-1".into(),
                },
            )]),
            cursor: 0,
            filter: None,
        });
        model.sync_selection_modal();

        let id = ComponentId::Overlay(OverlayId::SelectionModal);
        assert!(model.application.mounted(&id));
        let message = {
            let component = model
                .application
                .get_component_mut(&id)
                .expect("Selection modal mounted")
                .as_any_mut()
                .downcast_mut::<SelectionModalComponent>()
                .expect("Selection modal type");
            component.on(&Event::Keyboard(KeyEvent {
                code: Key::Esc,
                modifiers: KeyModifiers::NONE,
            }))
        };
        let Some(Msg::Shell(request)) = message else {
            panic!("Selection modal should emit a shell request");
        };
        model.handle_selection_modal_request(request);
        model.sync_selection_modal();

        assert!(model.app.selection_modal.is_none());
        assert!(!model.application.mounted(&id));
    }

    #[test]
    fn settings_popup_multiselect_shell_syncs_and_commits_component_choices() {
        let mut model = Model::new(make_app_stub());
        model.app.multiselect_popup = Some(MultiSelectPopup {
            kind: MultiSelectKind::HiddenLibraries,
            items: vec![("movies".into(), "Movies".into(), false)],
            cursor: 0,
        });
        model.sync_multiselect();

        let id = ComponentId::Popup(PopupId::Multiselect);
        let message = {
            let component = model
                .application
                .get_component_mut(&id)
                .expect("Multiselect mounted")
                .as_any_mut()
                .downcast_mut::<MultiselectComponent>()
                .expect("Multiselect type");
            component.on(&Event::Keyboard(KeyEvent {
                code: Key::Enter,
                modifiers: KeyModifiers::NONE,
            }))
        };
        let Some(Msg::Shell(request)) = message else {
            panic!("Multiselect should emit a shell request");
        };
        model.handle_multiselect_commit();
        assert!(matches!(request, ShellRequest::MultiselectCommit { .. }));
        model.sync_multiselect();
        assert!(model.app.multiselect_popup.is_none());
        assert!(!model.application.mounted(&id));
    }

    #[test]
    fn settings_popup_library_routes_shell_syncs_and_routes_escape() {
        let mut model = Model::new(make_app_stub());
        model.app.library_routes_popup = Some(LibraryRoutePopup {
            stage: LibraryRouteStage::PickLibrary {
                items: vec![("movies".into(), "Movies".into(), None)],
            },
            cursor: 0,
        });
        model.sync_library_routes();

        let id = ComponentId::Popup(PopupId::LibraryRoutes);
        let message = {
            let component = model
                .application
                .get_component_mut(&id)
                .expect("Library routes mounted")
                .as_any_mut()
                .downcast_mut::<LibraryRoutesComponent>()
                .expect("Library routes type");
            component.on(&Event::Keyboard(KeyEvent {
                code: Key::Esc,
                modifiers: KeyModifiers::NONE,
            }))
        };
        let Some(Msg::Shell(request)) = message else {
            panic!("Library routes should emit a shell request");
        };
        model.handle_library_routes_request(request);
        model.sync_library_routes();

        assert!(model.app.library_routes_popup.is_none());
        assert!(!model.application.mounted(&id));
    }

    #[test]
    fn settings_popup_feeds_manage_shell_syncs_and_routes_escape() {
        let mut model = Model::new(make_app_stub());
        model.app.feeds_manage_popup = Some(FeedsManagePopup::new());
        model.sync_feeds_manage();

        let id = ComponentId::Popup(PopupId::FeedManage);
        let message = {
            let component = model
                .application
                .get_component_mut(&id)
                .expect("Feed management mounted")
                .as_any_mut()
                .downcast_mut::<FeedsManageComponent>()
                .expect("Feed management type");
            component.on(&Event::Keyboard(KeyEvent {
                code: Key::Esc,
                modifiers: KeyModifiers::NONE,
            }))
        };
        let Some(Msg::Shell(ShellRequest::FeedsManageKey(key))) = message else {
            panic!("Feed management should emit a shell request");
        };
        model.handle_feeds_manage_request(key);
        model.sync_feeds_manage();

        assert!(model.app.feeds_manage_popup.is_none());
        assert!(!model.application.mounted(&id));
    }
}
