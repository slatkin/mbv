use super::components::{ComponentId, FeedsComponent};
use super::shell::Model;

impl Model {
    pub(super) fn mount_feeds(&mut self) {
        self.application
            .mount(ComponentId::Feeds, Box::new(FeedsComponent::new()), vec![])
            .expect("mount Feeds");
    }

    pub(super) fn sync_feeds(&mut self) {
        if !matches!(self.app.tab, super::TabSelection::Feeds) {
            return;
        }
        let state = &self.app.feed_tab;
        if let Some(comp) = self.application.get_component_mut(&ComponentId::Feeds) {
            if let Some(feeds) = comp.as_any_mut().downcast_mut::<FeedsComponent>() {
                feeds.set_images_enabled(self.app.images_enabled());
                feeds.set_content(
                    &state.subscriptions,
                    &state.entries,
                    &state.all_entries,
                    state.loading,
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
    use crate::app::PanelFocus;
    use mbv_core::config::{FeedKind, FeedSubscription};

    #[test]
    fn hidden_tab_does_not_overwrite_mounted_feeds_component() {
        let mut model = Model::new(make_app_stub());
        model.app.tab = super::super::TabSelection::Feeds;
        model.app.feed_tab.subscriptions = vec![FeedSubscription {
            name: "Visible Feed".into(),
            url: "https://example.test/visible".into(),
            kind: FeedKind::Audio,
        }];
        model.sync_feeds();

        model.app.tab = super::super::TabSelection::Home;
        model.app.feed_tab.subscriptions = vec![FeedSubscription {
            name: "Hidden Feed".into(),
            url: "https://example.test/hidden".into(),
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
        assert_eq!(component.subscription_names(), ["Visible Feed"]);
    }

    #[test]
    fn shell_syncs_feed_snapshot_into_mounted_component() {
        let mut model = Model::new(make_app_stub());
        model.app.tab = super::super::TabSelection::Feeds;
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

    // Task 4.5: the FeedsRowClick arm pulls panel focus to the Library
    // (mirrors the HomeRowClick arm).
    #[test]
    fn feeds_row_click_pulls_panel_focus_to_library() {
        let mut model = Model::new(make_app_stub());
        model.app.panel_focus = PanelFocus::Queue;
        let mut music_resize = false;
        let mut tv_resize = false;
        model.handle_terminal_message(
            crate::app::components::Msg::Shell(crate::app::components::ShellRequest::FeedsRowClick),
            &mut music_resize,
            &mut tv_resize,
        );
        assert_eq!(model.app.panel_focus, PanelFocus::Library);
    }
}
