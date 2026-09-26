//! Attached generic Emby Session commands remain direct operations.

use crate::app::tests::{install_test_emby, make_app_stub, make_item, make_session};
use crate::app::*;
use mbv_net::mock_http::MockHttp;

fn attached_app() -> App {
    let mut app = make_app_stub();
    app.connected_session_id = Some("session".into());
    app.connected_session_state = Some(make_session("Client", "Emby"));
    app.terminal_width = 160;
    app.player_tab.set_items(
        vec![make_item("a", "Movie"), make_item("b", "Movie")],
        app.player_tab.queue_cursor,
    );
    app
}

fn remote_command_app() -> (App, MockHttp) {
    let http = MockHttp::new();
    let mut app = attached_app();
    app.config.lock().unwrap().server_url = "http://127.0.0.1:1".into();
    let config = app.config.lock().unwrap().clone();
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
    let mut item_a = app.player_tab.emby_items()[0].clone();
    item_a.id = "a".into();
    item_a.playback_position_ticks = 100;
    let mut item_b = app.player_tab.emby_items()[1].clone();
    item_b.id = "b".into();
    item_b.playback_position_ticks = 200;
    let mut item_c = make_item("c", "Movie");
    item_c.playback_position_ticks = 300;
    app.player_tab.set_item_at(
        0,
        mbv_core::playback_queue::QueueItem::Emby(Box::new(item_a)),
    );
    app.player_tab.set_item_at(
        1,
        mbv_core::playback_queue::QueueItem::Emby(Box::new(item_b)),
    );
    app.player_tab
        .append_item(mbv_core::playback_queue::QueueItem::Emby(Box::new(item_c)));
    let mut session = make_session("Client", "Emby");
    session.id = "session".into();
    session.now_playing_item_id = Some("a".into());
    session.position_s = 60;
    session.runtime_s = 300;
    session.position_ticks = 60 * mbv_core::api::TICKS_PER_SECOND;
    session.runtime_ticks = 300 * mbv_core::api::TICKS_PER_SECOND;
    app.connected_session_state = Some(session);
    (app, http)
}

fn capture_error(http: &MockHttp, app: &mut App, act: impl FnOnce(&mut App)) -> String {
    http.respond(500, "command failed");
    act(app);
    let event = app
        .channels
        .sessions_rx
        .recv_timeout(std::time::Duration::from_secs(2))
        .expect("command error");
    assert!(matches!(event, SessionEvent::CommandError { .. }));
    app.handle_session_event(event);
    assert!(app.status.contains("Remote command failed"));
    http.requests().into_iter().last().unwrap()
}

#[test]
fn multi_item_play_dispatches_and_reports_errors_without_tracking() {
    let (mut app, http) = remote_command_app();
    let items = app.player_tab.emby_items();
    let request = capture_error(&http, &mut app, move |app| {
        app.submit_attached_sequence("session", &items, 1);
    });
    assert!(request.starts_with("POST /Sessions/session/Playing HTTP/1.1"));
    assert!(
        request.contains("PlayCommand")
            && request.contains("PlayNow")
            && request.contains("ItemIds")
            && request.contains("StartIndex")
            && request.contains("StartPositionTicks")
    );
}

#[test]
fn seek_dispatches_and_reports_errors_without_tracking() {
    let (mut app, http) = remote_command_app();
    let request = capture_error(&http, &mut app, |app| {
        app.dispatch(&crate::app::dispatch::action::Command::SeekRelative(5.0));
    });
    assert!(request
        .starts_with("POST /Sessions/session/Playing/Seek?SeekPositionTicks=650000000 HTTP/1.1"));
}

#[test]
fn session_item_change_stamps_canonical_active_slot_and_unblocks_removal() {
    let mut app = attached_app();
    let mut first = app.player_tab.emby_items()[0].clone();
    first.id = "a".into();
    let mut second = app.player_tab.emby_items()[1].clone();
    second.id = "b".into();
    app.player_tab.set_items(vec![first, second], 0);
    // Receiver auto-advanced: item "b" (slot 2) is now playing.
    let mut advanced = make_session("Client", "Emby");
    advanced.id = "session".into();
    advanced.now_playing_item_id = Some("b".into());
    app.handle_session_event(SessionEvent::Loaded {
        sessions: vec![advanced],
    });

    let b_slot = app.player_tab.queue.slots()[1].slot_id;
    assert_eq!(app.player_tab.queue.active_slot_id(), Some(b_slot));

    // The stale previously-played first row is deletable again: no confirm
    // modal, no silent no-op.
    let before = app.player_tab.total_queue_len();
    app.remove_from_queue(0);
    assert_eq!(
        app.player_tab.total_queue_len(),
        before - 1,
        "removing a non-playing row must not be blocked by a stale active slot"
    );
    assert!(
        app.pending_overlay.is_none(),
        "no confirm modal for a non-playing row"
    );
}
