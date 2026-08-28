//! Tracked-vs-untracked remote command payload parity tests, split out
//! of `tests_remote_reconciliation.rs` to keep that file within the
//! repository's file-size limit.

use super::{attached_app, projection, tracker};
use crate::app::tests::{install_test_emby, make_item, make_session};
use crate::app::*;
use mbv_core::remote_reconciliation::RemoteIntent;

// ── task 1.3: tracked vs untracked command payload parity ────────────────

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
    item_c.id = "c".into();
    item_c.playback_position_ticks = 300;
    app.player_tab.set_item_at(
        0,
        mbv_core::playback_queue::QueueItem::Emby(Box::new(item_a)),
    );
    app.player_tab.set_item_at(
        1,
        mbv_core::playback_queue::QueueItem::Emby(Box::new(item_b)),
    );
    app.player_tab.append_item(item_c);
    let mut s = make_session("Client", "Emby");
    s.id = "session".into();
    s.now_playing_item_id = Some("a".into());
    s.position_s = 60;
    s.runtime_s = 300;
    s.position_ticks = 60 * mbv_core::api::TICKS_PER_SECOND;
    s.runtime_ticks = 300 * mbv_core::api::TICKS_PER_SECOND;
    app.connected_session_state = Some(s);
    app
}

fn attach_tracking_to(app: &mut App) {
    let slots = app.player_tab.queue.slots();
    app.remote_tracker = Some(tracker(&["a", "b", "c"]));
    app.remote_queue_projection = Some(projection(
        app,
        &[
            (1, slots[0].slot_id),
            (2, slots[1].slot_id),
            (3, slots[2].slot_id),
        ],
    ));
}

fn accept_one(
    listener: &std::net::TcpListener,
    timeout: std::time::Duration,
) -> Option<std::net::TcpStream> {
    listener.set_nonblocking(true).unwrap();
    let deadline = std::time::Instant::now() + timeout;
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                listener.set_nonblocking(false).unwrap();
                return Some(stream);
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if std::time::Instant::now() > deadline {
                    listener.set_nonblocking(false).unwrap();
                    return None;
                }
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            Err(e) => {
                listener.set_nonblocking(false).unwrap();
                panic!("listener accept failed: {e}");
            }
        }
    }
}

fn read_http_request(stream: &std::net::TcpStream) -> String {
    use std::io::{BufRead, BufReader, Read};
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut head = String::new();
    let mut content_length = 0usize;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 {
            break;
        }
        if let Some(len) = line
            .strip_prefix("Content-Length:")
            .or_else(|| line.strip_prefix("content-length:"))
            .and_then(|rest| rest.trim().parse::<usize>().ok())
        {
            content_length = len;
        }
        let trimmed = line.trim_end();
        head.push_str(&line);
        if trimmed.is_empty() {
            break;
        }
    }
    let mut body = vec![0u8; content_length];
    if content_length > 0 {
        reader.read_exact(&mut body).unwrap();
    }
    format!("{head}{}", String::from_utf8_lossy(&body))
}

fn respond_http(stream: std::net::TcpStream, status: u16, body: &str) {
    use std::io::Write;
    let mut writer = stream.try_clone().unwrap();
    let _ = writer.write_all(
        format!(
            "HTTP/1.1 {status} OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        )
        .as_bytes(),
    );
}

/// Runs `act` (which dispatches a remote command on a background thread),
/// captures the command HTTP request that reaches `listener`, responds, and
/// drains the dispatch thread's follow-up `GET /Sessions` poll so the thread
/// completes instead of leaking.
fn capture_remote_command(
    listener: &std::net::TcpListener,
    app: &mut App,
    act: impl FnOnce(&mut App),
) -> String {
    act(app);
    let stream = accept_one(listener, std::time::Duration::from_secs(10))
        .expect("dispatch thread must send the command request");
    let request = read_http_request(&stream);
    respond_http(stream, 200, "");
    if let Some(second) = accept_one(listener, std::time::Duration::from_secs(10)) {
        let _ = read_http_request(&second);
        respond_http(second, 200, "[]");
    }
    request
}

#[test]
fn next_command_payload_is_identical_with_and_without_tracking() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let mut untracked = remote_command_app(&listener);
    let untracked_req = capture_remote_command(&listener, &mut untracked, |app| {
        app.dispatch(crate::app::action::Command::NextTrack);
    });

    let mut tracked = remote_command_app(&listener);
    attach_tracking_to(&mut tracked);
    let tracked_req = capture_remote_command(&listener, &mut tracked, |app| {
        app.dispatch(crate::app::action::Command::NextTrack);
    });

    assert_eq!(tracked_req, untracked_req);
    assert_eq!(
        tracked
            .remote_tracker
            .as_ref()
            .unwrap()
            .expected()
            .unwrap()
            .intent,
        RemoteIntent::Next { target: 2 },
        "tracking records the occurrence at the untracked destination"
    );
}

#[test]
fn previous_command_payload_is_identical_with_and_without_tracking() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let now_playing_b = |app: &mut App| {
        if let Some(s) = app.connected_session_state.as_mut() {
            s.now_playing_item_id = Some("b".into());
        }
    };
    let mut untracked = remote_command_app(&listener);
    now_playing_b(&mut untracked);
    let untracked_req = capture_remote_command(&listener, &mut untracked, |app| {
        app.dispatch(crate::app::action::Command::PreviousTrack);
    });

    let mut tracked = remote_command_app(&listener);
    now_playing_b(&mut tracked);
    attach_tracking_to(&mut tracked);
    let tracked_req = capture_remote_command(&listener, &mut tracked, |app| {
        app.dispatch(crate::app::action::Command::PreviousTrack);
    });

    assert_eq!(tracked_req, untracked_req);
    assert_eq!(
        tracked
            .remote_tracker
            .as_ref()
            .unwrap()
            .expected()
            .unwrap()
            .intent,
        RemoteIntent::Previous { target: 1 }
    );
}

#[test]
fn direct_selection_payload_is_identical_with_and_without_tracking() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let mut untracked = remote_command_app(&listener);
    untracked.player_tab.queue_cursor = 1;
    let untracked_req = capture_remote_command(&listener, &mut untracked, |app| {
        app.dispatch(crate::app::action::Command::QueuePlayCursor(1));
    });

    let mut tracked = remote_command_app(&listener);
    tracked.player_tab.queue_cursor = 1;
    attach_tracking_to(&mut tracked);
    let tracked_req = capture_remote_command(&listener, &mut tracked, |app| {
        app.dispatch(crate::app::action::Command::QueuePlayCursor(1));
    });

    assert_eq!(tracked_req, untracked_req);
    assert_eq!(
        tracked
            .remote_tracker
            .as_ref()
            .unwrap()
            .expected()
            .unwrap()
            .intent,
        RemoteIntent::Select { target: 2 }
    );
}

#[test]
fn restart_current_payload_is_identical_with_and_without_tracking() {
    // Restart maps to re-activating the now-playing item from the queue:
    // the untracked path re-submits the visible sequence at that index while
    // tracking issues a Select intent for the current occurrence — both send
    // the identical session_play_items payload.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let mut untracked = remote_command_app(&listener);
    untracked.player_tab.queue_cursor = 0;
    let untracked_req = capture_remote_command(&listener, &mut untracked, |app| {
        app.dispatch(crate::app::action::Command::QueuePlayCursor(0));
    });

    let mut tracked = remote_command_app(&listener);
    tracked.player_tab.queue_cursor = 0;
    attach_tracking_to(&mut tracked);
    let tracked_req = capture_remote_command(&listener, &mut tracked, |app| {
        app.dispatch(crate::app::action::Command::QueuePlayCursor(0));
    });

    assert_eq!(tracked_req, untracked_req);
    assert_eq!(
        tracked
            .remote_tracker
            .as_ref()
            .unwrap()
            .expected()
            .unwrap()
            .intent,
        RemoteIntent::Select { target: 1 }
    );
}

#[test]
fn seek_payload_is_identical_with_and_without_tracking() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let mut untracked = remote_command_app(&listener);
    let untracked_req = capture_remote_command(&listener, &mut untracked, |app| {
        app.dispatch(crate::app::action::Command::SeekRelative(5.0));
    });

    let mut tracked = remote_command_app(&listener);
    attach_tracking_to(&mut tracked);
    let tracked_req = capture_remote_command(&listener, &mut tracked, |app| {
        app.dispatch(crate::app::action::Command::SeekRelative(5.0));
    });

    assert_eq!(tracked_req, untracked_req);
    assert_eq!(
        tracked
            .remote_tracker
            .as_ref()
            .unwrap()
            .expected()
            .unwrap()
            .intent,
        RemoteIntent::Seek
    );
}

#[test]
fn play_pause_payload_is_identical_with_and_without_tracking() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let mut untracked = remote_command_app(&listener);
    let untracked_req = capture_remote_command(&listener, &mut untracked, |app| {
        app.dispatch(crate::app::action::Command::TogglePlayPause);
    });

    let mut tracked = remote_command_app(&listener);
    attach_tracking_to(&mut tracked);
    let tracked_req = capture_remote_command(&listener, &mut tracked, |app| {
        app.dispatch(crate::app::action::Command::TogglePlayPause);
    });

    assert_eq!(tracked_req, untracked_req);
}

#[test]
fn stop_payload_is_identical_with_and_without_tracking() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let mut untracked = remote_command_app(&listener);
    let untracked_req = capture_remote_command(&listener, &mut untracked, |app| {
        app.dispatch(crate::app::action::Command::Stop);
    });

    let mut tracked = remote_command_app(&listener);
    attach_tracking_to(&mut tracked);
    let tracked_req = capture_remote_command(&listener, &mut tracked, |app| {
        app.dispatch(crate::app::action::Command::Stop);
    });

    assert_eq!(tracked_req, untracked_req);
    assert_eq!(
        tracked
            .remote_tracker
            .as_ref()
            .unwrap()
            .expected()
            .unwrap()
            .intent,
        RemoteIntent::Stop
    );
}

#[test]
fn single_item_replacement_payload_is_identical_with_and_without_tracking() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let mut untracked = remote_command_app(&listener);
    let item = untracked.player_tab.emby_items()[0].clone();
    let untracked_req = capture_remote_command(&listener, &mut untracked, |app| {
        app.play_item(item.clone());
    });

    let mut tracked = remote_command_app(&listener);
    attach_tracking_to(&mut tracked);
    let tracked_req = capture_remote_command(&listener, &mut tracked, |app| {
        app.play_item(item.clone());
    });

    assert_eq!(tracked_req, untracked_req);
    assert!(
        tracked.remote_tracker.is_none(),
        "a single-item replacement retires tracking as a target replacement"
    );
}
