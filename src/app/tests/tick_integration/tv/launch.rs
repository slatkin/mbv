use super::*;

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

    assert!(harness.model().app.libs[0].nav_stack[0]
        .letter_filter
        .is_none());
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
    assert!(harness.model().app.libs[0].nav_stack[0]
        .letter_filter
        .is_none());
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
    "IncludeItemTypes=Episode"
)]
#[case::saved_s_z_shrunk_small(
    mbv_core::config::TvContentMode::Range(2),
    300,
    mbv_core::config::TvContentMode::All,
    "IncludeItemTypes=Series"
)]
#[case::saved_latest_shrunk_small(
    mbv_core::config::TvContentMode::Latest,
    300,
    mbv_core::config::TvContentMode::Latest,
    "IncludeItemTypes=Episode"
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
            item_types: Some(
                match &saved_mode {
                    mbv_core::config::TvContentMode::Latest
                    | mbv_core::config::TvContentMode::Upcoming => "Episode",
                    _ => "Series",
                }
                .into(),
            ),
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
        app.saved_library_position(0).unwrap().levels[0].tv_content_mode,
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
        requests
            .iter()
            .any(|request| request.contains(expected_route)),
        "expected {expected_route} request, got {requests:?}"
    );
    if current_total <= 300 {
        assert!(requests
            .iter()
            .all(|request| !request.contains("NameStartsWith")));
    }
}
