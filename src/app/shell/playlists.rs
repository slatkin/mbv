use super::components::{
    ComponentId, ModalId, OverlayId, PlaylistsComponent, PlaylistsContent, ShellRequest,
};
use super::Model;

impl Model {
    pub(in crate::app) fn update_playlists_content(&mut self) {
        let id = ComponentId::Overlay(OverlayId::Playlists);
        if !self.application.mounted(&id) {
            return;
        }
        let panel = crate::app::shell::chrome_panels::sync_panel_area(&self.app);
        if let Some(comp) = self.application.get_component_mut(&id) {
            if let Some(playlists) = comp.as_any_mut().downcast_mut::<PlaylistsComponent>() {
                playlists.set_content(PlaylistsContent {
                    playlists: self.app.playlists.clone(),
                    cursor: self.app.playlists_cursor,
                    scroll: self.app.playlists_scroll,
                    loading: self.app.playlists_loading,
                    open: self.app.playlists_open.clone(),
                    open_items: self.app.playlists_open_items.clone(),
                    open_cursor: self.app.playlists_open_cursor,
                    open_scroll: self.app.playlists_open_scroll,
                    open_loading: self.app.playlists_open_loading,
                    loaded_id: match &self.app.queue_source {
                        crate::config::QueueSource::Playlist { id: Some(id), .. } => {
                            Some(id.clone())
                        }
                        _ => None,
                    },
                });
                playlists.set_panel_area(panel);
            }
        }
    }

    pub(in crate::app) fn render_playlists_overlay(&mut self, frame: &mut ratatui::Frame) {
        let id = ComponentId::Overlay(OverlayId::Playlists);
        if !self.application.mounted(&id) {
            return;
        }
        self.application.view(&id, frame, frame.area());
    }

    pub(in crate::app) fn render_save_playlist_overlay(&mut self, frame: &mut ratatui::Frame) {
        let id = ComponentId::Modal(ModalId::SavePlaylist);
        if self.application.mounted(&id) {
            self.application.view(&id, frame, frame.area());
        }
    }

    pub(in crate::app) fn handle_playlists_request(&mut self, request: &ShellRequest) {
        match request {
            ShellRequest::PlaylistsBack => {
                self.app.playlists_open = None;
                self.app.playlists_open_items.clear();
            }
            ShellRequest::PlaylistsOpen(index) => {
                if let Some(playlist) = self.app.playlists.get(*index).cloned() {
                    self.app.spawn_open_playlist(playlist);
                }
            }
            ShellRequest::PlaylistsActivate { open, index } => {
                if *open {
                    let Some(selected_id) = self
                        .app
                        .playlists_open_items
                        .get(*index)
                        .map(|item| item.id.clone())
                    else {
                        return;
                    };
                    let Some(playlist) = self.app.playlists_open.as_ref() else {
                        return;
                    };
                    let items: Vec<_> = self
                        .app
                        .playlists_open_items
                        .iter()
                        .filter(|item| !item.is_folder)
                        .cloned()
                        .collect();
                    if items.is_empty() {
                        return;
                    }
                    let start_idx = items
                        .iter()
                        .position(|item| item.id == selected_id)
                        .unwrap_or(0);
                    self.app.request_queue_replacement(
                        crate::app::state::types::playback::PendingQueueAction::PlayItems {
                            items,
                            start_idx,
                            source: crate::config::QueueSource::Playlist {
                                id: Some(playlist.id.clone()),
                                name: playlist.name.clone(),
                            },
                            autostart: false,
                        },
                        crate::app::state::types::playback::ReplacementExecutor::Pending,
                    );
                    // No sidebar dismiss here: `run_replacement` raises it once
                    // the replacement actually runs (immediately on an empty
                    // queue, after confirmation on a populated one), so a
                    // cancelled load leaves the sidebar open.
                } else if let Some(playlist) = self.app.playlists.get(*index).cloned() {
                    self.app.load_and_play_playlist(playlist.id);
                }
            }
            ShellRequest::PlaylistsRename(index) => {
                if let Some(playlist) = self.app.playlists.get(*index).cloned() {
                    self.app
                        .open_save_playlist_dialog(crate::app::SavePlaylistDialog {
                            input: playlist.name,
                            stage: crate::app::SavePlaylistStage::RenamePlaylist {
                                id: playlist.id,
                            },
                        });
                }
            }
            ShellRequest::PlaylistsDelete(index) => {
                if let Some(playlist) = self.app.playlists.get(*index).cloned() {
                    self.app.ask_confirm(crate::app::ConfirmModal {
                        title: " Delete Playlist ".into(),
                        message: format!(
                            "Delete playlist '{}'?",
                            crate::app::infra::ui_util::trunc_str(&playlist.name, 40)
                        ),
                        hint: "[y] Confirm    [Esc] Cancel".into(),
                        on_confirm: crate::app::ConfirmAction::DeletePlaylist {
                            id: playlist.id,
                            name: playlist.name,
                        },
                    });
                }
            }
            ShellRequest::PlaylistsRefresh => {
                if let Some(playlist) = self.app.playlists_open.clone() {
                    self.app.playlists_open = None;
                    self.app.playlists_open_items.clear();
                    self.app.spawn_open_playlist(playlist);
                } else {
                    self.app.spawn_load_playlists();
                }
            }
            ShellRequest::DismissPlaylists => {
                self.dismiss_sidebar(super::SidebarId::Playlists);
            }
            // unreachable: shell/messages.rs routes only the Playlists* group
            // (Back/Open/Activate/Rename/Delete/Refresh/DismissPlaylists) here;
            // every one has an arm above.
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::components::msg::SavePlaylistIntent;
    use crate::app::components::msg::{Msg, TerminalObserverEvent};
    use crate::app::components::ShellRequest;
    use crate::app::tests::make_app_stub;
    use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};

    #[test]
    fn playlists_shell_mounts_and_routes_component() {
        let mut app = make_app_stub();
        app.pending_overlay = Some(
            crate::app::state::types::overlay::OverlayRequest::OpenSidebar(
                crate::app::SidebarId::Playlists,
            ),
        );
        let mut model = Model::new(app);
        model.sync_modal_requests();
        let id = ComponentId::Overlay(OverlayId::Playlists);
        let message = model
            .application
            .get_component_mut(&id)
            .expect("Playlists component mounted")
            .on(&Event::Keyboard(KeyEvent {
                code: Key::Down,
                modifiers: KeyModifiers::NONE,
            }));
        assert!(matches!(
            message,
            Some(Msg::TerminalEvent(TerminalObserverEvent::KeyClaimed))
        ));
    }
}
