use ratatui::backend::TestBackend;
use ratatui::layout::Position;
use ratatui::Terminal;
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

use crate::app::components::emby_library_content::EmbyLibraryContent as BrowserOwner;
use crate::app::components::inline_search::InlineSearchHost;
use crate::app::components::library_panel::{LibraryContentOwner, LibraryPanel};
use crate::app::components::{ComponentId, Msg, ShellRequest};
use crate::app::LibEvent;
use crate::app::render::make_movie_app;

use crate::app::tests::{install_test_emby, make_session};
use std::time::{Duration, Instant};
use crate::app::tests_tick_harness::TickHarness;

/// The migrated Movies/HomeVideos/Generic owner inside the mounted
/// `LibraryPanel` (task 6.1): the panel is the library area's one event
/// boundary, and the browse state is read through the owner the panel hosts
/// — never a destination component (`emby_browser_id` stays `None` for
/// these kinds).
fn browser_owner(harness: &TickHarness) -> &BrowserOwner {
    let (_, key, _) = harness
        .model().active_emby_library_owner()
        .expect("the active library's owner has migrated");
    harness
        .model()
        .application
        .get_component(&ComponentId::Library)
        .expect("Library panel mounted")
        .as_any()
        .downcast_ref::<LibraryPanel>()
        .expect("Library panel type")
        .owner(&key)
        .and_then(|owner| owner.as_any().downcast_ref::<BrowserOwner>())
        .expect("browser owner installed")
}

fn library_panel(harness: &TickHarness) -> &LibraryPanel {
    harness
        .model()
        .application
        .get_component(&ComponentId::Library)
        .expect("Library panel mounted")
        .as_any()
        .downcast_ref::<LibraryPanel>()
        .expect("Library panel type")
}

fn dispatch_messages(harness: &mut TickHarness, messages: Vec<Msg>) {
    let (mut music_resize, mut tv_resize) = (false, false);
    for message in messages {
        harness
            .model_mut()
            .handle_terminal_message(message, &mut music_resize, &mut tv_resize);
    }
    harness.model_mut().sync_mounted_surfaces();
}

fn selector_click_point(harness: &TickHarness, target: usize) -> Position {
    let panel = harness
        .model()
        .application
        .get_component(&ComponentId::Library)
        .expect("Library panel mounted")
        .as_any()
        .downcast_ref::<LibraryPanel>()
        .expect("Library panel type");
    let (rect, _) = panel
        .test_selector_hits()
        .regions()
        .iter()
        .find(|(_, candidate)| *candidate == target)
        .expect("selector target was painted");
    Position::new(rect.x, rect.y)
}

fn click_selector(harness: &mut TickHarness, target: usize) -> crate::app::tests_tick_harness::StepOutcome {
    let at = selector_click_point(harness, target);
    harness.inject(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: at.x,
        row: at.y,
        modifiers: KeyModifiers::NONE,
    }));
    harness.step()
}

fn draw(harness: &mut TickHarness, width: u16, height: u16) -> Terminal<TestBackend> {
    harness.model_mut().app.terminal_width = width;
    harness.model_mut().app.terminal_height = height;
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|frame| harness.model_mut().draw_frame(frame, false, false))
        .unwrap();
    harness.model_mut().sync_mounted_surfaces();
    terminal
}

fn draw_mounted(harness: &mut TickHarness, width: u16, height: u16) {
    let _ = draw(harness, width, height);
    let _ = draw(harness, width, height);
}

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
        match harness.model().app.lib_rx.recv().expect("destination fetch completes") {
            LibEvent::EmbyLatestSnapshotFetched { library_id, title, items } => {
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
        items.into_iter().map(|item| mbv_core::playback_queue::QueueItem::Emby(Box::new(item))).collect(),
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
        crate::app::home_latest::HomeLatestLaunchWindow {
            previous: Some(100),
            current: 200,
        };
    let mut item = crate::app::tests::make_item("New movie", "Movie");
    item.id = "new-movie".into();
    item.date_added = "1970-01-01T00:02:00Z".into();
    let replacement_item = || {
        mbv_core::playback_queue::QueueItem::Emby(Box::new(item.clone()))
    };
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
        crate::app::home_latest::HomeLatestLaunchWindow {
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
    assert!(outcome.raw_messages.iter().any(|message| matches!(
        message,
        Msg::Shell(ShellRequest::EmbyLibraryLatestSelected)
    )));
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
        assert!(outcome.raw_messages.iter().any(|message| match (message, expected_play) {
            (Msg::Shell(ShellRequest::EmbyLibraryPlay { item }), true)
            | (Msg::Shell(ShellRequest::EmbyLibraryEnqueue { item }), false) => {
                item.id == "latest-movie"
            }
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
    harness.model_mut().tv_latest_snapshots.insert("lib-movies".into(), snapshot);
    harness.model_mut().sync_mounted_surfaces();
    let _ = draw(&mut harness, 100, 30);

    let outcome = click_selector(&mut harness, 0);
    dispatch_messages(&mut harness, outcome.messages);
    let _ = draw(&mut harness, 100, 30);
    let outcome = click_selector(&mut harness, 1);
    assert!(outcome.raw_messages.iter().any(|message| matches!(
        message,
        Msg::Shell(ShellRequest::EmbyLibraryLatestExit { target: usize::MAX })
    )));
    dispatch_messages(&mut harness, outcome.messages);
    assert!(harness.model().app.libs[0].nav_stack[0].letter_filter.is_none());

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
        Some(mbv_core::config::LibraryItemIdentity::Emby { id: "movie-z".into() })
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
            vec![mbv_core::playback_queue::QueueItem::Emby(Box::new(bucket_latest))],
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
        Msg::Shell(ShellRequest::EmbyLibraryLatestExit { target: 2 })
    )));
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
    assert_eq!(browser_owner(&harness).cursor(), 1, "initial group row selected");

    let outcome = click_selector(&mut harness, 0);
    assert!(outcome.raw_messages.iter().any(|message| matches!(
        message,
        Msg::Shell(ShellRequest::EmbyLibraryLatestSelected)
    )));
    dispatch_messages(&mut harness, outcome.messages);
    let state = harness.model().app.libs[0]
        .feed_home_video
        .as_ref()
        .expect("group state retained");
    assert_eq!((state.selected_group, state.video_cursor, state.video_scroll), (1, 1, 1));
    assert!(browser_owner(&harness).latest_mode());

    let _ = draw(&mut harness, width, 30);
    let outcome = click_selector(&mut harness, 2);
    assert!(outcome.raw_messages.iter().any(|message| matches!(
        message,
        Msg::Shell(ShellRequest::EmbyLibraryLatestExit { target: 1 })
    )));
    dispatch_messages(&mut harness, outcome.messages);
    let state = harness.model().app.libs[0]
        .feed_home_video
        .as_ref()
        .expect("group state retained after return");
    assert_eq!((state.selected_group, state.video_cursor, state.video_scroll), (1, 1, 1));
    assert!(!browser_owner(&harness).latest_mode());
    assert_eq!(browser_owner(&harness).cursor(), 1);
}

#[test]
fn browser_wide_tick_moves_control_without_recomputing_app_cursor() {
    let mut app = make_movie_app();
    app.tab = crate::app::TabSelection::EmbyLibrary(0);
    app.panel_focus = crate::app::PanelFocus::Library;
    app.panel_mode = crate::app::PanelMode::LibraryOnly;
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();

    let _terminal = draw(&mut harness, 100, 30);

    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    assert!(outcome.raw_messages.iter().any(|message| {
        matches!(message, Msg::Shell(ShellRequest::EmbyLibraryCursorIndex { index: 1 }))
    }));
    assert_eq!(browser_owner(&harness).cursor(), 1);
    assert_eq!(harness.model().app.libs[0].nav_stack[0].resting().cursor(), 0);

    let _ = draw(&mut harness, 100, 30);
}

#[test]
fn inline_search_on_movies_library_receives_the_shell_pool_push() {
    let mut app = make_movie_app();
    app.tab = crate::app::TabSelection::EmbyLibrary(0);
    app.panel_focus = crate::app::PanelFocus::Library;
    app.panel_mode = crate::app::PanelMode::LibraryOnly;
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    let _terminal = draw(&mut harness, 100, 30);

    // `/` opens the embedded Inline Search through the shell, and the
    // shell's `OpenInlineSearch` host path resolves the Movies owner: the
    // nav-stack items are pushed as the search pool. (Regression: the
    // #695-era conversion collapsed the host path to the Music owner only,
    // so every non-Music library scored typed queries against an empty pool
    // and populated no results.)
    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Char('/'),
        modifiers: KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    assert!(
        outcome
            .messages
            .iter()
            .any(|message| matches!(message, Msg::Shell(ShellRequest::OpenInlineSearch))),
        "\"/\" emits the shell open request: {:?}",
        outcome.messages
    );
    // Drain the step's shell requests like the run loop: the open request
    // loads/pushes the pool into the owner's session. The fetch is NOT one
    // of them: an open box with an empty query loads nothing and shows no
    // results (search starts with the first typed character).
    let (mut music_resize, mut tv_resize) = (false, false);
    for message in outcome.messages {
        harness
            .model_mut()
            .handle_terminal_message(message, &mut music_resize, &mut tv_resize);
    }
    harness.model_mut().sync_mounted_surfaces();
    assert!(harness.model().active_inline_search_is_open());
    assert_eq!(
        browser_owner(&harness).inline_search().results_len(),
        0,
        "an open box with an empty query shows no results"
    );

    // A typed query arms the debounce; the first keystroke also asks the
    // shell to start the corpus load (deferred from open).
    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Char('o'),
        modifiers: KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    assert!(
        outcome
            .messages
            .iter()
            .any(|message| matches!(
                message,
                Msg::Shell(ShellRequest::InlineSearchQueryStarted)
            )),
        "the first keystroke emits the corpus-load request: {:?}",
        outcome.messages
    );
    // Fire the debounce with a clock tick past the deadline: "o" matches
    // both rows of the two-item nav-stack level.
    harness
        .model_mut()
        .tick_inline_search_clock(Instant::now() + Duration::from_millis(301));
    let owner = browser_owner(&harness);
    assert_eq!(owner.inline_search().query(), "o");
    assert_eq!(
        owner.inline_search().results_len(),
        2,
        "typed query resolves rows from the shell-pushed pool"
    );

    // A flat browse completion under an open session (Enter on a folder
    // result drills in: select_item pushes a loading placeholder level and
    // the fetch completes asynchronously) re-pushes the pool at the
    // lib-event boundary: without the `Loaded` re-push the search kept a
    // stale empty pool and the list/hero painted blank. The query is
    // re-scored against the new level's rows.
    harness.model_mut().app.libs[0].nav_stack[0].loading = true;
    harness
        .model_mut()
        .handle_inline_search_lib_event(crate::app::LibEvent::Loaded {
            lib_idx: 0,
            parent_id: "lib-movies".into(),
            level: Box::new(crate::app::BrowseLevel {
        fetched_rows: 0,
                parent_id: "lib-movies".into(),
                title: "Movies".into(),
                items: vec![
                    crate::app::tests::make_item("Anchor", "Movie"),
                    crate::app::tests::make_item("Another One", "Movie"),
                    crate::app::tests::make_item("Another Two", "Movie"),
                ],
                total_count: 3,
                resting: crate::app::state::types::browse::BrowseResting::new(0, 0),
                item_types: None,
                unplayed_only: false,
                sort_by: "SortName".into(),
                sort_order: "Ascending".into(),
                loading: false,
                all_items: None,
                letter_filter: None,
            tv_content_mode: None,
                music_grouping: None,
            }),
        });
    let owner = browser_owner(&harness);
    assert_eq!(
        owner.inline_search().results_len(),
        3,
        "the browse completion re-pushes the pool; the query scores its rows"
    );
}

#[test]
fn launch_reanchor_applies_letter_scope_through_app_before_item() {
    let mut app = make_movie_app();
    app.libs[0].library_total = Some(100);
    app.libs[0].nav_stack[0].total_count = 100;
    let mut harness = TickHarness::new(app);
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
            key: mbv_core::config::EmbySelectorKey::Letter(
                mbv_core::config::EmbyLetterBucket::GToI,
            ),
        }),
        item: Some(mbv_core::config::LibraryItemIdentity::Emby {
            id: "movie-second".into(),
        }),
    });

    // The first sync applies the selector through App, not just the owner's
    // local mirror. The refresh is still loading, so the item intent remains
    // pending for the next projection.
    harness.model_mut().sync_mounted_surfaces();
    assert_eq!(
        harness.model().app.libs[0].nav_stack[0]
            .letter_filter
            .as_ref()
            .map(|filter| filter.index),
        Some(2)
    );
    assert!(harness.model().app.pending_launch_state.is_some());

    // A settled projection gets a second real shell sync and resolves the
    // item against the authoritative filtered scope.
    let level = &mut harness.model_mut().app.libs[0].nav_stack[0];
    level.items = vec![
        crate::app::tests::make_item("Movie A", "Movie"),
        crate::app::tests::make_item("Movie B", "Movie"),
    ];
    level.items[1].id = "movie-second".into();
    level.loading = false;
    harness.model_mut().sync_mounted_surfaces();
    assert!(harness.model().app.pending_launch_state.is_none());
    assert_eq!(
        browser_owner(&harness).launch_snapshot().1,
        Some(mbv_core::config::LibraryItemIdentity::Emby {
            id: "movie-second".into()
        })
    );
}

#[test]
fn launch_reanchor_unfiltered_scope_keeps_full_movie_library() {
    let mut app = make_movie_app();
    app.libs[0].library_total = Some(100);
    let level = &mut app.libs[0].nav_stack[0];
    level.total_count = 100;
    level.items = vec![
        crate::app::tests::make_item("Movie A", "Movie"),
        crate::app::tests::make_item("Movie Z", "Movie"),
    ];
    level.items[1].id = "movie-zulu".into();
    level.loading = false;
    let mut harness = TickHarness::new(app);
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
            id: "movie-zulu".into(),
        }),
    });

    harness.model_mut().sync_mounted_surfaces();

    assert!(harness.model().app.libs[0].nav_stack[0].letter_filter.is_none());
    assert!(harness.model().app.pending_launch_state.is_none());
    assert_eq!(
        browser_owner(&harness).launch_snapshot().1,
        Some(mbv_core::config::LibraryItemIdentity::Emby {
            id: "movie-zulu".into()
        })
    );
}

#[test]
fn launch_reanchor_unfiltered_scope_clears_an_active_movie_pill() {
    let mut app = make_movie_app();
    app.libs[0].library_total = Some(100);
    let level = &mut app.libs[0].nav_stack[0];
    level.total_count = 100;
    level.letter_filter = crate::app::render::LetterFilter::for_index_for_kind(2, crate::app::render::LetterFilterKind::Movie);
    level.items = vec![crate::app::tests::make_item("Movie G", "Movie")];
    level.loading = false;
    let mut harness = TickHarness::new(app);
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
            id: "movie-zulu".into(),
        }),
    });

    harness.model_mut().sync_mounted_surfaces();
    assert!(harness.model().app.libs[0].nav_stack[0].letter_filter.is_none());
    assert!(harness.model().app.pending_launch_state.is_some());

    let level = &mut harness.model_mut().app.libs[0].nav_stack[0];
    level.items = vec![
        crate::app::tests::make_item("Movie A", "Movie"),
        crate::app::tests::make_item("Movie Z", "Movie"),
    ];
    level.items[1].id = "movie-zulu".into();
    level.loading = false;
    harness.model_mut().sync_mounted_surfaces();

    assert!(harness.model().app.pending_launch_state.is_none());
    assert_eq!(
        browser_owner(&harness).launch_snapshot().1,
        Some(mbv_core::config::LibraryItemIdentity::Emby {
            id: "movie-zulu".into()
        })
    );
}

#[test]
fn browser_narrow_tick_click_uses_retained_geometry() {
    let mut app = make_movie_app();
    app.panel_focus = crate::app::PanelFocus::Library;
    app.panel_mode = crate::app::PanelMode::LibraryOnly;
    // At 60 columns the stored panel mode is ignored and the mini view
    // derives the visible panel from `mini_view_focus`; a narrow *library*
    // view must select Library here.
    app.mini_view_focus = crate::app::PanelFocus::Library;
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    let _terminal = draw(&mut harness, 60, 30);

    // The panel's own retained Narrow geometry is the painted truth: the
    // fixed-row selected rectangle resolves to the stable target through the
    // owner's carrier — never a stale-geometry row re-resolution.
    let geometry = harness
        .model()
        .application
        .get_component(&ComponentId::Library)
        .expect("Library panel mounted")
        .as_any()
        .downcast_ref::<LibraryPanel>()
        .expect("Library panel type")
        .test_narrow_geometry()
        .expect("the panel painted a Narrow skeleton");
    let area = geometry.selected.expect("selected row retained geometry");
    assert!(!area.is_empty(), "selected row retained geometry");
    let position = Position::new(area.x, area.y);
    harness.inject(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: position.x,
        row: position.y,
        modifiers: KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    assert!(
        outcome.raw_messages.iter().any(|message| matches!(
            message,
            Msg::Shell(ShellRequest::EmbyLibraryRowClick { target: Some(target) })
                if target == "movie-focused"
        )),
        "the painted fixed-row browser resolves to the selected row: {:?}", outcome.raw_messages
    );
    assert_eq!(browser_owner(&harness).cursor(), 0);
    assert_eq!(harness.model().app.libs[0].nav_stack[0].resting().cursor(), 0);
    let _ = draw(&mut harness, 60, 30);
}

/// The generic catalog's narrow surface routes through the shared owner. (The
/// test previously pinned the Grid presentation for this surface; Grid was
/// deleted as unreachable by design D13 — no library in use lacks a hero.)
#[test]
fn browser_generic_narrow_tick_isolated_from_canonical_controls() {
    let mut app = make_movie_app();
    app.libs[0].library.collection_type = "other".into();
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    let _ = draw(&mut harness, 100, 30);
    assert_eq!(browser_owner(&harness).cursor(), 0);

    // Tick navigation moves the shared owner and echoes the resolved index.
    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    assert!(outcome.raw_messages.iter().any(|message| {
        matches!(message, Msg::Shell(ShellRequest::EmbyLibraryCursorIndex { index: 1 }))
    }));
    assert_eq!(browser_owner(&harness).cursor(), 1);

    // The next frame retains the selection in the shared owner.
    let _ = draw(&mut harness, 100, 30);
    assert_eq!(browser_owner(&harness).cursor(), 1);
}

#[test]
fn tick_play_prompt_mounts_and_accepts_local_fall_through() {
    let mut app = make_movie_app();
    app.panel_focus = crate::app::PanelFocus::Library;
    app.panel_mode = crate::app::PanelMode::LibraryOnly;
    let http = mbv_core::mock_http::MockHttp::new();
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
    http.respond(404, "");
    let (remote, player_rx, command_rx) =
        mbv_core::remote_player::RemotePlayer::stub_audio_only_with_command_rx(Vec::new(), 0);
    let session = make_session("audio-owner", "mbv");
    let endpoint = mbv_core::remote_player::DaemonEndpoint::Tcp("127.0.0.1:0".parse().unwrap());
    app.switch_to_direct_remote(&session, remote, player_rx, &endpoint);
    while command_rx.try_recv().is_ok() {}
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    let _ = draw(&mut harness, 100, 30);

    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Char('p'),
        modifiers: KeyModifiers::CONTROL,
    }));
    let outcome = harness.step();
    assert!(outcome.raw_messages.iter().any(|message| {
        matches!(
            message,
            Msg::Shell(ShellRequest::EmbyLibraryPlay { item })
                if item.id == "movie-focused"
        )
    }));
    let (mut music_resize, mut tv_resize) = (false, false);
    for message in outcome.messages.iter().cloned() {
        harness
            .model_mut()
            .handle_terminal_message(message, &mut music_resize, &mut tv_resize);
    }
    harness.model_mut().sync_mounted_surfaces();

    let confirm_id = ComponentId::Modal(crate::app::components::ModalId::Confirm);
    assert!(harness.model().application.get_component(&confirm_id).is_some());
    assert_eq!(harness.model().application.focus(), Some(&confirm_id));
    assert!(command_rx.try_recv().is_err(), "play stayed unsubmitted");

    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Char('y'),
        modifiers: KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    let (mut music_resize, mut tv_resize) = (false, false);
    for message in outcome.messages {
        harness
            .model_mut()
            .handle_terminal_message(message, &mut music_resize, &mut tv_resize);
    }
    harness.model_mut().sync_mounted_surfaces();

    assert!(harness.model().application.get_component(&confirm_id).is_none());
    assert!(!harness.model().app.player.is_remote());
    assert!(harness.model().app.player.status.lock().unwrap().active);
    assert!(!harness.model().app.direct_remote_connected);
    assert!(harness.model().app.remote_player_tab.is_none());
    assert_eq!(harness.model().app.queue_scope, crate::app::QueueScope::Local);
    let local_items = harness.model().app.player_tab.emby_items();
    assert_eq!(
        local_items.first().expect("local playback queue is non-empty").id,
        "movie-focused"
    );
    assert!(command_rx.try_iter().any(|command| {
        matches!(
            command,
            mbv_core::ctrl::CtrlCmd::PlaybackIntent(intent)
                if intent.action == mbv_core::ctrl::PlaybackIntentAction::Stop
        )
    }));
}

/// Task 3.3: a completed Movie/generic `NavigateLanding::Chain` re-seeds the
/// retained browser owner's cursor onto the navigated movie when the landing
/// changes the browse identity (the 34dbbd55 re-anchor), driven through the
/// shell drain and the `Application::tick()` sync pass.
#[test]
fn navigated_movie_reanchors_the_retained_browser_cursor() {
    use crate::app::state::types::events::NavigateLanding;
    use crate::app::{BrowseLevel, LibEvent};

    let mut app = make_movie_app();
    app.tab = crate::app::TabSelection::EmbyLibrary(0);
    app.panel_focus = crate::app::PanelFocus::Library;
    app.panel_mode = crate::app::PanelMode::LibraryOnly;
    // The retained owner is parked at a drilled-in level, so the landed root
    // level is a real identity change (depth + parent), not a same-level
    // refresh the owner preserves.
    let mut inside = crate::app::tests::make_item("Inside", "Movie");
    inside.id = "movie-inside".into();
    app.libs[0].nav_stack.push(BrowseLevel {
        fetched_rows: 0,
        parent_id: "folder-1".into(),
        title: "Folder".into(),
        items: vec![inside],
        total_count: 1,
        resting: crate::app::state::types::browse::BrowseResting::new(0, 0),
        item_types: None,
        unplayed_only: false,
        sort_by: "SortName".into(),
        sort_order: "Ascending".into(),
        loading: false,
        all_items: None,
        letter_filter: None,
            tv_content_mode: None,
        music_grouping: None,
    });
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    let _terminal = draw(&mut harness, 100, 30);
    assert_eq!(
        browser_owner(&harness).cursor(),
        0,
        "the retained owner starts on the drilled level"
    );

    // The queue's built chain lands at the library root with the cursor on
    // the navigated movie.
    let mut third = crate::app::tests::make_item("Third Movie", "Movie");
    third.id = "movie-third".into();
    let landed = BrowseLevel {
        fetched_rows: 0,
        parent_id: "lib-movies".into(),
        title: "Movies".into(),
        items: vec![
            crate::app::tests::make_item("Focused Movie", "Movie"),
            crate::app::tests::make_item("Second Movie", "Movie"),
            third,
        ],
        total_count: 3,
        resting: crate::app::state::types::browse::BrowseResting::new(2, 0),
        item_types: None,
        unplayed_only: false,
        sort_by: "SortName".into(),
        sort_order: "Ascending".into(),
        loading: false,
        all_items: None,
        letter_filter: None,
            tv_content_mode: None,
        music_grouping: None,
    };
    harness
        .model_mut()
        .handle_inline_search_lib_event(LibEvent::NavigateTo {
            lib_idx: 0,
            landing: NavigateLanding::Chain {
                nav_stack: vec![landed],
            },
            switch_tab: true,
        });
    harness.step();
    harness.model_mut().sync_mounted_surfaces();

    assert_eq!(
        harness.model().app.libs[0].nav_stack[0].resting().cursor(),
        2,
        "the App rests on the navigated movie"
    );
    assert_eq!(
        browser_owner(&harness).cursor(),
        2,
        "the retained browser re-seeds onto the navigated movie"
    );
}
