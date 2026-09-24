use crate::app::state::types::browse::BrowseResting;
use crate::app::{App, BrowseLevel, LibEvent};
use mbv_core::api::{EmbyClient, EmbyItem};

type BrowseRefresh = (
    usize,
    String,
    Option<String>,
    bool,
    String,
    String,
    usize,
    Option<crate::app::render::LetterFilter>,
    Option<mbv_core::config::TvContentMode>,
);
trait TvLatestSource {
    fn get_latest_episodes(&self, view_id: &str, limit: usize) -> Result<Vec<EmbyItem>, String>;
}

impl TvLatestSource for EmbyClient {
    fn get_latest_episodes(&self, view_id: &str, limit: usize) -> Result<Vec<EmbyItem>, String> {
        EmbyClient::get_latest_episodes(self, view_id, limit)
    }
}

trait TvUpcomingSource {
    fn get_upcoming(&self, parent_id: &str, limit: usize) -> Result<Vec<EmbyItem>, String>;
}

impl TvUpcomingSource for EmbyClient {
    fn get_upcoming(&self, parent_id: &str, limit: usize) -> Result<Vec<EmbyItem>, String> {
        EmbyClient::get_upcoming(self, parent_id, limit)
    }
}

fn build_tv_latest_level<S: TvLatestSource>(
    source: &S,
    parent_id: String,
    title: String,
) -> Result<BrowseLevel, String> {
    let items = source.get_latest_episodes(&parent_id, 30)?;
    let total_count = items.len();
    Ok(BrowseLevel {
        parent_id,
        title,
        items,
        fetched_rows: total_count,
        total_count,
        resting: BrowseResting::new(0, 0),
        item_types: Some("Episode".into()),
        unplayed_only: false,
        sort_by: "DateCreated".into(),
        sort_order: "Descending".into(),
        loading: false,
        all_items: None,
        letter_filter: None,
        tv_content_mode: None,
        music_grouping: None,
    })
}

fn build_tv_upcoming_level<S: TvUpcomingSource>(
    source: &S,
    parent_id: String,
    title: String,
) -> Result<BrowseLevel, String> {
    let items = source.get_upcoming(&parent_id, 30)?;
    let total_count = items.len();
    Ok(BrowseLevel {
        parent_id,
        title,
        items,
        fetched_rows: total_count,
        total_count,
        resting: BrowseResting::new(0, 0),
        item_types: Some("Episode".into()),
        unplayed_only: false,
        sort_by: "PremiereDate".into(),
        sort_order: "Ascending".into(),
        loading: false,
        all_items: None,
        letter_filter: None,
        tv_content_mode: None,
        music_grouping: None,
    })
}

impl App {
    pub(in crate::app) fn refresh_after_stop(&mut self) {
        if let Ok(content) = self.fetch_home() {
            // The fetch runs synchronously (order-sensitive side
            // effects); the computed content travels to Model-owned
            // `home_content` via lib_tx (task 5.3d).
            let _ = self
                .lib_tx
                .send(LibEvent::HomeContentRefreshed(Box::new(content)));
        }
        if self.last_played_completed {
            if let Some(ref item_id) = self.last_played_item_id.clone() {
                for lib_idx in 0..self.libs.len() {
                    if self.is_feed_home_video_group_view(lib_idx)
                        || self.is_feed_home_video_library(lib_idx)
                    {
                        self.remove_item_from_feed_home_video_cache(lib_idx, item_id);
                        if let Some(state) = self.libs[lib_idx].feed_home_video.as_mut() {
                            state.loading = true;
                        }
                        self.log_feed_home_video_state(lib_idx, "refresh_after_stop_completed");
                    }
                }
            }
        }
        let fetches: Vec<BrowseRefresh> = self
            .libs
            .iter()
            .enumerate()
            .filter_map(|(i, lib)| {
                lib.nav_stack.last().map(|lvl| {
                    (
                        i,
                        lvl.parent_id.clone(),
                        lvl.item_types.clone(),
                        lvl.unplayed_only,
                        lvl.sort_by.clone(),
                        lvl.sort_order.clone(),
                        lvl.items.len(),
                        lvl.letter_filter.clone(),
                        (lib.library.collection_type == "tvshows" && lib.nav_stack.len() == 1)
                            .then(|| {
                                lvl.tv_content_mode
                                    .clone()
                                    .or_else(|| lib.tv_content_mode.clone())
                            })
                            .flatten(),
                    )
                })
            })
            .collect();
        for (
            lib_idx,
            parent_id,
            item_types,
            unplayed_only,
            sort_by,
            sort_order,
            loaded_count,
            letter_filter,
            tv_content_mode,
        ) in fetches
        {
            match tv_content_mode {
                Some(mbv_core::config::TvContentMode::Latest) => {
                    self.spawn_tv_latest(
                        lib_idx,
                        parent_id,
                        self.libs[lib_idx].library.name.clone(),
                    );
                }
                Some(mbv_core::config::TvContentMode::Upcoming) => {
                    self.spawn_tv_upcoming(
                        lib_idx,
                        parent_id,
                        self.libs[lib_idx].library.name.clone(),
                    );
                }
                Some(mbv_core::config::TvContentMode::All)
                | Some(mbv_core::config::TvContentMode::Range(_))
                | None => self.spawn_refresh(
                    lib_idx,
                    parent_id,
                    item_types,
                    unplayed_only,
                    sort_by,
                    sort_order,
                    loaded_count,
                    letter_filter,
                ),
            }
        }
    }

    fn spawn_tv_content<F>(&self, lib_idx: usize, parent_id: String, title: String, build: F)
    where
        F: FnOnce(&EmbyClient, String, String) -> Result<BrowseLevel, String> + Send + 'static,
    {
        let Some(client) = self.emby_snapshot() else {
            return;
        };
        let tx = self.lib_tx.clone();
        std::thread::spawn(move || match build(&client, parent_id.clone(), title) {
            Ok(level) => {
                let _ = tx.send(LibEvent::Loaded {
                    lib_idx,
                    parent_id,
                    level: Box::new(level),
                });
            }
            Err(e) => {
                let _ = tx.send(LibEvent::Error(e));
            }
        });
    }

    pub(in crate::app) fn spawn_tv_latest(&self, lib_idx: usize, parent_id: String, title: String) {
        self.spawn_tv_content(
            lib_idx,
            parent_id,
            title,
            build_tv_latest_level::<EmbyClient>,
        );
    }

    pub(in crate::app) fn spawn_destination_latest_snapshot(&self, lib_idx: usize) {
        let Some(lib) = self.libs.get(lib_idx) else {
            return;
        };
        if matches!(
            lib.library.collection_type.as_str(),
            "tvshows" | "playlists" | "music"
        ) {
            return;
        }
        self.spawn_emby_latest_snapshot(lib.library.id.clone(), lib.library.name.clone());
    }

    pub(in crate::app) fn spawn_emby_latest_snapshot(&self, library_id: String, title: String) {
        let Some(client) = self.emby_snapshot() else {
            return;
        };
        let tx = self.lib_tx.clone();
        std::thread::spawn(move || {
            let items = client.get_latest(&library_id, 30);
            match items {
                Ok(items) => {
                    let _ = tx.send(LibEvent::EmbyLatestSnapshotFetched {
                        library_id,
                        title,
                        items,
                    });
                }
                Err(error) => {
                    let _ = tx.send(LibEvent::Error(error));
                }
            }
        });
    }

    pub(in crate::app) fn spawn_tv_upcoming(
        &self,
        lib_idx: usize,
        parent_id: String,
        title: String,
    ) {
        self.spawn_tv_content(
            lib_idx,
            parent_id,
            title,
            build_tv_upcoming_level::<EmbyClient>,
        );
    }
}

#[cfg(test)]
mod tv_latest_tests {
    use super::*;
    use crate::app::LibraryTab;
    use rstest::rstest;
    use std::cell::RefCell;

    struct FakeLatestSource {
        request: RefCell<Option<(String, usize)>>,
        items: Vec<EmbyItem>,
    }

    impl TvLatestSource for FakeLatestSource {
        fn get_latest_episodes(
            &self,
            view_id: &str,
            limit: usize,
        ) -> Result<Vec<EmbyItem>, String> {
            *self.request.borrow_mut() = Some((view_id.into(), limit));
            Ok(self.items.clone())
        }
    }

    struct FakeUpcomingSource {
        request: RefCell<Option<(String, usize)>>,
        items: Vec<EmbyItem>,
    }

    impl TvUpcomingSource for FakeUpcomingSource {
        fn get_upcoming(&self, parent_id: &str, limit: usize) -> Result<Vec<EmbyItem>, String> {
            *self.request.borrow_mut() = Some((parent_id.into(), limit));
            Ok(self.items.clone())
        }
    }

    #[test]
    fn latest_library_fetch_uses_home_feed_request_without_home_state() {
        let mut episode = crate::app::tests::make_item("Feed episode", "Episode");
        episode.id = "feed-episode".into();
        let source = FakeLatestSource {
            request: RefCell::new(None),
            items: vec![episode.clone()],
        };

        let level = build_tv_latest_level(&source, "view-id".into(), "TV".into())
            .expect("fake Latest request succeeds");

        assert_eq!(
            source.request.borrow().as_ref(),
            Some(&("view-id".into(), 30))
        );
        assert_eq!(level.items, vec![episode]);
        assert_eq!(level.item_types.as_deref(), Some("Episode"));
    }

    #[test]
    fn upcoming_library_fetch_uses_library_parent_and_flat_episode_rows() {
        let mut episode = crate::app::tests::make_item("Upcoming episode", "Episode");
        episode.id = "upcoming-episode".into();
        let source = FakeUpcomingSource {
            request: RefCell::new(None),
            items: vec![episode.clone()],
        };

        let level = build_tv_upcoming_level(&source, "library-id".into(), "TV".into())
            .expect("fake Upcoming request succeeds");

        assert_eq!(
            source.request.borrow().as_ref(),
            Some(&("library-id".into(), 30))
        );
        assert_eq!(level.items, vec![episode]);
        assert_eq!(level.item_types.as_deref(), Some("Episode"));
    }

    #[rstest]
    #[case::latest(mbv_core::config::TvContentMode::Latest)]
    #[case::upcoming(mbv_core::config::TvContentMode::Upcoming)]
    fn refresh_after_stop_reloads_selected_tv_mode(#[case] mode: mbv_core::config::TvContentMode) {
        let mut app = crate::app::tests::make_app_stub();
        let config = crate::config::Config {
            server_url: "http://127.0.0.1:1".into(),
            ..crate::config::Config::default()
        };
        let http = mbv_core::mock_http::MockHttp::new();
        let client = mbv_core::api::EmbyClient::new(config).with_test_agent(http.agent());
        app.emby_runtime = mbv_core::service_runtime::EmbyRuntime::ready(std::sync::Arc::new(
            std::sync::Mutex::new(client),
        ));

        let mut library = crate::app::tests::make_item("Shows", "CollectionFolder");
        library.id = "tv-library".into();
        library.collection_type = "tvshows".into();
        let mut level_item = crate::app::tests::make_item("Old episode", "Episode");
        level_item.id = "old-episode".into();
        app.libs.push(LibraryTab {
            library,
            nav_stack: vec![BrowseLevel {
                parent_id: "tv-library".into(),
                title: "Shows".into(),
                items: vec![level_item],
                fetched_rows: 1,
                total_count: 1,
                resting: BrowseResting::new(0, 0),
                item_types: Some("Episode".into()),
                unplayed_only: false,
                sort_by: "SortName".into(),
                sort_order: "Ascending".into(),
                loading: false,
                all_items: None,
                letter_filter: None,
                tv_content_mode: Some(mode.clone()),
                music_grouping: None,
            }],
            tv_content_mode: Some(mode),
            ..LibraryTab::new(crate::app::tests::make_item("unused", "CollectionFolder"))
        });

        // fetch_home makes three requests; the fourth response is the selected
        // TV mode's request.
        http.respond(
            200,
            r#"[{"ItemId":"tv-library","Name":"Shows","CollectionType":"tvshows"}]"#,
        );
        http.respond(200, r#"{"Items":[]}"#);
        http.respond(200, r#"{"Items":[]}"#);
        http.respond(
            200,
            r#"{"Items":[{"Id":"new-episode","Name":"New episode","Type":"Episode"}],"TotalRecordCount":1}"#,
        );

        app.refresh_after_stop();
        let event = loop {
            match app
                .lib_rx
                .recv()
                .expect("selected TV refresh must complete")
            {
                event @ LibEvent::Loaded { .. } | event @ LibEvent::Refreshed { .. } => {
                    break event
                }
                _ => continue,
            }
        };
        match event {
            LibEvent::Loaded { level, .. } => {
                assert_eq!(level.item_types.as_deref(), Some("Episode"));
                assert_eq!(level.items[0].id, "new-episode");
            }
            LibEvent::Refreshed { .. } => {
                panic!("Latest/Upcoming stop refresh must not use the generic ranged fetch")
            }
            _ => unreachable!(),
        }
    }
}
