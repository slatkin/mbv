use super::*;

#[rstest::rstest]
#[case::movies("movies", false, "Movies")]
#[case::home_videos("homevideos", true, "Home Videos")]
fn mounted_flat_latest_populates_from_destination_fetch(
    #[case] collection_type: &str,
    #[case] feed_view: bool,
    #[case] title: &str,
) {
    let mut app = make_movie_app();
    app.panel_focus = crate::app::PanelFocus::Library;
    app.panel_mode = crate::app::PanelMode::LibraryOnly;
    app.mini_view_focus = crate::app::PanelFocus::Library;
    app.libs[0].library.collection_type = collection_type.into();
    app.libs[0].library.name = title.into();
    app.libs[0].library_total = Some(100);
    app.libs[0].nav_stack.clear();
    if feed_view {
        app.config.lock().unwrap().feed_view_libraries = vec![title.to_lowercase()];
    }

    let http = mbv_core::mock_http::MockHttp::new();
    let response = r#"[{"Id":"destination-latest","Name":"Destination Latest","Type":"Movie","MediaType":"Video"}]"#;
    for _ in 0..8 {
        http.respond(200, response);
    }
    let config = crate::config::Config {
        server_url: "http://127.0.0.1:1".into(),
        ..crate::config::Config::default()
    };
    let client = mbv_core::api::EmbyClient::new(config).with_test_agent(http.agent());
    app.emby_runtime = mbv_core::service_runtime::EmbyRuntime::ready(std::sync::Arc::new(
        std::sync::Mutex::new(client),
    ));

    app.pending_launch_tab_resolved = true;
    app.pending_launch_state = Some(mbv_core::config::TuiLaunchState {
        version: mbv_core::config::TUI_LAUNCH_STATE_VERSION,
        tab: mbv_core::config::TabIdentity::ServiceLibrary {
            kind: mbv_core::config::ServiceKind::Emby,
            library_id: "lib-movies".into(),
        },
        panel_focus: mbv_core::config::LaunchPanelFocus::Library,
        selector: Some(mbv_core::config::SelectorIdentity::Emby {
            key: mbv_core::config::EmbySelectorKey::Latest,
        }),
        item: None,
    });
    let mut harness = TickHarness::new(app);
    harness.inject(Event::WindowResize(100, 30));
    let _ = harness.step();
    assert!(browser_owner(&harness).latest_mode());

    let (library_id, items, snapshot_title) = loop {
        match harness
            .model()
            .app
            .lib_rx
            .recv()
            .expect("destination fetch completes")
        {
            LibEvent::EmbyLatestSnapshotFetched {
                library_id,
                title,
                items,
            } => {
                break (library_id, items, title);
            }
            event @ LibEvent::Loaded { .. } => harness.model_mut().app.handle_lib_event(event),
            _ => continue,
        }
    };
    assert_eq!(snapshot_title, title);
    assert_eq!(
        http.requests()
            .iter()
            .filter(|request| {
                request.contains("/Items/Latest") && request.contains("ParentId=lib-movies")
            })
            .count(),
        1,
        "launching into Latest starts one destination snapshot fetch"
    );
    harness.model_mut().update_emby_latest_snapshot(
        library_id,
        snapshot_title,
        items
            .into_iter()
            .map(|item| mbv_core::playback_queue::QueueItem::Emby(Box::new(item)))
            .collect(),
    );
    harness.model_mut().sync_mounted_surfaces();
    assert_eq!(
        browser_owner(&harness).launch_snapshot().1,
        Some(mbv_core::config::LibraryItemIdentity::Emby {
            id: "destination-latest".into(),
        })
    );
}

#[rstest::rstest]
#[case::movies("movies", false, "Movies")]
#[case::home_videos("homevideos", true, "Home Videos")]
#[case::generic("other", false, "Other")]
fn mounted_flat_latest_marker_acknowledges_through_async_snapshot_replacement(
    #[case] collection_type: &str,
    #[case] feed_view: bool,
    #[case] title: &str,
) {
    let mut app = make_movie_app();
    app.tab = crate::app::TabSelection::EmbyLibrary(0);
    app.panel_focus = crate::app::PanelFocus::Library;
    app.panel_mode = crate::app::PanelMode::LibraryOnly;
    app.mini_view_focus = crate::app::PanelFocus::Library;
    app.libs[0].library.collection_type = collection_type.into();
    app.libs[0].library.name = if feed_view { "Movies" } else { title }.into();
    app.libs[0].library_total = Some(100);
    if feed_view {
        use crate::app::state::types::feed::{FeedHomeVideoGroup, FeedHomeVideoState};

        app.config.lock().unwrap().feed_view_libraries = vec!["movies".into()];
        let mut folder = crate::app::tests::make_item("Group One", "Folder");
        folder.id = "group-one".into();
        folder.is_folder = true;
        app.libs[0].nav_stack[0].items = vec![folder.clone()];
        app.libs[0].feed_home_video = Some(FeedHomeVideoState {
            all_items: Vec::new(),
            groups: vec![FeedHomeVideoGroup {
                folder,
                items: Vec::new(),
            }],
            selected_group: 1,
            video_cursor: 0,
            video_scroll: 0,
            loading: false,
        });
    }
    let mut harness = TickHarness::new(app);
    harness.model_mut().app.home_latest_launch_window =
        crate::app::state::home_latest::HomeLatestLaunchWindow {
            previous: Some(100),
            current: 200,
        };
    let mut item = crate::app::tests::make_item("New movie", "Movie");
    item.id = "new-movie".into();
    item.date_added = "1970-01-01T00:02:00Z".into();
    let replacement_item = || mbv_core::playback_queue::QueueItem::Emby(Box::new(item.clone()));
    harness.model_mut().update_emby_latest_snapshot(
        "lib-movies".into(),
        title.into(),
        vec![replacement_item()],
    );
    harness.model_mut().sync_mounted_surfaces();
    draw_mounted(&mut harness, 100, 30);

    assert!(harness.model().tv_latest_snapshots["lib-movies"].has_new_content);
    assert!(
        library_panel(&harness).test_selector_markers()[0],
        "unvisited Latest receives the shell-projected launch-window marker"
    );
    assert!(!browser_owner(&harness).latest_mode());

    let outcome = click_selector(&mut harness, 0);
    dispatch_messages(&mut harness, outcome.messages);
    draw_mounted(&mut harness, 100, 30);
    assert!(browser_owner(&harness).latest_mode());
    assert!(!library_panel(&harness).test_selector_markers()[0]);
    assert!(harness.model().acknowledged_home_latest_sources.contains(
        &crate::app::state::types::playback::DestinationLatestSource::Emby("lib-movies".into())
    ));

    // Model an asynchronous refresh completing with another launch-window item.
    harness.model_mut().update_emby_latest_snapshot(
        "lib-movies".into(),
        title.into(),
        vec![replacement_item()],
    );
    harness.model_mut().sync_mounted_surfaces();
    draw_mounted(&mut harness, 100, 30);
    assert_eq!(
        harness.model().tv_latest_snapshots["lib-movies"].items[0]
            .as_emby()
            .unwrap()
            .id,
        "new-movie"
    );
    assert!(
        !library_panel(&harness).test_selector_markers()[0],
        "a replacement snapshot cannot restore an acknowledged marker"
    );
}

#[test]
fn mounted_flat_latest_first_launch_has_no_new_content_marker() {
    let mut app = make_movie_app();
    app.tab = crate::app::TabSelection::EmbyLibrary(0);
    app.panel_focus = crate::app::PanelFocus::Library;
    app.panel_mode = crate::app::PanelMode::LibraryOnly;
    app.mini_view_focus = crate::app::PanelFocus::Library;
    app.libs[0].library_total = Some(100);
    let mut harness = TickHarness::new(app);
    harness.model_mut().app.home_latest_launch_window =
        crate::app::state::home_latest::HomeLatestLaunchWindow {
            previous: None,
            current: 200,
        };
    let mut item = crate::app::tests::make_item("New movie", "Movie");
    item.id = "new-movie".into();
    item.date_added = "1970-01-01T00:02:00Z".into();
    harness.model_mut().update_emby_latest_snapshot(
        "lib-movies".into(),
        "Movies".into(),
        vec![mbv_core::playback_queue::QueueItem::Emby(Box::new(item))],
    );
    harness.model_mut().sync_mounted_surfaces();
    draw_mounted(&mut harness, 100, 30);

    assert!(!library_panel(&harness).test_selector_markers()[0]);
}

#[rstest::rstest]
#[case::wide(100)]
#[case::narrow(60)]
fn mounted_movies_latest_click_and_item_actions_use_snapshot(#[case] width: u16) {
    use crate::app::state::types::playback::{DestinationLatestSnapshot, DestinationLatestSource};
    use mbv_core::playback_queue::QueueItem;

    let mut app = make_movie_app();
    app.panel_focus = crate::app::PanelFocus::Library;
    app.panel_mode = crate::app::PanelMode::LibraryOnly;
    app.mini_view_focus = crate::app::PanelFocus::Library;
    app.libs[0].library_total = Some(100);
    let mut latest = crate::app::tests::make_item("Latest Movie", "Movie");
    latest.id = "latest-movie".into();
    let snapshot = DestinationLatestSnapshot::new(
        "Movies".into(),
        DestinationLatestSource::Emby("lib-movies".into()),
        vec![QueueItem::Emby(Box::new(latest))],
    );
    let mut harness = TickHarness::new(app);
    harness
        .model_mut()
        .tv_latest_snapshots
        .insert("lib-movies".into(), snapshot);
    harness.model_mut().sync_mounted_surfaces();
    let _ = draw(&mut harness, width, 30);

    let outcome = click_selector(&mut harness, 0);
    assert!(outcome
        .raw_messages
        .iter()
        .any(|message| matches!(message, Msg::Shell(ref shell_boxed) if matches!(shell_boxed.as_ref(), ShellRequest::EmbyLibraryLatestSelected))));
    dispatch_messages(&mut harness, outcome.messages);
    assert!(browser_owner(&harness).latest_mode());
    assert!(
        harness.model().app.libs[0].nav_stack[0]
            .letter_filter
            .is_none(),
        "Latest selection leaves the original letter scope untouched"
    );
    assert_eq!(
        browser_owner(&harness).launch_snapshot().1,
        Some(mbv_core::config::LibraryItemIdentity::Emby {
            id: "latest-movie".into(),
        })
    );

    for (key, expected_play) in [(Key::Char('p'), true), (Key::Char('a'), false)] {
        harness.inject(Event::Keyboard(KeyEvent {
            code: key,
            modifiers: KeyModifiers::CONTROL,
        }));
        let outcome = harness.step();
        assert!(outcome.raw_messages.iter().any(|message| match message {
            Msg::Shell(shell_boxed) => match (shell_boxed.as_ref(), expected_play) {
                (ShellRequest::EmbyLibraryPlay { item }, true)
                | (ShellRequest::EmbyLibraryEnqueue { item }, false) => {
                    item.id == "latest-movie"
                }
                _ => false,
            },
            _ => false,
        }));
    }
}

#[test]
fn mounted_movies_latest_exit_restores_unfiltered_and_selected_letter_scope() {
    let mut app = make_movie_app();
    app.panel_focus = crate::app::PanelFocus::Library;
    app.panel_mode = crate::app::PanelMode::LibraryOnly;
    app.mini_view_focus = crate::app::PanelFocus::Library;
    app.libs[0].library_total = Some(100);
    let mut first = crate::app::tests::make_item("Movie A", "Movie");
    first.id = "movie-a".into();
    let mut second = crate::app::tests::make_item("Movie Z", "Movie");
    second.id = "movie-z".into();
    app.libs[0].nav_stack[0].items = vec![first.clone(), second.clone()];
    app.libs[0].nav_stack[0].total_count = 2;
    app.libs[0].nav_stack[0].loading = false;

    let mut latest = crate::app::tests::make_item("Latest Movie", "Movie");
    latest.id = "latest-movie".into();
    let snapshot = crate::app::state::types::playback::DestinationLatestSnapshot::new(
        "Movies".into(),
        crate::app::state::types::playback::DestinationLatestSource::Emby("lib-movies".into()),
        vec![mbv_core::playback_queue::QueueItem::Emby(Box::new(latest))],
    );
    let mut harness = TickHarness::new(app);
    harness
        .model_mut()
        .tv_latest_snapshots
        .insert("lib-movies".into(), snapshot);
    harness.model_mut().sync_mounted_surfaces();
    let _ = draw(&mut harness, 100, 30);

    let outcome = click_selector(&mut harness, 0);
    dispatch_messages(&mut harness, outcome.messages);
    let _ = draw(&mut harness, 100, 30);
    let outcome = click_selector(&mut harness, 1);
    assert!(outcome.raw_messages.iter().any(|message| matches!(
        message,
        Msg::Shell(ref shell_boxed)
     if matches!(shell_boxed.as_ref(), ShellRequest::EmbyLibraryLatestExit { target: usize::MAX }))));
    dispatch_messages(&mut harness, outcome.messages);
    assert!(harness.model().app.libs[0].nav_stack[0]
        .letter_filter
        .is_none());

    // Complete the refresh requested by the clear intent with the full
    // unfiltered result set, then verify the mounted owner navigates that set.
    let level = &mut harness.model_mut().app.libs[0].nav_stack[0];
    level.items = vec![first, second];
    level.total_count = 2;
    level.loading = false;
    harness.model_mut().sync_mounted_surfaces();
    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    }));
    harness.step();
    assert_eq!(browser_owner(&harness).cursor(), 1);
    assert_eq!(
        browser_owner(&harness).launch_snapshot().1,
        Some(mbv_core::config::LibraryItemIdentity::Emby {
            id: "movie-z".into()
        })
    );

    // Returning to the previously active bucket still selects that bucket.
    let mut bucket_app = make_movie_app();
    bucket_app.panel_focus = crate::app::PanelFocus::Library;
    bucket_app.panel_mode = crate::app::PanelMode::LibraryOnly;
    bucket_app.mini_view_focus = crate::app::PanelFocus::Library;
    bucket_app.libs[0].library_total = Some(100);
    let mut bucket_movie = crate::app::tests::make_item("Movie G", "Movie");
    bucket_movie.id = "movie-g".into();
    let level = &mut bucket_app.libs[0].nav_stack[0];
    level.letter_filter = crate::app::render::LetterFilter::for_index_for_kind(
        2,
        crate::app::render::LetterFilterKind::Movie,
    );
    level.items = vec![bucket_movie];
    level.total_count = 1;
    level.loading = false;
    let mut bucket_latest = crate::app::tests::make_item("Latest Movie", "Movie");
    bucket_latest.id = "latest-movie".into();
    let mut bucket_harness = TickHarness::new(bucket_app);
    bucket_harness.model_mut().tv_latest_snapshots.insert(
        "lib-movies".into(),
        crate::app::state::types::playback::DestinationLatestSnapshot::new(
            "Movies".into(),
            crate::app::state::types::playback::DestinationLatestSource::Emby("lib-movies".into()),
            vec![mbv_core::playback_queue::QueueItem::Emby(Box::new(
                bucket_latest,
            ))],
        ),
    );
    bucket_harness.model_mut().sync_mounted_surfaces();
    let _ = draw(&mut bucket_harness, 100, 30);
    let outcome = click_selector(&mut bucket_harness, 0);
    dispatch_messages(&mut bucket_harness, outcome.messages);
    let _ = draw(&mut bucket_harness, 100, 30);
    let outcome = click_selector(&mut bucket_harness, 3);
    assert!(outcome.raw_messages.iter().any(|message| matches!(
        message,
        Msg::Shell(ref shell_boxed)
     if matches!(shell_boxed.as_ref(), ShellRequest::EmbyLibraryLatestExit { target: 2 }))));
    dispatch_messages(&mut bucket_harness, outcome.messages);
    assert_eq!(
        bucket_harness.model().app.libs[0].nav_stack[0]
            .letter_filter
            .as_ref()
            .map(|filter| filter.index),
        Some(2)
    );
}

#[rstest::rstest]
#[case::wide(100)]
#[case::narrow(60)]
fn mounted_home_video_latest_round_trip_preserves_group_state(#[case] width: u16) {
    use crate::app::state::types::feed::{FeedHomeVideoGroup, FeedHomeVideoState};
    use crate::app::state::types::playback::{DestinationLatestSnapshot, DestinationLatestSource};
    use mbv_core::playback_queue::QueueItem;

    let mut app = make_movie_app();
    app.panel_focus = crate::app::PanelFocus::Library;
    app.panel_mode = crate::app::PanelMode::LibraryOnly;
    app.mini_view_focus = crate::app::PanelFocus::Library;
    app.libs[0].library.collection_type = "homevideos".into();
    app.config.lock().unwrap().feed_view_libraries = vec!["movies".into()];
    let mut folder = crate::app::tests::make_item("Group One", "Folder");
    folder.id = "group-one".into();
    folder.is_folder = true;
    let mut first = crate::app::tests::make_item("First video", "Movie");
    first.id = "video-first".into();
    let mut selected = crate::app::tests::make_item("Selected video", "Movie");
    selected.id = "video-selected".into();
    app.libs[0].nav_stack[0].items = vec![folder.clone()];
    app.libs[0].nav_stack[0].total_count = 1;
    app.libs[0].feed_home_video = Some(FeedHomeVideoState {
        all_items: vec![first.clone(), selected.clone()],
        groups: vec![FeedHomeVideoGroup {
            folder,
            items: vec![first, selected],
        }],
        selected_group: 1,
        video_cursor: 1,
        video_scroll: 1,
        loading: false,
    });
    let mut latest = crate::app::tests::make_item("Latest video", "Movie");
    latest.id = "latest-video".into();
    let snapshot = DestinationLatestSnapshot::new(
        "Home Videos".into(),
        DestinationLatestSource::Emby("lib-movies".into()),
        vec![QueueItem::Emby(Box::new(latest))],
    );

    let mut harness = TickHarness::new(app);
    harness
        .model_mut()
        .tv_latest_snapshots
        .insert("lib-movies".into(), snapshot);
    harness.model_mut().sync_mounted_surfaces();
    let _ = draw(&mut harness, width, 30);
    assert_eq!(
        browser_owner(&harness).cursor(),
        1,
        "initial group row selected"
    );

    let outcome = click_selector(&mut harness, 0);
    assert!(outcome
        .raw_messages
        .iter()
        .any(|message| matches!(message, Msg::Shell(ref shell_boxed) if matches!(shell_boxed.as_ref(), ShellRequest::EmbyLibraryLatestSelected))));
    dispatch_messages(&mut harness, outcome.messages);
    let state = harness.model().app.libs[0]
        .feed_home_video
        .as_ref()
        .expect("group state retained");
    assert_eq!(
        (state.selected_group, state.video_cursor, state.video_scroll),
        (1, 1, 1)
    );
    assert!(browser_owner(&harness).latest_mode());

    let _ = draw(&mut harness, width, 30);
    let outcome = click_selector(&mut harness, 2);
    assert!(outcome.raw_messages.iter().any(|message| matches!(
        message,
        Msg::Shell(ref shell_boxed)
     if matches!(shell_boxed.as_ref(), ShellRequest::EmbyLibraryLatestExit { target: 1 }))));
    dispatch_messages(&mut harness, outcome.messages);
    let state = harness.model().app.libs[0]
        .feed_home_video
        .as_ref()
        .expect("group state retained after return");
    assert_eq!(
        (state.selected_group, state.video_cursor, state.video_scroll),
        (1, 1, 1)
    );
    assert!(!browser_owner(&harness).latest_mode());
    assert_eq!(browser_owner(&harness).cursor(), 1);
}
