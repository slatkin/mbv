use super::*;
use crate::app::components::feeds_content::{FeedsContent, FeedsOwnerPush};
use crate::app::components::home_content::HomeContent;
use crate::app::components::msg::{HomeRowTarget, Msg, ShellRequest};
use crate::app::components::{ComponentId, LibraryKey};
use mbv_core::config::{FeedKind, FeedSubscription, ServiceKind};
use mbv_core::playback_queue::FeedEntry;
use crate::app::tests::*;
use crate::app::tests_tick_harness::TickHarness;
use rstest::rstest;
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};

#[test]
fn teardown_fast_when_player_thread_is_not_hung() {
    let mut app = make_app_stub();

    let started = std::time::Instant::now();
    app.teardown(Duration::from_secs(5), None);
    let elapsed = started.elapsed();

    assert!(
        elapsed < Duration::from_secs(1),
        "teardown against a player with no thread to join should return \
             promptly, not wait anywhere near the quit_timeout budget, took {elapsed:?}"
    );
}

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
    assert_eq!(app.tab, TabSelection::Home, "catalog identity is not ready yet");
    assert!(!app.pending_launch_tab_resolved);

    app.emby_catalog_ready = true;
    app.resolve_library_tab_pending();
    assert_eq!(app.tab, TabSelection::EmbyLibrary(0));
    assert!(app.pending_launch_tab_resolved);
    assert!(app.pending_launch_state.is_some(), "destination state remains for 3.2");
}

#[test]
fn pending_launch_tab_missing_stable_service_id_falls_back_to_home_even_with_catalog_entry() {
    let mut app = crate::app::render::make_movie_app();
    assert_eq!(app.libs.len(), 1, "the current catalog must have a service destination");
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
            vec![mbv_core::playback_queue::QueueItem::Emby(Box::new(make_item(
                "Home item",
                "Movie",
            )))],
            Vec::new(),
            false,
            std::collections::HashMap::new(),
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
    assert_eq!(harness.model().app.effective_panel_focus(), PanelFocus::Queue);
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
            Vec::new(),
            false,
            std::collections::HashMap::new(),
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
                vec![mbv_core::playback_queue::QueueItem::Emby(Box::new(selected))],
                Vec::new(),
                false,
                std::collections::HashMap::new(),
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
            Vec::new(),
            vec![(
                "Movies".into(),
                crate::app::types_playback::HomeLatestSource::Emby("lib-movies".into()),
                vec![
                    mbv_core::playback_queue::QueueItem::Emby(Box::new(make_item(
                        "Movie one", "Movie",
                    ))),
                    mbv_core::playback_queue::QueueItem::Emby(Box::new(make_item(
                        "Movie two", "Movie",
                    ))),
                ],
            )],
            false,
            std::collections::HashMap::new(),
        );
    });
    harness.model_mut().sync_mounted_surfaces();
    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    }));
    let _ = harness.step();

    let requests = [
        ShellRequest::HomePillClick { target: 1 },
        ShellRequest::HomeRowClick {
            target: HomeRowTarget {
                item_id: Some("id1".into()),
                source: Some("emby:lib-movies".into()),
                from_continue_watching: false,
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

#[test]
fn teardown_persists_active_library_route_when_auto_reconnect_enabled() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.config.lock().unwrap().auto_reconnect = true;
    app.active_route = Some("music".to_string());

    app.teardown(Duration::from_secs(1), None);

    assert_eq!(
        crate::config::load_last_remote_connection().unwrap(),
        Some(crate::config::LastRemoteConnection::LibraryRoute {
            library: "music".to_string()
        })
    );
}

#[test]
fn teardown_persists_connected_session_when_auto_reconnect_enabled() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.config.lock().unwrap().auto_reconnect = true;
    let sess = make_session("living-room-mbv", "mbv");
    app.connected_session_id = Some(sess.id.clone());
    app.connected_session_state = Some(sess);

    app.teardown(Duration::from_secs(1), None);

    assert_eq!(
        crate::config::load_last_remote_connection().unwrap(),
        Some(crate::config::LastRemoteConnection::DirectSession {
            device_name: "living-room-mbv".to_string()
        })
    );
}

#[test]
fn teardown_persists_direct_remote_when_auto_reconnect_enabled() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.config.lock().unwrap().auto_reconnect = true;
    let sess = make_session("living-room-mbv", "mbv");
    let (remote, remote_rx) = mbv_core::remote_player::RemotePlayer::stub(make_items(1), 0);

    app.switch_to_direct_remote(
        &sess,
        remote,
        remote_rx,
        &mbv_core::remote_player::DaemonEndpoint::Tcp("127.0.0.1:0".parse().unwrap()),
    );
    app.teardown(Duration::from_secs(1), None);

    assert_eq!(
        crate::config::load_last_remote_connection().unwrap(),
        Some(crate::config::LastRemoteConnection::DirectSession {
            device_name: "living-room-mbv".to_string()
        })
    );
}

#[test]
fn teardown_issues_no_stop_for_an_attached_cast_target() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.attach_cast("device-1".to_string());
    let (job_tx, calls) =
        super::types_cast::spawn_fake_cast_worker(super::types_cast::FakeCastTransport::default());
    app.set_cast_client("device-1", job_tx);

    app.teardown(Duration::from_secs(1), None);

    assert!(!calls.lock().unwrap().contains(&"stop".to_string()));
}

#[test]
fn teardown_clears_persisted_connection_when_exiting_local() {
    let _guard = crate::config::TestStateDirGuard::new();
    let _ = crate::config::save_last_remote_connection(Some(
        &crate::config::LastRemoteConnection::LibraryRoute {
            library: "music".to_string(),
        },
    ));
    let mut app = make_app_stub();
    app.config.lock().unwrap().auto_reconnect = true;

    app.teardown(Duration::from_secs(1), None);

    assert_eq!(crate::config::load_last_remote_connection().unwrap(), None);
}

#[test]
fn teardown_never_touches_persisted_state_when_auto_reconnect_disabled() {
    let _guard = crate::config::TestStateDirGuard::new();
    let _ = crate::config::save_last_remote_connection(Some(
        &crate::config::LastRemoteConnection::LibraryRoute {
            library: "music".to_string(),
        },
    ));
    let mut app = make_app_stub();
    assert!(!app.config.lock().unwrap().auto_reconnect);
    app.active_route = None;

    app.teardown(Duration::from_secs(1), None);

    // Feature is off: the file from before this test's own `app` even
    // existed must be left exactly as it was, not cleared just because
    // `active_route` is currently `None`.
    assert_eq!(
        crate::config::load_last_remote_connection().unwrap(),
        Some(crate::config::LastRemoteConnection::LibraryRoute {
            library: "music".to_string()
        })
    );
}

#[test]
fn teardown_persists_a_reconnected_target_for_a_local_daemon_launch_that_moved_remote() {
    // Regression guard for design.md's Decision 2 walk-through: a client
    // launched attached to the local daemon (`home_is_local_daemon = true`)
    // that has since reconnected to a genuinely remote target
    // (`player_endpoint` set to a non-local endpoint) must still have
    // that connection persisted at teardown -- the old gate, keyed on the
    // mutable `is_local_daemon`, would have wrongly skipped this.
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.config.lock().unwrap().auto_reconnect = true;
    app.launched_as_remote = true;
    app.home_is_local_daemon = true;
    app.config.lock().unwrap().stay_alive = true;
    app.player_endpoint = Some(mbv_core::remote_player::DaemonEndpoint::Tcp(
        "127.0.0.1:0".parse().unwrap(),
    ));
    app.active_route = Some("music".to_string());

    app.teardown(Duration::from_secs(1), None);

    assert_eq!(
        crate::config::load_last_remote_connection().unwrap(),
        Some(crate::config::LastRemoteConnection::LibraryRoute {
            library: "music".to_string()
        })
    );
}

#[test]
fn teardown_skips_persistence_for_an_explicit_remote_daemon_launch() {
    // The existing skip case, unaffected by rebasing the gate onto
    // `home_is_local_daemon`: an explicit `--connect-daemon` launch has
    // `home_is_local_daemon = false` for its entire lifetime, so it must
    // still be skipped and must not overwrite an existing saved record.
    let _guard = crate::config::TestStateDirGuard::new();
    let _ = crate::config::save_last_remote_connection(Some(
        &crate::config::LastRemoteConnection::LibraryRoute {
            library: "music".to_string(),
        },
    ));
    let mut app = make_app_stub();
    app.config.lock().unwrap().auto_reconnect = true;
    app.launched_as_remote = true;
    app.home_is_local_daemon = false;
    app.player_endpoint = Some(mbv_core::remote_player::DaemonEndpoint::Tcp(
        "127.0.0.1:0".parse().unwrap(),
    ));
    app.active_route = None;

    app.teardown(Duration::from_secs(1), None);

    assert_eq!(
        crate::config::load_last_remote_connection().unwrap(),
        Some(crate::config::LastRemoteConnection::LibraryRoute {
            library: "music".to_string()
        })
    );
}

#[test]
fn local_daemon_client_does_not_overwrite_authoritative_queue_on_teardown() {
    let _guard = crate::config::TestStateDirGuard::new();
    let old_items = make_items(1);
    crate::config::save_queue_state(&crate::config::QueueState {
        source: crate::config::QueueSource::Album,
        items: old_items
            .iter()
            .cloned()
            .map(|item| mbv_core::playback_queue::QueueItem::Emby(Box::new(item)))
            .collect(),
        cursor: 0,
        last_played_content_id: None,
        last_played_item_id: None,
        last_played_completed: false,
        positions: Default::default(),
    })
    .expect("save queue state");

    let mut app = make_local_daemon_app_stub(make_items(2));
    app.player_tab
        .set_items(make_items(3), app.player_tab.queue_cursor);
    app.teardown(Duration::from_secs(1), None);

    let state = crate::config::load_queue_state().expect("existing daemon snapshot");
    assert_eq!(
        state.items.iter().map(|item| item.id()).collect::<Vec<_>>(),
        old_items
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>()
    );
}

#[test]
fn wants_terminal_render_true_when_due() {
    let app = make_app_stub();
    let stale = Instant::now() - Duration::from_secs(10);
    assert!(app.wants_terminal_render(false, stale, Duration::from_secs(1)));
}

// A compact-banner poster fetch (or any list-image prefetch) can easily
// outlast the idle render cadence: with nothing playing and no remote
// session, the run loop only repaints once a second unless something
// sets `had_events` (a key/mouse event, or the fetch itself completing).
// That meant a loading placeholder was computed correctly by
// `compact_banner_layout` but never actually painted -- the only two
// frames drawn were "just navigated, fetch not even started yet" and
// "fetch just completed", with nothing in between showing the reserved
// placeholder box. Treating an in-flight image fetch the same as active
// playback (fast 150ms cadence instead of the 1s idle one) gives the
// loop a reason to repaint while the placeholder should be visible.
#[rstest]
#[case::render_interval_is_fast_while_a_card_image_fetch_is_in_flight(true, Duration::from_millis(150))]
#[case::render_interval_is_slow_when_idle_with_no_fetches_in_flight(false, Duration::from_secs(1))]
fn render_interval(#[case] image_loading: bool, #[case] expected: Duration) {
    let mut app = make_app_stub();
    if image_loading {
        app.card_image_loading.insert("movie-1:cmp_primary".into());
    }
    assert_eq!(app.render_interval(), expected);
}

#[test]
fn auto_reconnect_settings_row_displays_and_toggles_current_session() {
    let mut app = make_app_stub();
    app.config.lock().unwrap().auto_reconnect = false;

    let cfg = app.config.lock().unwrap().clone();
    assert_eq!(
        settings::setting_label(SettingKey::AutoReconnect),
        "Auto reconnect"
    );
    assert_eq!(
        settings::setting_value(SettingKey::AutoReconnect, &cfg, &app.ui_config_snapshot()),
        "off"
    );

    app.handle_settings_activate(SettingKey::AutoReconnect);
    let cfg = app.config.lock().unwrap().clone();
    assert!(cfg.auto_reconnect);
    assert_eq!(
        settings::setting_value(SettingKey::AutoReconnect, &cfg, &app.ui_config_snapshot()),
        "on"
    );
    assert!(
        app.settings_save_at.is_some(),
        "settings toggle must use the delayed save path"
    );

    app.handle_settings_activate(SettingKey::AutoReconnect);
    assert!(!app.config.lock().unwrap().auto_reconnect);
}

#[test]
fn enabling_auto_reconnect_persists_the_active_remote_target() {
    let mut app = make_app_stub();
    app.config.lock().unwrap().auto_reconnect = false;
    app.active_route = Some("music".to_string());

    app.handle_settings_activate(SettingKey::AutoReconnect);

    assert_eq!(
        crate::config::load_last_remote_connection().unwrap(),
        Some(crate::config::LastRemoteConnection::LibraryRoute {
            library: "music".to_string()
        })
    );
}

// ── transport_prev_next_available (issue #112) ─────────────────────────
// Drives whether playback transport is currently available at the queue
// boundaries. The header uses the `next` half directly, while the `P`/`N`
// keys still reuse both halves.

#[rstest]
#[case::transport_prev_next_unavailable_when_player_inactive(false, 0, 0, false, (false, false))]
#[case::transport_prev_next_both_available_mid_queue(true, 3, 1, false, (true, true))]
#[case::transport_prev_unavailable_on_first_item(true, 3, 0, false, (false, true))]
#[case::transport_next_unavailable_on_last_item(true, 3, 2, false, (true, false))]
#[case::transport_prev_next_both_available_for_connected_remote_session_regardless_of_local_status(true, 3, 2, true, (true, true))]
fn transport_prev_next(
    #[case] active: bool,
    #[case] queue_len: usize,
    #[case] current_idx: usize,
    #[case] connected: bool,
    #[case] expected: (bool, bool),
) {
    let mut app = make_app_stub();
    if connected {
        app.connected_session_id = Some("session-1".into());
    }
    {
        let mut st = app.player.status.lock().unwrap();
        st.active = active;
        st.queue_len = queue_len;
        st.current_idx = current_idx;
    }
    assert_eq!(app.transport_prev_next_available(), expected);
}

#[test]
fn remote_position_extrapolation_does_not_round_up_partial_seconds() {
    assert_eq!(
        App::extrapolated_remote_position(10, Duration::from_millis(1600)),
        11
    );
    assert_eq!(
        App::extrapolated_remote_position(10, Duration::from_secs(2)),
        12
    );
}
