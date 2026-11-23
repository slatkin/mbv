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
    ) -> (
        mbv_remote_player::RemotePlayer,
        std::sync::mpsc::Receiver<mbv_ctrl::player::PlayerEvent>,
    ) {
        CALLS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        mbv_remote_player::RemotePlayer::stub(Vec::new(), 0)
    }

    let _guard = crate::config::TestStateDirGuard::new();
    let _connect_guard = DAEMON_ROUTE_CONNECT_TEST_LOCK.lock().unwrap();
    CALLS.store(0, std::sync::atomic::Ordering::SeqCst);
    *DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = Some(counting_connect);
    let _app = make_app_stub();
    *DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = None;

    assert_eq!(CALLS.load(std::sync::atomic::Ordering::SeqCst), 0);
}
