use crate::app::{App, RemoteSlotState};

impl App {
    pub(in crate::app) fn queue_title_model(
        &self,
    ) -> mbv_render::components::queue::QueueTitleModel {
        let remote_state = self.remote_slot_state();
        let daemon_endpoint = self.config.lock().unwrap().daemon_client_endpoint.clone();
        let (icon, label) = self.remote_icon_and_label(remote_state, &daemon_endpoint);
        let remote_pill =
            (remote_state != RemoteSlotState::Off).then(|| format!(" {} {} ", icon, label.trim()));
        mbv_render::components::queue::QueueTitleModel { remote_pill }
    }
}
