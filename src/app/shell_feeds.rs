use super::components::feeds_content::{FeedsContent, FeedsOwnerPush};
use super::components::library_panel::LibraryKey;
use super::shell::Model;

impl Model {
    /// Event-scoped content projection for the Feeds owner inside the mounted
    /// `LibraryPanel` (task 7.3, design D2), addressed by `LibraryKey::Feeds`.
    /// The owner is retained across pushes, so its group/filter selection and
    /// the shared list owner's cursor/scroll survive a refresh; a hidden tab
    /// leaves the retained owner untouched.
    pub(super) fn sync_feeds(&mut self) {
        if !matches!(self.app.tab, super::TabSelection::Feeds) {
            return;
        }
        let selected_latest = self
            .application
            .get_component(&super::components::ComponentId::Library)
            .and_then(|component| {
                component
                    .as_any()
                    .downcast_ref::<super::components::library_panel::LibraryPanel>()
            })
            .and_then(|panel| panel.owner(&LibraryKey::Feeds))
            .and_then(|owner| owner.as_any().downcast_ref::<FeedsContent>())
            .is_some_and(FeedsContent::latest_selected);
        if selected_latest
            && !self
                .acknowledged_home_latest_sources
                .contains(&super::DestinationLatestSource::Feeds)
        {
            self.record_home_latest_acknowledgement(super::DestinationLatestSource::Feeds);
        }
        let state = &self.app.feed_tab;
        let has_new = state.all_entries.iter().any(|entry| {
            self.app
                .home_latest_launch_window
                .previous
                .is_some_and(|previous| {
                    entry.pub_date_secs.is_some_and(|timestamp| {
                        previous < timestamp
                            && timestamp <= self.app.home_latest_launch_window.current
                    })
                })
        });
        let acknowledged = self
            .acknowledged_home_latest_sources
            .contains(&super::DestinationLatestSource::Feeds);
        let push = FeedsOwnerPush {
            subscriptions: state.subscriptions.clone(),
            entries: state.entries.clone(),
            all_entries: state.all_entries.clone(),
            loading: state.loading,
        };
        self.update_feeds_owner(|feeds| {
            feeds.set_content(push);
            feeds.set_latest_marker(has_new, acknowledged);
        });
    }

    /// Mutate the Feeds owner inside the mounted `LibraryPanel` (design D2:
    /// the shell pushes content addressed by `LibraryKey`), creating it on
    /// first push. The owner is installed with the panel at startup (task
    /// 7.3), so the create branch is defensive.
    pub(super) fn update_feeds_owner<R>(
        &mut self,
        f: impl FnOnce(&mut FeedsContent) -> R,
    ) -> Option<R> {
        self.update_library_owner(LibraryKey::Feeds, || Box::new(FeedsContent::new()), f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::tests::make_app_stub;
    use crate::app::PanelFocus;
    use mbv_core::config::{FeedKind, FeedSubscription};

    fn subscription(name: &str) -> FeedSubscription {
        FeedSubscription {
            name: name.into(),
            url: format!("https://example.test/{name}"),
            kind: FeedKind::Audio,
        }
    }

    fn subscription_names(model: &mut Model) -> Vec<String> {
        model
            .update_feeds_owner(|feeds| {
                feeds
                    .subscription_names()
                    .into_iter()
                    .map(str::to_string)
                    .collect()
            })
            .expect("Feeds owner installed")
    }

    #[test]
    fn hidden_tab_does_not_overwrite_retained_feeds_owner() {
        let mut model = Model::new(make_app_stub());
        model.app.tab = super::super::TabSelection::Feeds;
        model.app.feed_tab.subscriptions = vec![subscription("Visible Feed")];
        model.sync_feeds();

        model.app.tab = super::super::TabSelection::Home;
        model.app.feed_tab.subscriptions = vec![subscription("Hidden Feed")];
        model.sync_feeds();

        assert_eq!(subscription_names(&mut model), ["Visible Feed"]);
    }

    #[test]
    fn shell_syncs_feed_snapshot_into_retained_owner() {
        let mut model = Model::new(make_app_stub());
        model.app.tab = super::super::TabSelection::Feeds;
        model.app.feed_tab.subscriptions = vec![subscription("Shell Feed")];
        model.sync_feeds();

        assert_eq!(subscription_names(&mut model), ["Shell Feed"]);
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
