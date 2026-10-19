//! The Emby Session worker contract (emby-session-owner-output task 3.1): the
//! worker runs submitted jobs in order and returns the poll result as the
//! owner's `DaemonEvent::EmbySession`, so the event loop never waits on HTTP.

use std::sync::mpsc;

use mbv_player::PlayerOwnerState;
use mbv_queue::{PlaybackQueue, QueueItem, QueueRevisionMint, QueueSlotId};
use rstest::rstest;

use crate::DaemonEvent;
use crate::tests::{emby_qi, item, video_feed_qi};

use super::EmbySessionEvent;
use super::fake::{FakeSessionApi, SessionCall, session};
use super::load::{admit, next_admissible_slot};
use super::worker::spawn_emby_session_worker;

#[test]
fn worker_runs_jobs_in_order_and_returns_the_poll_as_an_owner_event() {
    let fake = FakeSessionApi::with_sessions(vec![session("device-1", "session-1")]);
    let calls = fake.calls();
    let worker = spawn_emby_session_worker(move || fake);

    worker
        .send(Box::new(|api| {
            let _ = api.play("session-1", &["item-1"], 30);
        }))
        .expect("the worker accepts a play job");
    worker
        .send(Box::new(|api| {
            let _ = api.play_last("session-1", &["item-2"]);
        }))
        .expect("the worker accepts a play_last job");
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
                item_ids: vec!["item-1".to_string()],
                start_ticks: 30,
            },
            SessionCall::PlayLast {
                session_id: "session-1".to_string(),
                item_ids: vec!["item-2".to_string()],
            },
            SessionCall::GetSessions,
        ]
    );
}

/// Contract: row 4.1 / design D7 and spec "An Emby Session plays only Emby
/// items it can play" — an Emby item passes when the Session advertises its
/// media kind or advertises no kinds, and every other item fails with the
/// reason the Owner reports.
#[rstest]
#[case::video_on_a_session_with_no_advertised_kinds(&[], emby_qi("movie", "Video", "Movie"), None)]
#[case::video_on_an_audio_only_session(&["Audio"], emby_qi("movie", "Video", "Movie"), Some(("movie", "the Session does not play Video items")))]
#[case::feed_entry_on_any_session(&[], video_feed_qi("episode-1"), Some(("episode-1", "the Emby Session output plays only Emby items")))]
fn admission_admits_only_emby_items_the_session_plays(
    #[case] advertised: &[&str],
    #[case] item: QueueItem,
    #[case] expected_rejection: Option<(&str, &str)>,
) {
    let advertised: Vec<String> = advertised.iter().map(|kind| (*kind).to_string()).collect();

    let rejection = admit(&item, &advertised).err();

    assert_eq!(
        rejection.map(|rejected| (rejected.item, rejected.reason)),
        expected_rejection.map(|(item, reason)| (item.to_string(), reason.to_string()))
    );
}

/// Contract: row 9.2 / design D1, D14 and spec "Two items on the device" —
/// the device is given the queue neighbor and a loaded slot is never a slot
/// after itself: the Owner queues nothing rather than the same item twice.
#[test]
fn the_next_slot_is_the_queue_neighbor_and_never_the_loaded_one() {
    let queue = PlaybackQueue::from_items(
        vec![item("a", "Video", "Movie"), item("b", "Video", "Movie")],
        Some(0),
        std::sync::Arc::new(QueueRevisionMint::default()),
    );
    let slots: Vec<QueueSlotId> = queue.slots().iter().map(|slot| slot.slot_id).collect();
    let owner = PlayerOwnerState::new(queue, mbv_queue::QueueSource::default());
    let loaded = slots[0];

    let (next, stepped) = next_admissible_slot(&owner, loaded, &[]);

    assert_eq!(
        next.map(|(slot, _)| slot),
        Some(slots[1]),
        "the device is given the queue neighbor"
    );
    assert!(
        stepped.is_empty(),
        "an admissible neighbor steps past nothing"
    );

    let (next, stepped) = next_admissible_slot(&owner, slots[1], &[]);

    assert_eq!(
        next, None,
        "the queue ends after the last slot: no slot follows the loaded one"
    );
    assert!(stepped.is_empty(), "nothing is reported as stepped past");
}
