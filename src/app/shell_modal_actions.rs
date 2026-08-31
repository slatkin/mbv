use super::components::msg::{
    ConfirmIntent, DaemonLostIntent, RemoteReanchorIntent, SavePlaylistIntent,
};
use super::components::{
    ComponentId, ConfirmComponent, DaemonLostComponent, ModalId, RemoteReanchorComponent,
    SavePlaylistComponent,
};
use super::shell::Model;
use super::types_confirm::ConfirmAction;
use crossterm::event::{KeyCode, KeyEvent};

impl Model {
    pub(super) fn handle_confirm_intent(&mut self, intent: ConfirmIntent) {
        let key = match intent {
            ConfirmIntent::Accept => {
                KeyEvent::new(KeyCode::Char('y'), crossterm::event::KeyModifiers::NONE)
            }
            ConfirmIntent::Cancel => {
                KeyEvent::new(KeyCode::Esc, crossterm::event::KeyModifiers::NONE)
            }
            ConfirmIntent::Save => {
                KeyEvent::new(KeyCode::Char('s'), crossterm::event::KeyModifiers::NONE)
            }
            ConfirmIntent::Discard => {
                KeyEvent::new(KeyCode::Char('d'), crossterm::event::KeyModifiers::NONE)
            }
            ConfirmIntent::Dismiss => {
                KeyEvent::new(KeyCode::Char('x'), crossterm::event::KeyModifiers::NONE)
            }
        };
        self.handle_confirm_key(key);
    }

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

    pub(super) fn handle_daemon_lost_intent(&mut self, intent: DaemonLostIntent) -> bool {
        let id = ComponentId::Modal(ModalId::DaemonLost);
        if !self.application.mounted(&id) {
            return false;
        }
        match intent {
            DaemonLostIntent::RestartWithTray => {
                if let Err(error) = self.app.restart_local_daemon(true) {
                    self.set_daemon_lost_restart_error(error);
                }
                false
            }
            DaemonLostIntent::RestartWithoutTray => {
                if let Err(error) = self.app.restart_local_daemon(false) {
                    self.set_daemon_lost_restart_error(error);
                }
                false
            }
            DaemonLostIntent::Quit => {
                self.dismiss_modal(&id);
                self.app.try_quit()
            }
        }
    }

    pub(super) fn handle_daemon_lost_key(&mut self, key: KeyEvent) -> bool {
        let intent = match key.code {
            KeyCode::Char('r') | KeyCode::Char('R') => DaemonLostIntent::RestartWithTray,
            KeyCode::Char('s') | KeyCode::Char('S') => DaemonLostIntent::RestartWithoutTray,
            KeyCode::Char('q') | KeyCode::Char('Q') => DaemonLostIntent::Quit,
            _ => return false,
        };
        self.handle_daemon_lost_intent(intent)
    }

    fn set_daemon_lost_restart_error(&mut self, error: String) {
        let id = ComponentId::Modal(ModalId::DaemonLost);
        if let Some(component) = self.application.get_component_mut(&id) {
            if let Some(modal) = component.as_any_mut().downcast_mut::<DaemonLostComponent>() {
                modal.set_restart_error(error);
            }
        }
    }

    pub(super) fn handle_remote_reanchor_intent(&mut self, intent: RemoteReanchorIntent) {
        let id = ComponentId::Modal(ModalId::RemoteReanchor);
        if !self.application.mounted(&id) {
            return;
        }
        match intent {
            RemoteReanchorIntent::Dismiss => self.dismiss_modal(&id),
            RemoteReanchorIntent::MoveUp | RemoteReanchorIntent::MoveDown => {
                if let Some(component) = self.application.get_component_mut(&id) {
                    if let Some(popup) = component
                        .as_any_mut()
                        .downcast_mut::<RemoteReanchorComponent>()
                    {
                        popup.move_cursor(matches!(intent, RemoteReanchorIntent::MoveDown));
                    }
                }
            }
            RemoteReanchorIntent::Accept => {
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
        }
    }

    pub(super) fn handle_remote_reanchor_key(&mut self, key: KeyEvent) {
        let intent = match key.code {
            KeyCode::Esc => RemoteReanchorIntent::Dismiss,
            KeyCode::Up => RemoteReanchorIntent::MoveUp,
            KeyCode::Down => RemoteReanchorIntent::MoveDown,
            KeyCode::Enter => RemoteReanchorIntent::Accept,
            _ => return,
        };
        self.handle_remote_reanchor_intent(intent);
    }

    pub(super) fn handle_save_playlist_intent(&mut self, intent: SavePlaylistIntent) {
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
        if intent == SavePlaylistIntent::Dismiss {
            self.dismiss_modal(&id);
            self.app.force_clear = true;
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

    /// Compatibility bridge for callers that still provide crossterm keys.
    pub(super) fn handle_save_playlist_key(&mut self, key: KeyEvent) {
        let intent = match key.code {
            KeyCode::Esc => SavePlaylistIntent::Dismiss,
            KeyCode::Enter => SavePlaylistIntent::Submit,
            _ => return,
        };
        self.handle_save_playlist_intent(intent);
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
