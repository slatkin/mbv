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
    let mut app = make_app_stub();
    let config = app.config.lock().unwrap().clone();
    install_test_emby(&mut app, config);
    let endpoint = mbv_core::remote_player::DaemonEndpoint::Unix(std::path::PathBuf::from(
        "/tmp/mbv-music.sock",
    ));

    let result = app.try_daemon_route_connect(&endpoint, "Music");

    *DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = None;
    assert!(result.is_ok());
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
        search: None,
        nav_stack: Vec::new(),
        feed_home_video: None,
        album_track_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });
    let mut item = make_item("Song", "Audio");
    item.id = "song-1".to_string();
    app.tab = TabSelection::EmbyLibrary(0);

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
    app.switch_to_library_route(
        "music",
        remote,
        remote_rx,
        &mbv_core::remote_player::DaemonEndpoint::Tcp("127.0.0.1:0".parse().unwrap()),
    );
    let mut movies_item = make_item("Movies", "CollectionFolder");
    movies_item.id = "lib-movies".to_string();
    app.libs.push(LibraryTab {
        library: movies_item,
        search: None,
        nav_stack: Vec::new(),
        feed_home_video: None,
        album_track_focus: None,
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
fn apply_route_for_playback_double_failure_strips_using_local_playback() {
    // Regression: when `apply_route_for_playback` tries a new route while
    // already routed, both the target-route connect and the subsequent
    // Local-daemon restoration can fail. The route-failure message from
    // `try_daemon_route_connect` contains "using local playback", which is
    // wrong when the Local daemon is also unreachable. `restore_local_mode`
    // must strip that claim so the final warning is accurate.
    let _guard = crate::config::TestStateDirGuard::new();
    let _connect_guard = DAEMON_ROUTE_CONNECT_TEST_LOCK.lock().unwrap();
    fn always_fail(
        _endpoint: &mbv_core::remote_player::DaemonEndpoint,
        _auth_token: &str,
    ) -> Result<
        (
            mbv_core::remote_player::RemotePlayer,
            std::sync::mpsc::Receiver<PlayerEvent>,
        ),
        String,
    > {
        Err("connection refused".to_string())
    }

    let mut app = make_local_daemon_app_stub(make_items(2));
    app.library_routes
        .insert("music".to_string(), "tcp://127.0.0.1:9000".to_string());
    app.library_routes
        .insert("movies".to_string(), "tcp://127.0.0.1:9001".to_string());
    let (remote, remote_rx) = mbv_core::remote_player::RemotePlayer::stub(make_items(1), 0);
    app.switch_to_library_route(
        "music",
        remote,
        remote_rx,
        &mbv_core::remote_player::DaemonEndpoint::Tcp("127.0.0.1:0".parse().unwrap()),
    );
    assert_eq!(app.active_route.as_deref(), Some("music"));

    let mut lib_item = make_item("Movies", "CollectionFolder");
    lib_item.id = "lib-movies".to_string();
    app.libs.push(LibraryTab {
        library: lib_item,
        search: None,
        nav_stack: Vec::new(),
        feed_home_video: None,
        album_track_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });
    let mut item = make_item("Movie", "Movie");
    item.id = "movie-1".to_string();
    app.tab = TabSelection::EmbyLibrary(0);

    // Both the movies-route connect AND the Local-daemon restoration fail.
    *DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = Some(always_fail);
    app.apply_route_for_playback(&item);
    *DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = None;

    assert!(app.active_route.is_none());
    assert!(
        app.status.contains("local daemon unavailable"),
        "status was: {:?}",
        app.status
    );
    assert!(
        !app.status.contains("using local playback"),
        "double-failure status must not claim usable local playback: {:?}",
        app.status
    );
}
