//! The Twitch URL prompt (twitch-url-playback tasks 5.5, design D7): the
//! shell owns the modal's mount and unmount through the `TuiRealm` focus
//! stack, the owner-capability check, and the host check via
//! `TwitchItem::from_url`. The component (`crates/mbv-components`) owns the
//! typed input and reports `TwitchUrlIntent`s only.

use super::Model;
use mbv_components::TwitchUrlComponent;
use mbv_queue::{QueueItem, TwitchItem};
use mbv_ui_msg::{ComponentId, ModalId, TwitchUrlIntent};

impl Model {
    fn twitch_url_id() -> ComponentId {
        ComponentId::Modal(ModalId::TwitchUrl)
    }

    /// Mount the prompt modal and give it the focus (`TuiRealm` focus
    /// stack). The opener's `NoBlockingOverlay` gate keeps this reachable
    /// only with no blocking overlay mounted, and the mount dismisses any
    /// blocking modal first the same way the other modal mounts do.
    pub(in crate::app) fn mount_twitch_url_modal(&mut self) {
        self.dismiss_blocking_modals();
        let id = Self::twitch_url_id();
        self.application
            .mount(id.clone(), Box::new(TwitchUrlComponent::new()), vec![])
            .expect("mount TwitchUrl");
        self.application.active(&id).expect("activate TwitchUrl");
    }

    /// Render the prompt overlay if mounted.
    pub(in crate::app) fn render_twitch_url_overlay(&mut self, frame: &mut ratatui::Frame) {
        let id = Self::twitch_url_id();
        if !self.application.mounted(&id) {
            return;
        }
        self.application.view(&id, frame, frame.area());
    }

    pub(in crate::app) fn handle_twitch_url_intent(&mut self, intent: TwitchUrlIntent) {
        let id = Self::twitch_url_id();
        match intent {
            TwitchUrlIntent::Cancel => {
                self.dismiss_modal(&id);
            }
            TwitchUrlIntent::Submit(url) => {
                // Capability first (design D6): a Player owner that did not
                // advertise `twitch-queue` receives nothing — the refusal
                // stays in the prompt and no queue operation is sent. The
                // rejection at the Owner is the backstop, never the path.
                let link = self.app.playing_queue_link();
                let owner_supports_twitch =
                    self.app.queue_link(link).0.remote().supports_twitch_queue();
                if !owner_supports_twitch {
                    self.push_twitch_url_refusal(
                        &id,
                        "This Player owner cannot play Twitch items.",
                    );
                    return;
                }
                match TwitchItem::from_url(&url) {
                    Err(error) => {
                        // The prompt stays mounted with the input kept; the
                        // refusal names why the URL is not a Twitch URL.
                        self.push_twitch_url_refusal(&id, &error.to_string());
                    }
                    Ok(item) => {
                        self.dismiss_modal(&id);
                        self.app.submit_queue_item(QueueItem::Twitch(item), true);
                    }
                }
            }
        }
    }

    fn push_twitch_url_refusal(&mut self, id: &ComponentId, refusal: &str) {
        if let Some(component) = self.application.get_component_mut(id)
            && let Some(prompt) = component.as_any_mut().downcast_mut::<TwitchUrlComponent>()
        {
            prompt.push_refusal(refusal.to_string());
        }
    }
}
