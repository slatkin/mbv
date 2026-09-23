use ratatui::backend::TestBackend;
use ratatui::Terminal;
use rstest::rstest;
use tuirealm::application::PollStrategy;
use tuirealm::event::{
    Event, Key, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};

use crate::app::components::msg::TvHit;
use crate::app::components::tv_tree_target::TvTreeTarget;
use crate::app::components::library_panel::{LibraryContentOwner, LibraryPanel};
use crate::app::components::tv_content::TvContent;
use crate::app::components::{ComponentId, Msg, ShellRequest};
use crate::app::shell::{fold_keyboard_messages, fold_mouse_messages};
use crate::app::render::make_movie_app;
use crate::app::tests::install_test_emby;
use crate::app::tests_tick_harness::TickHarness;
use crate::app::types_events::NavigateLanding;
use crate::app::types_playback::{HomeContent as ModelHomeContent, HomeLatestSection, HomeLatestSource};
use mbv_core::mock_http::MockHttp;
use mbv_core::playback_queue::QueueItem;
use crate::app::{LibEvent, PanelFocus, PanelMode, TabSelection};
use std::time::{Duration, Instant};

fn tv_harness() -> TickHarness {
    let mut app = make_movie_app();
    app.tab = TabSelection::EmbyLibrary(0);
    app.panel_focus = PanelFocus::Library;
    app.panel_mode = PanelMode::Both;
    app.terminal_width = 160;
    app.terminal_height = 50;
    app.libs[0].library.collection_type = "tvshows".into();
    for (index, item) in app.libs[0].nav_stack[0].items.iter_mut().enumerate() {
        item.item_type = "Series".into();
        item.id = format!("series-{index}");
        item.overview.clear();
    }
    let mut season = crate::app::tests::make_item("Season 1", "Season");
    season.id = "season-1".into();
    let mut episode = crate::app::tests::make_item("Episode 1", "Episode");
    episode.id = "episode-1".into();
    app.series_detail_cache.insert(
        "series-0".into(),
        crate::app::SeriesDetail {
            seasons: vec![season],
            episodes: [("season-1".into(), vec![episode])].into_iter().collect(),
        },
    );
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_tv_content();
    harness.model_mut().sync_active_destination();
    harness
}

#[test]
fn home_latest_snapshot_merge_updates_the_tv_destination_through_tick() {
    let mut harness = flat_episode_harness(mbv_core::config::TvContentMode::Latest);
    let mut fresh = crate::app::tests::make_item("Home episode", "Episode");
    fresh.id = "home-episode".into();
    harness.model_mut().assign_home_content(ModelHomeContent {
        continue_items: Vec::new(),
        latest: vec![HomeLatestSection {
            title: "TV".into(),
            source: HomeLatestSource::Emby("lib-movies".into()),
            items: vec![QueueItem::Emby(Box::new(fresh))],
            has_new_content: true,
        }],
        loading: false,
        feed_names: Default::default(),
    });
    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Char('j'),
        modifiers: KeyModifiers::NONE,
    }));
    step_and_drain(&mut harness);
    draw(&mut harness);

    let snapshot = &harness.model().tv_latest_snapshots["lib-movies"];
    assert_eq!(
        snapshot.items[0].as_emby().unwrap().id,
        harness.model().app.libs[0].nav_stack[0].items[0].id
    );
    assert!(!snapshot.has_new_content);
}

#[test]
fn destination_latest_marker_uses_first_launch_baseline_and_closed_timestamp_window() {
    let mut harness = flat_episode_harness(mbv_core::config::TvContentMode::Upcoming);
    let source = "lib-movies".to_owned();
    harness.model_mut().app.home_latest_launch_window =
        crate::app::home_latest::HomeLatestLaunchWindow {
            previous: None,
            current: 200,
        };
    let mut at_launch = crate::app::tests::make_item("At launch", "Episode");
    at_launch.date_added = "1970-01-01T00:03:20Z".into();
    harness.model_mut().update_tv_latest_snapshot(
        source.clone(),
        "TV".into(),
        vec![QueueItem::Emby(Box::new(at_launch))],
    );
    assert!(!harness.model().tv_latest_snapshots[&source].has_new_content);

    harness.model_mut().app.home_latest_launch_window =
        crate::app::home_latest::HomeLatestLaunchWindow {
            previous: Some(100),
            current: 200,
        };
    let mut at_cutoff = crate::app::tests::make_item("At cutoff", "Episode");
    at_cutoff.date_added = "1970-01-01T00:01:40Z".into();
    let mut at_current = crate::app::tests::make_item("At current launch", "Episode");
    at_current.date_added = "1970-01-01T00:03:20Z".into();
    let mut after_current = crate::app::tests::make_item("After current launch", "Episode");
    after_current.date_added = "1970-01-01T00:03:21Z".into();
    harness.model_mut().update_tv_latest_snapshot(
        source.clone(),
        "TV".into(),
        vec![
            QueueItem::Emby(Box::new(at_cutoff)),
            QueueItem::Emby(Box::new(at_current)),
            QueueItem::Emby(Box::new(after_current)),
        ],
    );
    assert!(harness.model().tv_latest_snapshots[&source].has_new_content);

    let mut only_future = crate::app::tests::make_item("Future", "Episode");
    only_future.date_added = "1970-01-01T00:03:21Z".into();
    harness.model_mut().update_tv_latest_snapshot(
        source.clone(),
        "TV".into(),
        vec![QueueItem::Emby(Box::new(only_future))],
    );
    assert!(!harness.model().tv_latest_snapshots[&source].has_new_content);
}

#[test]
fn preselected_latest_acknowledges_snapshot_that_arrives_later() {
    let mut harness = flat_episode_harness(mbv_core::config::TvContentMode::Latest);
    harness.model_mut().app.home_latest_launch_window =
        crate::app::home_latest::HomeLatestLaunchWindow {
            previous: Some(100),
            current: 200,
        };
    let source = HomeLatestSource::Emby("lib-movies".into());
    assert!(harness
        .model()
        .acknowledged_home_latest_sources
        .contains(&source));

    let mut arriving = crate::app::tests::make_item("Arriving episode", "Episode");
    arriving.date_added = "1970-01-01T00:02:00Z".into();
    harness.model_mut().update_tv_latest_snapshot(
        "lib-movies".into(),
        "TV".into(),
        vec![QueueItem::Emby(Box::new(arriving))],
    );
    assert!(!harness.model().tv_latest_snapshots["lib-movies"].has_new_content);
    draw(&mut harness);
    assert!(!panel(&harness).test_selector_markers()[0]);
}

#[test]
fn shrunken_tv_restore_does_not_replace_latest_snapshot_with_stale_items_through_tick() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut harness = tv_harness();
    let mut current = crate::app::tests::make_item("Current latest", "Episode");
    current.id = "current-latest".into();
    harness.model_mut().assign_home_content(ModelHomeContent {
        continue_items: Vec::new(),
        latest: vec![HomeLatestSection {
            title: "TV".into(),
            source: HomeLatestSource::Emby("lib-movies".into()),
            items: vec![QueueItem::Emby(Box::new(current))],
            has_new_content: true,
        }],
        loading: false,
        feed_names: Default::default(),
    });

    let saved_level = crate::config::LibraryPositionLevel {
        parent_id: "lib-movies".into(),
        title: "TV".into(),
        item_types: Some("Episode".into()),
        tv_content_mode: Some(mbv_core::config::TvContentMode::Latest),
        library_total: Some(10),
        ..Default::default()
    };
    let requested_position = crate::config::LibraryPosition {
        levels: vec![saved_level.clone()],
        ..Default::default()
    };
    harness
        .model_mut()
        .app
        .replace_saved_library_position(0, requested_position.clone());
    assert_eq!(
        harness.model().app.saved_library_position(0),
        Some(requested_position.clone())
    );
    let mut stale = crate::app::tests::make_item("Stale restored latest", "Episode");
    stale.id = "stale-latest".into();
    let mut restored_level =
        crate::app::BrowseLevel::from_position_level(&saved_level, vec![stale], 1, 10);
    restored_level.tv_content_mode = Some(mbv_core::config::TvContentMode::Latest);
    let event = LibEvent::RestoreLibraryPosition {
        lib_idx: 0,
        requested_position: requested_position.clone(),
        position: requested_position.clone(),
        nav_stack: vec![restored_level],
    };
    harness.model().app.lib_tx.send(event).unwrap();
    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Char('j'),
        modifiers: KeyModifiers::NONE,
    }));
    let _ = harness.step(); // Exercise the mounted Application::tick() composition path.
    assert_eq!(
        harness.model().app.saved_library_position(0),
        Some(requested_position.clone()),
        "tick must not stale the pending restore guard"
    );
    let event = harness.model().app.lib_rx.try_recv().expect("restore event");
    harness
        .model_mut()
        .handle_restored_library_position_event(event);
    harness.model_mut().sync_mounted_surfaces();
    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Char('j'),
        modifiers: KeyModifiers::NONE,
    }));
    step_and_drain(&mut harness);

    let lib = &harness.model().app.libs[0];
    assert_eq!(lib.tv_content_mode, Some(mbv_core::config::TvContentMode::Latest));
    assert_eq!(lib.nav_stack[0].items[0].id, "current-latest");
    assert_ne!(lib.nav_stack[0].items[0].id, "stale-latest");
    let latest = &harness.model().tv_latest_snapshots["lib-movies"].items;
    assert_eq!(latest[0].as_emby().unwrap().id, "current-latest");
    assert_ne!(latest[0].as_emby().unwrap().id, "stale-latest");
}

#[test]
fn tv_latest_refresh_updates_destination_snapshot_with_one_fetch_through_tick() {
    let _guard = crate::config::TestStateDirGuard::new();
    let http = MockHttp::new();
    http.respond(
        200,
        r#"{"Items":[{"Id":"fresh-episode","Name":"Fresh episode","Type":"Episode"}],"TotalRecordCount":1}"#,
    );
    let mut harness = tv_harness();
    let mut config = harness.model().app.config.lock().unwrap().clone();
    config.emby_setup = Some(mbv_core::config::EmbySetup::new(
        "http://127.0.0.1:1",
        "user-1",
    ));
    install_test_emby(&mut harness.model_mut().app, config);
    let mut client = harness
        .model()
        .app
        .emby_runtime
        .client
        .as_ref()
        .unwrap()
        .lock()
        .unwrap()
        .clone()
        .with_test_agent(http.agent());
    client.user_id = "user-1".into();
    client.config.server_url = "http://127.0.0.1:1".into();
    harness.model_mut().app.emby_runtime = mbv_core::service_runtime::EmbyRuntime::ready(
        std::sync::Arc::new(std::sync::Mutex::new(client)),
    );
    harness.model_mut().app.home_latest_launch_window =
        crate::app::home_latest::HomeLatestLaunchWindow {
            previous: Some(100),
            current: 200,
        };
    harness.model_mut().app.libs[0].library.collection_type = "tvshows".into();
    harness.model_mut().app.libs[0].tv_content_mode =
        Some(mbv_core::config::TvContentMode::Latest);
    harness.model_mut().app.libs[0].library_total = Some(301);
    let level = &mut harness.model_mut().app.libs[0].nav_stack[0];
    level.tv_content_mode = Some(mbv_core::config::TvContentMode::Latest);
    level.item_types = Some("Episode".into());
    level.items = vec![crate::app::tests::make_item("Old episode", "Episode")];
    level.loading = false;
    harness.model_mut().home_content.latest = vec![HomeLatestSection {
        title: "TV".into(),
        source: HomeLatestSource::Emby("lib-movies".into()),
        items: vec![QueueItem::Emby(Box::new(crate::app::tests::make_item(
            "Old episode",
            "Episode",
        )))],
        has_new_content: true,
    }];

    harness.model_mut().app.refresh_lib(0);
    let event = harness
        .model()
        .app
        .lib_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("Latest refresh completion");
    let LibEvent::Loaded { level, .. } = &event else {
        if let LibEvent::Error(error) = &event {
            panic!("Latest refresh failed: {error}; requests: {:?}", http.requests());
        }
        panic!("expected Latest level load; requests: {:?}", http.requests());
    };
    let mut items: Vec<_> = level
        .items
        .iter()
        .cloned()
        .map(|item| QueueItem::Emby(Box::new(item)))
        .collect();
    if let Some(QueueItem::Emby(item)) = items.first_mut() {
        item.date_added = "1970-01-01T00:02:00Z".into();
    }
    harness.model_mut().app.handle_lib_event(event);
    harness.model_mut().update_tv_latest_snapshot(
        "lib-movies".into(),
        "TV".into(),
        items,
    );
    harness.model_mut().sync_mounted_surfaces();
    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Char('j'),
        modifiers: KeyModifiers::NONE,
    }));
    step_and_drain(&mut harness);
    draw(&mut harness);

    let latest = &harness.model().tv_latest_snapshots["lib-movies"];
    assert_eq!(latest.items[0].as_emby().unwrap().id, "fresh-episode");
    assert_eq!(
        harness.model().app.libs[0].nav_stack[0].items[0].id,
        "fresh-episode"
    );
    assert!(
        !latest.has_new_content,
        "refresh must not restore a marker after Latest was selected"
    );
    let latest_fetches = http
        .requests()
        .iter()
        .filter(|request| request.contains("IncludeItemTypes=Episode"))
        .count();
    assert_eq!(latest_fetches, 1, "requests: {:?}", http.requests());
}

#[test]
fn deep_latest_library_refreshes_its_level_without_touching_shared_snapshot_through_tick() {
    let _guard = crate::config::TestStateDirGuard::new();
    let http = MockHttp::new();
    http.respond(
        200,
        r#"{"Items":[{"Id":"deep-fresh","Name":"Deep episode","Type":"Episode"}],"TotalRecordCount":1}"#,
    );
    let mut harness = tv_harness();
    let mut config = harness.model().app.config.lock().unwrap().clone();
    config.emby_setup = Some(mbv_core::config::EmbySetup::new(
        "http://127.0.0.1:1",
        "user-1",
    ));
    install_test_emby(&mut harness.model_mut().app, config);
    let mut client = harness
        .model()
        .app
        .emby_runtime
        .client
        .as_ref()
        .unwrap()
        .lock()
        .unwrap()
        .clone()
        .with_test_agent(http.agent());
    client.user_id = "user-1".into();
    client.config.server_url = "http://127.0.0.1:1".into();
    harness.model_mut().app.emby_runtime = mbv_core::service_runtime::EmbyRuntime::ready(
        std::sync::Arc::new(std::sync::Mutex::new(client)),
    );
    let mut snapshot_item = crate::app::tests::make_item("Snapshot episode", "Episode");
    snapshot_item.id = "snapshot-episode".into();
    harness.model_mut().assign_home_content(ModelHomeContent {
        continue_items: Vec::new(),
        latest: vec![HomeLatestSection {
            title: "TV".into(),
            source: HomeLatestSource::Emby("lib-movies".into()),
            items: vec![QueueItem::Emby(Box::new(snapshot_item))],
            has_new_content: true,
        }],
        loading: false,
        feed_names: Default::default(),
    });
    let lib = &mut harness.model_mut().app.libs[0];
    lib.tv_content_mode = Some(mbv_core::config::TvContentMode::Latest);
    lib.library_total = Some(301);
    let root = &mut lib.nav_stack[0];
    root.tv_content_mode = Some(mbv_core::config::TvContentMode::Latest);
    root.item_types = Some("Series".into());
    let mut old_episode = crate::app::tests::make_item("Old deep episode", "Episode");
    old_episode.id = "old-deep".into();
    lib.nav_stack.push(crate::app::BrowseLevel {
        fetched_rows: 1,
        parent_id: "series-0".into(),
        title: "Series One".into(),
        items: vec![old_episode],
        total_count: 1,
        resting: crate::app::types_browse::BrowseResting::new(0, 0),
        item_types: Some("Episode".into()),
        unplayed_only: false,
        sort_by: "SortName".into(),
        sort_order: "Ascending".into(),
        loading: false,
        all_items: None,
        letter_filter: None,
        tv_content_mode: None,
        music_grouping: None,
    });
    harness.model_mut().sync_mounted_surfaces();

    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Char('r'),
        modifiers: KeyModifiers::NONE,
    }));
    step_and_drain(&mut harness);
    let event = harness
        .model()
        .app
        .lib_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("deep refresh completion");
    let LibEvent::Refreshed { parent_id, .. } = &event else {
        panic!("expected normal deep-level refresh");
    };
    assert_eq!(parent_id, "series-0");
    harness.model_mut().app.handle_lib_event(event);

    let lib = &harness.model().app.libs[0];
    assert_eq!(lib.nav_stack.len(), 2);
    assert_eq!(lib.nav_stack[1].parent_id, "series-0");
    assert_eq!(lib.nav_stack[1].items[0].id, "deep-fresh");
    assert_eq!(
        harness.model().tv_latest_snapshots["lib-movies"].items[0]
            .as_emby()
            .unwrap()
            .id,
        "snapshot-episode"
    );
    let requests = http.requests();
    assert!(requests.iter().any(|request| request.contains("ParentId=series-0")));
    assert!(
        requests.iter().all(|request| !request.contains("Shows/Latest")),
        "deep refresh must not issue a Latest request: {requests:?}"
    );
}

#[test]
fn tv_latest_selection_acknowledges_the_destination_marker_through_tick() {
    let mut harness = flat_episode_harness(mbv_core::config::TvContentMode::Upcoming);
    harness.model_mut().app.home_latest_launch_window =
        crate::app::home_latest::HomeLatestLaunchWindow {
            previous: Some(100),
            current: 200,
        };
    let mut item = crate::app::tests::make_item("New episode", "Episode");
    item.id = "new-episode".into();
    item.date_added = "1970-01-01T00:02:00Z".into();
    harness.model_mut().update_tv_latest_snapshot(
        "lib-movies".into(),
        "Latest TV".into(),
        vec![QueueItem::Emby(Box::new(item))],
    );
    harness.model_mut().sync_mounted_surfaces();
    draw(&mut harness);
    assert!(panel(&harness).test_selector_markers()[0]);

    let (pill, _) = panel(&harness)
        .test_selector_hits()
        .regions()
        .first()
        .cloned()
        .expect("Latest pill geometry");
    harness.inject(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: pill.x,
        row: pill.y,
        modifiers: KeyModifiers::NONE,
    }));
    step_and_drain(&mut harness);
    draw(&mut harness);

    assert!(!panel(&harness).test_selector_markers()[0]);
    assert!(harness
        .model()
        .acknowledged_home_latest_sources
        .contains(&HomeLatestSource::Emby("lib-movies".into())));

}

#[test]
fn launch_reanchor_applies_tv_letter_scope_through_app_before_item() {
    let mut harness = tv_harness();
    harness.model_mut().app.libs[0].library_total = Some(100);
    harness.model_mut().app.libs[0].nav_stack[0].total_count = 100;
    harness.model_mut().app.pending_launch_tab_resolved = true;
    harness.model_mut().app.pending_launch_state = Some(mbv_core::config::TuiLaunchState {
        version: mbv_core::config::TUI_LAUNCH_STATE_VERSION,
        tab: mbv_core::config::TabIdentity::ServiceLibrary {
            kind: mbv_core::config::ServiceKind::Emby,
            library_id: "lib-movies".into(),
        },
        panel_focus: mbv_core::config::LaunchPanelFocus::Library,
        selector: Some(mbv_core::config::SelectorIdentity::Emby {
            key: mbv_core::config::EmbySelectorKey::Letter(
                mbv_core::config::EmbyLetterBucket::GToI,
            ),
        }),
        item: Some(mbv_core::config::LibraryItemIdentity::Emby {
            id: "series-1".into(),
        }),
    });

    harness.model_mut().sync_mounted_surfaces();
    assert_eq!(
        harness.model().app.libs[0].nav_stack[0]
            .letter_filter
            .as_ref()
            .map(|filter| filter.index),
        Some(2)
    );
    assert!(harness.model().app.pending_launch_state.is_some());

    let level = &mut harness.model_mut().app.libs[0].nav_stack[0];
    level.items = vec![
        crate::app::tests::make_item("Series A", "Series"),
        crate::app::tests::make_item("Series B", "Series"),
    ];
    level.items[1].id = "series-1".into();
    level.loading = false;
    harness.model_mut().sync_mounted_surfaces();
    assert!(harness.model().app.pending_launch_state.is_none());
    assert_eq!(
        tv(&harness).launch_snapshot().1,
        Some(mbv_core::config::LibraryItemIdentity::Emby {
            id: "series-1".into()
        })
    );
}

#[test]
fn launch_reanchor_unfiltered_scope_keeps_full_tv_library() {
    let mut harness = tv_harness();
    harness.model_mut().app.libs[0].library_total = Some(100);
    let level = &mut harness.model_mut().app.libs[0].nav_stack[0];
    level.total_count = 100;
    level.items = vec![
        crate::app::tests::make_item("Series A", "Series"),
        crate::app::tests::make_item("Series Z", "Series"),
    ];
    level.items[1].id = "series-zulu".into();
    level.loading = false;
    harness.model_mut().sync_mounted_surfaces();
    harness.model_mut().app.pending_launch_tab_resolved = true;
    harness.model_mut().app.pending_launch_state = Some(mbv_core::config::TuiLaunchState {
        version: mbv_core::config::TUI_LAUNCH_STATE_VERSION,
        tab: mbv_core::config::TabIdentity::ServiceLibrary {
            kind: mbv_core::config::ServiceKind::Emby,
            library_id: "lib-movies".into(),
        },
        panel_focus: mbv_core::config::LaunchPanelFocus::Library,
        selector: Some(mbv_core::config::SelectorIdentity::Emby {
            key: mbv_core::config::EmbySelectorKey::Unfiltered,
        }),
        item: Some(mbv_core::config::LibraryItemIdentity::Emby {
            id: "series-zulu".into(),
        }),
    });

    harness.model_mut().sync_mounted_surfaces();

    assert!(harness.model().app.libs[0].nav_stack[0].letter_filter.is_none());
    assert!(harness.model().app.pending_launch_state.is_none());
    assert_eq!(
        tv(&harness).launch_snapshot().1,
        Some(mbv_core::config::LibraryItemIdentity::Emby {
            id: "series-zulu".into()
        })
    );
}

#[test]
fn launch_reanchor_unfiltered_scope_clears_an_active_tv_pill() {
    let mut harness = tv_harness();
    harness.model_mut().app.libs[0].library_total = Some(100);
    let level = &mut harness.model_mut().app.libs[0].nav_stack[0];
    level.total_count = 100;
    level.letter_filter = crate::app::render::LetterFilter::for_index_for_kind(
        2,
        crate::app::render::LetterFilterKind::Tv,
    );
    level.items = vec![crate::app::tests::make_item("Series G", "Series")];
    level.loading = false;
    harness.model_mut().sync_mounted_surfaces();
    harness.model_mut().app.pending_launch_tab_resolved = true;
    harness.model_mut().app.pending_launch_state = Some(mbv_core::config::TuiLaunchState {
        version: mbv_core::config::TUI_LAUNCH_STATE_VERSION,
        tab: mbv_core::config::TabIdentity::ServiceLibrary {
            kind: mbv_core::config::ServiceKind::Emby,
            library_id: "lib-movies".into(),
        },
        panel_focus: mbv_core::config::LaunchPanelFocus::Library,
        selector: Some(mbv_core::config::SelectorIdentity::Emby {
            key: mbv_core::config::EmbySelectorKey::Unfiltered,
        }),
        item: Some(mbv_core::config::LibraryItemIdentity::Emby {
            id: "series-zulu".into(),
        }),
    });

    harness.model_mut().sync_mounted_surfaces();
    assert!(harness.model().app.libs[0].nav_stack[0].letter_filter.is_none());
    assert!(harness.model().app.pending_launch_state.is_some());

    let level = &mut harness.model_mut().app.libs[0].nav_stack[0];
    level.items = vec![
        crate::app::tests::make_item("Series A", "Series"),
        crate::app::tests::make_item("Series Z", "Series"),
    ];
    level.items[1].id = "series-zulu".into();
    level.loading = false;
    harness.model_mut().sync_mounted_surfaces();

    assert!(harness.model().app.pending_launch_state.is_none());
    assert_eq!(
        tv(&harness).launch_snapshot().1,
        Some(mbv_core::config::LibraryItemIdentity::Emby {
            id: "series-zulu".into()
        })
    );
}

#[rstest]
#[case::saved_all_grown_large(
    mbv_core::config::TvContentMode::All,
    301,
    mbv_core::config::TvContentMode::Latest,
    "IncludeItemTypes=Episode",
)]
#[case::saved_s_z_shrunk_small(
    mbv_core::config::TvContentMode::Range(2),
    300,
    mbv_core::config::TvContentMode::All,
    "IncludeItemTypes=Series",
)]
#[case::saved_latest_shrunk_small(
    mbv_core::config::TvContentMode::Latest,
    300,
    mbv_core::config::TvContentMode::Latest,
    "IncludeItemTypes=Episode",
)]
fn reopening_reclamps_saved_tv_mode_before_fetch_and_tick_paint(
    #[case] saved_mode: mbv_core::config::TvContentMode,
    #[case] current_total: usize,
    #[case] expected_mode: mbv_core::config::TvContentMode,
    #[case] expected_route: &str,
) {
    let _guard = crate::config::TestStateDirGuard::new();
    let http = MockHttp::new();
    let mut app = make_movie_app();
    app.tab = TabSelection::EmbyLibrary(0);
    app.panel_focus = PanelFocus::Library;
    app.panel_mode = PanelMode::Both;
    app.terminal_width = 160;
    app.terminal_height = 50;
    app.libs[0].library.collection_type = "tvshows".into();
    app.libs[0].nav_stack.clear();
    let mut config = app.config.lock().unwrap().clone();
    config.server_url = "http://127.0.0.1:1".into();
    install_test_emby(&mut app, config);
    let client = app
        .emby_runtime
        .client
        .as_ref()
        .unwrap()
        .lock()
        .unwrap()
        .clone()
        .with_test_agent(http.agent());
    app.emby_runtime = mbv_core::service_runtime::EmbyRuntime::ready(std::sync::Arc::new(
        std::sync::Mutex::new(client),
    ));

    let saved = mbv_core::config::LibraryPosition {
        levels: vec![mbv_core::config::LibraryPositionLevel {
            parent_id: "lib-movies".into(),
            title: "TV".into(),
            item_types: Some(match &saved_mode {
                mbv_core::config::TvContentMode::Latest
                | mbv_core::config::TvContentMode::Upcoming => "Episode",
                _ => "Series",
            }
            .into()),
            letter_filter_index: match &saved_mode {
                mbv_core::config::TvContentMode::Range(index) => Some(*index),
                _ => None,
            },
            tv_content_mode: Some(saved_mode),
            library_total: Some(current_total),
            ..Default::default()
        }],
        ..Default::default()
    };
    app.replace_saved_library_position(0, saved);
    let response_item = match &expected_mode {
        mbv_core::config::TvContentMode::Latest => {
            r#"{"Id":"episode-1","Name":"Latest","Type":"Episode"}"#
        }
        _ => r#"{"Id":"series-1","Name":"Series","Type":"Series"}"#,
    };
    http.respond(
        200,
        &format!("{{\"Items\":[{response_item}],\"TotalRecordCount\":1}}"),
    );

    app.activate_library_position(0);
    assert_eq!(app.libs[0].tv_content_mode, Some(expected_mode.clone()));
    assert_eq!(
        app.saved_library_position(0)
            .unwrap()
            .levels[0]
            .tv_content_mode,
        Some(expected_mode.clone()),
        "the resolved mode is saved before the restore worker fetches"
    );

    let mut harness = TickHarness::new(app);
    draw(&mut harness);
    assert_eq!(
        harness.model().app.libs[0].nav_stack[0].tv_content_mode,
        Some(expected_mode.clone()),
        "the pending restore paints its resolved mode before the fetch completes"
    );
    assert_eq!(
        panel(&harness).test_selector_hits().regions().len(),
        3 + usize::from(current_total > 300) * 2
    );

    let restored = harness
        .model()
        .app
        .lib_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("restored library event");
    assert!(matches!(restored, LibEvent::RestoreLibraryPosition { .. }));
    harness.model_mut().app.handle_lib_event(restored);
    draw(&mut harness);
    assert_eq!(
        harness.model().app.libs[0].nav_stack[0].tv_content_mode,
        Some(expected_mode.clone())
    );

    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Char('j'),
        modifiers: KeyModifiers::NONE,
    }));
    step_and_drain(&mut harness);
    draw(&mut harness);
    assert_eq!(
        harness.model().app.libs[0].tv_content_mode,
        Some(expected_mode),
        "the mounted TV owner receives its resolved mode through the shell sync pass"
    );
    assert_eq!(
        panel(&harness).test_selector_hits().regions().len(),
        3 + usize::from(current_total > 300) * 2
    );

    let requests = http.requests();
    assert!(
        requests.iter().any(|request| request.contains(expected_route)),
        "expected {expected_route} request, got {requests:?}"
    );
    if current_total <= 300 {
        assert!(requests.iter().all(|request| !request.contains("NameStartsWith")));
    }
}

/// The one TV owner (task 8.4, design D2): registered inside the mounted
/// `LibraryPanel` under `LibraryKey::Service(TvShows)` at every breakpoint.
fn tv(harness: &TickHarness) -> &TvContent {
    harness.model().test_tv_owner()
}

/// The mounted `LibraryPanel` hosting the TV owner.
fn panel(harness: &TickHarness) -> &LibraryPanel {
    harness
        .model()
        .application
        .get_component(&ComponentId::Library)
        .expect("library panel mounted")
        .as_any()
        .downcast_ref::<LibraryPanel>()
        .expect("LibraryPanel")
}

fn flat_episode_harness(mode: mbv_core::config::TvContentMode) -> TickHarness {
    let mut harness = tv_harness();
    let mut episode = crate::app::tests::make_item("Latest Episode", "Episode");
    episode.id = "latest-episode".into();
    // Keep the fixture on the direct single-item play path; the series
    // continuation policy is unrelated to flat-mode activation.
    episode.series_id.clear();
    let level = &mut harness.model_mut().app.libs[0].nav_stack[0];
    level.items = vec![episode];
    level.item_types = Some("Episode".into());
    level.tv_content_mode = Some(mode.clone());
    level.loading = false;
    harness.model_mut().app.libs[0].library_total = Some(301);
    harness.model_mut().app.libs[0].tv_content_mode = Some(mode);
    harness.model_mut().sync_mounted_surfaces();
    harness
}

fn draw(harness: &mut TickHarness) {
    let width = harness.model().app.terminal_width;
    let height = harness.model().app.terminal_height;
    // One throwaway draw publishes `root_frame` (the shell's startup draw);
    // the sync then mounts/activates the panel, and the recorded draw paints
    // it — the steady state the deleted component's tests saw after its
    // second `view`.
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|frame| harness.model_mut().draw_frame(frame, false, false))
        .unwrap();
    harness.model_mut().sync_mounted_surfaces();
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|frame| harness.model_mut().draw_frame(frame, false, false))
        .unwrap();
}

fn step_without_sync(harness: &mut TickHarness) -> Vec<Msg> {
    let pre_fold_focus = harness.model().application.focus().cloned();
    let raw_messages = harness
        .model_mut()
        .application
        .tick(PollStrategy::Once(std::time::Duration::from_millis(500)))
        .expect("tick injected event");
    let folded = fold_mouse_messages(raw_messages);
    let router = harness.model_mut().router_outcome(&folded);
    fold_keyboard_messages(folded, pre_fold_focus.as_ref(), &router)
}

/// One tick whose shell requests are handled like the run loop's.
fn step_and_drain(harness: &mut TickHarness) {
    let outcome = harness.step();
    let (mut music_resize, mut tv_resize) = (false, false);
    for message in outcome.messages {
        harness
            .model_mut()
            .handle_terminal_message(message, &mut music_resize, &mut tv_resize);
    }
}

/// Opens Inline Search, types a query, and fires its debounce with a clock
/// tick past the deadline (no wall-clock waiting).
fn search_series(harness: &mut TickHarness, query: &str) {
    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Char('/'),
        modifiers: KeyModifiers::NONE,
    }));
    step_and_drain(harness);
    for ch in query.chars() {
        harness.inject(Event::Keyboard(KeyEvent {
            code: Key::Char(ch),
            modifiers: KeyModifiers::NONE,
        }));
        step_and_drain(harness);
    }
    harness
        .model_mut()
        .tick_inline_search_clock(Instant::now() + Duration::from_millis(301));
}

/// Enter on a Series search result navigates the library list to the
/// series' natural place and opens its workspace (Wide) / the Library Hero
/// overlay (Narrow) -- the ordinary browser Enter flow, launched from
/// search (inline-library-search spec, "Enter on a Series result").
#[test]
fn enter_on_a_series_search_result_navigates_and_opens_the_workspace() {
    let mut harness = tv_harness();
    search_series(&mut harness, "Second");
    assert!(harness.model().active_inline_search_is_open());

    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Enter,
        modifiers: KeyModifiers::NONE,
    }));
    step_and_drain(&mut harness);
    harness.model_mut().sync_mounted_surfaces();

    assert!(
        !harness.model().active_inline_search_is_open(),
        "activation dismisses Inline Search"
    );
    let level = harness.model().app.libs[0].nav_stack.last().unwrap();
    assert_eq!(
        level.resting().cursor(),
        1,
        "the list cursor rests on the activated series"
    );
    assert_eq!(
        tv(&harness).selected_item().map(|item| item.id),
        Some("series-1".to_string()),
        "the workspace targets the activated series"
    );
    assert!(
        tv(&harness).episode_pane_focused(),
        "the workspace is active: episode selection holds the focus"
    );
}

#[test]
fn enter_on_a_series_search_result_narrow_opens_the_hero_overlay() {
    let mut harness = tv_harness();
    harness.model_mut().app.terminal_width = 80;
    harness.model_mut().sync_mounted_surfaces();
    draw(&mut harness);
    search_series(&mut harness, "Second");

    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Enter,
        modifiers: KeyModifiers::NONE,
    }));
    step_and_drain(&mut harness);
    harness.model_mut().sync_mounted_surfaces();

    assert!(
        !harness.model().active_inline_search_is_open(),
        "activation dismisses Inline Search"
    );
    assert!(
        panel(&harness).test_hero_overlay_open(),
        "narrow activation opens the Library Hero overlay"
    );
    let level = harness.model().app.libs[0].nav_stack.last().unwrap();
    assert_eq!(level.resting().cursor(), 1);
    assert!(
        tv(&harness).episode_pane_focused(),
        "the overlay's workspace is active: episode selection holds the focus"
    );
}

#[test]
fn tv_narrow_tick_navigation_updates_the_painted_control() {
    let mut harness = tv_harness();
    harness.model_mut().app.terminal_width = 80;
    harness.model_mut().sync_mounted_surfaces();
    draw(&mut harness);
    let before = tv(&harness)
        .selected_tree_show()
        .expect("narrow TV tree selection")
        .id;

    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    }));
    harness.step();
    draw(&mut harness);

    let after = tv(&harness)
        .selected_tree_show()
        .expect("narrow TV tree selection after navigation")
        .id;
    assert_ne!(after, before);

    // Narrow TV is the same panel-hosted owner (task 8.4) -- no second
    // surface, and the owner stays installed at every breakpoint.
    assert!(harness
        .model()
        .library_panel_has_owner(&harness.model().test_tv_owner_key()));
}

/// unify-screens-under-panel-components task 8.1 (design D12, stable-target
/// re-anchor): the merged owner keeps its selected target across a
/// Wide->Narrow->Wide breakpoint round trip driven through real ticks.
#[test]
fn tv_wide_narrow_wide_tick_navigation_keeps_the_selected_target() {
    let mut harness = tv_harness();
    draw(&mut harness);
    assert_eq!(
        tv(&harness).selected_tree_show().map(|item| item.id),
        Some("series-0".into())
    );

    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    }));
    harness.step();
    draw(&mut harness);
    assert_eq!(
        tv(&harness).selected_tree_show().map(|item| item.id),
        Some("series-1".into())
    );
    let wide_target = tv(&harness)
        .selected_tree_show()
        .expect("wide TV tree selection")
        .id;

    // Narrow: the same owner keeps the same selected target and viewport
    // offset across the shared Wide/Inline presentation transition.
    harness.model_mut().app.terminal_width = 80;
    harness.model_mut().sync_mounted_surfaces();
    draw(&mut harness);
    assert_eq!(
        tv(&harness).selected_tree_show().map(|item| item.id),
        Some("series-1".into())
    );
    let narrow_target = tv(&harness)
        .selected_tree_show()
        .expect("narrow TV tree selection")
        .id;
    assert_eq!(narrow_target, wide_target);

    // Wide again: still the same target and viewport offset.
    harness.model_mut().app.terminal_width = 160;
    harness.model_mut().sync_mounted_surfaces();
    draw(&mut harness);
    assert_eq!(
        tv(&harness).selected_tree_show().map(|item| item.id),
        Some("series-1".into())
    );
    let final_target = tv(&harness)
        .selected_tree_show()
        .expect("final Wide TV tree selection")
        .id;
    assert_eq!(final_target, narrow_target);
}

#[rstest]
#[case::latest(mbv_core::config::TvContentMode::Latest)]
#[case::upcoming(mbv_core::config::TvContentMode::Upcoming)]
fn flat_episode_activation_plays_without_opening_a_series_workspace(
    #[case] mode: mbv_core::config::TvContentMode,
) {
    let mut harness = flat_episode_harness(mode);
    let stack_len = harness.model().app.libs[0].nav_stack.len();

    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Enter,
        modifiers: KeyModifiers::NONE,
    }));
    step_and_drain(&mut harness);

    assert_eq!(harness.model().app.libs[0].nav_stack.len(), stack_len);
    assert_eq!(tv(&harness).selected_series_snapshot(), None);
    assert!(!tv(&harness).episode_pane_focused());
    assert_eq!(
        harness.model().app.playback_queue().emby_items()[0].id,
        "latest-episode",
        "flat episode activation must play the selected episode id"
    );
}

#[test]
fn flat_episode_mini_view_routes_keys_to_the_browser_carrier() {
    let mut harness = flat_episode_harness(mbv_core::config::TvContentMode::Latest);
    let mut second = crate::app::tests::make_item("Upcoming Episode", "Episode");
    second.id = "upcoming-episode".into();
    second.series_id.clear();
    harness.model_mut().app.libs[0].nav_stack[0].items.push(second);
    harness.model_mut().app.terminal_width = crate::app::MINI_VIEW_THRESHOLD - 1;
    harness.model_mut().app.mini_view_focus = PanelFocus::Library;
    harness.model_mut().sync_mounted_surfaces();
    draw(&mut harness);

    assert!(panel(&harness).test_hero_overlay_open());
    assert!(!tv(&harness).episode_pane_focused());
    assert_eq!(
        tv(&harness)
            .viewport_anchor(tv(&harness).painted_viewport_height())
            .expect("flat episode viewport anchor")
            .selected_target,
        "latest-episode"
    );
    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    }));
    let movement = harness.step();
    assert!(movement.messages.iter().any(|message| matches!(
        message,
        Msg::Shell(ShellRequest::EmbyLibraryCursorIndex { index: 1 })
    )));
    assert_eq!(tv(&harness).selected_item_id(), Some("upcoming-episode".into()));
    assert!(!tv(&harness).episode_pane_focused());

    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Char(']'),
        modifiers: KeyModifiers::NONE,
    }));
    let cycle = harness.step();
    assert!(cycle.messages.iter().any(|message| matches!(
        message,
        Msg::Shell(ShellRequest::TvCycleLetterPill { delta: 1 })
    )));
    assert!(!cycle.messages.iter().any(|message| matches!(
        message,
        Msg::Shell(ShellRequest::TvSeasonMove { .. })
    )));

    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Esc,
        modifiers: KeyModifiers::NONE,
    }));
    let _ = harness.step();
    assert!(!panel(&harness).test_hero_overlay_open());
    assert!(!tv(&harness).episode_pane_focused());
}

#[test]
fn flat_episode_hero_is_painted_only_in_mini_view() {
    let mut mini = flat_episode_harness(mbv_core::config::TvContentMode::Latest);
    mini.model_mut().app.terminal_width = crate::app::MINI_VIEW_THRESHOLD - 1;
    mini.model_mut().app.mini_view_focus = PanelFocus::Library;
    mini.model_mut().sync_mounted_surfaces();
    draw(&mut mini);
    assert!(panel(&mini).test_hero_overlay_open());
    assert_eq!(
        mini
            .model_mut()
            .test_tv_owner_mut()
            .hero_data()
            .map(|data| data.facts.title),
        Some("Latest Episode".into())
    );
    assert!(panel(&mini).test_overlay_geometry().is_some());

    let mut narrow = flat_episode_harness(mbv_core::config::TvContentMode::Latest);
    narrow.model_mut().app.terminal_width = crate::app::MINI_VIEW_THRESHOLD;
    narrow.model_mut().sync_mounted_surfaces();
    draw(&mut narrow);
    assert!(!panel(&narrow).test_hero_overlay_open());
    assert!(narrow
        .model_mut()
        .test_tv_owner_mut()
        .hero_data()
        .is_none());
    assert!(panel(&narrow).test_overlay_geometry().is_none());
}

#[test]
fn tv_wide_tick_navigation_updates_the_painted_control() {
    let mut harness = tv_harness();
    draw(&mut harness);
    assert_eq!(
        tv(&harness).selected_tree_show().map(|item| item.id),
        Some("series-0".into())
    );

    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    }));
    harness.step();
    draw(&mut harness);

    assert_eq!(
        tv(&harness).selected_tree_show().map(|item| item.id),
        Some("series-1".into())
    );
}

/// Task 8.4: a catalog-retained TV owner keeps its local cursor and scroll
/// while inactive. This goes through the real Application::tick path for the
/// navigation and for each tab transition's sync pass.
#[test]
fn tv_owner_retains_cursor_and_scroll_while_inactive() {
    let mut harness = tv_harness();
    harness.model_mut().app.terminal_width = 80;
    harness.model_mut().app.terminal_height = 20;
    for index in 2..20 {
        let mut item = crate::app::tests::make_item(&format!("Series {index}"), "Series");
        item.id = format!("series-{index}");
        harness
            .model_mut()
            .app
            .libs[0]
            .nav_stack[0]
            .items
            .push(item);
    }
    harness.model_mut().sync_mounted_surfaces();
    draw(&mut harness);

    for _ in 0..8 {
        harness.inject(Event::Keyboard(KeyEvent {
            code: Key::Down,
            modifiers: KeyModifiers::NONE,
        }));
        harness.step();
        draw(&mut harness);
    }
    let before = tv(&harness)
        .selected_tree_show()
        .expect("TV tree selection before inactive transition")
        .id;
    assert_eq!(before, "series-8");
    // The fixed-row owner may keep the selected row visible at offset zero;
    // clamping is asserted by the carrier tests for both viewport sizes.

    harness.model_mut().app.tab = TabSelection::Home;
    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Char('x'),
        modifiers: KeyModifiers::NONE,
    }));
    harness.step();
    draw(&mut harness);
    let tv_key = harness.model().test_tv_owner_key_at(0);
    assert!(harness.model().library_panel_has_owner(&tv_key));

    harness.model_mut().app.tab = TabSelection::EmbyLibrary(0);
    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Char('x'),
        modifiers: KeyModifiers::NONE,
    }));
    harness.step();
    draw(&mut harness);
    let after = tv(&harness)
        .selected_tree_show()
        .expect("TV tree selection after inactive transition")
        .id;
    assert_eq!(after, before);
}

/// Task 8.4: the season pills are resolved by the mounted panel from the
/// geometry it painted (its Workspace selector row), and the owner translates
/// the resolved slot event into the same `TvHit::SeasonTab` message the
/// deleted component emitted.
#[test]
fn tv_wide_tick_click_resolves_season_pill() {
    let mut harness = tv_harness();
    draw(&mut harness);
    // First frame geometry: the panel retained the season pills it painted.
    let (rect, _) = panel(&harness)
        .test_workspace_selector_hits()
        .regions()
        .first()
        .cloned()
        .expect("painted season pill");
    harness.inject(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: rect.x,
        row: rect.y,
        modifiers: KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    assert!(outcome.messages.iter().any(|message| matches!(
        message,
        Msg::Shell(ShellRequest::TvHitClick {
            hit: TvHit::SeasonTab(0)
        })
    )), "tick messages: {:?}", outcome.messages);
}

/// Task 8.4: the episode rows live in the hero pane's Workspace box; the
/// panel resolves the pointer against its own painted hero pane and the owner
/// resolves the episode target through its own carrier.
#[test]
fn tv_wide_tick_click_resolves_episode_row() {
    let mut harness = tv_harness();
    draw(&mut harness);
    assert_eq!(
        tv(&harness).selected_episode_item().map(|item| item.id),
        Some("episode-1".into())
    );
    let episode_claim = panel(&harness)
        .test_wide_geometry()
        .and_then(|geometry| geometry.workspace)
        .expect("painted episode rows")
        .1;
    harness.inject(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: episode_claim.x,
        row: episode_claim.y,
        modifiers: KeyModifiers::NONE,
    }));
    let messages = step_without_sync(&mut harness);
    assert!(messages.iter().any(|message| matches!(
        message,
        Msg::Shell(ShellRequest::TvHitClick {
            hit: TvHit::EpisodeRow(target)
        }) if target == "episode-1"
    )), "tick messages: {:?}", messages);
}

fn navigated_series(id: &str, name: &str) -> Box<mbv_core::api::EmbyItem> {
    let mut item = crate::app::tests::make_item(name, "Series");
    item.id = id.into();
    Box::new(item)
}

/// Task 3.1: a landing on a Series runs the same detail hand-off Inline
/// Search runs -- the retained TV owner re-anchors, the Wide workspace opens
/// with episode selection focused -- driven through the shell drain and the
/// `Application::tick()` sync pass.
#[test]
fn navigated_series_opens_the_wide_workspace() {
    let mut harness = tv_harness();
    // The retained owner starts on series-0; the navigation targets series-1.
    harness.model_mut().handle_inline_search_lib_event(LibEvent::NavigateTo {
        lib_idx: 0,
        landing: NavigateLanding::Series {
            reveal: navigated_series("series-1", "Second"),
            episode_id: None,
        },
        switch_tab: true,
    });
    harness.step();

    assert_eq!(
        harness.model().app.tab,
        TabSelection::EmbyLibrary(0),
        "the landing switches to the target library"
    );
    assert_eq!(
        tv(&harness).selected_item().map(|item| item.id),
        Some("series-1".to_string()),
        "the retained TV owner re-anchors onto the navigated series"
    );
    assert!(
        tv(&harness).episode_pane_focused(),
        "the Wide workspace is open with episode selection focused"
    );
}

/// Task 3.1: the Narrow landing opens the Library Hero overlay for the
/// navigated show, through the same hand-off. Starts on the Home tab so the
/// hand-off must run AFTER the sync pass retargets the panel's active owner
/// to the landed library; opening the overlay during the event drain would
/// target the pre-navigation owner instead.
#[test]
fn navigated_series_opens_the_hero_overlay_narrow() {
    let mut harness = tv_harness();
    harness.model_mut().app.terminal_width = 80;
    harness.model_mut().app.tab = TabSelection::Home;
    harness.model_mut().sync_mounted_surfaces();
    draw(&mut harness);

    harness.model_mut().handle_inline_search_lib_event(LibEvent::NavigateTo {
        lib_idx: 0,
        landing: NavigateLanding::Series {
            reveal: navigated_series("series-1", "Second"),
            episode_id: None,
        },
        switch_tab: true,
    });
    harness.step();
    harness.model_mut().sync_mounted_surfaces();
    draw(&mut harness);

    assert_eq!(harness.model().app.tab, TabSelection::EmbyLibrary(0));
    assert!(harness.model().app.wide_tv_library_area(0).is_none());
    assert!(
        panel(&harness).test_hero_overlay_open(),
        "narrow navigation opens the Library Hero overlay for the show"
    );
    assert_eq!(
        tv(&harness).selected_item().map(|item| item.id),
        Some("series-1".to_string()),
        "the retained TV owner re-anchors onto the navigated series"
    );
    assert!(
        tv(&harness).episode_pane_focused(),
        "the overlay belongs to the TV owner, not the pre-navigation one"
    );
}

/// Task 3.1 design watch: when the target library's corpus cannot satisfy the
/// landing yet, the hand-off must fire on the pending-landing retry drain --
/// not at the original `NavigateTo` -- and still open the Wide workspace.
#[test]
fn deferred_series_landing_runs_the_handoff_on_its_retry_drain() {
    let mut harness = tv_harness();
    harness.model_mut().app.tab = TabSelection::Home;
    {
        // A paginated root: the whole-library corpus (`all_items`) is absent,
        // so the show can still be satisfied by the prefetch drain.
        let level = harness
            .model_mut()
            .app
            .libs[0]
            .nav_stack
            .last_mut()
            .expect("root level");
        level.total_count = 5;
        level.all_items = None;
    }
    harness.model_mut().sync_mounted_surfaces();

    harness.model_mut().handle_inline_search_lib_event(LibEvent::NavigateTo {
        lib_idx: 0,
        landing: NavigateLanding::Series {
            reveal: navigated_series("series-9", "Ninth"),
            episode_id: None,
        },
        switch_tab: true,
    });
    assert!(
        harness.model().app.pending_series_landing.is_some(),
        "an unsaturated corpus arms the pending landing"
    );
    assert!(
        harness.model().app.pending_series_handoff.is_none(),
        "no hand-off before the landing actually completes"
    );
    assert_eq!(
        harness.model().app.tab,
        TabSelection::Home,
        "the tab switch is deferred with the landing"
    );

    harness
        .model_mut()
        .handle_inline_search_lib_event(LibEvent::AllItemsPrefetched {
            lib_idx: 0,
            parent_id: "lib-movies".into(),
            items: vec![
                *navigated_series("series-0", "First"),
                *navigated_series("series-9", "Ninth"),
            ],
        });
    assert!(
        harness.model().app.pending_series_landing.is_none(),
        "the retry landed"
    );
    assert_eq!(harness.model().app.tab, TabSelection::EmbyLibrary(0));
    harness.step();

    assert_eq!(
        tv(&harness).selected_item().map(|item| item.id),
        Some("series-9".to_string()),
        "the deferred landing's hand-off re-anchors the owner"
    );
    assert!(
        tv(&harness).episode_pane_focused(),
        "the deferred landing opens the Wide workspace"
    );
}

/// A flat `Upcoming` level holding three Emby `/Shows/Upcoming` placeholders:
/// `Type: Episode` with no `Id` and a `series_id` set (the real-server shape
/// for unaired/not-downloaded rows).
fn upcoming_placeholder_harness() -> TickHarness {
    let mut harness = tv_harness();
    let episodes = (0..3)
        .map(|index| {
            let mut episode = crate::app::tests::make_item(
                &format!("Upcoming Episode {index}"),
                "Episode",
            );
            episode.id.clear();
            episode.series_id = format!("series-{index}");
            episode.series_name = format!("The Show {index}");
            episode
        })
        .collect();
    let level = &mut harness.model_mut().app.libs[0].nav_stack[0];
    level.items = episodes;
    level.item_types = Some("Episode".into());
    level.tv_content_mode = Some(mbv_core::config::TvContentMode::Upcoming);
    level.loading = false;
    harness.model_mut().app.libs[0].library_total = Some(301);
    harness.model_mut().app.libs[0].tv_content_mode =
        Some(mbv_core::config::TvContentMode::Upcoming);
    harness.model_mut().sync_mounted_surfaces();
    harness
}

/// Assert that activation leaves flat Upcoming mode and lands the selected
/// placeholder's series Workspace through the shared ensure-then-land path.
fn assert_placeholder_workspace(harness: &mut TickHarness, index: usize) {
    let expected_id = format!("series-{index}");
    assert_eq!(harness.model().app.player_tab.total_queue_len(), 0);
    let pending = harness
        .model()
        .app
        .pending_series_landing
        .as_ref()
        .expect("the series landing is armed");
    assert_eq!(pending.reveal.id, expected_id);
    assert_eq!(pending.reveal.item_type, "Series");

    // The whole-library corpus drain lands the reveal and opens the Workspace
    // (the same path a Series search/navigation result uses).
    let mut series = crate::app::tests::make_item(&format!("The Show {index}"), "Series");
    series.id = expected_id.clone();
    harness
        .model_mut()
        .handle_inline_search_lib_event(LibEvent::AllItemsPrefetched {
            lib_idx: 0,
            parent_id: "lib-movies".into(),
            items: vec![series],
        });
    harness.step();
    assert!(harness.model().app.pending_series_landing.is_none());
    assert_eq!(tv(harness).selected_item().map(|item| item.id), Some(expected_id));
    assert!(tv(harness).episode_pane_focused());
}

#[rstest]
#[case::first_row(0)]
#[case::middle_row(1)]
#[case::last_row(2)]
fn keyboard_activation_of_each_upcoming_placeholder_opens_its_series_workspace(
    #[case] index: usize,
) {
    let mut harness = upcoming_placeholder_harness();
    for _ in 0..index {
        harness.inject(Event::Keyboard(KeyEvent {
            code: Key::Down,
            modifiers: KeyModifiers::NONE,
        }));
        step_and_drain(&mut harness);
    }
    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Enter,
        modifiers: KeyModifiers::NONE,
    }));
    step_and_drain(&mut harness);
    assert_placeholder_workspace(&mut harness, index);
}

#[rstest]
#[case::first_row(0)]
#[case::middle_row(1)]
#[case::last_row(2)]
fn mouse_activation_of_each_upcoming_placeholder_opens_the_clicked_series_workspace(
    #[case] index: usize,
) {
    let mut harness = upcoming_placeholder_harness();
    draw(&mut harness);
    let row_area = panel(&harness)
        .test_wide_geometry()
        .expect("painted browser")
        .list_area;
    let point = (row_area.x + 1, row_area.y + index as u16);
    let click = Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: point.0,
        row: point.1,
        modifiers: KeyModifiers::NONE,
    });
    harness.inject(click.clone());
    step_and_drain(&mut harness);
    harness.inject(click);
    step_and_drain(&mut harness);
    assert_placeholder_workspace(&mut harness, index);
}

/// Mouse and keyboard activation of a playable flat episode both play it
/// directly instead of navigating to a series Workspace.
#[rstest]
#[case::latest(mbv_core::config::TvContentMode::Latest)]
#[case::upcoming(mbv_core::config::TvContentMode::Upcoming)]
fn mouse_double_click_on_a_playable_episode_still_plays_through_tick(
    #[case] mode: mbv_core::config::TvContentMode,
) {
    let mut harness = flat_episode_harness(mode);
    draw(&mut harness);
    let row_area = panel(&harness)
        .test_wide_geometry()
        .expect("painted browser")
        .list_area;
    let click = Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: row_area.x + 1,
        row: row_area.y,
        modifiers: KeyModifiers::NONE,
    });
    harness.inject(click.clone());
    step_and_drain(&mut harness);
    harness.inject(click);
    step_and_drain(&mut harness);

    assert_eq!(
        harness.model().app.playback_queue().emby_items()[0].id,
        "latest-episode"
    );
    assert!(harness.model().app.pending_series_landing.is_none());
}

fn tv_tree_geometry(width: u16, mini: bool) -> TickHarness {
    let mut harness = tv_harness();
    harness.model_mut().app.terminal_width = width;
    if mini {
        harness.model_mut().app.mini_view_focus = PanelFocus::Library;
    }
    harness.model_mut().sync_mounted_surfaces();
    draw(&mut harness);
    harness
}

fn tv_tree_text_position(harness: &mut TickHarness, text: &str) -> (u16, u16) {
    let width = harness.model().app.terminal_width;
    let height = harness.model().app.terminal_height;
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|frame| harness.model_mut().draw_frame(frame, false, false))
        .unwrap();
    let buffer = terminal.backend().buffer();
    for y in 0..buffer.area.height {
        let mut line = String::new();
        for x in 0..buffer.area.width {
            line.push_str(buffer[(x, y)].symbol());
        }
        if let Some(x) = line.find(text) {
            return (x as u16, y);
        }
    }
    panic!("TV tree row {text:?} was not painted");
}

fn tick_tv_key(harness: &mut TickHarness, code: Key) -> Vec<Msg> {
    harness.inject(Event::Keyboard(KeyEvent {
        code,
        modifiers: KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    let messages = outcome.messages.clone();
    let (mut music_resize, mut tv_resize) = (false, false);
    for message in outcome.messages {
        harness
            .model_mut()
            .handle_terminal_message(message, &mut music_resize, &mut tv_resize);
    }
    harness.model_mut().sync_mounted_surfaces();
    messages
}

#[rstest]
#[case::wide(160, false)]
#[case::narrow(80, false)]
#[case::mini(crate::app::MINI_VIEW_THRESHOLD - 1, true)]
fn tv_tree_keyboard_navigation_resolves_show_target_through_shell_sync(
    #[case] width: u16,
    #[case] mini: bool,
) {
    let mut harness = tv_tree_geometry(width, mini);
    assert_eq!(
        tv(&harness).selected_tree_target(),
        Some(&TvTreeTarget::Show("tv-id:8:series-0".into()))
    );

    let messages = tick_tv_key(&mut harness, Key::Down);
    assert!(messages.iter().any(|message| matches!(
        message,
        Msg::Shell(ShellRequest::TvHitClick {
            hit: TvHit::SeriesRow(target)
        }) if target == "series-1"
    )), "tick must resolve the selected tree target before shell dispatch: {messages:?}");
    assert_eq!(
        tv(&harness).selected_tree_target(),
        Some(&TvTreeTarget::Show("tv-id:8:series-1".into())),
        "the shell sync pass must preserve the resolved stable tree target"
    );
    assert_eq!(harness.model().app.libs[0].nav_stack[0].resting().cursor(), 1);
}

#[rstest]
#[case::wide(160, false)]
#[case::narrow(80, false)]
#[case::mini(crate::app::MINI_VIEW_THRESHOLD - 1, true)]
fn tv_tree_show_activation_uses_the_selected_target_in_every_geometry(
    #[case] width: u16,
    #[case] mini: bool,
) {
    let mut harness = tv_tree_geometry(width, mini);
    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Enter,
        modifiers: KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    if width == 160 {
        assert!(outcome.messages.iter().any(|message| matches!(
            message,
            Msg::Shell(ShellRequest::TvActivate { item }) if item.id == "series-0"
        )), "Wide Enter must carry the selected show's stable identity: {:?}", outcome.messages);
    } else {
        assert!(
            !outcome.messages.iter().any(|message| matches!(
                message,
                Msg::Shell(ShellRequest::TvActivate { .. })
            )),
            "non-Wide show activation opens the Library Hero overlay"
        );
    }
    let (mut music_resize, mut tv_resize) = (false, false);
    for message in outcome.messages {
        harness
            .model_mut()
            .handle_terminal_message(message, &mut music_resize, &mut tv_resize);
    }
    harness.model_mut().sync_mounted_surfaces();

    assert_eq!(tv(&harness).selected_tree_target(), Some(&TvTreeTarget::Show("tv-id:8:series-0".into())));
    if width == 160 {
        assert!(tv(&harness).episode_pane_focused());
    } else {
        assert!(panel(&harness).test_hero_overlay_open());
    }
}

#[rstest]
#[case::wide(160, false)]
#[case::narrow(80, false)]
#[case::mini(crate::app::MINI_VIEW_THRESHOLD - 1, true)]
fn tv_tree_mouse_click_resolves_the_painted_show_target_in_every_geometry(
    #[case] width: u16,
    #[case] mini: bool,
) {
    let mut harness = tv_tree_geometry(width, mini);
    let (column, row) = tv_tree_text_position(&mut harness, "Second Movie");
    harness.inject(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column,
        row,
        modifiers: KeyModifiers::NONE,
    }));

    let outcome = harness.step();
    assert_eq!(
        outcome
            .messages
            .iter()
            .filter(|message| matches!(
                message,
                Msg::Shell(ShellRequest::TvHitClick {
                    hit: TvHit::SeriesRow(target)
                }) if target == "series-1"
            ))
            .count(),
        1,
        "one Library Panel painter must resolve the row exactly once: {:?}",
        outcome.messages
    );
    let (mut music_resize, mut tv_resize) = (false, false);
    for message in outcome.messages {
        harness
            .model_mut()
            .handle_terminal_message(message, &mut music_resize, &mut tv_resize);
    }
    harness.model_mut().sync_mounted_surfaces();
    assert_eq!(
        tv(&harness).selected_tree_target(),
        Some(&TvTreeTarget::Show("tv-id:8:series-1".into())),
        "the Library Panel click must update the canonical TV tree selection"
    );
}

#[rstest]
#[case::wide(160, false)]
#[case::narrow(80, false)]
#[case::mini(crate::app::MINI_VIEW_THRESHOLD - 1, true)]
fn tv_tree_mouse_uses_the_latest_painted_geometry_across_resize(
    #[case] width: u16,
    #[case] mini: bool,
) {
    let mut harness = tv_tree_geometry(width, mini);
    let (row_column, row) = tv_tree_text_position(&mut harness, "Second Movie");
    let painted_list = panel(&harness)
        .test_list_rect()
        .expect("the TV tree painted through the Library Panel");
    let column = if width == 160 {
        painted_list.right().saturating_sub(1)
    } else {
        row_column
    };

    // Resize the shell and run its sync pass without drawing. The component
    // must continue to resolve input against the frame it actually painted.
    harness.model_mut().app.terminal_width = if width == 160 { 80 } else { 160 };
    harness.model_mut().sync_mounted_surfaces();
    harness.inject(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column,
        row,
        modifiers: KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    assert_eq!(
        outcome
            .messages
            .iter()
            .filter(|message| matches!(
                message,
                Msg::Shell(ShellRequest::TvHitClick {
                    hit: TvHit::SeriesRow(target)
                }) if target == "series-1"
            ))
            .count(),
        1,
        "mouse delivery must use the last painted tree geometry exactly once: {:?}",
        outcome.messages
    );
    let (mut music_resize, mut tv_resize) = (false, false);
    for message in outcome.messages {
        harness
            .model_mut()
            .handle_terminal_message(message, &mut music_resize, &mut tv_resize);
    }
    harness.model_mut().sync_mounted_surfaces();
    assert_eq!(
        tv(&harness).selected_tree_target(),
        Some(&TvTreeTarget::Show("tv-id:8:series-1".into())),
        "the stale painted-frame event resolves to the same stable target"
    );
}

#[rstest]
#[case::wide(160, false)]
#[case::narrow(80, false)]
#[case::mini(crate::app::MINI_VIEW_THRESHOLD - 1, true)]
fn right_on_expanded_show_activates_its_workspace_through_tick(
    #[case] width: u16,
    #[case] mini: bool,
) {
    let mut harness = tv_tree_geometry(width, mini);
    tick_tv_key(&mut harness, Key::Right);

    let messages = tick_tv_key(&mut harness, Key::Right);
    assert!(messages.iter().any(|message| matches!(
        message,
        Msg::Shell(ShellRequest::TvActivate { item }) if item.id == "series-0"
    )), "Right on the expanded Show must emit its stable-target activation: {messages:?}");
    assert_eq!(
        tv(&harness).selected_tree_target(),
        Some(&TvTreeTarget::Show("tv-id:8:series-0".into())),
        "Workspace activation must not collapse or move the tree selection"
    );
}

#[rstest]
#[case::wide(160, false)]
#[case::narrow(80, false)]
#[case::mini(crate::app::MINI_VIEW_THRESHOLD - 1, true)]
fn tv_tree_episode_activation_plays_the_resolved_episode_in_every_geometry(
    #[case] width: u16,
    #[case] mini: bool,
) {
    let mut harness = tv_tree_geometry(width, mini);
    tick_tv_key(&mut harness, Key::Right); // Expand the cached show detail.
    assert_eq!(
        tv(&harness).selected_tree_target(),
        Some(&TvTreeTarget::Show("tv-id:8:series-0".into()))
    );
    tick_tv_key(&mut harness, Key::Down); // Select Season 1.
    assert!(matches!(
        tv(&harness).selected_tree_target(),
        Some(TvTreeTarget::Season { show, season, .. })
            if show == "tv-id:8:series-0" && season == "season-1"
    ));
    tick_tv_key(&mut harness, Key::Enter); // Expand Season 1.
    let messages = tick_tv_key(&mut harness, Key::Right);
    assert!(!messages.iter().any(|message| matches!(
        message,
        Msg::Shell(ShellRequest::TvActivate { .. })
    )), "Right on an expanded Season must not activate its Show Workspace");
    tick_tv_key(&mut harness, Key::Down); // Still-visible Episode 1 proves Right did not collapse.
    assert!(matches!(
        tv(&harness).selected_tree_target(),
        Some(TvTreeTarget::Episode { show, season, episode, .. })
            if show == "tv-id:8:series-0" && season == "season-1" && episode == "episode-1"
    ));

    let messages = tick_tv_key(&mut harness, Key::Enter);
    assert!(messages.iter().any(|message| matches!(
        message,
        Msg::Shell(ShellRequest::TvEpisodeActivate { episode }) if episode.id == "episode-1"
    )), "Enter must resolve the selected episode identity: {messages:?}");
    assert_eq!(
        harness.model().app.playback_queue().emby_items()[0].id,
        "episode-1",
        "shell dispatch must play the episode carried by the selected tree target"
    );
}
