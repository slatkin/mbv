use super::super::*;
use super::*;

#[test]
fn pending_launch_tab_resolves_after_catalog_arrival_and_restores_existing_tab() {
    let mut app = crate::app::render::make_movie_app();
    app.tab = TabSelection::Home;
    app.pending_launch_state = Some(mbv_core::config::TuiLaunchState {
        version: mbv_core::config::TUI_LAUNCH_STATE_VERSION,
        tab: mbv_core::config::TabIdentity::ServiceLibrary {
            kind: ServiceKind::Emby,
            library_id: "lib-movies".into(),
        },
        panel_focus: mbv_core::config::LaunchPanelFocus::Library,
        selector: None,
        item: None,
    });
    app.resolve_library_tab_pending();
    assert_eq!(
        app.tab,
        TabSelection::Home,
        "catalog identity is not ready yet"
    );
    assert!(!app.pending_launch_tab_resolved);

    app.emby_catalog_ready = true;
    app.resolve_library_tab_pending();
    assert_eq!(app.tab, TabSelection::EmbyLibrary(0));
    assert!(app.pending_launch_tab_resolved);
    assert!(
        app.pending_launch_state.is_some(),
        "destination state remains for 3.2"
    );
}

/// A local-daemon/remote launch attaches a live Emby client at construction
/// and never spawns the Emby startup worker, so `apply_emby_bootstrap` never
/// runs. Its live catalog arrives through `fetch_home`'s view rebuild, which
/// must mark the catalog ready or the saved launch tab never resolves. The
/// resolved tab must also load its library's content, not just select the
/// tab, or the panel's owner is empty and the tab paints blank.
#[test]
fn restored_launch_tab_loads_its_library_content_not_just_the_tab() {
    let mut app = crate::app::render::make_movie_app();
    app.emby_catalog_ready = false;

    let mut second_library = make_item("Shows", "CollectionFolder");
    second_library.id = "lib-shows".into();
    second_library.is_folder = true;
    second_library.collection_type = "tvshows".into();
    app.libs.push(crate::app::LibraryTab::new(second_library));

    let views: Vec<mbv_core::api::EmbyItem> =
        app.libs.iter().map(|lib| lib.library.clone()).collect();
    app.rebuild_library_tabs_from_views(&views);
    assert!(
        app.emby_catalog_ready,
        "rebuilding tabs from live views is the Emby catalog boundary"
    );

    app.tab = TabSelection::Home;
    app.pending_launch_state = Some(mbv_core::config::TuiLaunchState {
        version: mbv_core::config::TUI_LAUNCH_STATE_VERSION,
        tab: mbv_core::config::TabIdentity::ServiceLibrary {
            kind: ServiceKind::Emby,
            library_id: "lib-shows".into(),
        },
        panel_focus: mbv_core::config::LaunchPanelFocus::Library,
        selector: None,
        item: None,
    });

    app.resolve_library_tab_pending();

    assert_eq!(app.tab, TabSelection::EmbyLibrary(1));
    assert!(app.pending_launch_tab_resolved);
    assert_eq!(
        app.libs[1].nav_stack.len(),
        1,
        "the restored tab must load its root level"
    );
    assert!(app.libs[1].nav_stack[0].loading);
    assert!(
        app.pending_launch_state.is_some(),
        "destination state remains for the pill/item re-anchor"
    );
}

#[test]
fn pending_launch_tab_missing_stable_service_id_falls_back_to_home_even_with_catalog_entry() {
    let mut app = crate::app::render::make_movie_app();
    assert_eq!(
        app.libs.len(),
        1,
        "the current catalog must have a service destination"
    );
    app.pending_launch_state = Some(mbv_core::config::TuiLaunchState {
        version: mbv_core::config::TUI_LAUNCH_STATE_VERSION,
        tab: mbv_core::config::TabIdentity::ServiceLibrary {
            kind: ServiceKind::Emby,
            library_id: "gone".into(),
        },
        panel_focus: mbv_core::config::LaunchPanelFocus::Library,
        selector: None,
        item: None,
    });
    app.emby_catalog_ready = true;

    app.resolve_library_tab_pending();

    assert_eq!(app.tab, TabSelection::Home);
    assert!(app.pending_launch_tab_resolved);
}

#[test]
fn explicit_tab_movement_consumes_pending_launch_tab_before_refresh() {
    let mut app = crate::app::render::make_movie_app();
    app.pending_launch_state = Some(mbv_core::config::TuiLaunchState {
        version: mbv_core::config::TUI_LAUNCH_STATE_VERSION,
        tab: mbv_core::config::TabIdentity::ServiceLibrary {
            kind: ServiceKind::Emby,
            library_id: "lib-movies".into(),
        },
        panel_focus: mbv_core::config::LaunchPanelFocus::Library,
        selector: None,
        item: None,
    });
    app.set_library_tab(0);
    app.emby_catalog_ready = true;
    app.resolve_library_tab_pending();

    assert_eq!(app.tab, TabSelection::Home);
    assert!(!app.pending_launch_tab_resolved);
    assert!(app.pending_launch_state.is_none());
}

#[test]
fn tick_restores_queue_panel_focus_after_destination_ready_without_queue_target() {
    let mut app = make_app_stub();
    app.panel_focus = PanelFocus::Library;
    app.player_tab.set_items(make_items(3), 0);
    app.player_tab.queue_cursor = 99;
    let mut harness = TickHarness::new(app);
    harness.model_mut().app.pending_launch_tab_resolved = true;
    harness.model_mut().app.pending_launch_state = Some(mbv_core::config::TuiLaunchState {
        version: mbv_core::config::TUI_LAUNCH_STATE_VERSION,
        tab: mbv_core::config::TabIdentity::Home,
        panel_focus: mbv_core::config::LaunchPanelFocus::Queue,
        selector: None,
        item: None,
    });
    harness.model_mut().update_home_owner(|home| {
        home.set_content(
            vec![mbv_core::playback_queue::QueueItem::Emby(Box::new(
                make_item("Home item", "Movie"),
            ))],
            false,
        );
    });

    // This is the production Application::tick path: the sync pass before
    // the tick waits for destination readiness, then restores the saved Panel
    // focus without supplying any Queue target.
    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Char('z'),
        modifiers: KeyModifiers::NONE,
    }));
    harness.step();

    assert_eq!(
        harness.model().application.focus(),
        Some(&ComponentId::Queue),
        "Queue receives framework focus after destination restoration"
    );
    assert_eq!(
        harness.model().app.effective_panel_focus(),
        PanelFocus::Queue
    );
    assert_eq!(
        harness.model().app.player_tab.queue_cursor,
        0,
        "Queue selection follows normal initialization, not launch state"
    );
    let queue = harness
        .model()
        .application
        .get_component(&ComponentId::Queue)
        .and_then(|component| {
            component
                .as_any()
                .downcast_ref::<crate::app::components::QueueComponent>()
        })
        .expect("Queue mounted");
    assert_eq!(queue.test_cursor(), 0);
}

#[test]
fn destination_reanchor_consumes_pending_state_before_a_later_refresh() {
    let mut harness = TickHarness::new(make_app_stub());
    harness.model_mut().sync_mounted_surfaces();
    harness.model_mut().app.tab = TabSelection::Home;
    harness.model_mut().app.pending_launch_tab_resolved = true;
    harness.model_mut().app.pending_launch_state = Some(mbv_core::config::TuiLaunchState {
        version: mbv_core::config::TUI_LAUNCH_STATE_VERSION,
        tab: mbv_core::config::TabIdentity::Home,
        panel_focus: mbv_core::config::LaunchPanelFocus::Library,
        selector: Some(mbv_core::config::SelectorIdentity::Home {
            key: mbv_core::config::HomeSelectorKey::Continue,
        }),
        item: Some(mbv_core::config::LibraryItemIdentity::Home { id: "first".into() }),
    });
    harness.model_mut().update_home_owner(|home| {
        let mut first = make_item("First", "Movie");
        first.id = "first".into();
        home.set_content(
            vec![mbv_core::playback_queue::QueueItem::Emby(Box::new(first))],
            false,
        );
    });

    // The real Application::tick path runs the shell sync pass before the
    // injected event; the pending intent must be consumed there, not by a
    // direct helper call that bypasses composition.
    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Char('z'),
        modifiers: KeyModifiers::NONE,
    }));
    harness.step();
    assert!(harness.model().app.pending_launch_state.is_none());
    assert!(!harness.model().app.pending_launch_tab_resolved);

    // A later owner refresh has no pending intent available to replay.
    harness.model_mut().reanchor_pending_launch_destination();
    assert!(harness.model().app.pending_launch_state.is_none());
}

#[test]
fn orderly_teardown_writes_only_the_selected_destination_launch_snapshot() {
    let mut model = Model::new(make_app_stub());
    model.app.panel_focus = PanelFocus::Queue;
    model.app.player_tab.queue_cursor = 7;

    let mut selected = make_item("Selected home item", "Movie");
    selected.id = "selected-home-item".into();
    model.update_library_owner(
        LibraryKey::Home,
        || Box::new(HomeContent::new()),
        |owner| {
            owner.set_content(
                vec![mbv_core::playback_queue::QueueItem::Emby(Box::new(
                    selected,
                ))],
                false,
            );
        },
    );

    let unselected_feed_item = FeedEntry {
        guid: "unselected-feed-item".into(),
        title: "Unselected feed item".into(),
        enclosure_url: Some("https://example.test/unselected-feed-item.mp3".into()),
        link: None,
        mime_type: Some("audio/mpeg".into()),
        duration_ticks: None,
        pub_date_secs: None,
        feed_kind: Some(FeedKind::Audio),
        feed_id: Some("unselected-feed-selector".into()),
        position_ticks: 0,
        played: false,
    };
    model.update_library_owner(
        LibraryKey::Feeds,
        || Box::new(FeedsContent::new()),
        |owner| {
            owner.set_content(FeedsOwnerPush {
                subscriptions: vec![FeedSubscription {
                    name: "Unselected feed".into(),
                    url: "https://example.test/unselected-feed-selector".into(),
                    kind: FeedKind::Audio,
                }],
                entries: vec![vec![unselected_feed_item.clone()]],
                all_entries: vec![unselected_feed_item],
                loading: false,
            });
            owner.cycle_group(1);
        },
    );
    model.sync_library_panel();

    model.teardown(Duration::from_secs(1));

    let state = mbv_core::config::load_tui_launch_state().expect("launch snapshot after teardown");
    assert_eq!(state.tab, mbv_core::config::TabIdentity::Home);
    assert_eq!(state.panel_focus, mbv_core::config::LaunchPanelFocus::Queue);
    assert_eq!(
        state.selector,
        Some(mbv_core::config::SelectorIdentity::Home {
            key: mbv_core::config::HomeSelectorKey::Continue,
        })
    );
    assert_eq!(
        state.item,
        Some(mbv_core::config::LibraryItemIdentity::Home {
            id: "selected-home-item".into(),
        })
    );

    let serialized = std::fs::read_to_string(mbv_core::config::tui_launch_state_path())
        .expect("serialized launch snapshot");
    assert!(!serialized.contains("queue_cursor"));
    assert!(!serialized.contains("queue_slot"));
    assert!(!serialized.contains("unselected-feed-item"));
    assert!(!serialized.contains("unselected-feed-selector"));
}

#[test]
fn two_apps_diverge_in_memory_and_last_orderly_exit_replaces_whole_snapshot() {
    let _guard = crate::config::TestStateDirGuard::new();

    let mut first = Model::new(make_app_stub());
    first.app.panel_focus = PanelFocus::Library;
    let mut first_item = make_item("First app item", "Movie");
    first_item.id = "first-app-item".into();
    first.update_library_owner(
        LibraryKey::Home,
        || Box::new(HomeContent::new()),
        |owner| {
            owner.set_content(
                vec![mbv_core::playback_queue::QueueItem::Emby(Box::new(
                    first_item,
                ))],
                false,
            );
        },
    );
    first.sync_library_panel();

    let mut second = Model::new(make_app_stub());
    second.app.tab = TabSelection::Feeds;
    second.app.panel_focus = PanelFocus::Queue;
    let second_feed_item = FeedEntry {
        guid: "second-app-item".into(),
        title: "Second app item".into(),
        enclosure_url: Some("https://example.test/second-app-item.mp3".into()),
        link: None,
        mime_type: Some("audio/mpeg".into()),
        duration_ticks: None,
        pub_date_secs: None,
        feed_kind: Some(FeedKind::Audio),
        feed_id: Some("second-app-feed".into()),
        position_ticks: 0,
        played: false,
    };
    second.update_library_owner(
        LibraryKey::Feeds,
        || Box::new(FeedsContent::new()),
        |owner| {
            owner.set_content(FeedsOwnerPush {
                subscriptions: vec![FeedSubscription {
                    name: "Second app feed".into(),
                    url: "https://example.test/second-app-feed".into(),
                    kind: FeedKind::Audio,
                }],
                entries: vec![vec![second_feed_item.clone()]],
                all_entries: vec![second_feed_item],
                loading: false,
            });
            owner.cycle_group(1);
        },
    );
    second.sync_library_panel();

    let first_state = mbv_core::config::TuiLaunchState {
        version: mbv_core::config::TUI_LAUNCH_STATE_VERSION,
        tab: mbv_core::config::TabIdentity::Home,
        panel_focus: mbv_core::config::LaunchPanelFocus::Library,
        selector: Some(mbv_core::config::SelectorIdentity::Home {
            key: mbv_core::config::HomeSelectorKey::Continue,
        }),
        item: Some(mbv_core::config::LibraryItemIdentity::Home {
            id: "first-app-item".into(),
        }),
    };
    let second_state = mbv_core::config::TuiLaunchState {
        version: mbv_core::config::TUI_LAUNCH_STATE_VERSION,
        tab: mbv_core::config::TabIdentity::Feeds,
        panel_focus: mbv_core::config::LaunchPanelFocus::Queue,
        selector: Some(mbv_core::config::SelectorIdentity::Feeds {
            key: mbv_core::config::FeedsSelectorKey::Group(mbv_core::config::FeedGroupKey::Feed(
                "https://example.test/second-app-feed".into(),
            )),
        }),
        item: Some(mbv_core::config::LibraryItemIdentity::Feeds {
            id: "second-app-item".into(),
        }),
    };

    assert_eq!(mbv_core::config::load_tui_launch_state(), None);
    assert_eq!(second.launch_state_snapshot(), second_state);

    first.teardown(Duration::from_secs(1));
    assert_eq!(
        mbv_core::config::load_tui_launch_state(),
        Some(first_state.clone())
    );
    // The first App's exit must not alter the second App's independent
    // component-owned launch state before its own orderly exit.
    assert_eq!(second.launch_state_snapshot(), second_state);

    second.teardown(Duration::from_secs(1));
    assert_eq!(
        mbv_core::config::load_tui_launch_state(),
        Some(second_state)
    );
}

#[test]
fn mounted_tick_navigation_does_not_write_launch_snapshot() {
    let _guard = crate::config::TestStateDirGuard::new();
    let initial = mbv_core::config::TuiLaunchState {
        version: mbv_core::config::TUI_LAUNCH_STATE_VERSION,
        tab: mbv_core::config::TabIdentity::Home,
        panel_focus: mbv_core::config::LaunchPanelFocus::Library,
        selector: None,
        item: None,
    };
    mbv_core::config::save_tui_launch_state(&initial).expect("save launch fixture");
    let before = std::fs::read(mbv_core::config::tui_launch_state_path()).expect("read fixture");

    let mut harness = TickHarness::new(crate::app::render::make_movie_app());
    harness.model_mut().update_home_owner(|home| {
        home.set_content(
            vec![
                mbv_core::playback_queue::QueueItem::Emby(Box::new(make_item(
                    "Movie one",
                    "Movie",
                ))),
                mbv_core::playback_queue::QueueItem::Emby(Box::new(make_item(
                    "Movie two",
                    "Movie",
                ))),
            ],
            false,
        );
    });
    harness.model_mut().sync_mounted_surfaces();
    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    }));
    let _ = harness.step();

    let requests = [
        ShellRequest::HomeRowClick {
            target: HomeRowTarget {
                item_id: Some("id1".into()),
                source: None,
                from_continue_watching: true,
            },
        },
        ShellRequest::QueueRowClick { slot_id: None },
        ShellRequest::LibraryPanelFocus,
        ShellRequest::TabSelect(1),
    ];
    for request in requests {
        let (mut music_resize, mut tv_resize) = (false, false);
        harness.model_mut().handle_terminal_message(
            Msg::Shell(request),
            &mut music_resize,
            &mut tv_resize,
        );
        let after = std::fs::read(mbv_core::config::tui_launch_state_path())
            .expect("launch snapshot remains present");
        assert_eq!(after, before, "live navigation must not write launch state");
    }
}

#[test]
fn orderly_teardown_falls_back_to_home_for_stale_service_tab_without_active_destination() {
    let mut model = Model::new(make_app_stub());
    model.app.tab = TabSelection::EmbyLibrary(0);
    model.app.panel_focus = PanelFocus::Queue;

    model.teardown(Duration::from_secs(1));

    let state = mbv_core::config::load_tui_launch_state().expect("launch snapshot after teardown");
    assert_eq!(state.tab, mbv_core::config::TabIdentity::Home);
    assert_eq!(state.panel_focus, mbv_core::config::LaunchPanelFocus::Queue);
    assert_eq!(state.selector, None);
    assert_eq!(state.item, None);
}
