//! Home Continue Watching sync and typed effects for the shell `Model`.

use super::Model;
use mbv_components::home_content::HomeContent;
use mbv_queue::QueueItem;
use mbv_ui_model::library::LibraryKey;
use mbv_ui_model::targets::HomeItemTarget;
use mbv_ui_msg::HomeRequest;

impl Model {
    pub(in crate::app) fn handle_home_request(&mut self, request: HomeRequest) {
        match request {
            HomeRequest::Play(target) => self.home_play_request(&target),
            HomeRequest::Enqueue(targets) => self.home_enqueue_request(targets),
            HomeRequest::ContextMenu(targets, anchor) => {
                self.home_context_menu_request(targets, anchor);
            }
            HomeRequest::Delete(targets) => self.home_delete_request(targets),
            HomeRequest::ToggleWatched(targets) => self.home_toggle_watched_request(targets),
        }
    }

    fn home_play_request(&mut self, target: &HomeItemTarget) {
        if let Some(item) = self.home_stable_target(target) {
            self.app.home_play_target(item, target.is_continue());
        }
    }

    fn home_enqueue_request(&mut self, targets: Vec<HomeItemTarget>) {
        for target in targets {
            if let Some(item) = self.home_stable_target(&target) {
                self.app.home_enqueue_target(item, target.is_continue());
            }
        }
    }

    fn home_context_menu_request(
        &mut self,
        targets: Vec<HomeItemTarget>,
        anchor: Option<(u16, u16)>,
    ) {
        let origin = mbv_ui_model::media_list::SelectionOrigin::Library(
            mbv_ui_model::media_list::LibrarySelectionOrigin::Home,
        );
        self.context_menu_origin = Some(origin.clone());
        self.context_action_snapshot = Some(mbv_ui_model::context_menu::ContextActionSnapshot {
            origin,
            values: vec![mbv_ui_model::context_menu::ContextMenuTargets::Home(
                targets.clone(),
            )],
        });
        if targets.len() > 1 {
            self.home_multi_context_menu(&targets, anchor);
        } else {
            self.home_single_context_menu(targets.into_iter().next(), anchor);
        }
    }

    /// The multi-selection Home context menu (`openspec/specs/home-latest-sections`): an all-Emby selection — Continue rows,
    /// `Recently added` rows, or both — keeps the Emby multi menu, with
    /// Continue rows the only Remove targets (Remove is Continue-only);
    /// any non-Emby row makes the selection span Services, and that menu
    /// offers Play and Add to Queue only. An unresolved target contributes
    /// nothing, as in enqueue.
    fn home_multi_context_menu(&mut self, targets: &[HomeItemTarget], anchor: Option<(u16, u16)>) {
        let resolved: Vec<(QueueItem, bool)> = targets
            .iter()
            .filter_map(|target| Some((self.home_stable_target(target)?, target.is_continue())))
            .collect();
        if resolved
            .iter()
            .any(|(item, _)| !matches!(item, QueueItem::Emby(_)))
        {
            let mixed_items = resolved.into_iter().map(|(item, _)| item).collect();
            self.app.open_home_mixed_context_menu(mixed_items, anchor);
            return;
        }
        let removes: Vec<mbv_ui_model::context_menu::BulkRemoveTarget> = resolved
            .iter()
            .filter_map(|(item, from_cw)| match item {
                QueueItem::Emby(item) if *from_cw => Some(
                    mbv_ui_model::context_menu::BulkRemoveTarget::ContinueWatching(item.clone()),
                ),
                _ => None,
            })
            .collect();
        let items: Vec<mbv_emby_model::EmbyItem> = resolved
            .into_iter()
            .filter_map(|(item, _)| match item {
                QueueItem::Emby(item) => Some(*item),
                _ => None,
            })
            .collect();
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
    }

    /// The single-row Home context menu (`openspec/specs/home-latest-sections`): a Continue row keeps today's Continue
    /// Watching menu; a `Recently added` row opens its Service's destination
    /// menu — the same entries the row offers on that Service's Latest list.
    /// An absent or unresolved target opens nothing.
    fn home_single_context_menu(
        &mut self,
        target: Option<HomeItemTarget>,
        anchor: Option<(u16, u16)>,
    ) {
        let Some(target) = target else {
            return;
        };
        let Some(item) = self.home_stable_target(&target) else {
            return;
        };
        if target.is_continue() {
            // A Continue row is always Emby content (`openspec/specs/home-latest-sections`).
            let item = match item {
                QueueItem::Emby(item) => Some(*item),
                QueueItem::Feed(_) | QueueItem::Audiobookshelf(_) => None,
            };
            self.home_context_item.clone_from(&item);
            if let Some((x, y)) = anchor {
                self.app.open_context_menu_at(x, y, true, item);
            } else {
                self.app.open_context_menu(true, item);
            }
            return;
        }
        match item {
            QueueItem::Emby(item) => {
                // Bare `Play` and `Add to Queue` execute through the shell's
                // `home_context_item` (no tab-dependent re-read), so the
                // resolved row is pinned there before the menu opens.
                self.home_context_item = Some(item.as_ref().clone());
                self.app
                    .open_explicit_item_context_menu(item.as_ref(), anchor);
            }
            QueueItem::Feed(entry) => {
                self.app.open_feeds_context_menu(vec![entry], anchor);
            }
            QueueItem::Audiobookshelf(item) => {
                self.app.open_audiobookshelf_context_menu(
                    vec![mbv_ui_model::context_menu::AudiobookshelfMenuTarget::for_item(&item)],
                    anchor,
                );
            }
        }
    }

    fn home_delete_request(&mut self, targets: Vec<HomeItemTarget>) {
        for target in targets {
            match target {
                HomeItemTarget::Continue(_) => {
                    if let Some(QueueItem::Emby(item)) = self.home_stable_target(&target) {
                        self.app.remove_from_continue_watching(&item);
                    }
                }
                // `openspec/specs/home-latest-sections`: Delete is a
                // documented no-op on a `Recently added` row; it never
                // removes from Continue Watching.
                HomeItemTarget::Recent(_) => {}
            }
        }
    }

    fn home_toggle_watched_request(&mut self, targets: Vec<HomeItemTarget>) {
        for target in targets {
            match target {
                HomeItemTarget::Continue(_) => {
                    if let Some(QueueItem::Emby(item)) = self.home_stable_target(&target) {
                        self.app.cw_toggle_watched(&item);
                    }
                }
                HomeItemTarget::Recent(_) => {
                    if let Some(item) = self.home_stable_target(&target) {
                        self.app.toggle_recent_item_played_state(&item);
                    }
                }
            }
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
        let now_secs = mbv_render::screens::feeds_model::current_time_secs();
        self.update_home_owner(|home| {
            home.set_content(continue_items, loading);
            home.set_recent_rows(recent_rows, now_secs);
        });
    }

    /// The merged, newest-first `Recently added` rows, built at push time
    /// from the per-source store plus Feeds (`openspec/specs/home-latest-sections`). Feeds has no store entry: its rows derive
    /// from `all_entries`, filtered to the same 14-day cutoff, with the
    /// subscription name resolved by the shell. A merged row without a valid
    /// date has no age branch, so it can never appear.
    pub(in crate::app) fn home_recent_rows(&self) -> Vec<mbv_ui_model::home_latest::HomeRecentRow> {
        use mbv_ui_model::home_latest::{HomeRecentRow, home_age_group, provider_timestamp_secs};
        use std::cmp::Reverse;
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
                .filter(|entry| home_age_group(entry.pub_date_secs, window.current).is_some())
                .map(|entry| {
                    let feed_name = entry.feed_id.as_deref().and_then(|feed_id| {
                        self.app
                            .feed_tab
                            .subscriptions
                            .iter()
                            .find(|subscription| subscription.url == feed_id)
                            .map(|subscription| subscription.name.clone())
                    });
                    HomeRecentRow {
                        item: QueueItem::Feed(entry.clone()),
                        feed_name,
                    }
                }),
        );
        rows.sort_by_cached_key(|row| Reverse(provider_timestamp_secs(&row.item)));
        rows
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::tests::{make_app_stub, make_items};
    use mbv_emby_model::test_support::make_item;

    fn target(id: &str) -> HomeItemTarget {
        HomeItemTarget::continue_item(id)
    }

    fn podcast_episode(episode_id: &str, pub_date_secs: u64) -> mbv_queue::AudiobookshelfQueueItem {
        mbv_queue::AudiobookshelfQueueItem {
            library_item_id: "pod-lib".into(),
            episode_id: episode_id.into(),
            title: "Episode".into(),
            show_title: None,
            author: None,
            description: None,
            duration_ticks: None,
            position_ticks: 0,
            played: false,
            pub_date_secs: Some(pub_date_secs),
            is_finished: false,
            cover_path: None,
        }
    }

    fn recent_movie(id: &str, date_added: String) -> mbv_emby_model::EmbyItem {
        let mut item = make_item("Movie", "Movie");
        item.id = id.into();
        item.date_added = date_added;
        item
    }

    fn feed_entry(guid: &str, pub_date_secs: u64) -> mbv_queue::FeedEntry {
        mbv_queue::FeedEntry {
            guid: guid.into(),
            title: "Entry".into(),
            enclosure_url: None,
            link: None,
            mime_type: None,
            duration_ticks: None,
            pub_date_secs: Some(pub_date_secs),
            feed_kind: None,
            feed_id: None,
            position_ticks: 0,
            played: false,
        }
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

        model.handle_home_request(HomeRequest::Enqueue(vec![target("id2")]));
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
        model.handle_home_request(HomeRequest::Play(target("id0")));
        assert_eq!(model.app.status, "Emby is unavailable");
    }

    #[test]
    fn continue_watching_context_menu_uses_explicit_target() {
        let mut model = Model::new(make_app_stub());
        let item = make_item("cw-target", "Movie");
        model.home_content.continue_items = vec![item.clone()];
        model.handle_home_request(HomeRequest::ContextMenu(vec![target(&item.id)], None));
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

    /// (`openspec/specs/home-latest-sections`) `push_home_content` sends the
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
        let episode = podcast_episode("episode", current - 2 * day);
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
    }

    /// Spec scenario "Podcast row context menu" (`home-latest-sections`): a
    /// single `Recently added` podcast row opens the same menu that
    /// episode's row offers on its podcast library's Latest list — Play,
    /// Add to Queue, then the one mark entry the cached progress state
    /// chooses (a missing entry counts as unfinished, so Mark Played).
    #[test]
    fn single_recent_podcast_row_opens_the_audiobookshelf_destination_menu() {
        let mut model = Model::new(make_app_stub());
        let current = model.app.home_latest_launch_window.current;
        let episode = podcast_episode("episode", current - 24 * 60 * 60);
        model.accept_home_recently_added(
            crate::app::DestinationLatestSource::Audiobookshelf("pod-lib".into()),
            model.app.audiobookshelf_runtime.generation(),
            vec![QueueItem::Audiobookshelf(
                mbv_queue::AudiobookshelfItem::Episode(episode.clone()),
            )],
        );

        model.handle_home_request(HomeRequest::ContextMenu(
            vec![HomeItemTarget::Recent(episode.content_id())],
            None,
        ));

        let Some(mbv_ui_model::overlay::OverlayRequest::ContextMenu(menu)) =
            model.app.pending_overlay
        else {
            panic!("a single Recently added podcast row must open its destination menu");
        };
        let labels: Vec<&str> = menu.entries.iter().map(|entry| entry.label).collect();
        assert_eq!(labels, vec!["Play", "Add to Queue", "Mark Played"]);
    }

    /// Spec scenario "Mixed-Service multi-selection" (`home-latest-sections`):
    /// a multi-selection holding an Emby movie and a feed entry offers Play
    /// and Add to Queue only, through the mixed-Service selection actions
    /// that carry the resolved queue items.
    #[test]
    fn mixed_service_selection_menu_offers_play_and_add_to_queue_only() {
        let mut model = Model::new(make_app_stub());
        let current = model.app.home_latest_launch_window.current;
        let day = 24 * 60 * 60;
        model.accept_home_recently_added(
            crate::app::DestinationLatestSource::Emby("lib".into()),
            model.app.emby_runtime.generation(),
            vec![QueueItem::Emby(Box::new(recent_movie(
                "movie",
                date_added_iso(i64::try_from(current - 3 * day).unwrap()),
            )))],
        );
        model.app.feed_tab.all_entries = vec![feed_entry("entry", current - day)];

        model.handle_home_request(HomeRequest::ContextMenu(
            vec![
                HomeItemTarget::Recent(mbv_queue::QueueItemContentId::Emby("movie".into())),
                HomeItemTarget::Recent(mbv_queue::QueueItemContentId::Feed("entry".into())),
            ],
            None,
        ));

        let Some(mbv_ui_model::overlay::OverlayRequest::ContextMenu(menu)) =
            model.app.pending_overlay
        else {
            panic!("a mixed-Service selection must open its menu");
        };
        let offered: Vec<(&str, bool)> = menu
            .entries
            .iter()
            .map(|entry| {
                (
                    entry.label,
                    matches!(
                        entry.action,
                        Some(
                            crate::app::ContextAction::HomePlaySelection(_)
                                | crate::app::ContextAction::HomeEnqueueSelection(_)
                        )
                    ),
                )
            })
            .collect();
        assert_eq!(
            offered,
            vec![("Play", true), ("Add to Queue", true)],
            "a selection spanning Services offers Play and Add to Queue only"
        );
    }
}
