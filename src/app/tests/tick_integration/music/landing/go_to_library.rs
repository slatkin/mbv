use super::album_landing::landed_grouped_album_stack;
use super::*;

/// A Grouped Music app with a scripted in-memory Emby transport installed,
/// the same boundary `spawn_navigate_to_item` resolves through.
fn grouped_music_app_with_mock_emby(http: &mbv_core::mock_http::MockHttp) -> crate::app::App {
    let mut app = crate::app::render::make_music_group_app();
    let mut config = app.config.lock().unwrap().clone();
    config.server_url = "http://127.0.0.1:1".into();
    crate::app::tests::install_test_emby(&mut app, config);
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
    app
}

/// Queue the scripted Emby responses for a queued-track "Go to Library"
/// landing on `alb1` in the `lib-music` grouped fixture (`music.levels`
/// `["group", "album"]`): the track + album fetches, the configured
/// album-index walk (levels 0 and 1), then the activation worker's level
/// fetches. Response #3 is also the body the pre-fix `get_ancestors` round
/// trip would have read (a listing object, silently an empty ancestor chain),
/// which is exactly the depth mismatch D7 diagnoses.
fn script_grouped_album_landing(http: &mbv_core::mock_http::MockHttp) {
    http.respond(200, r#"{"Items":[{"Id":"trk1","Name":"Song","Type":"Audio","AlbumId":"alb1"}],"TotalRecordCount":1}"#);
    http.respond(
        200,
        r#"{"Items":[{"Id":"alb1","Name":"The Album","Type":"MusicAlbum"}],"TotalRecordCount":1}"#,
    );
    http.respond(
        200,
        r#"{"Items":[{"Id":"group-0","Name":"Alpha","Type":"MusicArtist"}],"TotalRecordCount":1}"#,
    );
    http.respond(
        200,
        r#"{"Items":[{"Id":"alb1","Name":"The Album","Type":"MusicAlbum"}],"TotalRecordCount":1}"#,
    );
    http.respond(
        200,
        r#"{"Items":[{"Id":"group-0","Name":"Alpha","Type":"MusicArtist"}],"TotalRecordCount":1}"#,
    );
    http.respond(
        200,
        r#"{"Items":[{"Id":"alb1","Name":"The Album","Type":"MusicAlbum"}],"TotalRecordCount":1}"#,
    );
}

/// Feed the queued-track navigation through the production shell drains: the
/// resolve worker's `NavigateTo`, then the recursive-activation drain.
fn drive_grouped_album_navigation(harness: &mut TickHarness, item_id: &str, item_type: &str) {
    harness.model_mut().app.spawn_navigate_to_item(
        item_id.into(),
        item_type.into(),
        vec![(0, "lib-music".into(), "music".into())],
    );
    let ev = harness
        .model()
        .app
        .lib_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("navigate event");
    assert!(
        matches!(ev, crate::app::LibEvent::NavigateTo { .. }),
        "expected a NavigateTo landing"
    );
    harness.model_mut().handle_inline_search_lib_event(ev);

    let ev = harness
        .model()
        .app
        .lib_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("album activated");
    let crate::app::LibEvent::RecursiveAlbumActivated {
        library_id,
        nav_stack,
    } = ev
    else {
        panic!("expected RecursiveAlbumActivated");
    };
    harness
        .model_mut()
        .on_recursive_album_activated(library_id, nav_stack);
}

/// A structural snapshot of the shell's nav stack (`BrowseLevel` is neither
/// Clone nor PartialEq): parent id, item ids, and resting cursor per level.
fn nav_stack_snapshot(harness: &TickHarness) -> Vec<(String, Vec<String>, usize)> {
    harness.model().app.libs[0]
        .nav_stack
        .iter()
        .map(|level| {
            (
                level.parent_id.clone(),
                level.items.iter().map(|item| item.id.clone()).collect(),
                level.resting().cursor(),
            )
        })
        .collect()
}

/// Task 6.1 (design D7): the reported silent no-op in the user's shape — a
/// Grouped Music destination already mounted with a queued track. The
/// scripted ancestors response is a listing object, which the pre-fix
/// `get_ancestors` round trip reads as an EMPTY ancestor chain; the landing
/// then installs a one-level stack for which no Music owner is eligible, so
/// the tab moves but the tree never re-anchors (no fallback painter, no
/// error). After the fix the worker resolves the album through the configured
/// `music.levels` album shape and the tree lands on `alb1`.
#[test]
fn go_to_library_on_a_queued_track_lands_the_configured_grouped_album() {
    let _guard = crate::config::TestStateDirGuard::new();
    let http = mbv_core::mock_http::MockHttp::new();
    let mut app = grouped_music_app_with_mock_emby(&http);
    // The queued track: the "Go to Library" target.
    let mut track = crate::app::tests::make_item("Song", "Audio");
    track.id = "trk1".into();
    track.album_id = "alb1".into();
    app.replace_playback_queue(vec![track.clone()], 0);
    // Pre-seed the album-track cache so the sync pass never spawns a track
    // fetch that would race the navigation events on `lib_rx`.
    let mut fixture_track = crate::app::tests::make_item("Fixtures", "Audio");
    fixture_track.id = "fixture-track".into();
    app.album_tracks_cache
        .insert("album-1".into(), vec![fixture_track]);
    app.album_tracks_cache.insert("alb1".into(), vec![track]);
    script_grouped_album_landing(&http);

    // The destination is already mounted/retained before the navigation.
    let mut harness = TickHarness::new(app);
    while harness.model().app.lib_rx.try_recv().is_ok() {}
    harness.model_mut().sync_mounted_surfaces();
    while harness.model().app.lib_rx.try_recv().is_ok() {}
    let id = ComponentId::Library;
    assert_eq!(
        music_selected_album_id(&harness, &id).as_deref(),
        Some("album-1"),
        "the retained tree starts on the fixture album"
    );

    drive_grouped_album_navigation(&mut harness, "trk1", "Audio");
    harness.step();

    assert!(
        harness.model().app.is_viewing_album_folders(0),
        "the landing installs the configured album level (silent no-op otherwise)"
    );
    assert_eq!(
        music_selected_album_id(&harness, &id).as_deref(),
        Some("alb1"),
        "the tree re-anchors to the resolved album"
    );
}

/// Task 6.3: navigation into a previously mounted tree re-anchors to the
/// resolved album and selects the navigated track in the Workspace, even
/// though the retained owner had moved to a sibling album first.
#[test]
fn go_to_library_into_a_retained_tree_reanchors_and_selects_the_navigated_track() {
    let _guard = crate::config::TestStateDirGuard::new();
    let http = mbv_core::mock_http::MockHttp::new();
    let mut app = grouped_music_app_with_mock_emby(&http);
    app.terminal_width = 160;
    app.terminal_height = 40;
    // A sibling album so the retained tree can be moved off the navigated one.
    let mut sibling = crate::app::tests::make_item("Second Album", "MusicAlbum");
    sibling.id = "album-2".into();
    sibling.artist = "Alpha".into();
    app.libs[0].nav_stack[1].items.push(sibling);
    app.libs[0].nav_stack[1].total_count = 2;

    // The queued track lands on `alb1`, whose navigated track sits second in
    // the fetched track list, so a selected row of 1 proves the deep
    // selection rather than the default first row.
    let mut queued = crate::app::tests::make_item("Song", "Audio");
    queued.id = "trk1".into();
    queued.album_id = "alb1".into();
    app.replace_playback_queue(vec![queued.clone()], 0);
    let mut other = crate::app::tests::make_item("Other Song", "Audio");
    other.id = "trk0".into();
    app.album_tracks_cache
        .insert("alb1".into(), vec![other, queued]);
    let mut fixture = crate::app::tests::make_item("Fixture", "Audio");
    fixture.id = "fixture-track".into();
    app.album_tracks_cache
        .insert("album-1".into(), vec![fixture.clone()]);
    app.album_tracks_cache
        .insert("album-2".into(), vec![fixture]);
    script_grouped_album_landing(&http);

    let mut harness = TickHarness::new(app);
    while harness.model().app.lib_rx.try_recv().is_ok() {}
    harness.model_mut().sync_mounted_surfaces();
    while harness.model().app.lib_rx.try_recv().is_ok() {}
    let id = ComponentId::Library;

    // The retained tree moves off the navigated album.
    harness.inject(key(Key::Down));
    harness.step();
    assert_eq!(
        music_selected_album_id(&harness, &id).as_deref(),
        Some("album-2"),
        "the retained tree had moved to the sibling album"
    );

    drive_grouped_album_navigation(&mut harness, "trk1", "Audio");
    harness.step();

    assert_eq!(
        music_selected_album_id(&harness, &id).as_deref(),
        Some("alb1"),
        "the tree re-anchors to the resolved album"
    );
    assert_eq!(
        harness.model().test_music_owner().track_selected_row(),
        Some(1),
        "the navigated track is selected in the Workspace track list"
    );
    assert!(
        harness.model().pending_music_track_selection.is_none(),
        "the deep selection is fully consumed"
    );
}

/// Task 6.4 (design D7): an apply-stage failure after album resolution — a
/// prepared grouped-catalog stack whose chain is broken — reports through the
/// existing library-error feedback and commits NOTHING: the active tab, nav
/// stack, saved Library position, and retained component selection are all
/// structurally unchanged.
#[test]
fn rejected_grouped_album_apply_flashes_and_leaves_every_committed_value_unchanged() {
    let (mut harness, id) = wide_music_harness();
    let before_tab = harness.model().app.tab;
    let before_stack = nav_stack_snapshot(&harness);
    let before_saved = harness.model().app.saved_library_position(0);
    let before_album = music_selected_album_id(&harness, &id);
    assert_eq!(
        before_album.as_deref(),
        Some("album-1"),
        "the retained tree starts on the fixture album"
    );

    // Album resolution already succeeded; the prepared stack fails to
    // reproduce the configured path (its album level hangs off a
    // nonexistent artist).
    let mut invalid = landed_grouped_album_stack("alb1");
    invalid[1].parent_id = "missing-artist".into();
    harness
        .model_mut()
        .on_recursive_album_activated("lib-music".into(), invalid);
    harness.step();

    assert!(
        harness.model().app.status.contains("Library error"),
        "the existing library-error feedback fires: {}",
        harness.model().app.status
    );
    assert_eq!(
        harness.model().app.status_severity,
        crate::app::dispatch::notify::ToastSeverity::Error
    );
    assert_eq!(
        harness.model().app.tab,
        before_tab,
        "the active tab is unchanged"
    );
    assert_eq!(
        nav_stack_snapshot(&harness),
        before_stack,
        "the nav stack is unchanged"
    );
    assert_eq!(
        harness.model().app.saved_library_position(0),
        before_saved,
        "the saved Library position is unchanged"
    );
    assert_eq!(
        music_selected_album_id(&harness, &id),
        before_album,
        "the retained component selection is unchanged"
    );
}

/// A configured album-terminating shape whose first level is not the grouped
/// Music level cannot provide a Music owner. Landing validation must reject it
/// before the active tab, browse state, saved position, or retained owner can
/// be changed.
#[test]
fn rejected_non_grouped_music_shape_flashes_and_leaves_every_committed_value_unchanged() {
    let (mut harness, id) = wide_music_harness();
    let before_tab = harness.model().app.tab;
    let before_stack = nav_stack_snapshot(&harness);
    let before_saved = harness.model().app.saved_library_position(0);
    let before_album = music_selected_album_id(&harness, &id);
    harness.model_mut().app.music_levels = vec!["genre".into(), "album".into()];

    harness
        .model_mut()
        .on_recursive_album_activated("lib-music".into(), landed_grouped_album_stack("alb1"));
    harness.step();

    assert!(
        harness.model().app.status.contains("Library error"),
        "the existing library-error feedback fires: {}",
        harness.model().app.status
    );
    assert_eq!(
        harness.model().app.status_severity,
        crate::app::dispatch::notify::ToastSeverity::Error
    );
    assert_eq!(
        harness.model().app.tab,
        before_tab,
        "the active tab is unchanged"
    );
    assert_eq!(
        nav_stack_snapshot(&harness),
        before_stack,
        "the nav stack is unchanged"
    );
    assert_eq!(
        harness.model().app.saved_library_position(0),
        before_saved,
        "the saved Library position is unchanged"
    );
    let key = crate::app::components::library_panel::LibraryKey::Service {
        service: mbv_core::config::ServiceKind::Emby,
        library_id: "lib-music".into(),
        kind: crate::app::components::library_panel::LibraryKind::Music,
    };
    let retained_album = music_panel(&harness)
        .owner(&key)
        .and_then(|owner| {
            owner
                .as_any()
                .downcast_ref::<crate::app::components::music_content::MusicContent>()
        })
        .and_then(|owner| owner.selected_item().map(|item| item.id.clone()));
    assert_eq!(
        retained_album, before_album,
        "the retained component selection is unchanged"
    );
}
