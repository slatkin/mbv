//! The TV root fetch tests (the latest fetch, the upcoming fetch retries, and
//! the pill fetch's stale key).

use super::*;

#[cfg(test)]
mod tv_latest_tests {
    use super::*;
    use crate::app::LibraryTab;
    use rstest::rstest;
    use std::cell::RefCell;

    pub(in crate::app) struct FakeLatestSource {
        pub(in crate::app) request: RefCell<Option<(String, usize)>>,
        pub(in crate::app) items: Vec<EmbyItem>,
    }

    impl TvLatestSource for FakeLatestSource {
        fn get_latest_episodes(
            &self,
            view_id: &str,
            limit: usize,
        ) -> Result<Vec<EmbyItem>, mbv_emby::EmbyError> {
            *self.request.borrow_mut() = Some((view_id.into(), limit));
            Ok(self.items.clone())
        }
    }

    #[test]
    fn latest_library_fetch_uses_home_feed_request_without_home_state() {
        let mut episode = mbv_emby_model::test_support::make_item("Feed episode", "Episode");
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

    #[rstest]
    #[case::latest(mbv_queue::TvContentMode::Latest)]
    fn refresh_after_stop_reloads_selected_tv_mode(#[case] mode: mbv_queue::TvContentMode) {
        let mut app = crate::app::tests::make_app_stub();
        let config = crate::app::config::Config {
            server_url: "http://127.0.0.1:1".into(),
            ..crate::app::config::Config::default()
        };
        let http = mbv_net::mock_http::MockHttp::new();
        let client = mbv_emby::EmbyClient::new(config).with_test_agent(http.agent());
        app.emby_runtime = crate::app::state::service_runtime::EmbyRuntime::ready(
            std::sync::Arc::new(std::sync::Mutex::new(client)),
        );

        let mut library = mbv_emby_model::test_support::make_item("Shows", "CollectionFolder");
        library.id = "tv-library".into();
        library.collection_type = "tvshows".into();
        let mut level_item = mbv_emby_model::test_support::make_item("Old episode", "Episode");
        level_item.id = "old-episode".into();
        app.libs.push(LibraryTab {
            library,
            nav_stack: vec![BrowseLevel {
                parent_id: "tv-library".into(),
                title: "Shows".into(),
                items: vec![level_item],
                rows: ServerRows::complete(1),
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
            ..LibraryTab::new(mbv_emby_model::test_support::make_item(
                "unused",
                "CollectionFolder",
            ))
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
            let event = app
                .channels
                .lib_rx
                .recv()
                .expect("selected TV refresh must complete");
            if let event @ LibEvent::Browse(
                BrowseEvent::Loaded { .. } | BrowseEvent::Refreshed { .. },
            ) = event
            {
                break event;
            }
        };
        match event {
            LibEvent::Browse(BrowseEvent::Loaded { level, .. }) => {
                assert_eq!(level.item_types.as_deref(), Some("Episode"));
                assert_eq!(level.items[0].id, "new-episode");
            }
            LibEvent::Browse(BrowseEvent::Refreshed { .. }) => {
                panic!("Latest/Upcoming stop refresh must not use the generic ranged fetch")
            }
            _ => unreachable!(),
        }
    }
}

#[cfg(test)]
mod tv_upcoming_retry_tests {
    use super::*;
    use std::cell::Cell;
    use std::collections::VecDeque;
    use std::time::Duration;

    struct FakeUpcomingSource {
        results: std::cell::RefCell<VecDeque<Result<Vec<UpcomingPremiere>, mbv_emby::EmbyError>>>,
        attempts: Cell<usize>,
    }

    impl TvUpcomingSource for FakeUpcomingSource {
        fn get_upcoming(
            &self,
            _parent_id: &str,
            _limit: usize,
        ) -> Result<Vec<UpcomingPremiere>, mbv_emby::EmbyError> {
            self.attempts.set(self.attempts.get() + 1);
            self.results
                .borrow_mut()
                .pop_front()
                .expect("every retry test scripts one result per attempt")
        }
    }

    fn one_premiere() -> Vec<UpcomingPremiere> {
        vec![UpcomingPremiere {
            series_id: mbv_ids::ItemId::new("series-1"),
            series_name: "Series 1".into(),
            season: Some(1),
            episode: Some(1),
            title: "Premiere".into(),
            premiere_date: "2026-10-12T00:00:00.0000000Z".into(),
        }]
    }

    /// The eager Upcoming fetch retries on failure so a server-side metadata
    /// refresh blocking one attempt (2026-10-15: 30063 ms server log) does
    /// not leave the session snapshot empty; zero-length delays keep the
    /// test sleep-free.
    #[test]
    fn upcoming_fetch_retries_a_failed_attempt() {
        let source = FakeUpcomingSource {
            results: std::cell::RefCell::new(VecDeque::from([Err(error()), Ok(one_premiere())])),
            attempts: Cell::new(0),
        };

        let items = fetch_upcoming_with_retries(&source, "lib-1", &[Duration::ZERO])
            .expect("the second attempt succeeds");

        assert_eq!(items.len(), 1);
        assert_eq!(source.attempts.get(), 2);
    }

    #[test]
    fn upcoming_fetch_stops_after_the_final_attempt() {
        let source = FakeUpcomingSource {
            results: std::cell::RefCell::new(VecDeque::from([Err(error())])),
            attempts: Cell::new(0),
        };

        let Err(error) = fetch_upcoming_with_retries(&source, "lib-1", &[]) else {
            panic!("an exhausted retry must return the final error");
        };
        assert_eq!(
            source.attempts.get(),
            1,
            "an empty delay slice is one attempt"
        );
        assert_eq!(error.kind_name(), "emby.resolve");
    }

    fn error() -> mbv_emby::EmbyError {
        mbv_emby::EmbyError::resolve("test failure")
    }
}

#[cfg(test)]
mod tv_pill_stale_key_tests {
    use super::tv_latest_tests::FakeLatestSource;
    use super::*;
    use crate::app::LibraryTab;
    use std::cell::RefCell;

    /// Regression (2026-10-06): a TV root armed for a pill fetch kept its
    /// legacy `SortName` fetch key, so `drop_stale_root_completion` discarded
    /// the pill's own `DateCreated`/`PremiereDate` response and re-issued a
    /// generic alphabetical all-episodes fetch that replaced the Latest and
    /// Upcoming pill content with old episodes.
    #[test]
    fn pill_response_arriving_after_pill_spawn_is_not_dropped_as_stale() {
        let mut app = crate::app::tests::make_app_stub();
        let config = crate::app::config::Config {
            server_url: "http://127.0.0.1:1".into(),
            ..crate::app::config::Config::default()
        };
        let http = mbv_net::mock_http::MockHttp::new();
        let client = mbv_emby::EmbyClient::new(config).with_test_agent(http.agent());
        app.emby_runtime = crate::app::state::service_runtime::EmbyRuntime::ready(
            std::sync::Arc::new(std::sync::Mutex::new(client)),
        );

        let mut library = mbv_emby_model::test_support::make_item("Shows", "CollectionFolder");
        library.id = "tv-library".into();
        library.collection_type = "tvshows".into();
        app.libs.push(LibraryTab {
            library,
            nav_stack: vec![BrowseLevel {
                parent_id: "tv-library".into(),
                title: "Shows".into(),
                items: vec![],
                rows: ServerRows::new(0),
                resting: BrowseResting::new(0, 0),
                item_types: Some("Episode".into()),
                unplayed_only: false,
                sort_by: "SortName".into(),
                sort_order: "Ascending".into(),
                loading: true,
                all_items: None,
                letter_filter: None,
                tv_content_mode: Some(mbv_queue::TvContentMode::Latest),
                music_grouping: None,
            }],
            tv_content_mode: Some(mbv_queue::TvContentMode::Latest),
            // A captured total keeps the first-capture default-pill refetch
            // out of this stale-key contract.
            library_total: Some(1),
            ..LibraryTab::new(mbv_emby_model::test_support::make_item(
                "unused",
                "CollectionFolder",
            ))
        });

        // Arming the pill fetch aligns the root level's fetch fields with the
        // fields the pill's response carries.
        http.respond(200, r#"{"Items":[],"TotalRecordCount":0}"#);
        app.spawn_tv_latest(0, "tv-library".into(), "TV".into());
        let level = &app.libs[0].nav_stack[0];
        assert_eq!(level.sort_by, "DateCreated");
        assert_eq!(level.sort_order, "Descending");

        // A pill response — here a duplicate of the spawn's own completion,
        // as happens when two pill fetches race — must apply, not be dropped
        // and replaced by a generic refetch.
        let mut episode = mbv_emby_model::test_support::make_item("New episode", "Episode");
        episode.id = "new-episode".into();
        let source = FakeLatestSource {
            request: RefCell::new(None),
            items: vec![episode.clone()],
        };
        let level = build_tv_latest_level(&source, "tv-library".into(), "TV".into())
            .expect("fake Latest request succeeds");
        app.handle_lib_event(
            LibEvent::Browse(BrowseEvent::Loaded {
                lib_idx: 0,
                parent_id: "tv-library".into(),
                level: Box::new(level),
            }),
            None,
        );

        let level = &app.libs[0].nav_stack[0];
        assert_eq!(level.items, vec![episode]);
        assert!(!level.loading);
    }
}
