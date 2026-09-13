//! Attached generic Emby Session commands remain direct operations.

use super::attached_app;
use crate::app::tests::{install_test_emby, make_item, make_session};
use crate::app::*;
use std::io::{BufRead, BufReader, Read, Write};

fn remote_command_app(listener: &std::net::TcpListener) -> App {
    let url = format!("http://{}", listener.local_addr().unwrap());
    let mut app = attached_app();
    app.config.lock().unwrap().server_url = url;
    let config = app.config.lock().unwrap().clone();
    install_test_emby(&mut app, config);
    let mut item_a = app.player_tab.emby_items()[0].clone();
    item_a.id = "a".into();
    item_a.playback_position_ticks = 100;
    let mut item_b = app.player_tab.emby_items()[1].clone();
    item_b.id = "b".into();
    item_b.playback_position_ticks = 200;
    let mut item_c = make_item("c", "Movie");
    item_c.playback_position_ticks = 300;
    app.player_tab.set_item_at(0, mbv_core::playback_queue::QueueItem::Emby(Box::new(item_a)));
    app.player_tab.set_item_at(1, mbv_core::playback_queue::QueueItem::Emby(Box::new(item_b)));
    app.player_tab.append_item(item_c);
    let mut session = make_session("Client", "Emby");
    session.id = "session".into();
    session.now_playing_item_id = Some("a".into());
    session.position_s = 60;
    session.runtime_s = 300;
    session.position_ticks = 60 * mbv_core::api::TICKS_PER_SECOND;
    session.runtime_ticks = 300 * mbv_core::api::TICKS_PER_SECOND;
    app.connected_session_state = Some(session);
    app
}

fn accept_command_request(
    listener: &std::net::TcpListener,
    endpoint: &str,
) -> (std::net::TcpStream, String) {
    listener.set_nonblocking(true).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                let request = read_http_request(&stream);
                if request.starts_with(&format!("POST {endpoint} HTTP/1.1")) {
                    listener.set_nonblocking(false).unwrap();
                    return (stream, request);
                }
                // Complete unrelated requests so their worker can proceed, then
                // keep listening for the command request.
                respond(stream, 200, "[]");
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                assert!(std::time::Instant::now() <= deadline, "command was not dispatched");
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            Err(e) => panic!("listener accept failed: {e}"),
        }
    }
}

fn read_http_request(stream: &std::net::TcpStream) -> String {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut request = String::new();
    let mut content_length = 0;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 {
            break;
        }
        if let Some(length) = line.strip_prefix("Content-Length:")
            .or_else(|| line.strip_prefix("content-length:"))
            .and_then(|value| value.trim().parse::<usize>().ok())
        {
            content_length = length;
        }
        let empty = line.trim_end().is_empty();
        request.push_str(&line);
        if empty {
            break;
        }
    }
    let mut body = vec![0; content_length];
    if content_length > 0 {
        reader.read_exact(&mut body).unwrap();
        request.push_str(&String::from_utf8_lossy(&body));
    }
    request
}

fn assert_post(request: &str, endpoint: &str) {
    assert!(
        request.starts_with(&format!("POST {endpoint} HTTP/1.1")),
        "unexpected request: {request}"
    );
}

fn respond(stream: std::net::TcpStream, status: u16, body: &str) {
    let mut writer = stream.try_clone().unwrap();
    write!(
        writer,
        "HTTP/1.1 {status} Error\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    )
    .unwrap();
}

/// Captures dispatch and returns the command error through the normal user-visible path.
fn capture_error(
    listener: &std::net::TcpListener,
    app: &mut App,
    expected_endpoint: &str,
    act: impl FnOnce(&mut App),
) -> String {
    act(app);
    let (stream, request) = accept_command_request(listener, expected_endpoint);
    respond(stream, 500, "command failed");
    let event = app
        .sessions_rx
        .recv_timeout(std::time::Duration::from_secs(10))
        .expect("command error must be reported");
    assert!(matches!(event, SessionEvent::CommandError { reconciliation: None, .. }));
    app.handle_session_event(event);
    assert!(app.status.contains("Remote command failed"));
    assert!(app.remote_tracker.is_none());
    request
}

#[test]
fn multi_item_play_dispatches_and_reports_errors_without_tracking() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let mut app = remote_command_app(&listener);
    let items = app.player_tab.emby_items();
    let request = capture_error(&listener, &mut app, "/Sessions/session/Playing", move |app| {
        app.submit_attached_sequence("session", &items, 1);
    });
    assert_post(&request, "/Sessions/session/Playing");
    assert!(request.contains("PlayCommand"));
    assert!(request.contains("PlayNow"));
    assert!(request.contains("ItemIds"));
    assert!(request.contains("StartIndex"));
    assert!(request.contains("StartPositionTicks"));
}

#[test]
fn pause_play_dispatches_and_reports_errors_without_tracking() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let mut app = remote_command_app(&listener);
    let request = capture_error(
        &listener,
        &mut app,
        "/Sessions/session/Playing/PlayPause",
        |app| {
        app.dispatch(action::Command::TogglePlayPause);
    });
    assert_post(&request, "/Sessions/session/Playing/PlayPause");
}

#[test]
fn seek_dispatches_and_reports_errors_without_tracking() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let mut app = remote_command_app(&listener);
    let request = capture_error(
        &listener,
        &mut app,
        "/Sessions/session/Playing/Seek?SeekPositionTicks=650000000",
        |app| {
        app.dispatch(action::Command::SeekRelative(5.0));
    });
    assert_post(
        &request,
        "/Sessions/session/Playing/Seek?SeekPositionTicks=650000000",
    );
}

#[test]
fn stop_dispatches_and_reports_errors_without_tracking() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let mut app = remote_command_app(&listener);
    let request = capture_error(
        &listener,
        &mut app,
        "/Sessions/session/Playing/Stop",
        |app| {
        app.dispatch(action::Command::Stop);
    });
    assert_post(&request, "/Sessions/session/Playing/Stop");
}

#[test]
fn next_dispatches_and_reports_errors_without_tracking() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let mut app = remote_command_app(&listener);
    let request = capture_error(&listener, &mut app, "/Sessions/session/Playing", |app| {
        app.dispatch(action::Command::NextTrack);
    });
    assert_post(&request, "/Sessions/session/Playing");
    assert!(request.contains("ItemIds"));
    assert!(request.contains("StartIndex"));
    assert!(request.contains("StartPositionTicks"));
}

#[test]
fn previous_dispatches_and_reports_errors_without_tracking() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let mut app = remote_command_app(&listener);
    app.connected_session_state.as_mut().unwrap().now_playing_item_id = Some("b".into());
    let request = capture_error(&listener, &mut app, "/Sessions/session/Playing", |app| {
        app.dispatch(action::Command::PreviousTrack);
    });
    assert_post(&request, "/Sessions/session/Playing");
    assert!(request.contains("ItemIds"));
    assert!(request.contains("StartIndex"));
    assert!(request.contains("StartPositionTicks"));
}

#[test]
fn direct_selection_dispatches_and_reports_errors_without_tracking() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let mut app = remote_command_app(&listener);
    let request = capture_error(&listener, &mut app, "/Sessions/session/Playing", |app| {
        app.dispatch(action::Command::QueuePlayCursor(1));
    });
    assert_post(&request, "/Sessions/session/Playing");
    assert!(request.contains("ItemIds"));
    assert!(request.contains("StartIndex"));
    assert!(request.contains("StartPositionTicks"));
}
