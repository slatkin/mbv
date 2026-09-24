use super::*;

#[test]
fn legacy_launch_migration_uses_stable_item_and_ignores_cursor_index() {
    let mut app = crate::app::render::make_movie_app();
    app.tab = TabSelection::Home;
    app.legacy_launch_tab = Some(1);
    app.emby_catalog_ready = true;
    app.library_position_state.libraries.insert(
        "lib-movies".into(),
        crate::config::LibraryPosition {
            levels: vec![
                crate::config::LibraryPositionLevel {
                    parent_id: "lib-movies".into(),
                    title: "Movies".into(),
                    focused_item_id: Some("movie-focused".into()),
                    cursor_index: 99,
                    ..Default::default()
                },
                crate::config::LibraryPositionLevel {
                    parent_id: "series-1".into(),
                    title: "Series".into(),
                    focused_item_id: Some("deep-item-must-not-migrate".into()),
                    ..Default::default()
                },
            ],
            ..Default::default()
        },
    );
    let mut other_library = make_item("Shows", "CollectionFolder");
    other_library.id = "lib-shows".into();
    app.libs.push(crate::app::LibraryTab::new(other_library));
    app.library_position_state.libraries.insert(
        "lib-shows".into(),
        crate::config::LibraryPosition {
            levels: vec![crate::config::LibraryPositionLevel {
                parent_id: "lib-shows".into(),
                focused_item_id: Some("unselected-library-item".into()),
                ..Default::default()
            }],
            ..Default::default()
        },
    );

    app.resolve_library_tab_pending();

    let state = app
        .pending_launch_state
        .expect("legacy state migrated once");
    assert_eq!(
        state.tab,
        mbv_core::config::TabIdentity::ServiceLibrary {
            kind: ServiceKind::Emby,
            library_id: "lib-movies".into(),
        }
    );
    assert_eq!(
        state.item,
        Some(mbv_core::config::LibraryItemIdentity::Emby {
            id: "movie-focused".into()
        })
    );
    assert_ne!(
        state.item,
        Some(mbv_core::config::LibraryItemIdentity::Emby { id: "99".into() })
    );
    assert_ne!(
        state.item,
        Some(mbv_core::config::LibraryItemIdentity::Emby {
            id: "deep-item-must-not-migrate".into()
        })
    );
    assert!(!matches!(
        state.item,
        Some(mbv_core::config::LibraryItemIdentity::Emby { ref id })
            if id == "unselected-library-item"
    ));
    assert!(state.selector.is_none());
    assert!(app.legacy_launch_migration_attempted);

    // A second resolution cannot derive another snapshot from the retained
    // read-only legacy document.
    app.pending_launch_state = None;
    app.resolve_library_tab_pending();
    assert!(app.pending_launch_state.is_none());
}

#[test]
fn versioned_launch_state_takes_precedence_over_legacy_migration() {
    let mut app = crate::app::render::make_movie_app();
    let saved = mbv_core::config::TuiLaunchState {
        version: mbv_core::config::TUI_LAUNCH_STATE_VERSION,
        tab: mbv_core::config::TabIdentity::Home,
        panel_focus: mbv_core::config::LaunchPanelFocus::Queue,
        selector: None,
        item: None,
    };
    app.pending_launch_state = Some(saved.clone());
    app.legacy_launch_tab = Some(1);
    app.emby_catalog_ready = true;

    app.resolve_library_tab_pending();

    assert_eq!(app.pending_launch_state, Some(saved));
    assert!(!app.legacy_launch_migration_attempted);
}

#[test]
fn malformed_and_nonnumeric_legacy_preferences_fall_back_cleanly_through_construct() {
    let _guard = crate::config::TestStateDirGuard::new();
    for prefs in ["{not-json", r#"{"library_tab":"not-a-number"}"#] {
        std::fs::write(crate::config::prefs_path(), prefs).expect("write malformed prefs");

        let mut app = crate::app::tests::make_built_app();
        app.resolve_library_tab_pending();

        assert_eq!(app.legacy_launch_tab, None);
        assert_eq!(app.tab, TabSelection::Home);
        assert!(app.pending_launch_state.is_none());
        assert!(app.legacy_launch_migration_attempted);
    }
}

#[test]
fn aliased_legacy_preference_falls_back_to_home_through_construct() {
    let _guard = crate::config::TestStateDirGuard::new();
    std::fs::write(
        crate::config::prefs_path(),
        serde_json::json!({ "power_left_tab": 1 }).to_string(),
    )
    .expect("write aliased prefs");

    let mut app = crate::app::tests::make_built_app();
    assert_eq!(app.legacy_launch_tab, Some(1));
    app.resolve_library_tab_pending();

    assert_eq!(app.tab, TabSelection::Home);
    assert_eq!(
        app.pending_launch_state.as_ref().map(|state| &state.tab),
        Some(&mbv_core::config::TabIdentity::Home)
    );
}

#[test]
fn legacy_audiobookshelf_podcast_item_is_not_migrated() {
    let mut app = crate::app::tests::make_app_stub();
    app.tab = TabSelection::Home;
    app.legacy_launch_tab = Some(1);
    app.audiobookshelf_catalog_ready = true;
    app.audiobookshelf_libraries
        .push(mbv_core::audiobookshelf::AudiobookshelfLibrary {
            id: "abs-podcasts".into(),
            name: "Podcasts".into(),
            media_type: "podcast".into(),
        });
    app.config.lock().unwrap().audiobookshelf_setup = Some(
        mbv_core::config::AudiobookshelfSetup::new("https://abs.example"),
    );
    app.library_position_state.libraries.insert(
        "audiobookshelf:https://abs.example:abs-podcasts".into(),
        crate::config::LibraryPosition {
            levels: vec![crate::config::LibraryPositionLevel {
                parent_id: "abs-podcasts".into(),
                item_types: Some("podcast".into()),
                focused_item_id: Some("retired-show-id".into()),
                ..Default::default()
            }],
            ..Default::default()
        },
    );

    app.resolve_library_tab_pending();

    let state = app
        .pending_launch_state
        .expect("legacy state migrated once");
    assert_eq!(
        state.tab,
        mbv_core::config::TabIdentity::ServiceLibrary {
            kind: ServiceKind::Audiobookshelf,
            library_id: "abs-podcasts".into(),
        }
    );
    assert!(state.item.is_none());
}
