use super::Model;
use crate::app::dispatch::session::daemon_lost::DaemonLostAnswer;
use mbv_components::{ConfirmComponent, DaemonLostComponent, SavePlaylistComponent};
use mbv_ui_model::confirm::ConfirmAction;
use mbv_ui_msg::{ComponentId, Key, KeyEvent, ModalId};
use mbv_ui_msg::{ConfirmIntent, DaemonLostIntent, SavePlaylistIntent};

impl Model {
    pub(in crate::app) fn handle_confirm_intent(&mut self, intent: ConfirmIntent) {
        let code = match intent {
            ConfirmIntent::Accept => Key::Enter,
            ConfirmIntent::Cancel => Key::Esc,
            ConfirmIntent::Save => Key::Char('s'),
            ConfirmIntent::Discard => Key::Char('d'),
            ConfirmIntent::Dismiss => Key::Char('x'),
        };
        self.handle_confirm_key(KeyEvent::new(code, mbv_ui_msg::KeyModifiers::NONE));
    }

    pub(in crate::app) fn handle_confirm_key(&mut self, key: KeyEvent) {
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

    /// The `DaemonLost` modal's answer, from its component's key intent: the
    /// answer's effect is the shared `App::apply_daemon_lost_intent`, and
    /// this handler owns only where the dialog and its diagnostic line live
    /// in the TUI (the mounted component).
    pub(in crate::app) fn handle_daemon_lost_intent(&mut self, intent: DaemonLostIntent) -> bool {
        let id = ComponentId::Modal(ModalId::DaemonLost);
        if !self.application.mounted(&id) {
            return false;
        }
        match self.app.apply_daemon_lost_intent(intent) {
            DaemonLostAnswer::Restarted => false,
            DaemonLostAnswer::RestartFailed(error) => {
                self.set_daemon_lost_restart_error(error);
                false
            }
            DaemonLostAnswer::Quit(quit) => {
                self.dismiss_modal(&id);
                quit
            }
        }
    }

    fn set_daemon_lost_restart_error(&mut self, error: String) {
        let id = ComponentId::Modal(ModalId::DaemonLost);
        if let Some(component) = self.application.get_component_mut(&id)
            && let Some(modal) = component.as_any_mut().downcast_mut::<DaemonLostComponent>()
        {
            modal.set_restart_error(error);
        }
    }

    pub(in crate::app) fn handle_save_playlist_intent(&mut self, intent: SavePlaylistIntent) {
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
            self.app.ask_confirm(super::ConfirmModal::two_button(
                format!(
                    "\"{}\" already exists.",
                    mbv_ui_model::ui_util::trunc_str(&name, 40)
                ),
                "Overwrite",
                "Back",
                ConfirmAction::SaveOverwritePlaylist {
                    existing_id: existing.id,
                    name,
                },
            ));
        } else {
            self.dismiss_modal(&id);
            self.app.force_clear = true;
            self.app.save_queue_as_playlist(name);
        }
    }
}

/// Whether answering `action` with `key` should also close the modal. Only
/// the keys an action actually answers on dismiss it; any other key is a
/// no-op that leaves the modal open.
fn confirm_key_dismisses(action: &ConfirmAction, key: Key) -> bool {
    match action {
        ConfirmAction::DiscardOrSaveDirtyPlaylist => {
            matches!(key, Key::Char('s' | 'S' | 'd' | 'D') | Key::Esc)
        }
        _ => matches!(key, Key::Enter | Key::Esc),
    }
}
