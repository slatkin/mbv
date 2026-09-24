use super::*;

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

    assert!(harness.model().app.libs[0].nav_stack[0]
        .letter_filter
        .is_none());
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
    level.letter_filter = crate::app::render::LetterFilter::for_index_for_kind(
        2,
        crate::app::render::LetterFilterKind::Movie,
    );
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
    assert!(harness.model().app.libs[0].nav_stack[0]
        .letter_filter
        .is_none());
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
