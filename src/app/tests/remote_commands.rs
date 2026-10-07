//! Attached generic Emby Session commands remain direct operations.

use crate::app::tests::{QueueViewTestExt, install_test_emby, live_owner_channel, make_app_stub};
use crate::app::*;
use mbv_emby_model::test_support::make_item;
use mbv_net::mock_http::MockHttp;

fn attached_app() -> App {
    let mut app = make_app_stub();
    // The stub player starts active (a remote-owner stand-in); this
    // scenario watches a remote session, so park the local player idle
    // to reach the session-reported now-playing path.
    app.player.update_status(|status| status.active = false);
    app.connected_session_id = Some("session".into());
    app.connected_session_state = Some(mbv_emby::test_support::make_session("Client", "Emby"));
    app.terminal_width = 160;
    app.local_view.adopt_items(
        vec![make_item("a", "Movie"), make_item("b", "Movie")],
        app.local_view.cursor(),
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
    app.emby_runtime = crate::app::state::service_runtime::EmbyRuntime::ready(std::sync::Arc::new(
        std::sync::Mutex::new(client),
    ));
    let mut item_a = app.local_view.emby_items()[0].clone();
    item_a.id = "a".into();
    item_a.playback_position_ticks = 100;
    let mut item_b = app.local_view.emby_items()[1].clone();
    item_b.id = "b".into();
    item_b.playback_position_ticks = 200;
    let mut item_c = make_item("c", "Movie");
    item_c.playback_position_ticks = 300;
    app.local_view.adopt_items(vec![item_a, item_b, item_c], 0);
    let mut session = mbv_emby::test_support::make_session("Client", "Emby");
    session.id = "session".into();
    session.now_playing_item_id = Some("a".into());
    session.position_s = 60;
    session.runtime_s = 300;
    session.position_ticks = 60 * mbv_emby_model::TICKS_PER_SECOND;
    session.runtime_ticks = 300 * mbv_emby_model::TICKS_PER_SECOND;
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
    let items = app.local_view.emby_items();
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
    assert!(
        request.starts_with(
            "POST /Sessions/session/Playing/Seek?SeekPositionTicks=650000000 HTTP/1.1"
        )
    );
}

#[test]
fn session_item_change_selects_owner_reported_queue_slot_and_unblocks_removal() {
    let mut app = attached_app();
    let mut first = app.local_view.emby_items()[0].clone();
    first.id = "a".into();
    let mut second = app.local_view.emby_items()[1].clone();
    second.id = "b".into();
    app.local_view.adopt_items(vec![first, second], 0);
    // Receiver auto-advanced: item "b" (slot 2) is now playing.
    let mut advanced = mbv_emby::test_support::make_session("Client", "Emby");
    advanced.id = "session".into();
    advanced.now_playing_item_id = Some("b".into());
    app.handle_session_event(SessionEvent::Loaded {
        sessions: vec![advanced],
    });

    // The client does not stamp the owner-reported active slot into its view;
    // removal still reaches the Player owner rather than being locally refused.
    let cmd_rx = live_owner_channel(&mut app);
    // The live stub starts active on row 0; this scenario watches the remote
    // session's now-playing item, so park the local player idle again.
    app.player.update_status(|status| status.active = false);
    app.remove_from_queue(0);
    assert!(
        app.pending_overlay.is_none(),
        "no confirm modal for a non-playing row"
    );
    assert!(
        matches!(
            cmd_rx.try_recv().unwrap(),
            mbv_ctrl::CtrlCmd::UnifiedQueueRemoveSlot { op: None, .. }
        ),
        "removing a non-playing row must reach the Player owner"
    );
}
