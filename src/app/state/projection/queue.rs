use crate::app::{App, RemoteSlotState};

impl App {
    pub(in crate::app) fn queue_title_model(
        &self,
    ) -> mbv_render::components::queue::QueueTitleModel {
        let remote_state = self.remote_slot_state();
        let daemon_endpoint = self.config.lock().unwrap().daemon_client_endpoint.clone();
        let (icon, label) = self.remote_icon_and_label(remote_state, &daemon_endpoint);
        // With nerd fonts the icon leads the pill; the autosave glyph's trailing
        // space is the single gap between them.
        let lead = if self.use_nerd_fonts { "" } else { " " };
        let remote_pill = (remote_state != RemoteSlotState::Off)
            .then(|| format!("{lead}{} {} ", icon, label.trim()));
        mbv_render::components::queue::QueueTitleModel { remote_pill }
    }
}
