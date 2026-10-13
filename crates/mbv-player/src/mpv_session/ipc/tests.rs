//! Socket-pair contracts of the IPC session (design D9). The peer is the other
//! end of a `UnixStream::pair()`, so no mpv process runs.

use std::io::Write;
use std::os::unix::net::UnixStream;
use std::time::Duration;

use serde_json::json;

use super::IpcSession;
use crate::mpv_error::MpvError;
use crate::mpv_event::{MpvEvent, MpvFailure, PropertyValue};

fn session_with_peer() -> (IpcSession, UnixStream) {
    let (ours, peer) = UnixStream::pair().expect("socket pair");
    let session = IpcSession::from_stream(ours).expect("session over a socket pair");
    (session, peer)
}

fn send(peer: &mut UnixStream, lines: &str) {
    peer.write_all(lines.as_bytes()).expect("peer write");
}

#[test]
fn event_before_reply_is_queued_in_order() {
    let (mut session, mut peer) = session_with_peer();
    send(
        &mut peer,
        "{\"event\":\"playback-restart\"}\n\
         {\"event\":\"property-change\",\"id\":1,\"name\":\"pause\",\"data\":false}\n\
         {\"request_id\":1,\"error\":\"success\",\"data\":42}\n",
    );

    let data = session.request(&["get_property", "time-pos"], Duration::ZERO);

    assert_eq!(data.expect("reply data"), json!(42));
    assert!(matches!(
        session.next_event(),
        Some(MpvEvent::PlaybackRestart)
    ));
    assert!(matches!(
        session.next_event(),
        Some(MpvEvent::PropertyChange { ref name, value: PropertyValue::Flag(false) })
            if name == "pause"
    ));
    assert!(session.next_event().is_none());
}

#[test]
fn reply_that_never_comes_is_a_timeout() {
    let (mut session, _peer) = session_with_peer();

    let result = session.request(&["get_property", "time-pos"], Duration::ZERO);

    assert!(matches!(result, Err(MpvError::Timeout)));
}

// S8: mpv closes the socket after a quit and sends no shutdown event, so a
// close after a requested quit is a shutdown, reported once. The peer closes
// with the quit line unread, so the kernel resets the socket, and the session
// reads that reset as the close.
#[test]
fn s8_close_after_quit_is_shutdown_once() {
    let (mut session, mut peer) = session_with_peer();
    send(&mut peer, "{\"request_id\":1,\"error\":\"success\"}\n");
    session
        .request(&["quit"], Duration::ZERO)
        .expect("quit reply");
    drop(peer);

    assert!(matches!(session.next_event(), Some(MpvEvent::Shutdown)));
    assert!(session.next_event().is_none());
}

// S8: the socket closes with no quit requested, so the child process exited
// on its own and the run fails.
#[test]
fn s8_close_without_quit_is_process_exited() {
    let (mut session, peer) = session_with_peer();
    drop(peer);

    assert!(matches!(
        session.next_event(),
        Some(MpvEvent::Failed(MpvFailure::ProcessExited))
    ));
}
