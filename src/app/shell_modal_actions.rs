use super::components::{
    ComponentId, ConfirmComponent, DaemonLostComponent, ModalId, RemoteReanchorComponent,
    SavePlaylistComponent,
};
use super::shell::Model;
use super::types_confirm::ConfirmAction;
use crossterm::event::{KeyCode, KeyEvent};

impl Model {
    pub(super) fn handle_confirm_key(&mut self, key: KeyEvent) {
        let id = ComponentId::Modal(ModalId::Confirm);
        let Some(action) = self
            .application
            .get_component(&id)
            .and_then(|component| component.as_any().downcast_ref::<ConfirmComponent>())
            .and_then(ConfirmComponent::confirm_action)
        else {
            return;
        };
        if confirm_key_dismisses(&action, key.code) {
            self.dismiss_modal(&id);
        }
        self.app.apply_confirm_action(action, key);
    }

    pub(super) fn handle_daemon_lost_key(&mut self, key: KeyEvent) -> bool {
        let id = ComponentId::Modal(ModalId::DaemonLost);
        if !self.application.mounted(&id) {
            return false;
        }
        match key.code {
            KeyCode::Char('r') | KeyCode::Char('R') => {
                if let Err(error) = self.app.restart_local_daemon(true) {
                    self.set_daemon_lost_restart_error(error);
                }
                false
            }
            KeyCode::Char('s') | KeyCode::Char('S') => {
                if let Err(error) = self.app.restart_local_daemon(false) {
                    self.set_daemon_lost_restart_error(error);
                }
                false
            }
            KeyCode::Char('q') | KeyCode::Char('Q') => {
                self.dismiss_modal(&id);
                self.app.try_quit()
            }
            _ => false,
        }
    }

    fn set_daemon_lost_restart_error(&mut self, error: String) {
        let id = ComponentId::Modal(ModalId::DaemonLost);
        if let Some(component) = self.application.get_component_mut(&id) {
            if let Some(modal) = component.as_any_mut().downcast_mut::<DaemonLostComponent>() {
                modal.set_restart_error(error);
            }
        }
    }

    pub(super) fn handle_remote_reanchor_key(&mut self, key: KeyEvent) {
        let id = ComponentId::Modal(ModalId::RemoteReanchor);
        if !self.application.mounted(&id) {
            return;
        }
        match key.code {
            KeyCode::Esc => self.dismiss_modal(&id),
            KeyCode::Up | KeyCode::Down => {
                if let Some(component) = self.application.get_component_mut(&id) {
                    if let Some(popup) = component
                        .as_any_mut()
                        .downcast_mut::<RemoteReanchorComponent>()
                    {
                        popup.move_cursor(key.code == KeyCode::Down);
                    }
                }
            }
            KeyCode::Enter => {
                let target = self
                    .application
                    .get_component(&id)
                    .and_then(|component| {
                        component.as_any().downcast_ref::<RemoteReanchorComponent>()
                    })
                    .and_then(RemoteReanchorComponent::selected_target);
                self.dismiss_modal(&id);
                if let Some(target) = target {
                    self.app.reanchor_remote_target(target);
                }
            }
            _ => {}
        }
    }

    pub(super) fn handle_save_playlist_key(&mut self, key: KeyEvent) {
        let id = ComponentId::Modal(ModalId::SavePlaylist);
        let Some((input, rename, rename_id)) = self
            .application
            .get_component(&id)
            .and_then(|component| component.as_any().downcast_ref::<SavePlaylistComponent>())
            .map(|dialog| {
                (
                    dialog.input().to_string(),
                    dialog.is_rename(),
                    dialog.rename_id().map(str::to_owned),
                )
            })
        else {
            return;
        };
        if key.code == KeyCode::Esc {
            self.dismiss_modal(&id);
            self.app.force_clear = true;
            return;
        }
        if key.code != KeyCode::Enter {
            return;
        }
        let name = input.trim().to_string();
        if name.is_empty() {
            return;
        }
        if rename {
            self.dismiss_modal(&id);
            self.app.force_clear = true;
            if let Some(playlist_id) = rename_id {
                self.app.spawn_rename_playlist(playlist_id, name);
            }
            return;
        }
        let playlists = {
            let Some(client) = self.app.emby_client() else {
                return;
            };
            let client = client.lock().unwrap();
            client.get_playlists().unwrap_or_default()
        };
        if let Some(existing) = playlists
            .into_iter()
            .find(|playlist| playlist.name.to_lowercase() == name.to_lowercase())
        {
            self.dismiss_modal(&id);
            self.app.ask_confirm(super::ConfirmModal {
                title: " Overwrite Playlist ".into(),
                message: format!(
                    "\"{}\" already exists.",
                    super::ui_util::trunc_str(&name, 40)
                ),
                hint: "[y] Overwrite    [Esc] Back".into(),
                on_confirm: ConfirmAction::SaveOverwritePlaylist {
                    existing_id: existing.id,
                    name,
                },
            });
        } else {
            self.dismiss_modal(&id);
            self.app.force_clear = true;
            self.app.save_queue_as_playlist(name);
        }
    }
}

fn confirm_key_dismisses(action: &ConfirmAction, key: KeyCode) -> bool {
    match action {
        ConfirmAction::ClearQueue
        | ConfirmAction::RemoveActiveQueueItem(_)
        | ConfirmAction::RescanLibrary(_)
        | ConfirmAction::RemoveFeedSubscription(_) => true,
        ConfirmAction::SaveOverwritePlaylist { .. } | ConfirmAction::DeletePlaylist { .. } => {
            matches!(key, KeyCode::Char('y') | KeyCode::Esc)
        }
        ConfirmAction::RemoveEmby | ConfirmAction::RemoveAudiobookshelf => {
            matches!(
                key,
                KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter | KeyCode::Esc
            )
        }
        ConfirmAction::ReplaceEmby(_) | ConfirmAction::ReplaceAudiobookshelf(_) => {
            matches!(
                key,
                KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter | KeyCode::Esc
            )
        }
        ConfirmAction::DiscardOrSaveDirtyPlaylist => matches!(
            key,
            KeyCode::Char('s')
                | KeyCode::Char('S')
                | KeyCode::Char('d')
                | KeyCode::Char('D')
                | KeyCode::Char('c')
                | KeyCode::Char('C')
                | KeyCode::Esc
        ),
    }
}
