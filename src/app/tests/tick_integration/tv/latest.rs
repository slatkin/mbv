use super::*;

#[test]
fn destination_latest_marker_uses_first_launch_baseline_and_closed_timestamp_window() {
    let mut harness = flat_episode_harness(mbv_core::config::TvContentMode::Upcoming);
    let source = "lib-movies".to_owned();
    harness.model_mut().app.home_latest_launch_window =
        crate::app::state::home_latest::HomeLatestLaunchWindow {
            previous: None,
            current: 200,
        };
    let mut at_launch = crate::app::tests::make_item("At launch", "Episode");
    at_launch.date_added = "1970-01-01T00:03:20Z".into();
    harness.model_mut().update_emby_latest_snapshot(
        source.clone(),
        "TV".into(),
        vec![QueueItem::Emby(Box::new(at_launch))],
    );
    assert!(!harness.model().tv_latest_snapshots[&source].has_new_content);

    harness.model_mut().app.home_latest_launch_window =
        crate::app::state::home_latest::HomeLatestLaunchWindow {
            previous: Some(100),
            current: 200,
        };
    let mut at_cutoff = crate::app::tests::make_item("At cutoff", "Episode");
    at_cutoff.date_added = "1970-01-01T00:01:40Z".into();
    let mut at_current = crate::app::tests::make_item("At current launch", "Episode");
    at_current.date_added = "1970-01-01T00:03:20Z".into();
    let mut after_current = crate::app::tests::make_item("After current launch", "Episode");
    after_current.date_added = "1970-01-01T00:03:21Z".into();
    harness.model_mut().update_emby_latest_snapshot(
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
    harness.model_mut().update_emby_latest_snapshot(
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
        crate::app::state::home_latest::HomeLatestLaunchWindow {
            previous: Some(100),
            current: 200,
        };
    let source = DestinationLatestSource::Emby("lib-movies".into());
    assert!(harness
        .model()
        .acknowledged_home_latest_sources
        .contains(&source));

    let mut arriving = crate::app::tests::make_item("Arriving episode", "Episode");
    arriving.date_added = "1970-01-01T00:02:00Z".into();
    harness.model_mut().update_emby_latest_snapshot(
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
    harness.model_mut().update_emby_latest_snapshot(
        "lib-movies".into(),
        "TV".into(),
        vec![QueueItem::Emby(Box::new(current))],
    );

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
    let event = harness
        .model()
        .app
        .lib_rx
        .try_recv()
        .expect("restore event");
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
    assert_eq!(
        lib.tv_content_mode,
        Some(mbv_core::config::TvContentMode::Latest)
    );
    assert_eq!(lib.nav_stack[0].items[0].id, "current-latest");
    assert_ne!(lib.nav_stack[0].items[0].id, "stale-latest");
    let latest = &harness.model().tv_latest_snapshots["lib-movies"].items;
    assert_eq!(latest[0].as_emby().unwrap().id, "current-latest");
    assert_ne!(latest[0].as_emby().unwrap().id, "stale-latest");
}

#[test]
fn non_tv_library_refresh_populates_only_its_latest_snapshot_without_home() {
    let _guard = crate::config::TestStateDirGuard::new();
    let http = MockHttp::new();
    http.respond(
        200,
        r#"[{"Id":"fresh-movie","Name":"Fresh movie","Type":"Movie"}]"#,
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
    let library = &mut harness.model_mut().app.libs[0];
    library.library.id = "movies-id".into();
    library.library.name = "Movies".into();
    library.library.collection_type = "movies".into();
    harness.model_mut().tv_latest_snapshots.insert(
        "tv-id".into(),
        DestinationLatestSnapshot {
            title: "TV".into(),
            source: DestinationLatestSource::Emby("tv-id".into()),
            items: vec![QueueItem::Emby(Box::new(crate::app::tests::make_item(
                "TV episode",
                "Episode",
            )))],
            has_new_content: false,
        },
    );
    // Isolate the library-scoped Latest request from the independent current-level refresh.
    harness.model_mut().app.libs[0].nav_stack.clear();

    harness.model_mut().app.refresh_lib(0);
    loop {
        let event = harness
            .model()
            .app
            .lib_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("Emby latest snapshot completion");
        match event {
            LibEvent::EmbyLatestSnapshotFetched {
                library_id,
                title,
                items,
            } => {
                harness.model_mut().update_emby_latest_snapshot(
                    library_id,
                    title,
                    items
                        .into_iter()
                        .map(|item| QueueItem::Emby(Box::new(item)))
                        .collect(),
                );
                break;
            }
            other => harness.model_mut().app.handle_lib_event(other),
        }
    }

    let requests = http.requests();
    let latest_requests: Vec<_> = requests
        .iter()
        .filter(|request| request.contains("Items/Latest"))
        .collect();
    assert_eq!(latest_requests.len(), 1, "requests: {requests:?}");
    assert!(
        latest_requests[0].contains("ParentId=movies-id"),
        "requests: {requests:?}"
    );
    assert_eq!(
        harness.model().tv_latest_snapshots["movies-id"].items[0]
            .as_emby()
            .unwrap()
            .id,
        "fresh-movie"
    );
    assert_eq!(
        harness.model().tv_latest_snapshots["tv-id"].items[0]
            .as_emby()
            .unwrap()
            .name,
        "TV episode"
    );
    assert!(harness.model().home_content.continue_items.is_empty());
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
        crate::app::state::home_latest::HomeLatestLaunchWindow {
            previous: Some(100),
            current: 200,
        };
    harness.model_mut().app.libs[0].library.collection_type = "tvshows".into();
    harness.model_mut().app.libs[0].tv_content_mode = Some(mbv_core::config::TvContentMode::Latest);
    harness.model_mut().app.libs[0].library_total = Some(301);
    let level = &mut harness.model_mut().app.libs[0].nav_stack[0];
    level.tv_content_mode = Some(mbv_core::config::TvContentMode::Latest);
    level.item_types = Some("Episode".into());
    level.items = vec![crate::app::tests::make_item("Old episode", "Episode")];
    level.loading = false;
    harness.model_mut().update_emby_latest_snapshot(
        "lib-movies".into(),
        "TV".into(),
        vec![QueueItem::Emby(Box::new(crate::app::tests::make_item(
            "Old episode",
            "Episode",
        )))],
    );

    harness.model_mut().app.refresh_lib(0);
    let event = harness
        .model()
        .app
        .lib_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("Latest refresh completion");
    let LibEvent::Loaded { level, .. } = &event else {
        if let LibEvent::Error(error) = &event {
            panic!(
                "Latest refresh failed: {error}; requests: {:?}",
                http.requests()
            );
        }
        panic!(
            "expected Latest level load; requests: {:?}",
            http.requests()
        );
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
    harness
        .model_mut()
        .update_emby_latest_snapshot("lib-movies".into(), "TV".into(), items);
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
    assert!(
        harness.model().home_content.continue_items.is_empty(),
        "destination Latest must not populate Home"
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
    harness.model_mut().update_emby_latest_snapshot(
        "lib-movies".into(),
        "TV".into(),
        vec![QueueItem::Emby(Box::new(snapshot_item))],
    );
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
        resting: crate::app::state::types::browse::BrowseResting::new(0, 0),
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
    assert!(requests
        .iter()
        .any(|request| request.contains("ParentId=series-0")));
    assert!(
        requests
            .iter()
            .all(|request| !request.contains("Shows/Latest")),
        "deep refresh must not issue a Latest request: {requests:?}"
    );
}

#[test]
fn tv_latest_selection_acknowledges_the_destination_marker_through_tick() {
    let mut harness = flat_episode_harness(mbv_core::config::TvContentMode::Upcoming);
    harness.model_mut().app.home_latest_launch_window =
        crate::app::state::home_latest::HomeLatestLaunchWindow {
            previous: Some(100),
            current: 200,
        };
    let mut item = crate::app::tests::make_item("New episode", "Episode");
    item.id = "new-episode".into();
    item.date_added = "1970-01-01T00:02:00Z".into();
    harness.model_mut().update_emby_latest_snapshot(
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
        .contains(&DestinationLatestSource::Emby("lib-movies".into())));
}
