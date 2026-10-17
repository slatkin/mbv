//! The Emby Session worker contract (emby-session-owner-output task 3.1): the
//! worker runs submitted jobs in order and returns the poll result as the
//! owner's `DaemonEvent::EmbySession`, so the event loop never waits on HTTP.

use std::sync::mpsc;

use crate::DaemonEvent;

use super::EmbySessionEvent;
use super::fake::{FakeSessionApi, SessionCall, session};
use super::worker::spawn_emby_session_worker;

#[test]
fn worker_runs_jobs_in_order_and_returns_the_poll_as_an_owner_event() {
    let fake = FakeSessionApi::with_sessions(vec![session("device-1", "session-1")]);
    let calls = fake.calls();
    let worker = spawn_emby_session_worker(move || fake);

    worker
        .send(Box::new(|api| {
            let _ = api.play("session-1", "item-1", 30);
        }))
        .expect("the worker accepts a play job");
    let (events_tx, events_rx) = mpsc::channel();
    worker
        .send(Box::new(move |api| {
            let result = api.get_sessions();
            let _ = events_tx.send(DaemonEvent::EmbySession(EmbySessionEvent::Sessions {
                poll_id: 7,
                result,
            }));
        }))
        .expect("the worker accepts a poll job");

    let DaemonEvent::EmbySession(EmbySessionEvent::Sessions { poll_id, result }) = events_rx
        .recv()
        .expect("the worker returns the poll result")
    else {
        panic!("the poll result is the returned event");
    };
    assert_eq!(poll_id, 7);
    let sessions = result.expect("the scripted Sessions are returned");
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].device_id, "device-1");
    assert_eq!(
        calls.lock().unwrap().as_slice(),
        &[
            SessionCall::Play {
                session_id: "session-1".to_string(),
                item_id: "item-1".to_string(),
                start_ticks: 30,
            },
            SessionCall::GetSessions,
        ]
    );
}
