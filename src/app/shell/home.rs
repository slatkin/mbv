//! Home Continue Watching sync and typed effects for the shell `Model`.

use super::Model;
use mbv_components::home_content::HomeContent;
use mbv_queue::QueueItem;
use mbv_ui_model::library::LibraryKey;
use mbv_ui_msg::ShellRequest;

impl Model {
    pub(in crate::app) fn handle_home_request(&mut self, request: ShellRequest) {
        match request {
            ShellRequest::HomePlay(target) => {
                if let Some((item, from_cw)) = self.home_stable_target(&target) {
                    self.app.home_play_target(item, from_cw);
                }
            }
            ShellRequest::HomeEnqueue(targets) => {
                for target in targets {
                    if let Some((item, from_cw)) = self.home_stable_target(&target) {
                        self.app.home_enqueue_target(item, from_cw);
                    }
                }
            }
            ShellRequest::RowContextMenu(
                mbv_ui_model::context_menu::ContextMenuTargets::Home(targets),
                anchor,
            ) => {
                let origin = mbv_ui_model::media_list::SelectionOrigin::Library(
                    mbv_ui_model::media_list::LibrarySelectionOrigin::Home,
                );
                self.context_menu_origin = Some(origin.clone());
                self.context_action_snapshot =
                    Some(mbv_ui_model::context_menu::ContextActionSnapshot {
                        origin,
                        values: vec![mbv_ui_model::context_menu::ContextMenuTargets::Home(
                            targets.clone(),
                        )],
                    });
                let mut items = Vec::new();
                let mut removes = Vec::new();
                for target in &targets {
                    if let Some((QueueItem::Emby(item), true)) = self.home_stable_target(target) {
                        removes.push(
                            mbv_ui_model::context_menu::BulkRemoveTarget::ContinueWatching(
                                item.clone(),
                            ),
                        );
                        items.push(*item);
                    }
                }
                if targets.len() > 1 {
                    let capabilities = items
                        .iter()
                        .map(crate::app::state::context_menu_capabilities::emby_item_capabilities)
                        .collect();
                    self.app.open_context_menu_for_selection(
                        &items,
                        anchor,
                        mbv_ui_model::settings::PanelFocus::Library,
                        capabilities,
                        removes,
                    );
                    return;
                }
                let target = targets.into_iter().next();
                let item = target
                    .as_ref()
                    .and_then(|target| self.home_stable_target(target))
                    .and_then(|(item, _)| match item {
                        QueueItem::Emby(item) => Some(*item),
                        _ => None,
                    });
                self.home_context_item.clone_from(&item);
                let cw_selected = target.is_some();
                if let Some((x, y)) = anchor {
                    self.app.open_context_menu_at(x, y, cw_selected, item);
                } else {
                    self.app.open_context_menu(cw_selected, item);
                }
            }
            ShellRequest::HomeDelete(targets) => {
                for target in targets {
                    if let Some((QueueItem::Emby(item), true)) = self.home_stable_target(&target) {
                        self.app.remove_from_continue_watching(&item);
                    }
                }
            }
            ShellRequest::HomeToggleWatched(targets) => {
                for target in targets {
                    if let Some((QueueItem::Emby(item), true)) = self.home_stable_target(&target) {
                        self.app.cw_toggle_watched(&item);
                    }
                }
            }
            _ => {}
        }
    }

    pub(in crate::app) fn update_home_owner<R>(
        &mut self,
        f: impl FnOnce(&mut HomeContent) -> R,
    ) -> Option<R> {
        self.update_library_owner(&LibraryKey::Home, || Box::new(HomeContent::new()), f)
    }

    pub(in crate::app) fn push_home_content(&mut self) {
        let continue_items = self
            .home_content
            .continue_items
            .iter()
            .cloned()
            .map(|item| QueueItem::Emby(Box::new(item)))
            .collect();
        let loading = self.home_content.loading;
        let recent_rows = self.home_recent_rows();
        let launch_window = self.app.home_latest_launch_window;
        self.update_home_owner(|home| {
            home.set_content(continue_items, loading);
            home.set_recent_rows(recent_rows, launch_window);
        });
    }

    /// The merged, newest-first `Recently added` rows, built at push time
    /// from the per-source store plus Feeds (design D2 of
    /// `home-recently-added-tree`). Feeds has no store entry: its rows derive
    /// from `all_entries`, filtered to the same 14-day cutoff, with the
    /// subscription name resolved by the shell. A merged row without a valid
    /// date has no age branch, so it can never appear.
    pub(in crate::app) fn home_recent_rows(&self) -> Vec<mbv_ui_model::home_latest::HomeRecentRow> {
        use mbv_ui_model::home_latest::{HomeRecentRow, provider_timestamp_secs, recent_age};
        let window = self.app.home_latest_launch_window;
        let mut rows: Vec<HomeRecentRow> = self
            .home_recently_added
            .values()
            .flatten()
            .map(|item| HomeRecentRow {
                item: item.clone(),
                feed_name: None,
            })
            .collect();
        rows.extend(
            self.app
                .feed_tab
                .all_entries
                .iter()
                .map(|entry| {
                    let feed_name = entry.feed_id.as_deref().and_then(|feed_id| {
                        self.app
                            .feed_tab
                            .subscriptions
                            .iter()
                            .find(|subscription| subscription.url == feed_id)
                            .map(|subscription| subscription.name.clone())
                    });
                    (QueueItem::Feed(entry.clone()), feed_name)
                })
                .filter(|(item, _)| recent_age(provider_timestamp_secs(item), window).is_some())
                .map(|(item, feed_name)| HomeRecentRow { item, feed_name }),
        );
        rows.sort_by(|a, b| {
            provider_timestamp_secs(&b.item).cmp(&provider_timestamp_secs(&a.item))
        });
        rows
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::tests::{make_app_stub, make_items};
    use mbv_emby_model::test_support::make_item;
    use mbv_ui_model::targets::HomeItemTarget;

    fn target(id: &str) -> HomeItemTarget {
        HomeItemTarget::continue_item(id)
    }

    #[test]
    fn continue_watching_effects_use_the_resolved_item_target() {
        let _guard = crate::config::TestStateDirGuard::new();
        let mut app = make_app_stub();
        // Row 5.3: the enqueue is an Append owner op, so the resolved item is
        // observed on the wire rather than in an optimistic Client write.
        let cmd_rx = crate::app::tests::live_owner_channel(&mut app);
        let mut model = Model::new(app);
        model.home_content.continue_items = make_items(3);

        model.handle_home_request(ShellRequest::HomeEnqueue(vec![target("id2")]));
        let appended: Vec<String> = cmd_rx
            .try_iter()
            .filter_map(|command| match command {
                mbv_ctrl::CtrlCmd::UnifiedQueueAppend { items, .. } => match &*items {
                    [mbv_queue::QueueItem::Emby(item)] => Some(item.id.clone()),
                    _ => None,
                },
                _ => None,
            })
            .collect();
        assert_eq!(appended, vec!["id2"], "the CW enqueue resolves id2");

        model.app.status.clear();
        model.handle_home_request(ShellRequest::HomePlay(target("id0")));
        assert_eq!(model.app.status, "Emby is unavailable");
    }

    #[test]
    fn continue_watching_context_menu_uses_explicit_target() {
        let mut model = Model::new(make_app_stub());
        let item = make_item("cw-target", "Movie");
        model.home_content.continue_items = vec![item.clone()];
        model.handle_home_request(ShellRequest::RowContextMenu(
            mbv_ui_model::context_menu::ContextMenuTargets::Home(vec![target(&item.id)]),
            None,
        ));
        let Some(mbv_ui_model::overlay::OverlayRequest::ContextMenu(menu)) =
            model.app.pending_overlay
        else {
            panic!("Home context-menu request must open a menu");
        };
        assert!(menu.entries.iter().any(|entry| {
            entry
                .action
                .as_ref()
                .is_some_and(|action| matches!(action, crate::app::ContextAction::Play))
        }));
    }

    fn date_added_iso(secs: i64) -> String {
        use time::format_description::well_known::Iso8601;
        time::OffsetDateTime::from_unix_timestamp(secs)
            .expect("in-range timestamp")
            .format(&Iso8601::DEFAULT)
            .expect("ISO-8601 formatting always succeeds")
    }

    /// Row 4.4 (`home-recently-added-tree`): `push_home_content` sends the
    /// merged `Recently added` projection to the Home owner — Emby and
    /// Audiobookshelf fetch events plus feed entries, interleaved newest
    /// first by provider date, with feed entry names resolved from the
    /// subscriptions — where a fetched item older than the 14-day cutoff
    /// contributes nothing and the launch window rides along.
    #[test]
    fn home_pushes_merged_recently_added_rows_newest_first_inside_14_days() {
        let mut model = Model::new(make_app_stub());
        let current = model.app.home_latest_launch_window.current;
        let day = 24 * 60 * 60;

        let mut movie = make_item("Movie", "Movie");
        movie.id = "movie".into();
        movie.date_added = date_added_iso(i64::try_from(current - 3 * day).unwrap());
        let mut old_movie = make_item("OldMovie", "Movie");
        old_movie.id = "movie-old".into();
        old_movie.date_added = date_added_iso(i64::try_from(current - 20 * day).unwrap());
        let episode = mbv_queue::AudiobookshelfQueueItem {
            library_item_id: "pod-lib".into(),
            episode_id: "episode".into(),
            title: "Episode".into(),
            show_title: None,
            author: None,
            description: None,
            duration_ticks: None,
            position_ticks: 0,
            played: false,
            pub_date_secs: Some(current - 2 * day),
            is_finished: false,
            cover_path: None,
        };
        model.accept_home_recently_added(
            crate::app::DestinationLatestSource::Emby("lib".into()),
            model.app.emby_runtime.generation(),
            vec![
                QueueItem::Emby(Box::new(movie)),
                QueueItem::Emby(Box::new(old_movie)),
            ],
        );
        model.accept_home_recently_added(
            crate::app::DestinationLatestSource::Audiobookshelf("pod-lib".into()),
            model.app.audiobookshelf_runtime.generation(),
            vec![QueueItem::Audiobookshelf(
                mbv_queue::AudiobookshelfItem::Episode(episode),
            )],
        );
        let url = "https://feeds.test/latest.xml".to_string();
        model.app.feed_tab.subscriptions = vec![mbv_config::FeedSubscription {
            name: "Feedly".into(),
            url: url.clone(),
            kind: mbv_queue::FeedKind::Video,
        }];
        model.app.feed_tab.all_entries = vec![mbv_queue::FeedEntry {
            guid: "entry".into(),
            title: "Entry".into(),
            enclosure_url: None,
            link: None,
            mime_type: None,
            duration_ticks: None,
            pub_date_secs: Some(current - day),
            feed_kind: Some(mbv_queue::FeedKind::Video),
            feed_id: Some(url),
            position_ticks: 0,
            played: false,
        }];

        model.push_home_content();

        let owner = model
            .home_owner_shared()
            .expect("Home destination owner mounted");
        let titles: Vec<String> = owner
            .recent_rows()
            .iter()
            .map(|row| match &row.item {
                QueueItem::Emby(item) => item.name.clone(),
                QueueItem::Audiobookshelf(mbv_queue::AudiobookshelfItem::Episode(episode)) => {
                    episode.title.clone()
                }
                QueueItem::Audiobookshelf(mbv_queue::AudiobookshelfItem::Book(book)) => {
                    book.title.clone()
                }
                QueueItem::Feed(entry) => entry.title.clone(),
            })
            .collect();
        assert_eq!(titles, vec!["Entry", "Episode", "Movie"]);
        assert_eq!(
            owner.recent_rows()[0].feed_name.as_deref(),
            Some("Feedly"),
            "feed rows carry the subscription display name"
        );
        let feeds_named_rows = owner
            .recent_rows()
            .iter()
            .filter(|row| row.feed_name.is_some())
            .count();
        assert_eq!(feeds_named_rows, 1, "Service rows carry no feed name");
        assert!(
            !titles.contains(&"OldMovie".to_string()),
            "fetched row older than the 14-day cutoff is absent"
        );
        assert_eq!(owner.launch_window(), model.app.home_latest_launch_window);
    }
}
