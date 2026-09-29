use super::*;

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
    fn counting_connect(
        _endpoint: &mbv_remote_player::DaemonEndpoint,
    ) -> crate::app::test_seams::DaemonRouteConnectOutcome {
        CALLS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let (remote, events) = mbv_remote_player::RemotePlayer::stub(Vec::new(), 0);
        crate::app::test_seams::DaemonRouteConnectOutcome::Connected(remote, events)
    }

    let _guard = crate::config::TestStateDirGuard::new();
    let _connect_guard = DAEMON_ROUTE_CONNECT_TEST_LOCK.lock().unwrap();
    CALLS.store(0, std::sync::atomic::Ordering::SeqCst);
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
    app.libs.push(LibraryTab::new(lib_item));
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
