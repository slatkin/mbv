use super::components::{ComponentId, FeedsComponent};
use super::shell::Model;
use super::PanelFocus;

impl Model {
    pub(super) fn mount_feeds(&mut self) {
        self.application
            .mount(ComponentId::Feeds, Box::new(FeedsComponent::new()), vec![])
            .expect("mount Feeds");
    }

    pub(super) fn sync_feeds(&mut self) {
        let state = &self.app.feed_tab;
        let focused = matches!(self.app.effective_panel_focus(), PanelFocus::Library);
        if let Some(comp) = self.application.get_component_mut(&ComponentId::Feeds) {
            if let Some(feeds) = comp.as_any_mut().downcast_mut::<FeedsComponent>() {
                feeds.set_content(
                    &state.subscriptions,
                    &state.entries,
                    &state.all_entries,
                    state.loading,
                    focused,
                );
            }
        }
    }

    pub(super) fn render_feeds_component(&mut self, frame: &mut ratatui::Frame) {
        if !matches!(self.app.tab, super::TabSelection::Feeds) {
            return;
        }
        let area = self.app.layout.main.feeds_area;
        if area.width == 0 || area.height == 0 {
            return;
        }
        self.application.view(&ComponentId::Feeds, frame, area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::tests::make_app_stub;
    use mbv_core::config::{FeedKind, FeedSubscription};

    #[test]
    fn shell_syncs_feed_snapshot_into_mounted_component() {
        let mut model = Model::new(make_app_stub());
        model.app.feed_tab.subscriptions = vec![FeedSubscription {
            name: "Shell Feed".into(),
            url: "https://example.test/feed".into(),
            kind: FeedKind::Audio,
        }];
        model.sync_feeds();

        let component = model
            .application
            .get_component(&ComponentId::Feeds)
            .expect("Feeds component mounted")
            .as_any()
            .downcast_ref::<FeedsComponent>()
            .expect("Feeds component type");
        assert_eq!(component.subscription_names(), ["Shell Feed"]);
    }
}
