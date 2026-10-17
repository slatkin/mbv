use super::*;

#[test]
fn enqueue_route_conflict_rejects_mismatched_route() {
    let mut app = make_app_stub();
    app.active_route = Some("music".to_string());
    assert!(app.enqueue_route_conflict(Some(&"movies".to_string())));
    assert!(app.status.contains("Can't mix libraries in a routed queue"));
}

#[test]
fn enqueue_route_conflict_allows_enqueue_while_on_a_non_route_direct_remote() {
    let mut app = make_app_stub();
    let (remote, remote_rx) = mbv_remote_player::RemotePlayer::stub(make_items(1), 0);
    app.player = mbv_player::PlayerProxy::from_remote(remote, false);
    app.player_rx = remote_rx;
    // active_route stays None: this is a Sessions-panel direct-remote
    // connection, not a library route.
    assert!(!app.enqueue_route_conflict(Some(&"music".to_string())));
}
