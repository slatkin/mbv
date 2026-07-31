use super::*;
use crate::app::tests::*;

#[test]
fn try_daemon_route_connect_returns_remote_player_on_successful_connect() {
    let _guard = crate::config::TestStateDirGuard::new();
    let _connect_guard = DAEMON_ROUTE_CONNECT_TEST_LOCK.lock().unwrap();
    fn route_connect_success(
        _endpoint: &mbv_core::remote_player::DaemonEndpoint,
        _auth_token: &str,
    ) -> Result<
        (
            mbv_core::remote_player::RemotePlayer,
            mpsc::Receiver<PlayerEvent>,
        ),
        String,
    > {
        Ok(mbv_core::remote_player::RemotePlayer::stub(
            make_items(1),
            0,
        ))
    }

    *DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = Some(route_connect_success);
    let app = make_app_stub();
    let endpoint = mbv_core::remote_player::DaemonEndpoint::Unix(std::path::PathBuf::from(
        "/tmp/mbv-music.sock",
    ));

    let result = app.try_daemon_route_connect(&endpoint, "Music");

    *DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = None;
    assert!(result.is_ok());
}

#[test]
fn try_daemon_route_connect_returns_a_ready_to_display_warning_without_flashing_on_failure() {
    let _guard = crate::config::TestStateDirGuard::new();
    let _connect_guard = DAEMON_ROUTE_CONNECT_TEST_LOCK.lock().unwrap();
    fn route_connect_failure(
        _endpoint: &mbv_core::remote_player::DaemonEndpoint,
        _auth_token: &str,
    ) -> Result<
        (
            mbv_core::remote_player::RemotePlayer,
            mpsc::Receiver<PlayerEvent>,
        ),
        String,
    > {
        Err("connection refused".to_string())
    }

    *DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = Some(route_connect_failure);
    let app = make_app_stub();
    let endpoint = mbv_core::remote_player::DaemonEndpoint::Unix(std::path::PathBuf::from(
        "/tmp/mbv-music.sock",
    ));

    let result = app.try_daemon_route_connect(&endpoint, "Music");

    *DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = None;
    // `RemotePlayer` derives only `Clone` (no `PartialEq`/`Debug` --
    // confirmed against `crates/mbv-core/src/remote_player.rs`), so the
    // whole `Result` can't go through `assert_eq!` directly; match out
    // the `Err` payload instead.
    match result {
        Ok(_) => panic!("expected a connect failure to return Err, got Ok"),
        Err(message) => {
            assert_eq!(
                message,
                "\u{26a0} Music route unreachable, using local playback (mbv.log)"
            );
        }
    }
    // The primitive itself must never flash -- that is the caller's
    // job (see Architecture). `make_app_stub()` starts with an empty
    // status, so this pins down that `try_daemon_route_connect` left
    // it untouched.
    assert!(app.status.is_empty());
}

#[test]
fn app_construction_never_attempts_a_daemon_route_connect() {
    // #222 acceptance criterion: "No connection attempt happens before
    // the first play/enqueue action that needs one." There is no
    // production call site wiring `try_daemon_route_connect` into
    // startup yet (that wiring is #223's job) -- this test pins the
    // invariant down as a regression guard so a future startup-time
    // call is caught immediately instead of silently reintroducing the
    // eager-connect behavior #222 replaces.
    static CALLS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let _guard = crate::config::TestStateDirGuard::new();
    let _connect_guard = DAEMON_ROUTE_CONNECT_TEST_LOCK.lock().unwrap();
    CALLS.store(0, std::sync::atomic::Ordering::SeqCst);
    fn counting_connect(
        _endpoint: &mbv_core::remote_player::DaemonEndpoint,
        _auth_token: &str,
    ) -> Result<
        (
            mbv_core::remote_player::RemotePlayer,
            mpsc::Receiver<PlayerEvent>,
        ),
        String,
    > {
        CALLS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(mbv_core::remote_player::RemotePlayer::stub(Vec::new(), 0))
    }

    *DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = Some(counting_connect);
    let _app = make_app_stub();
    *DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = None;

    assert_eq!(CALLS.load(std::sync::atomic::Ordering::SeqCst), 0);
}

#[test]
fn apply_route_for_playback_swaps_to_routed_daemon_on_success() {
    // #256: library-route resolution is now a pure config read -- no
    // live session lookup, no SESSIONS_LOAD_OVERRIDE seam needed here.
    let _guard = crate::config::TestStateDirGuard::new();
    let _connect_guard = DAEMON_ROUTE_CONNECT_TEST_LOCK.lock().unwrap();
    fn route_connect_success(
        _endpoint: &mbv_core::remote_player::DaemonEndpoint,
        _auth_token: &str,
    ) -> Result<
        (
            mbv_core::remote_player::RemotePlayer,
            mpsc::Receiver<PlayerEvent>,
        ),
        String,
    > {
        Ok(mbv_core::remote_player::RemotePlayer::stub(
            make_items(1),
            0,
        ))
    }
    *DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = Some(route_connect_success);

    let mut app = make_app_stub();
    app.library_routes
        .insert("music".to_string(), "tcp://127.0.0.1:9000".to_string());
    let mut lib_item = make_item("Music", "CollectionFolder");
    lib_item.id = "lib-music".to_string();
    app.libs.push(LibraryTab {
        library: lib_item,
        nav_stack: Vec::new(),
        search: None,
        feed_home_video: None,
        album_track_focus: None,
        artist_header_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });
    let mut item = make_item("Song", "Audio");
    item.id = "song-1".to_string();
    app.library_tab = 1;

    app.apply_route_for_playback(&item);

    *DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = None;
    assert_eq!(app.active_route.as_deref(), Some("music"));
    assert!(app.player.is_remote());
}

#[test]
fn apply_route_for_playback_falls_back_to_local_with_warning_on_connect_failure() {
    // #256: library-route resolution is now a pure config read -- no
    // live session lookup, no SESSIONS_LOAD_OVERRIDE seam needed here.
    let _guard = crate::config::TestStateDirGuard::new();
    let _connect_guard = DAEMON_ROUTE_CONNECT_TEST_LOCK.lock().unwrap();
    fn route_connect_failure(
        _endpoint: &mbv_core::remote_player::DaemonEndpoint,
        _auth_token: &str,
    ) -> Result<
        (
            mbv_core::remote_player::RemotePlayer,
            mpsc::Receiver<PlayerEvent>,
        ),
        String,
    > {
        Err("connection refused".to_string())
    }
    *DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = Some(route_connect_failure);

    let mut app = make_app_stub();
    app.library_routes
        .insert("music".to_string(), "tcp://127.0.0.1:9000".to_string());
    let mut lib_item = make_item("Music", "CollectionFolder");
    lib_item.id = "lib-music".to_string();
    app.libs.push(LibraryTab {
        library: lib_item,
        nav_stack: Vec::new(),
        search: None,
        feed_home_video: None,
        album_track_focus: None,
        artist_header_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });
    let mut item = make_item("Song", "Audio");
    item.id = "song-1".to_string();
    app.library_tab = 1;

    app.apply_route_for_playback(&item);

    *DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = None;
    assert!(app.active_route.is_none());
    assert!(!app.player.is_remote());
    assert!(app.status.contains("unreachable"));
}

#[test]
fn apply_route_for_playback_is_noop_when_item_already_matches_active_route() {
    // #256: resolution is now a pure config read -- no live session
    // lookup, no SESSIONS_LOAD_OVERRIDE seam needed to reach the no-op
    // branch (`name == current`), even though this test's whole point
    // is that no *connect* attempt happens.
    let mut app = make_app_stub();
    app.library_routes
        .insert("music".to_string(), "tcp://127.0.0.1:9000".to_string());
    app.active_route = Some("music".to_string());
    let mut lib_item = make_item("Music", "CollectionFolder");
    lib_item.id = "lib-music".to_string();
    app.libs.push(LibraryTab {
        library: lib_item,
        nav_stack: Vec::new(),
        search: None,
        feed_home_video: None,
        album_track_focus: None,
        artist_header_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });
    let mut item = make_item("Song", "Audio");
    item.id = "song-1".to_string();
    app.library_tab = 1;

    app.apply_route_for_playback(&item);

    // No connect attempt was needed (no DAEMON_ROUTE_CONNECT_OVERRIDE
    // set, so a real connect attempt would panic/hang if this weren't
    // a no-op) -- active_route and local-ness are unchanged.
    assert_eq!(app.active_route.as_deref(), Some("music"));
    assert!(!app.player.is_remote());
}

#[test]
fn apply_route_for_playback_restores_local_when_item_has_no_route() {
    let mut app = make_app_stub();
    let (remote, remote_rx) = mbv_core::remote_player::RemotePlayer::stub(make_items(1), 0);
    app.switch_to_library_route("music", remote, remote_rx, false);
    let mut movies_item = make_item("Movies", "CollectionFolder");
    movies_item.id = "lib-movies".to_string();
    app.libs.push(LibraryTab {
        library: movies_item,
        nav_stack: Vec::new(),
        search: None,
        feed_home_video: None,
        album_track_focus: None,
        artist_header_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });
    let mut item = make_item("Movie", "Movie");
    item.id = "movie-1".to_string();

    app.apply_route_for_playback(&item);

    assert!(app.active_route.is_none());
    assert!(!app.player.is_remote());
}

#[test]
fn apply_route_for_playback_restores_local_via_restore_local_mode_when_swap_to_a_different_route_fails(
) {
    // Regression guard for the `Err` branch's `was_routed.is_some()`
    // arm: already on a different route ("music"), an item resolving
    // to a *new* route ("movies") whose connect attempt fails must be
    // torn down through `restore_local_mode` -- not just flashed a
    // warning while silently staying attached to the stale "music"
    // remote player.
    // #256: library-route resolution is now a pure config read -- no
    // live session lookup, no SESSIONS_LOAD_OVERRIDE seam needed here.
    let _guard = crate::config::TestStateDirGuard::new();
    let _connect_guard = DAEMON_ROUTE_CONNECT_TEST_LOCK.lock().unwrap();
    fn route_connect_failure(
        _endpoint: &mbv_core::remote_player::DaemonEndpoint,
        _auth_token: &str,
    ) -> Result<
        (
            mbv_core::remote_player::RemotePlayer,
            mpsc::Receiver<PlayerEvent>,
        ),
        String,
    > {
        Err("connection refused".to_string())
    }

    let mut app = make_app_stub();
    app.library_routes
        .insert("music".to_string(), "tcp://127.0.0.1:9000".to_string());
    app.library_routes
        .insert("movies".to_string(), "tcp://127.0.0.1:9001".to_string());
    let (remote, remote_rx) = mbv_core::remote_player::RemotePlayer::stub(make_items(1), 0);
    app.switch_to_library_route("music", remote, remote_rx, false);
    assert_eq!(app.active_route.as_deref(), Some("music"));
    assert!(app.player.is_remote());

    let mut lib_item = make_item("Movies", "CollectionFolder");
    lib_item.id = "lib-movies".to_string();
    app.libs.push(LibraryTab {
        library: lib_item,
        nav_stack: Vec::new(),
        search: None,
        feed_home_video: None,
        album_track_focus: None,
        artist_header_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });
    let mut item = make_item("Movie", "Movie");
    item.id = "movie-1".to_string();
    app.library_tab = 1;

    *DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = Some(route_connect_failure);
    app.apply_route_for_playback(&item);
    *DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = None;

    assert!(app.active_route.is_none());
    assert!(!app.player.is_remote());
    assert!(app.status.contains("unreachable"));
}
