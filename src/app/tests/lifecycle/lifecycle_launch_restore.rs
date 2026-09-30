use super::*;
use mbv_ui_model::library::LibraryKey;

#[rstest]
#[case::unconfigured(None, mbv_core::service_runtime::ServiceState::NotConfigured)]
#[case::configured_without_credential(
    Some(mbv_config::EmbySetup::new("http://emby.example", "user")),
    mbv_core::service_runtime::ServiceState::NeedsAuthentication
)]
fn saved_service_tab_stays_on_home_keeps_focus_and_later_configure_does_not_move_tab(
    #[case] setup: Option<mbv_config::EmbySetup>,
    #[case] initial_state: mbv_core::service_runtime::ServiceState,
) {
    // #810: a Service unavailable at build resolves once; later setup/catalog arrival cannot yank the tab.
    let mut app = crate::app::tests::render_fixtures::make_movie_app();
    app.config.lock().unwrap().emby_setup = setup;
    app.emby_runtime.state = initial_state;
    app.panel_focus = PanelFocus::Library;
    app.launch_restore = crate::app::state::app_struct::LaunchRestore::Pending(launch_state(
        ServiceKind::Emby,
        "lib-movies",
        mbv_config::LaunchPanelFocus::Queue,
    ));

    app.resolve_library_tab_pending();

    assert_eq!(app.tab, TabSelection::Home);
    assert!(matches!(
        app.launch_restore,
        crate::app::state::app_struct::LaunchRestore::TabSettled {
            tab: TabSelection::Home,
            ..
        }
    ));

    let mut model = Model::new(app);
    model.sync_mounted_surfaces();

    assert_eq!(model.app.panel_focus, PanelFocus::Queue);
    assert_eq!(
        model.app.launch_restore,
        crate::app::state::app_struct::LaunchRestore::Done
    );

    model.app.config.lock().unwrap().emby_setup =
        Some(mbv_config::EmbySetup::new("http://emby.example", "user"));
    let views: Vec<mbv_emby_model::EmbyItem> = model
        .app
        .libs
        .iter()
        .map(|library| library.library.clone())
        .collect();
    model.app.rebuild_library_tabs_from_views(&views);

    assert_eq!(model.app.tab, TabSelection::Home);
}

pub(super) fn launch_state(
    kind: ServiceKind,
    library_id: &str,
    panel_focus: mbv_config::LaunchPanelFocus,
) -> mbv_config::TuiLaunchState {
    mbv_config::TuiLaunchState {
        version: mbv_config::TUI_LAUNCH_STATE_VERSION,
        tab: mbv_config::TabIdentity::ServiceLibrary {
            kind,
            library_id: library_id.into(),
        },
        panel_focus,
        selector: None,
        item: None,
    }
}

fn pending_emby_launch() -> crate::app::App {
    let mut app = crate::app::tests::render_fixtures::make_movie_app();
    app.emby_runtime.state = mbv_core::service_runtime::ServiceState::Connecting;
    app.launch_restore = crate::app::state::app_struct::LaunchRestore::Pending(launch_state(
        ServiceKind::Emby,
        "lib-movies",
        mbv_config::LaunchPanelFocus::Queue,
    ));
    app
}

pub(super) fn assert_expired_launch_restores_saved_focus(app: crate::app::App) {
    assert_eq!(app.tab, TabSelection::Home);
    assert!(matches!(
        app.launch_restore,
        crate::app::state::app_struct::LaunchRestore::TabSettled {
            tab: TabSelection::Home,
            ..
        }
    ));

    let mut model = Model::new(app);
    model.sync_mounted_surfaces();
    assert_eq!(model.app.panel_focus, PanelFocus::Queue);
    assert_eq!(
        model.app.launch_restore,
        crate::app::state::app_struct::LaunchRestore::Done
    );
}

/// #810: failed startup expires only the pending Emby launch and later catalog
/// arrival cannot move the selected tab.
#[test]
fn failed_emby_startup_then_successful_catalog_keeps_tab_unchanged() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = pending_emby_launch();
    let generation = app.emby_runtime.generation();

    app.apply_emby_completion(crate::app::dispatch::session::service_startup::Completion {
        generation,
        result: Err(mbv_emby::EmbyFailure::unavailable("startup failed")),
    });
    assert_eq!(app.tab, TabSelection::Home);

    let views = app
        .libs
        .iter()
        .map(|library| library.library.clone())
        .collect::<Vec<_>>();
    app.rebuild_library_tabs_from_views(&views);
    assert_eq!(app.tab, TabSelection::Home);
}

/// #810: a current startup Err resolves Home and preserves saved Panel focus.
#[test]
fn emby_startup_error_expires_launch_and_restores_saved_focus() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = pending_emby_launch();

    let generation = app.emby_runtime.generation();
    app.apply_emby_completion(crate::app::dispatch::session::service_startup::Completion {
        generation,
        result: Err(mbv_emby::EmbyFailure::unavailable("startup failed")),
    });

    assert_expired_launch_restores_saved_focus(app);
}

/// #810: setup Err resolves Home and preserves saved Panel focus.
#[test]
fn emby_setup_error_expires_launch_and_restores_saved_focus() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = pending_emby_launch();
    let generation = app.emby_runtime.begin_setup();

    app.apply_emby_setup_completion_without_network(
        crate::app::dispatch::session::service_startup::SetupCompletion {
            generation,
            previous_state: mbv_core::service_runtime::ServiceState::NotConfigured,
            result: Err(mbv_emby::EmbyError::resolve("setup failed")),
        },
    );

    assert_expired_launch_restores_saved_focus(app);
}

/// #810: startup-worker disconnect resolves Home and preserves saved Panel focus.
#[test]
fn emby_startup_worker_disconnect_expires_launch_and_restores_saved_focus() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = pending_emby_launch();

    let generation = app.emby_runtime.generation();
    app.handle_emby_startup_worker_disconnect(generation);

    assert_expired_launch_restores_saved_focus(app);
}

/// #810: a later runtime failure resolves Home and preserves saved Panel focus.
#[test]
fn emby_runtime_failure_expires_launch_and_restores_saved_focus() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = pending_emby_launch();

    app.handle_emby_runtime_failure(mbv_emby::EmbyFailure::unavailable("request failed"));

    assert_expired_launch_restores_saved_focus(app);
}

#[test]
fn pending_launch_tab_resolves_after_catalog_arrival_and_restores_existing_tab() {
    let mut app = crate::app::tests::render_fixtures::make_movie_app();
    app.tab = TabSelection::Home;
    app.emby_runtime.state = mbv_core::service_runtime::ServiceState::Connecting;
    let saved = launch_state(
        ServiceKind::Emby,
        "lib-movies",
        mbv_config::LaunchPanelFocus::Library,
    );
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
    app.launch_restore = crate::app::state::app_struct::LaunchRestore::Pending(launch_state(
        ServiceKind::Emby,
        "lib-shows",
        mbv_config::LaunchPanelFocus::Library,
    ));

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
    app.launch_restore = crate::app::state::app_struct::LaunchRestore::Pending(launch_state(
        ServiceKind::Emby,
        "lib-movies",
        mbv_config::LaunchPanelFocus::Library,
    ));
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
    app.launch_restore = crate::app::state::app_struct::LaunchRestore::Pending(launch_state(
        ServiceKind::Emby,
        "lib-movies",
        mbv_config::LaunchPanelFocus::Library,
    ));
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
        state: launch_state(
            ServiceKind::Emby,
            "lib-movies",
            mbv_config::LaunchPanelFocus::Queue,
        ),
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
