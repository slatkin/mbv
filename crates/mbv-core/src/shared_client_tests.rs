use super::{event_request_id, run_client_worker, with_request_id};
use crate::shared_protocol::{SharedDataCmd, SharedDataEvent};
use crate::shared_state::{SharedDocumentKind, SharedRecord};
use std::io::{BufReader, Read, Write};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

#[derive(Clone, Default)]
struct TestStream {
    written: Arc<Mutex<Vec<u8>>>,
    response: Option<Arc<Mutex<Vec<u8>>>>,
}

impl Read for TestStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let Some(response) = &self.response else {
            return Err(std::io::ErrorKind::WouldBlock.into());
        };
        let mut response = response.lock().unwrap();
        if response.is_empty() {
            return Err(std::io::ErrorKind::WouldBlock.into());
        }
        let count = buf.len().min(response.len());
        buf[..count].copy_from_slice(&response[..count]);
        response.drain(..count);
        Ok(count)
    }
}

impl Write for TestStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.written.lock().unwrap().extend_from_slice(buf);
        if buf
            .windows(b"\"type\":\"ping\"".len())
            .any(|window| window == b"\"type\":\"ping\"")
        {
            if let Some(response) = &self.response {
                response
                    .lock()
                    .unwrap()
                    .extend_from_slice(b"{\"type\":\"pong\"}\n");
            }
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn request_ids_round_trip_through_commands_and_responses() {
    let command = with_request_id(
        SharedDataCmd::UpdateDocument {
            request_id: 0,
            kind: SharedDocumentKind::QueueState,
            expected_revision: 3,
            value: serde_json::json!({"queue": true}),
        },
        42,
    );
    let response = match command {
        SharedDataCmd::UpdateDocument { request_id, .. } => SharedDataEvent::DocumentUpdated {
            request_id,
            kind: SharedDocumentKind::QueueState,
            record: SharedRecord {
                revision: 4,
                value: serde_json::json!({"queue": true}),
            },
        },
        _ => unreachable!(),
    };
    assert_eq!(event_request_id(&response), Some(42));
}

#[test]
fn notifications_have_no_request_id() {
    let notification = SharedDataEvent::DocumentNotification {
        kind: SharedDocumentKind::QueueState,
        record: SharedRecord::default(),
    };
    assert_eq!(event_request_id(&notification), None);
}

#[test]
fn idle_peer_timeout_emits_connection_closed() {
    let stream = TestStream::default();
    let written = stream.written.clone();
    let (cmd_tx, cmd_rx) = mpsc::channel();
    let (event_tx, event_rx) = mpsc::channel();
    std::thread::spawn(move || {
        run_client_worker(
            BufReader::new(stream),
            cmd_rx,
            event_tx,
            true,
            Duration::from_millis(10),
            Duration::from_millis(30),
        );
    });

    assert!(matches!(
        event_rx.recv_timeout(Duration::from_millis(200)),
        Ok(SharedDataEvent::ConnectionClosed)
    ));
    assert!(written
        .lock()
        .unwrap()
        .windows(b"\"type\":\"ping\"".len())
        .any(|window| { window == b"\"type\":\"ping\"" }));
    drop(cmd_tx);
}

#[test]
fn healthy_idle_peer_answers_heartbeat() {
    let stream = TestStream {
        response: Some(Arc::new(Mutex::new(Vec::new()))),
        ..TestStream::default()
    };
    let (cmd_tx, cmd_rx) = mpsc::channel();
    let (event_tx, event_rx) = mpsc::channel();
    std::thread::spawn(move || {
        run_client_worker(
            BufReader::new(stream),
            cmd_rx,
            event_tx,
            true,
            Duration::from_millis(10),
            Duration::from_millis(30),
        );
    });

    std::thread::sleep(Duration::from_millis(100));
    assert!(event_rx
        .try_iter()
        .all(|event| { !matches!(event, SharedDataEvent::ConnectionClosed) }));
    drop(cmd_tx);
}
