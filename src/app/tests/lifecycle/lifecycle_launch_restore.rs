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
    let mut app = crate::app::tests::render_fixtures::make_movie_app();
    app.emby_runtime.state = mbv_core::service_runtime::ServiceState::Connecting;
    app.launch_restore = crate::app::state::app_struct::LaunchRestore::Pending(launch_state(
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
    app.local_view
        .adopt_items(vec![make_item("Queue Item", "Movie")], 0);
    app
}

/// Rebuild the library tabs from the app's current libraries, as a catalog arrival does.
pub(super) fn rebuild_tabs(app: &mut App) {
    let views: Vec<mbv_emby_model::EmbyItem> = app
        .libs
        .iter()
        .map(|library| library.library.clone())
        .collect();
    app.rebuild_library_tabs_from_views(&views);
}

pub(super) fn assert_expired_launch_restores_saved_focus(app: App) {
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

/// #859: every Emby Service failure outcome expires the pending launch the
/// same way — Home tab, settled restore, saved panel focus on the next sync.
#[test]
fn emby_service_outcome_expiry_restores_saved_focus() {
    fn expire(trigger: impl FnOnce(&mut App)) {
        let _guard = crate::config::TestStateDirGuard::new();
        let mut app = pending_emby_launch();
        trigger(&mut app);
        assert_expired_launch_restores_saved_focus(app);
    }
    // Startup Err.
    expire(|app| {
        let generation = app.emby_runtime.generation();
        app.apply_emby_completion(crate::app::dispatch::session::service_startup::Completion {
            generation,
            result: Err(mbv_emby::EmbyFailure::unavailable("startup failed")),
        });
    });
    // Setup Err.
    expire(|app| {
        let generation = app.emby_runtime.begin_setup();
        app.apply_emby_setup_completion_without_network(
            crate::app::dispatch::session::service_startup::SetupCompletion {
                generation,
                previous_state: mbv_core::service_runtime::ServiceState::NotConfigured,
                result: Err(mbv_emby::EmbyError::resolve("setup failed")),
            },
        );
    });
    // Startup-worker disconnect.
    expire(|app| {
        let generation = app.emby_runtime.generation();
        app.handle_emby_startup_worker_disconnect(generation);
    });
    // Later runtime failure.
    expire(|app| {
        app.handle_emby_runtime_failure(mbv_emby::EmbyFailure::unavailable("request failed"));
    });
}

#[test]
fn pending_launch_tab_resolves_after_catalog_arrival_and_restores_existing_tab() {
    let mut app = pending_emby_launch_at("lib-movies", mbv_config::LaunchPanelFocus::Library);
    app.tab = TabSelection::Home;
    let saved = launch_state(
        ServiceKind::Emby,
        "lib-movies",
        mbv_config::LaunchPanelFocus::Library,
    );
    app.resolve_launch_tab_on_sync();
    assert_eq!(app.tab, TabSelection::Home, "catalog has not arrived yet");
    assert!(matches!(
        app.launch_restore,
        crate::app::state::app_struct::LaunchRestore::Pending(_)
    ));

    rebuild_tabs(&mut app);
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
    let mut app = pending_emby_launch_at("lib-shows", mbv_config::LaunchPanelFocus::Library);
    let mut second_library = make_item("Shows", "CollectionFolder");
    second_library.id = "lib-shows".into();
    second_library.is_folder = true;
    second_library.collection_type = "tvshows".into();
    app.libs.push(crate::app::LibraryTab::new(second_library));
    app.tab = TabSelection::Home;

    rebuild_tabs(&mut app);

    assert_eq!(app.tab, TabSelection::EmbyLibrary(1));
    assert_eq!(
        app.libs[1].nav_stack.len(),
        1,
        "the restored tab must load its root level"
    );
    assert!(app.libs[1].nav_stack[0].loading);
}

#[test]
fn explicit_tab_movement_consumes_pending_launch_tab_before_refresh() {
    let mut app = pending_emby_launch_at("lib-movies", mbv_config::LaunchPanelFocus::Library);
    app.set_library_tab(0);
    // #810: a catalog arriving after an explicit move must not restore the
    // abandoned Service tab over the user's selection.
    rebuild_tabs(&mut app);

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
    // The saved focus is Queue, so the displayed queue must not be empty:
    // with the empty-queue column hidden (change `hide-empty-queue-column`,
    // D2) the teardown sync would move the stored focus to Library and the
    // snapshot would record it.
    model
        .app
        .local_view
        .adopt_items(vec![make_item("Queue Item", "Movie")], 0);

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

/// #745: a UI-state reset abandons pending launch restoration before it
/// selects Home, so a catalog arriving after the reset cannot re-settle the
/// snapshot the reset discarded.
#[test]
fn reset_ui_state_abandons_pending_launch_restore() {
    let mut app = pending_emby_launch_at("lib-movies", mbv_config::LaunchPanelFocus::Library);
    app.tab = TabSelection::Home;
    let mut model = Model::new(app);

    model.reset_ui_state();

    assert_eq!(model.app.tab, TabSelection::Home);
    assert_eq!(
        model.app.launch_restore,
        crate::app::state::app_struct::LaunchRestore::Done
    );

    // The late catalog arrival must not restore the discarded location.
    rebuild_tabs(&mut model.app);
    assert_eq!(model.app.tab, TabSelection::Home);
    assert_eq!(
        model.app.launch_restore,
        crate::app::state::app_struct::LaunchRestore::Done
    );
}

/// 2026-10-05 user rule: a pinned launch's default screen is always the
/// Queue panel. Both saved sources of Panel focus lose to it — the persisted
/// prefs focus and the saved launch snapshot's focus that the destination
/// re-anchor re-applies — so a Library-focused snapshot cannot land the
/// pinned panel on the Library column.
#[test]
fn pinned_launch_overrides_saved_focus_with_queue() {
    let mut app = pending_emby_launch_at("lib-movies", mbv_config::LaunchPanelFocus::Library);
    app.panel_focus = PanelFocus::Library;

    app.pin_launch_focus_to_queue();

    assert_eq!(app.panel_focus, PanelFocus::Queue);
    assert!(matches!(
        app.launch_restore,
        crate::app::state::app_struct::LaunchRestore::Pending(mbv_config::TuiLaunchState {
            panel_focus: mbv_config::LaunchPanelFocus::Queue,
            ..
        })
    ));
}

/// The same rule when no launch snapshot exists: the persisted prefs focus
/// alone is overridden, and the forcing path tolerates `LaunchRestore::Done`.
#[test]
fn pinned_launch_without_snapshot_still_starts_on_queue() {
    let mut app = crate::app::tests::render_fixtures::make_movie_app();
    app.panel_focus = PanelFocus::Library;
    app.launch_restore = crate::app::state::app_struct::LaunchRestore::Done;

    app.pin_launch_focus_to_queue();

    assert_eq!(app.panel_focus, PanelFocus::Queue);
    assert_eq!(
        app.launch_restore,
        crate::app::state::app_struct::LaunchRestore::Done
    );
}
