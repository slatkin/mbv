use super::*;
use mbv_ui_model::library::LibraryKey;

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

fn pending_emby_launch_at(library_id: &str, panel_focus: mbv_config::LaunchPanelFocus) -> App {
    let mut app = mbv_client_core::tests::render_fixtures::make_movie_app();
    app.seed_emby_service_state_for_test(mbv_core::service_runtime::ServiceState::Connecting);
    app.seed_launch_restore_pending_for_test(launch_state(
        ServiceKind::Emby,
        library_id,
        panel_focus,
    ));
    app
}

fn pending_emby_launch() -> App {
    // The saved focus is Queue, so the displayed queue must not be empty:
    // with the empty-queue column hidden (change `hide-empty-queue-column`,
    // D2) a sync moves the stored focus to Library and the expiry restore
    // would be asserted against a state that no longer holds.
    let mut app = pending_emby_launch_at("lib-movies", mbv_config::LaunchPanelFocus::Queue);
    app.seed_local_queue_emby_items_for_test(vec![make_item("Queue Item", "Movie")], 0);
    app
}

/// Rebuild the library tabs from the app's current libraries, as a catalog arrival does.
pub(super) fn rebuild_tabs(app: &mut App) {
    let views: Vec<mbv_emby_model::EmbyItem> = app
        .emby_libraries()
        .iter()
        .map(|library| library.library.clone())
        .collect();
    app.rebuild_library_tabs_from_views_for_test(&views);
}

pub(super) fn assert_expired_launch_restores_saved_focus(app: App) {
    assert_eq!(app.active_tab(), TabSelection::Home);
    assert_eq!(
        app.launch_restore_tab_settled_for_test()
            .map(|(_, tab)| tab),
        Some(TabSelection::Home)
    );

    let mut model = Model::new(app);
    model.sync_mounted_surfaces();
    assert_eq!(model.app.stored_panel_focus(), PanelFocus::Queue);
    assert!(model.app.launch_restore_settled_for_test());
}

/// #859: every Emby Service failure outcome expires the pending launch the
/// same way — Home tab, settled restore, saved panel focus on the next sync.
#[test]
fn emby_service_outcome_expiry_restores_saved_focus() {
    fn expire(trigger: impl FnOnce(&mut App)) {
        let _guard = mbv_client_core::config::TestStateDirGuard::new();
        let mut app = pending_emby_launch();
        trigger(&mut app);
        assert_expired_launch_restores_saved_focus(app);
    }
    // Startup Err.
    expire(|app| {
        app.apply_emby_startup_failure_for_test(mbv_emby::EmbyFailure::unavailable(
            "startup failed",
        ));
    });
    // Setup Err.
    expire(|app| {
        app.begin_emby_setup_for_test();
        app.apply_emby_setup_failure_for_test(
            mbv_core::service_runtime::ServiceState::NotConfigured,
            mbv_emby::EmbyError::resolve("setup failed"),
        );
    });
    // Startup-worker disconnect.
    expire(|app| {
        let generation = app.emby_setup_generation_for_test();
        app.handle_emby_startup_worker_disconnect_for_test(generation);
    });
    // Later runtime failure.
    expire(|app| {
        app.handle_emby_runtime_failure_for_test(mbv_emby::EmbyFailure::unavailable(
            "request failed",
        ));
    });
}

#[test]
fn pending_launch_tab_resolves_after_catalog_arrival_and_restores_existing_tab() {
    let mut app = pending_emby_launch_at("lib-movies", mbv_config::LaunchPanelFocus::Library);
    app.seed_active_tab_for_test(TabSelection::Home);
    let saved = launch_state(
        ServiceKind::Emby,
        "lib-movies",
        mbv_config::LaunchPanelFocus::Library,
    );
    app.resolve_launch_tab_on_sync();
    assert_eq!(
        app.active_tab(),
        TabSelection::Home,
        "catalog has not arrived yet"
    );
    assert!(app.launch_restore_pending_state_for_test().is_some());

    rebuild_tabs(&mut app);
    assert_eq!(app.active_tab(), TabSelection::EmbyLibrary(0));
    // #810: catalog arrival binds the destination to the selected tab.
    assert_eq!(
        app.launch_restore_tab_settled_for_test(),
        Some((&saved, TabSelection::EmbyLibrary(0)))
    );
}

/// The selected tab must load its library's content, not just select the
/// tab, or the panel's owner is empty and the tab paints blank. #810 regression.
#[test]
fn restored_launch_tab_loads_its_library_content_not_just_the_tab() {
    let mut app = pending_emby_launch_at("lib-shows", mbv_config::LaunchPanelFocus::Library);
    let mut second_library = make_item("Shows", "CollectionFolder");
    second_library.id = "lib-shows".into();
    second_library.is_folder = true;
    second_library.collection_type = "tvshows".into();
    app.push_library_for_test(mbv_ui_model::library_tab::LibraryTab::new(second_library));
    app.seed_active_tab_for_test(TabSelection::Home);

    rebuild_tabs(&mut app);

    assert_eq!(app.active_tab(), TabSelection::EmbyLibrary(1));
    assert_eq!(
        app.emby_library_level_facts(1)
            .map_or(0, |facts| facts.depth),
        1,
        "the restored tab must load its root level"
    );
    assert!(app.emby_library_root_loading(1));
}

#[test]
fn explicit_tab_movement_consumes_pending_launch_tab_before_refresh() {
    let mut app = pending_emby_launch_at("lib-movies", mbv_config::LaunchPanelFocus::Library);
    app.set_library_tab(0);
    // #810: a catalog arriving after an explicit move must not restore the
    // abandoned Service tab over the user's selection.
    rebuild_tabs(&mut app);

    assert_eq!(app.active_tab(), TabSelection::Home);
    assert!(app.launch_restore_settled_for_test());
}

#[test]
fn reanchor_does_not_apply_to_a_tab_changed_by_stale_destination_normalization() {
    // #810: asynchronous catalog removal must not redirect a resolved launch
    // destination's remaining focus state onto the normalized Home tab.
    let mut app = mbv_client_core::tests::render_fixtures::make_movie_app();
    app.set_panel_focus_for_test(PanelFocus::Library);
    app.seed_launch_restore_tab_settled_for_test(
        launch_state(
            ServiceKind::Emby,
            "lib-movies",
            mbv_config::LaunchPanelFocus::Queue,
        ),
        TabSelection::EmbyLibrary(0),
    );
    let mut model = Model::new(app);
    model.app.clear_libraries_for_test();

    assert!(model.app.normalize_stale_browse_destination());
    model.reanchor_pending_launch_destination();

    assert_eq!(model.app.active_tab(), TabSelection::Home);
    assert_eq!(model.app.stored_panel_focus(), PanelFocus::Library);
    assert!(model.app.launch_restore_settled_for_test());
}

#[test]
fn orderly_teardown_writes_only_the_selected_destination_launch_snapshot() {
    let mut model = Model::new(make_app_stub());
    model.app.set_panel_focus_for_test(PanelFocus::Queue);
    model.app.move_local_queue_cursor_for_test(7);
    // The saved focus is Queue, so the displayed queue must not be empty:
    // with the empty-queue column hidden (change `hide-empty-queue-column`,
    // D2) the teardown sync would move the stored focus to Library and the
    // snapshot would record it.
    model
        .app
        .seed_local_queue_emby_items_for_test(vec![make_item("Queue Item", "Movie")], 0);

    let mut selected = make_item("Selected home item", "Movie");
    selected.id = "selected-home-item".into();
    model.update_library_owner(
        &LibraryKey::Home,
        || Box::new(HomeContent::new()),
        |owner| {
            owner.set_snapshot(
                vec![mbv_queue::QueueItem::Emby(Box::new(selected))],
                false,
                Vec::new(),
                0,
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
    // The selected item never persists: restoration always lands on the
    // first row of the restored scope.
    assert_eq!(state.item, None);

    let serialized = std::fs::read_to_string(mbv_config::tui_launch_state_path())
        .expect("serialized launch snapshot");
    assert!(!serialized.contains("queue_cursor"));
    assert!(!serialized.contains("queue_slot"));
    assert!(!serialized.contains("unselected-feed-item"));
    assert!(!serialized.contains("unselected-feed-selector"));
}

/// #745: a UI-state reset abandons pending launch restoration before it
/// selects Home, so a catalog arriving after the reset cannot re-settle the
/// snapshot the reset discarded.
#[test]
fn reset_ui_state_abandons_pending_launch_restore() {
    let mut app = pending_emby_launch_at("lib-movies", mbv_config::LaunchPanelFocus::Library);
    app.seed_active_tab_for_test(TabSelection::Home);
    let mut model = Model::new(app);

    model.reset_ui_state();

    assert_eq!(model.app.active_tab(), TabSelection::Home);
    assert!(model.app.launch_restore_settled_for_test());

    // The late catalog arrival must not restore the discarded location.
    rebuild_tabs(&mut model.app);
    assert_eq!(model.app.active_tab(), TabSelection::Home);
    assert!(model.app.launch_restore_settled_for_test());
}

/// 2026-10-05 user rule: a pinned launch's default screen is always the
/// Queue panel. Both saved sources of Panel focus lose to it — the persisted
/// prefs focus and the saved launch snapshot's focus that the destination
/// re-anchor re-applies — so a Library-focused snapshot cannot land the
/// pinned panel on the Library column.
#[test]
fn pinned_launch_overrides_saved_focus_with_queue() {
    let mut app = pending_emby_launch_at("lib-movies", mbv_config::LaunchPanelFocus::Library);
    app.set_panel_focus_for_test(PanelFocus::Library);

    app.pin_launch_focus_to_queue();

    assert_eq!(app.stored_panel_focus(), PanelFocus::Queue);
    assert_eq!(
        app.launch_restore_pending_state_for_test()
            .map(|state| state.panel_focus),
        Some(mbv_config::LaunchPanelFocus::Queue)
    );
}

/// The same rule when no launch snapshot exists: the persisted prefs focus
/// alone is overridden, and the forcing path tolerates `LaunchRestore::Done`.
#[test]
fn pinned_launch_without_snapshot_still_starts_on_queue() {
    let mut app = mbv_client_core::tests::render_fixtures::make_movie_app();
    app.set_panel_focus_for_test(PanelFocus::Library);
    app.seed_launch_restore_done_for_test();

    app.pin_launch_focus_to_queue();

    assert_eq!(app.stored_panel_focus(), PanelFocus::Queue);
    assert!(app.launch_restore_settled_for_test());
}

/// Spec `destination-latest-modes`, scenario "Restart selects Latest"
/// (change `latest-pill-restart-default`): a restart after a session that
/// exited a Latest-bearing library with a letter range selected opens that
/// library on Latest — the snapshot records the Latest scope instead of the
/// live letter pill, and restoration applies it.
#[test]
fn restart_after_a_letter_pill_session_selects_latest() {
    // Exit with a letter pill active on a large movie library.
    let mut app = mbv_client_core::tests::render_fixtures::make_movie_app();
    app.set_library_total_for_test(0, 400);
    app.set_library_level_letter_filter_for_test(
        0,
        0,
        mbv_ui_model::sort_filter::LetterFilter::default_filter_for_kind(
            mbv_ui_model::sort_filter::LetterFilterKind::Movie,
        ),
    );
    let mut model = Model::new(app);
    model.sync_mounted_surfaces();
    model.teardown(Duration::from_secs(1));

    let saved = mbv_config::load_tui_launch_state().expect("launch snapshot after teardown");
    assert_eq!(
        saved.selector,
        Some(mbv_config::SelectorIdentity::Emby {
            key: mbv_config::EmbySelectorKey::Latest,
        }),
        "the live letter pill never persists; the snapshot records the Latest scope"
    );

    // Restart on the same library: the selector opens on Latest and the
    // letter pill does not reappear.
    let mut app = mbv_client_core::tests::render_fixtures::make_movie_app();
    app.set_library_total_for_test(0, 400);
    app.seed_launch_restore_tab_settled_for_test(saved, TabSelection::EmbyLibrary(0));
    let mut model = Model::new(app);
    model.sync_mounted_surfaces();
    assert!(
        model.active_emby_library_owner_is_latest(),
        "the restored library opens on Latest"
    );
    assert!(model.app.root_letter_filter_is_unset_for_test(0));
}

/// 2026-10-10 user amendment to change `latest-pill-restart-default`
/// (spec `destination-latest-modes`, scenario "Exit outside the
/// pill-bearing view"): a teardown inside a folder paints no selector row,
/// so the exit snapshot records no scope — the restored library still
/// opens on Latest instead of its first-load default pill (the first
/// letter range).
#[test]
fn restart_with_an_absent_recorded_scope_selects_latest() {
    let mut app = mbv_client_core::tests::render_fixtures::make_movie_app();
    app.set_library_total_for_test(0, 400);
    app.seed_launch_restore_tab_settled_for_test(
        launch_state(
            ServiceKind::Emby,
            "lib-movies",
            mbv_config::LaunchPanelFocus::Library,
        ),
        TabSelection::EmbyLibrary(0),
    );
    let mut model = Model::new(app);
    model.sync_mounted_surfaces();

    assert!(
        model.active_emby_library_owner_is_latest(),
        "an absent recorded scope still restores Latest"
    );
}
