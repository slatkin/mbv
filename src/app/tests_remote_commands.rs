//! Attached generic Emby Session commands remain direct operations.

use crate::app::tests::{install_test_emby, make_app_stub, make_item, make_session};
use crate::app::*;
use mbv_core::mock_http::MockHttp;

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
    let client = app.emby_runtime.client.as_ref().unwrap().lock().unwrap().clone().with_test_agent(http.agent());
    app.emby_runtime = mbv_core::service_runtime::EmbyRuntime::ready(std::sync::Arc::new(std::sync::Mutex::new(client)));
    let mut item_a = app.player_tab.emby_items()[0].clone();
    item_a.id = "a".into();
    item_a.playback_position_ticks = 100;
    let mut item_b = app.player_tab.emby_items()[1].clone();
    item_b.id = "b".into();
    item_b.playback_position_ticks = 200;
    let mut item_c = make_item("c", "Movie");
    item_c.playback_position_ticks = 300;
    app.player_tab
        .set_item_at(0, mbv_core::playback_queue::QueueItem::Emby(Box::new(item_a)));
    app.player_tab
        .set_item_at(1, mbv_core::playback_queue::QueueItem::Emby(Box::new(item_b)));
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
    let event = app.sessions_rx.recv_timeout(std::time::Duration::from_secs(2)).expect("command error");
    assert!(matches!(event, SessionEvent::CommandError { .. }));
    app.handle_session_event(event);
    assert!(app.status.contains("Remote command failed"));
    http.requests().into_iter().last().unwrap()
}

#[test]
fn session_item_change_updates_state_without_mutating_queue() {
    let mut app = attached_app();
    app.queue_dirty = true;
    let slots: Vec<_> = app.player_tab.queue.slots().iter().map(|slot| (slot.slot_id, slot.item.id().to_string())).collect();
    let mutation_state = format!("{:?}", app.playlist_mutations);
    let mut changed = make_session("Client", "Emby");
    changed.id = "session".into();
    changed.now_playing_item_id = Some("b".into());
    app.handle_session_event(SessionEvent::Loaded { sessions: vec![changed] });
    assert_eq!(app.connected_session_state.as_ref().and_then(|s| s.now_playing_item_id.as_deref()), Some("b"));
    let current: Vec<_> = app.player_tab.queue.slots().iter().map(|slot| (slot.slot_id, slot.item.id().to_string())).collect();
    assert_eq!(current, slots);
    assert_eq!(app.player_tab.queue_cursor, 0);
    assert!(app.queue_dirty);
    assert_eq!(format!("{:?}", app.playlist_mutations), mutation_state);
}

#[test]
fn multi_item_play_dispatches_and_reports_errors_without_tracking() {
    let (mut app, http) = remote_command_app();
    let items = app.player_tab.emby_items();
    let request = capture_error(&http, &mut app, move |app| app.submit_attached_sequence("session", &items, 1));
    assert!(request.starts_with("POST /Sessions/session/Playing HTTP/1.1"));
    assert!(request.contains("PlayCommand") && request.contains("PlayNow") && request.contains("ItemIds") && request.contains("StartIndex") && request.contains("StartPositionTicks"));
}

#[test]
fn pause_play_dispatches_and_reports_errors_without_tracking() {
    let (mut app, http) = remote_command_app();
    let request = capture_error(&http, &mut app, |app| { app.dispatch(crate::app::dispatch::action::Command::TogglePlayPause); });
    assert!(request.starts_with("POST /Sessions/session/Playing/PlayPause HTTP/1.1"));
}

#[test]
fn seek_dispatches_and_reports_errors_without_tracking() {
    let (mut app, http) = remote_command_app();
    let request = capture_error(&http, &mut app, |app| { app.dispatch(crate::app::dispatch::action::Command::SeekRelative(5.0)); });
    assert!(request.starts_with("POST /Sessions/session/Playing/Seek?SeekPositionTicks=650000000 HTTP/1.1"));
}

#[test]
fn stop_dispatches_and_reports_errors_without_tracking() {
    let (mut app, http) = remote_command_app();
    let request = capture_error(&http, &mut app, |app| { app.dispatch(crate::app::dispatch::action::Command::Stop); });
    assert!(request.starts_with("POST /Sessions/session/Playing/Stop HTTP/1.1"));
}

#[test]
fn next_dispatches_and_reports_errors_without_tracking() {
    let (mut app, http) = remote_command_app();
    let request = capture_error(&http, &mut app, |app| { app.dispatch(crate::app::dispatch::action::Command::NextTrack); });
    assert!(request.starts_with("POST /Sessions/session/Playing HTTP/1.1"));
    assert!(request.contains("ItemIds") && request.contains("StartIndex") && request.contains("StartPositionTicks"));
}

#[test]
fn previous_dispatches_and_reports_errors_without_tracking() {
    let (mut app, http) = remote_command_app();
    app.connected_session_state.as_mut().unwrap().now_playing_item_id = Some("b".into());
    let request = capture_error(&http, &mut app, |app| { app.dispatch(crate::app::dispatch::action::Command::PreviousTrack); });
    assert!(request.starts_with("POST /Sessions/session/Playing HTTP/1.1"));
    assert!(request.contains("ItemIds") && request.contains("StartIndex") && request.contains("StartPositionTicks"));
}

#[test]
fn direct_selection_dispatches_and_reports_errors_without_tracking() {
    let (mut app, http) = remote_command_app();
    let request = capture_error(&http, &mut app, |app| {
        app.dispatch(crate::app::dispatch::action::Command::QueuePlayCursor(1));
    });
    assert!(request.starts_with("POST /Sessions/session/Playing HTTP/1.1"));
    assert!(request.contains("ItemIds"));
    assert!(request.contains("StartIndex"));
    assert!(request.contains("StartPositionTicks"));
}

#[test]
fn remote_jump_target_resolves_next_and_previous_without_tracking() {
    let mut app = attached_app();
    let mut first = app.player_tab.emby_items()[0].clone(); first.id = "a".into(); first.playback_position_ticks = 10;
    let mut second = app.player_tab.emby_items()[1].clone(); second.id = "a".into(); second.playback_position_ticks = 20;
    let mut third = make_item("b", "Movie"); third.id = "b".into(); third.playback_position_ticks = 30;
    app.player_tab.set_items(vec![first, second, third], 0);
    let target = crate::app::dispatch::session::command::remote_jump_target(&app.player_tab, Some("a"), 1);
    assert_eq!(target, Some((1, 20)));
    assert_eq!(crate::app::dispatch::session::command::remote_jump_target(&app.player_tab, Some("b"), -1), Some((1, 20)));
}

/// A receiver-driven track change must stamp the canonical queue's active
/// slot: without it the queue keeps the originally submitted slot "active"
/// and Delete on that stale row hits `RequiresActiveConfirmation` and
/// silently vanishes (queue regrade, session-follow was display-only).
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
    app.handle_session_event(SessionEvent::Loaded { sessions: vec![advanced] });

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
    assert!(app.pending_overlay.is_none(), "no confirm modal for a non-playing row");
}
