use super::components::{ComponentId, ModalId, OverlayId, PlaybackPromptComponent};
use super::shell::Model;

impl Model {
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

    pub(super) fn render_playback_prompt(&mut self, frame: &mut ratatui::Frame) {
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
        self.application.view(&id, frame, frame.area());
    }

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
}
