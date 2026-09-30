use super::*;
use mbv_ui_model::library::LibraryKey;

#[test]
fn pending_launch_tab_resolves_after_catalog_arrival_and_restores_existing_tab() {
    let mut app = crate::app::tests::render_fixtures::make_movie_app();
    app.tab = TabSelection::Home;
    let saved = mbv_config::TuiLaunchState {
        version: mbv_config::TUI_LAUNCH_STATE_VERSION,
        tab: mbv_config::TabIdentity::ServiceLibrary {
            kind: ServiceKind::Emby,
            library_id: "lib-movies".into(),
        },
        panel_focus: mbv_config::LaunchPanelFocus::Library,
        selector: None,
        item: None,
    };
    app.launch_restore = crate::app::state::app_struct::LaunchRestore::Pending(saved.clone());
    app.resolve_library_tab_pending();
    assert_eq!(app.tab, TabSelection::Home, "catalog has not arrived yet");
    assert!(matches!(
        app.launch_restore,
        crate::app::state::app_struct::LaunchRestore::Pending(_)
    ));

    let views: Vec<mbv_emby_model::EmbyItem> = app
        .libs
        .iter()
        .map(|library| library.library.clone())
        .collect();
    app.rebuild_library_tabs_from_views(&views);
    assert_eq!(app.tab, TabSelection::EmbyLibrary(0));
    // #810: catalog arrival binds the destination to the selected tab.
    assert_eq!(
        app.launch_restore,
        crate::app::state::app_struct::LaunchRestore::TabSettled {
            state: saved,
            tab: TabSelection::EmbyLibrary(0),
        }
    );
}

/// The selected tab must load its library's content, not just select the
/// tab, or the panel's owner is empty and the tab paints blank. #810 regression.
#[test]
fn restored_launch_tab_loads_its_library_content_not_just_the_tab() {
    let mut app = crate::app::tests::render_fixtures::make_movie_app();
    let mut second_library = make_item("Shows", "CollectionFolder");
    second_library.id = "lib-shows".into();
    second_library.is_folder = true;
    second_library.collection_type = "tvshows".into();
    app.libs.push(crate::app::LibraryTab::new(second_library));
    app.tab = TabSelection::Home;
    app.launch_restore =
        crate::app::state::app_struct::LaunchRestore::Pending(mbv_config::TuiLaunchState {
            version: mbv_config::TUI_LAUNCH_STATE_VERSION,
            tab: mbv_config::TabIdentity::ServiceLibrary {
                kind: ServiceKind::Emby,
                library_id: "lib-shows".into(),
            },
            panel_focus: mbv_config::LaunchPanelFocus::Library,
            selector: None,
            item: None,
        });

    let views: Vec<mbv_emby_model::EmbyItem> = app
        .libs
        .iter()
        .map(|library| library.library.clone())
        .collect();
    app.rebuild_library_tabs_from_views(&views);

    assert_eq!(app.tab, TabSelection::EmbyLibrary(1));
    assert!(matches!(
        app.launch_restore,
        crate::app::state::app_struct::LaunchRestore::TabSettled { .. }
    ));
    assert_eq!(
        app.libs[1].nav_stack.len(),
        1,
        "the restored tab must load its root level"
    );
    assert!(app.libs[1].nav_stack[0].loading);
    assert!(matches!(
        app.launch_restore,
        crate::app::state::app_struct::LaunchRestore::TabSettled { .. }
    ));
}

/// A daemon attach has a live Emby client but no startup worker; `fetch_home`
/// supplies the catalog and must resolve launch restoration (#810).
#[test]
fn daemon_attach_fetch_home_restores_service_tab_without_startup_worker() {
    let mut app = crate::app::tests::render_fixtures::make_movie_app();
    app.tab = TabSelection::Home;
    app.launch_restore =
        crate::app::state::app_struct::LaunchRestore::Pending(mbv_config::TuiLaunchState {
            version: mbv_config::TUI_LAUNCH_STATE_VERSION,
            tab: mbv_config::TabIdentity::ServiceLibrary {
                kind: ServiceKind::Emby,
                library_id: "lib-movies".into(),
            },
            panel_focus: mbv_config::LaunchPanelFocus::Library,
            selector: None,
            item: None,
        });
    let http = mbv_net::mock_http::MockHttp::new();
    http.respond(
        200,
        r#"[{"ItemId":"lib-movies","Name":"Movies","CollectionType":"movies"}]"#,
    );
    http.respond(200, r#"{"Items":[]}"#);
    http.respond(200, r#"{"Items":[]}"#);
    app.config.lock().unwrap().server_url = "http://127.0.0.1:1".into();
    let mut client =
        mbv_emby::EmbyClient::new(app.config.lock().unwrap().clone()).with_test_agent(http.agent());
    client.user_id = "user".into();
    app.emby_runtime = crate::app::state::service_runtime::EmbyRuntime::ready(std::sync::Arc::new(
        std::sync::Mutex::new(client),
    ));

    app.fetch_home().expect("daemon-attach catalog fetch");

    assert_eq!(app.tab, TabSelection::EmbyLibrary(0));
    assert!(matches!(
        app.launch_restore,
        crate::app::state::app_struct::LaunchRestore::TabSettled { .. }
    ));
}

#[test]
fn explicit_tab_movement_consumes_pending_launch_tab_before_refresh() {
    let mut app = crate::app::tests::render_fixtures::make_movie_app();
    app.launch_restore =
        crate::app::state::app_struct::LaunchRestore::Pending(mbv_config::TuiLaunchState {
            version: mbv_config::TUI_LAUNCH_STATE_VERSION,
            tab: mbv_config::TabIdentity::ServiceLibrary {
                kind: ServiceKind::Emby,
                library_id: "lib-movies".into(),
            },
            panel_focus: mbv_config::LaunchPanelFocus::Library,
            selector: None,
            item: None,
        });
    app.set_library_tab(0);
    // #810: a catalog arriving after an explicit move must not restore the
    // abandoned Service tab over the user's selection.
    let views: Vec<mbv_emby_model::EmbyItem> = app
        .libs
        .iter()
        .map(|library| library.library.clone())
        .collect();
    app.rebuild_library_tabs_from_views(&views);

    assert_eq!(app.tab, TabSelection::Home);
    assert_eq!(
        app.launch_restore,
        crate::app::state::app_struct::LaunchRestore::Done
    );
}

#[test]
fn reanchor_does_not_apply_to_a_tab_changed_by_stale_destination_normalization() {
    // #810: asynchronous catalog removal must not redirect a resolved launch
    // destination's remaining focus state onto the normalized Home tab.
    let mut app = crate::app::tests::render_fixtures::make_movie_app();
    app.panel_focus = PanelFocus::Library;
    app.launch_restore = crate::app::state::app_struct::LaunchRestore::TabSettled {
        state: mbv_config::TuiLaunchState {
            version: mbv_config::TUI_LAUNCH_STATE_VERSION,
            tab: mbv_config::TabIdentity::ServiceLibrary {
                kind: ServiceKind::Emby,
                library_id: "lib-movies".into(),
            },
            panel_focus: mbv_config::LaunchPanelFocus::Queue,
            selector: None,
            item: None,
        },
        tab: TabSelection::EmbyLibrary(0),
    };
    let mut model = Model::new(app);
    model.app.libs.clear();

    assert!(model.app.normalize_stale_browse_destination());
    model.reanchor_pending_launch_destination();

    assert_eq!(model.app.tab, TabSelection::Home);
    assert_eq!(model.app.panel_focus, PanelFocus::Library);
    assert_eq!(
        model.app.launch_restore,
        crate::app::state::app_struct::LaunchRestore::Done
    );
}

#[test]
fn orderly_teardown_writes_only_the_selected_destination_launch_snapshot() {
    let mut model = Model::new(make_app_stub());
    model.app.panel_focus = PanelFocus::Queue;
    model.app.local_view.set_cursor(7);

    let mut selected = make_item("Selected home item", "Movie");
    selected.id = "selected-home-item".into();
    model.update_library_owner(
        &LibraryKey::Home,
        || Box::new(HomeContent::new()),
        |owner| {
            owner.set_content(vec![mbv_queue::QueueItem::Emby(Box::new(selected))], false);
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
        &LibraryKey::Feeds,
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

    let state = mbv_config::load_tui_launch_state().expect("launch snapshot after teardown");
    assert_eq!(state.tab, mbv_config::TabIdentity::Home);
    assert_eq!(state.panel_focus, mbv_config::LaunchPanelFocus::Queue);
    assert_eq!(
        state.selector,
        Some(mbv_config::SelectorIdentity::Home {
            key: mbv_config::HomeSelectorKey::Continue,
        })
    );
    assert_eq!(
        state.item,
        Some(mbv_config::LibraryItemIdentity::Home {
            id: "selected-home-item".into(),
        })
    );

    let serialized = std::fs::read_to_string(mbv_config::tui_launch_state_path())
        .expect("serialized launch snapshot");
    assert!(!serialized.contains("queue_cursor"));
    assert!(!serialized.contains("queue_slot"));
    assert!(!serialized.contains("unselected-feed-item"));
    assert!(!serialized.contains("unselected-feed-selector"));
}
