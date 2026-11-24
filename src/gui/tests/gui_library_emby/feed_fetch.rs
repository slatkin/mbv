//! Tests of the GUI Library's Emby feed-group fetch path (gui-collection-
//! dropdown design D2, D3): a painted frame's claim does not block the saved
//! folder set, and a failed folder fetch retries on a user press alone. The
//! Emby boundary is the in-memory mock transport, so no request leaves the
//! process and no worker outlives the assertion.

use super::*;

use mbv_client_core::state::events::PendingSeriesLanding;

/// A painted frame does not defeat the saved folder set (gui-collection-
/// dropdown design D2, D3): the shown pass claims the library's fetched
/// results without storing a selection, so the launch restore still installs
/// the saved folders over the painted frame instead of opening on All.
#[test]
fn a_shown_pass_before_the_restore_still_restores_the_saved_folder_set() {
    let _guard = mbv_client_core::config::TestStateDirGuard::new();
    let http = MockHttp::new();
    let mut app = emby_app(&http, "homevideos", FEED_ID, "YouTube");
    app.config_for_test(|config| config.feed_view_libraries.push("youtube".into()));
    // No client: the shown pass claims the library and starts no fetch, so the
    // test observes the restore alone.
    app.disconnect_emby_client_for_test();
    app.edit_library_for_test(0, |library| {
        library.nav_stack.push(level(
            FEED_ID,
            "YouTube",
            vec![
                folder_item("folder-1", "Channel 1"),
                folder_item("folder-2", "Channel 2"),
            ],
        ));
    });
    app.edit_library_for_test(0, |library| {
        library.feed_home_video = Some(FeedHomeVideoState::default());
    });

    crate::gui::test_support::ensure_gui_destination(
        &mut app,
        GuiDestinationKind::Library { lib_idx: 0 },
    );
    assert!(
        crate::gui::test_support::stub_gui_state(|gui| gui
            .emby_libs
            .get(&0)
            .is_some_and(|stored| stored.feed_claim.is_some())),
        "the shown pass claims the library's fetched results"
    );

    let core =
        crate::gui::GuiCore::new(std::mem::replace(&mut app, mbv_client_core::handoff_app()));
    let mut state = core.gui_launch_state();
    app = core.into_app();
    let destination = state.destination.clone();
    state.collections = vec![mbv_config::GuiDestinationCollections {
        destination: destination.clone(),
        target: mbv_config::GuiLaunchCollectionTarget::EmbyFeed,
        group_ids: vec!["folder-2".to_string()],
        order: None,
        picks: Vec::new(),
    }];

    assert!(crate::gui::test_support::restore_launch_collections(
        &mut app,
        &state,
        &destination
    ));

    let stored = crate::gui::test_support::stub_gui_state(|gui| {
        gui.emby_feed_sets
            .get(&0)
            .cloned()
            .expect("the saved folder set installed over the painted frame")
    });
    assert_eq!(
        stored.set.checked_keys(),
        ["folder-2".to_string()].as_slice(),
        "the destination opens on the saved folder, not on All"
    );
}

/// A failed folder fetch retries on a user press and not on a repaint
/// (gui-collection-dropdown design D3, scenario "A failed collection is
/// fetched again on a press"): the failed target retires its in-flight mark
/// and keeps a terminal failure mark, so the shown pass offers no request for
/// it while a press clears the mark and requests it again.
#[test]
fn a_failed_feed_folder_fetch_retries_on_a_press_and_not_on_a_repaint() {
    let _guard = mbv_client_core::config::TestStateDirGuard::new();
    let http = MockHttp::new();
    // The first checked folder's fetch fails, so the press's retry is the one
    // request the count can see; the second folder answers.
    http.respond_when("ParentId=folder-1", 500, "");
    http.respond_when("ParentId=folder-2", 200, VIDEO_BODY);
    let mut app = emby_app(&http, "homevideos", FEED_ID, "YouTube");
    app.config_for_test(|config| config.feed_view_libraries.push("youtube".into()));
    app.edit_library_for_test(0, |library| {
        library.nav_stack.push(level(
            FEED_ID,
            "YouTube",
            vec![
                folder_item("folder-1", "Channel 1"),
                folder_item("folder-2", "Channel 2"),
            ],
        ));
    });
    app.edit_library_for_test(0, |library| {
        library.feed_home_video = Some(FeedHomeVideoState::default());
    });

    toggle_folder(&mut app, "folder-1");
    land(&mut app);
    let fetched = http.request_count();
    assert!(
        app.libs_for_test()[0]
            .feed_home_video
            .as_ref()
            .is_some_and(|state| state.fetching.is_none()),
        "the failed fetch retired its in-flight mark"
    );

    crate::gui::test_support::ensure_gui_destination(
        &mut app,
        GuiDestinationKind::Library { lib_idx: 0 },
    );
    assert_eq!(
        http.request_count(),
        fetched,
        "a repaint issues no request for the failed folder"
    );

    toggle_folder(&mut app, "folder-2");
    land(&mut app);
    assert_eq!(
        http.request_count(),
        fetched + 1,
        "the press requests the failed folder again"
    );
}

/// A failed folder fetch runs the library error cleanup (gui-collection-
/// dropdown design D3): the fetch reports its own target now, so its arm must
/// still disarm the deferred tab switch and series landing that the plain
/// library error used to clear. Without that cleanup a later album activation
/// switches tabs after the library error.
#[test]
fn a_failed_feed_folder_fetch_disarms_deferred_navigation() {
    let _guard = mbv_client_core::config::TestStateDirGuard::new();
    let http = MockHttp::new();
    http.respond_when("ParentId=folder-1", 500, "");
    let mut app = emby_app(&http, "homevideos", FEED_ID, "YouTube");
    app.config_for_test(|config| config.feed_view_libraries.push("youtube".into()));
    app.edit_library_for_test(0, |library| {
        library.nav_stack.push(level(
            FEED_ID,
            "YouTube",
            vec![folder_item("folder-1", "Channel 1")],
        ));
    });
    app.edit_library_for_test(0, |library| {
        library.feed_home_video = Some(FeedHomeVideoState::default());
    });
    app.seed_pending_navigate_tab_switch_for_test(Some(0));
    app.seed_pending_series_landing_for_test(PendingSeriesLanding {
        lib_idx: 0,
        reveal: Box::new(make_item("Show", "Series")),
        switch_tab: true,
        episode_id: None,
    });

    toggle_folder(&mut app, "folder-1");
    land_until_fetched(&mut app);

    assert_eq!(
        app.pending_navigate_tab_switch_for_test(),
        None,
        "the failure disarms the deferred tab switch"
    );
    assert!(
        app.pending_series_landing_is_none_for_test(),
        "the failure disarms the deferred series landing"
    );
}
