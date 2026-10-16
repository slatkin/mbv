//! Cast-attached play dispatch (cast-session-control "Playing while
//! attached"): every play route -- submit, library, and queue cursor --
//! dispatches to the attached receiver and never begins local playback.

use super::queue_op::{answered_local_daemon_app, inject_player_rx};
use crate::app::tests::make_items;
use mbv_ctrl::CtrlCmd;
use mbv_ctrl::player::PlayerEvent;
use mbv_queue::{QueueItem, QueueSource};
use std::sync::Arc;
use std::time::Duration;

fn feed_queue_item() -> QueueItem {
    QueueItem::Feed(mbv_queue::FeedEntry {
        guid: "cast-play".into(),
        title: "Episode".into(),
        enclosure_url: Some("https://feed/example.mp3".into()),
        link: None,
        mime_type: Some("audio/mpeg".into()),
        duration_ticks: None,
        pub_date_secs: None,
        feed_kind: None,
        feed_id: Some("feed".into()),
        position_ticks: 0,
        played: false,
    })
}

/// No cast call was recorded. The dispatch thread is spawned only after the
/// owner's answer gates the dispatch, so a rejected load has already returned
/// before any dispatch could record a call.
fn assert_no_cast_call(calls: &Arc<std::sync::Mutex<Vec<String>>>) {
    assert!(
        calls.lock().unwrap().is_empty(),
        "the receiver must not be handed a queue the owner never accepted"
    );
}

#[test]
fn cast_attached_play_loads_the_owner_queue_without_starting_playback() {
    // cast-session-control "Attaching to a cast target does not engage the
    // local player": playing while attached dispatches the selection to the
    // receiver and SHALL NOT begin local playback of it. The owner's
    // `Replace` op always starts playback, so the owner queue is loaded
    // through the idle-load route instead, and the receiver is dispatched the
    // played item (queue-owner-process #857 review follow-up).
    let (mut app, cmd_rx) = answered_local_daemon_app();
    let tx = inject_player_rx(&mut app);
    // The idle load's correlated answer gates the dispatch; preload the
    // owner's acceptance so the pump finds it without waiting.
    tx.send(PlayerEvent::UnifiedQueueLoadResult {
        request_id: 1,
        result: mbv_ctrl::QueueLoadResult::Accepted,
    })
    .unwrap();
    app.attach_cast("device-1".to_string());
    let (job_tx, calls) = mbv_cast::spawn_fake_cast_worker(mbv_cast::FakeCastTransport::default());
    app.set_cast_client("device-1", job_tx);

    assert!(app.submit_queue_item(feed_queue_item(), true));

    match cmd_rx.try_recv().unwrap() {
        CtrlCmd::UnifiedQueueLoadIdle { slots, cursor, .. } => {
            assert_eq!(slots.len(), 1, "the played item is the whole loaded queue");
            assert_eq!(cursor, 0, "the load starts at the played item");
        }
        other => {
            panic!("cast-attached play must not send a playback-starting command, got {other:?}")
        }
    }
    assert!(
        cmd_rx.try_iter().all(|command| !matches!(
            command,
            CtrlCmd::UnifiedQueueReplace { .. } | CtrlCmd::UnifiedQueuePlaySlot { .. }
        )),
        "no playback-starting command reaches the owner"
    );

    // The receiver is dispatched the played selection: the worker reports the
    // dispatch only after the transport has recorded its load.
    let event = app
        .channels
        .cast_rx
        .recv_timeout(Duration::from_secs(2))
        .unwrap();
    assert!(
        matches!(
            event,
            crate::app::state::types::cast::CastEvent::Dispatched { outcome: Ok(_), .. }
        ),
        "the played selection dispatches to the receiver"
    );
    // The pong ahead of the load resets the heartbeat clock: the load's
    // reply waits for the receiver to load the item, and the receiver
    // terminates a sender whose pongs lapse during that wait. The receiver
    // is loaded with the single played item, not the whole queue (a whole
    // queue in one QUEUE_LOAD exceeds the receiver's message bound and
    // kills the connection -- 2026-10-15 logs).
    assert_eq!(
        calls.lock().unwrap().as_slice(),
        ["keep_alive", "load(https://feed/example.mp3, 0)"]
    );
}

#[test]
fn cast_attached_play_dispatches_nothing_when_the_owner_rejects_the_idle_load() {
    // The receiver is handed the owner-accepted queue, so a rejected idle
    // load — the owner's "another idle queue load is pending" gate — must
    // dispatch nothing: the correlated answer gates the dispatch like the
    // answered queue ops (queue-owner-process #857 review follow-up).
    let (mut app, cmd_rx) = answered_local_daemon_app();
    let tx = inject_player_rx(&mut app);
    tx.send(PlayerEvent::UnifiedQueueLoadResult {
        request_id: 1,
        result: mbv_ctrl::QueueLoadResult::Rejected {
            reason: "another idle queue load is pending".into(),
        },
    })
    .unwrap();
    app.attach_cast("device-1".to_string());
    let (job_tx, calls) = mbv_cast::spawn_fake_cast_worker(mbv_cast::FakeCastTransport::default());
    app.set_cast_client("device-1", job_tx);

    assert!(
        !app.submit_queue_item(feed_queue_item(), true),
        "a rejected load fails the submit"
    );

    assert!(
        app.status.contains("Queue load rejected"),
        "the rejection is flashed, got {:?}",
        app.status
    );
    assert!(
        matches!(
            cmd_rx.try_recv().unwrap(),
            CtrlCmd::UnifiedQueueLoadIdle { .. }
        ),
        "the load was sent before the rejection came back"
    );
    assert_no_cast_call(&calls);
}

#[test]
fn cast_attached_library_play_dispatches_instead_of_playing_locally() {
    // cast-session-control "Playing while attached": the library play route
    // (`play_item` -- also carried by home rows and episode activation) must
    // dispatch to the receiver and SHALL NOT begin local playback of it. The
    // route previously fell through to the owner's Replace and played
    // locally.
    let (mut app, cmd_rx) = answered_local_daemon_app();
    let tx = inject_player_rx(&mut app);
    tx.send(PlayerEvent::UnifiedQueueLoadResult {
        request_id: 1,
        result: mbv_ctrl::QueueLoadResult::Accepted,
    })
    .unwrap();
    app.attach_cast("device-1".to_string());

    app.play_item(make_items(1).remove(0));

    match cmd_rx.try_recv().unwrap() {
        CtrlCmd::UnifiedQueueLoadIdle { slots, cursor, .. } => {
            assert_eq!(slots.len(), 1, "the played item is the whole loaded queue");
            assert_eq!(cursor, 0, "the load starts at the played item");
        }
        other => {
            panic!(
                "cast-attached library play must not send a playback-starting command, got {other:?}"
            )
        }
    }
    assert!(
        cmd_rx.try_iter().all(|command| !matches!(
            command,
            CtrlCmd::UnifiedQueueReplace { .. } | CtrlCmd::UnifiedQueuePlaySlot { .. }
        )),
        "no playback-starting command reaches the owner"
    );

    // The stub config has no Emby server, so dispatch resolution surfaces
    // the item uncastable instead of handing anything to the receiver.
    let event = app
        .channels
        .cast_rx
        .recv_timeout(Duration::from_secs(2))
        .unwrap();
    app.handle_cast_event(event);
    assert!(
        app.status.contains("Not cast:"),
        "the uncastable item is flashed by name, got {:?}",
        app.status
    );
}

#[test]
fn cast_attached_library_selection_dispatches_instead_of_playing_locally() {
    // cast-session-control "Playing while attached": the multi-item library
    // selection route (`play_items_routed` -- also carried by album, artist,
    // folder, and shuffle plays) must dispatch the whole selection from the
    // played index and SHALL NOT begin local playback of it.
    let (mut app, cmd_rx) = answered_local_daemon_app();
    let tx = inject_player_rx(&mut app);
    tx.send(PlayerEvent::UnifiedQueueLoadResult {
        request_id: 1,
        result: mbv_ctrl::QueueLoadResult::Accepted,
    })
    .unwrap();
    app.attach_cast("device-1".to_string());

    app.play_items_routed(make_items(2), 1, QueueSource::Album);

    match cmd_rx.try_recv().unwrap() {
        CtrlCmd::UnifiedQueueLoadIdle { slots, cursor, .. } => {
            assert_eq!(slots.len(), 2, "the whole selection is loaded idle");
            assert_eq!(cursor, 1, "the load starts at the played index");
        }
        other => {
            panic!(
                "cast-attached selection play must not send a playback-starting command, got {other:?}"
            )
        }
    }
    assert!(
        cmd_rx.try_iter().all(|command| !matches!(
            command,
            CtrlCmd::UnifiedQueueReplace { .. } | CtrlCmd::UnifiedQueuePlaySlot { .. }
        )),
        "no playback-starting command reaches the owner"
    );
}

#[test]
fn cast_attached_queue_cursor_play_dispatches_without_commanding_the_owner() {
    // cast-session-control "Playing while attached": Enter on a queue row
    // dispatches the queue to the receiver starting at the cursor instead of
    // starting local playback. The queue is already the owner's, so no owner
    // command is sent at all -- the owner's Replace/PlaySlot would start
    // local playback.
    let (mut app, cmd_rx) = answered_local_daemon_app();
    app.attach_cast("device-1".to_string());

    app.dispatch_queue_play_cursor(1);

    assert!(
        cmd_rx.try_recv().is_err(),
        "queue-cursor play while attached must not command the owner"
    );
    // Dispatch resolution reports the stub-unresolvable Emby item uncastable.
    let event = app
        .channels
        .cast_rx
        .recv_timeout(Duration::from_secs(2))
        .unwrap();
    app.handle_cast_event(event);
    assert!(
        app.status.contains("Not cast:"),
        "the uncastable item is flashed by name, got {:?}",
        app.status
    );
}
