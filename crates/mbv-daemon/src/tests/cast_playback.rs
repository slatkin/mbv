// cast-owner-output rows 5.1 and 5.2: the submission funnels and the slot
// jump dispatch a Cast load through the owner's pending resolve, and the
// transition gate settles on a confirming receiver status and expires on a
// failed resolve.

use super::*;

use mbv_cast::CastStatus;

use super::cast_output::{cast_selection, connect_output_client, pump_cast_event, select_output};
use super::cast_resolve::wait_for_worker_jobs;
use super::r#loop::{TestLoop, test_loop_with_queue, test_loop_with_role};
use crate::cast::PlaybackOutput;

mod transport;

/// A feed queue item with an enclosure URL: the resolve needs no service
/// client, so the funnel tests stay hermetic.
pub(super) fn feed_qi_with_url(guid: &str, url: &str) -> QueueItem {
    QueueItem::Feed(FeedEntry {
        guid: guid.into(),
        title: guid.into(),
        enclosure_url: Some(url.into()),
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

/// A Cast connector whose connect always succeeds, backed by a fake transport
/// that answers every status poll with `status`; returns the connector and the
/// transport's call log.
pub(super) fn status_connector(status: CastStatus) -> (CastConnector, Arc<Mutex<Vec<String>>>) {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let log = Arc::clone(&calls);
    let connector: CastConnector = Box::new(move |request: CastConnectRequest| {
        let transport = mbv_cast::FakeCastTransport {
            calls: Arc::clone(&log),
            status: Some(status.clone()),
            refuse_commands: false,
        };
        let (worker, _calls) = mbv_cast::spawn_fake_cast_worker(transport);
        let _ = request.reply.send(CastConnectOutcome::Connected(worker));
    });
    (connector, calls)
}

/// Selects the test receiver as the output and pumps the connect outcome.
/// Returns the client id, its event channel, and the transport's call log.
pub(super) fn select_cast_output(
    t: &mut TestLoop,
    status: CastStatus,
) -> (u64, mpsc::Receiver<CtrlOutbound>, Arc<Mutex<Vec<String>>>) {
    let (connector, calls) = status_connector(status);
    t.event_loop.cast_connector = Some(connector);
    let (client_id, reply_tx, rx) = connect_output_client(&t.event_loop.ctrl_clients, true);
    select_output(t, client_id, &reply_tx, cast_selection());
    pump_cast_event(t);
    (client_id, rx, calls)
}

/// Pumps one `DaemonEvent` the loop produced and applies it.
pub(super) fn pump(t: &mut TestLoop) {
    let event = t.merged_rx.recv().expect("the loop produced an event");
    t.event_loop.handle_event(event);
}

/// The loaded item's content id is the resolved media's URL.
fn loaded_content_id(t: &TestLoop) -> String {
    let PlaybackOutput::Cast(cast) = &t.event_loop.owner.output else {
        panic!("the receiver is the output");
    };
    cast.loaded
        .as_ref()
        .expect("the load records the item")
        .content_id
        .clone()
}

/// A receiver status that confirms `content_id` is playing.
fn playing_status(content_id: &str) -> CastStatus {
    CastStatus {
        position_seconds: Some(1.0),
        duration_seconds: None,
        playback_rate: 1.0,
        state: mbv_cast::CastPlaybackState::Playing,
        playing_content_id: Some(content_id.to_string()),
        idle_reason: None,
    }
}

/// Contract: cast-owner-output row 5.2 — a receiver status naming the loaded
/// content id in Playing settles the in-flight transition, so the next press
/// steps from the settled slot: the final load lands two slots ahead of the
/// start, one load per step, and no second press re-loads the first target.
#[test]
fn a_confirming_status_settles_the_cast_step_and_the_next_load_lands_two_ahead() {
    let mut t = test_loop_with_queue(
        crate::DaemonRole::Local,
        vec![
            feed_qi_with_url("a", "https://feed/a.mp3"),
            feed_qi_with_url("b", "https://feed/b.mp3"),
            feed_qi_with_url("c", "https://feed/c.mp3"),
        ],
        0,
    );
    let slots: Vec<_> = t
        .event_loop
        .owner
        .core
        .queue
        .slots()
        .iter()
        .map(|slot| slot.slot_id)
        .collect();
    // The transport reports the receiver playing the first target's URL: the
    // confirmation the settle needs.
    let (_client_id, _rx, calls) = select_cast_output(&mut t, playing_status("https://feed/b.mp3"));

    // First Next: the transition goes in flight, the resolve loads slot b.
    t.event_loop
        .handle_event(DaemonEvent::Transport(mbv_ctrl::TransportCommand::Step(
            mbv_ctrl::Direction::Next,
        )));
    pump(&mut t);
    wait_for_worker_jobs(&t.event_loop.owner.output);
    assert_eq!(
        loaded_content_id(&t),
        "https://feed/b.mp3",
        "the first Next loads the successor"
    );
    assert!(t.event_loop.owner.core.transitions.in_flight().is_some());

    // The poll reports the receiver playing the loaded content: the
    // transition settles.
    let start = std::time::Instant::now();
    t.event_loop.tick(start);
    pump(&mut t);
    assert!(
        t.event_loop.owner.core.transitions.in_flight().is_none(),
        "the confirming status settles the in-flight transition"
    );
    assert_eq!(
        t.event_loop.owner.core.observed_active_slot(),
        Some(slots[1]),
        "the settled slot becomes the observed active slot"
    );

    // Second Next: it steps from the settled slot b to c — the final load is
    // two slots ahead of the start, not a re-load of b.
    t.event_loop
        .handle_event(DaemonEvent::Transport(mbv_ctrl::TransportCommand::Step(
            mbv_ctrl::Direction::Next,
        )));
    pump(&mut t);
    wait_for_worker_jobs(&t.event_loop.owner.output);

    assert_eq!(
        *calls.lock().unwrap(),
        vec![
            "keep_alive".to_string(),
            "load(https://feed/b.mp3, 0)".to_string(),
            "keep_alive".to_string(),
            "keep_alive".to_string(),
            "load(https://feed/c.mp3, 0)".to_string()
        ],
        "each step loads its own slot once; the final load is two slots ahead; every load and poll pongs first"
    );
}

/// Contract: cast-owner-output row 5.1, the #944 regression — playing a
/// several-hundred-slot queue on a receiver loads only the selected slot's
/// item, once: the whole queue is never sent to the receiver and local
/// playback does not start.
#[test]
fn playing_a_300_slot_queue_loads_only_the_selected_slot_on_the_receiver() {
    let http = MockHttp::new();
    // One scripted response for the selected slot's cast PlaybackInfo; the
    // resolve asks only for the selected item.
    http.respond(
        200,
        r#"{"MediaSources":[{"Id":"ms-1","SupportsDirectPlay":true}],"PlaySessionId":"ps-1"}"#,
    );
    let mut client = EmbyClient::new(Config {
        // An IP literal: the mock transport ignores the host, but the default
        // resolver must never attempt DNS for it (see tests/emby/client.rs).
        server_url: "http://127.0.0.1:1".into(),
        ..Config::default()
    })
    .with_test_agent(http.agent());
    client.token = "test-token".to_string();
    client.user_id = "test-user".to_string();

    let mut t = test_loop_with_role(crate::DaemonRole::Local);
    t.event_loop.client = Arc::new(Mutex::new(client));
    let (client_id, _rx, calls) = select_cast_output(&mut t, mbv_cast::CastStatus::idle());

    let fetched: Vec<EmbyItem> = (0..300)
        .map(|index| item(&format!("item-{index}"), "Video", "Movie"))
        .collect();
    let (request_id, generation) = (7, 7);
    t.event_loop.owner.intents.accept(
        client_id,
        PlaybackIntent {
            request_id,
            generation,
            action: PlaybackIntentAction::Play {
                item_ids: vec!["item-5".into()],
                start_idx: 5,
                start_ticks: 0,
                source: QueueSource::Remote,
            },
        },
        false,
    );

    t.event_loop.handle_event(DaemonEvent::PlaybackResolved {
        start_idx: 5,
        start_ticks: 0,
        source: QueueSource::Remote,
        client_id,
        request_id,
        generation,
        fetched: Ok(fetched),
    });
    pump(&mut t);
    wait_for_worker_jobs(&t.event_loop.owner.output);

    let PlaybackOutput::Cast(cast) = &t.event_loop.owner.output else {
        panic!("the receiver is the output");
    };
    assert!(cast.loaded.is_some(), "the selected slot's item is loaded");
    assert!(
        !t.event_loop.player.status.lock().unwrap().active,
        "local playback does not start while the receiver is the output"
    );
    assert_eq!(
        http.request_count(),
        1,
        "the resolve asked the service only for the selected item"
    );
    assert_eq!(
        *calls.lock().unwrap(),
        vec!["keep_alive".to_string(), "load(http://127.0.0.1:1/Videos/item-5/stream?static=true&api_key=test-token&MediaSourceId=ms-1, 0)".to_string()],
        "the receiver is given exactly one load: the selected slot's item"
    );
}

/// Contract: cast-owner-output row 5.1 — playing an Audiobookshelf book while
/// a receiver is the output names the item and the reason, loads nothing, and
/// expires the in-flight transition the play registered (design D5).
#[test]
fn playing_an_audiobookshelf_book_on_cast_loads_nothing_and_reports() {
    let mut t = test_loop_with_queue(
        crate::DaemonRole::Local,
        vec![book_qi("li_1"), emby_qi("movie", "Video", "Movie")],
        0,
    );
    let (client_id, rx, calls) = select_cast_output(&mut t, mbv_cast::CastStatus::idle());
    let slot = t.event_loop.owner.core.queue.slots()[0].slot_id;

    t.event_loop.handle_event(DaemonEvent::Ctrl(
        CtrlCmd::UnifiedQueuePlaySlot {
            slot_id: mbv_ctrl::slot_id_to_u64(slot),
            op: None,
        },
        client_id,
        mpsc::channel().0,
    ));
    pump(&mut t);
    wait_for_worker_jobs(&t.event_loop.owner.output);

    assert!(
        !calls
            .lock()
            .unwrap()
            .iter()
            .any(|call| call.starts_with("load(")),
        "an uncastable book loads nothing"
    );
    // Selecting the output broadcasts a PlaybackOutput event, and the play
    // attempt broadcasts queue state, before the rejection arrives.
    assert!(
        matches!(recv_event(&rx), CtrlEvent::PlaybackOutput(_)),
        "the output selection is broadcast first"
    );
    assert!(
        matches!(recv_event(&rx), CtrlEvent::UnifiedQueueState(_)),
        "the play attempt broadcasts queue state next"
    );
    let CtrlEvent::CommandRejected(reason) = recv_event(&rx) else {
        panic!("the uncastable play is reported");
    };
    assert!(
        reason.contains("Test Book") && reason.contains("can't be cast"),
        "the report names the item and the reason: {reason}"
    );
    assert!(
        t.event_loop.owner.core.transitions.in_flight().is_none(),
        "the failed resolve expires the in-flight transition"
    );
    assert!(t.event_loop.owner.pending_cast_resolve.is_none());
}

/// A feed queue item with no media URL at all: its Cast resolve fails as
/// uncastable, with no service client involved.
pub(super) fn feed_qi_without_url(guid: &str) -> QueueItem {
    QueueItem::Feed(FeedEntry {
        enclosure_url: None,
        ..feed_qi_with_url(guid, "unused").as_feed().unwrap().clone()
    })
}

/// Drains a client's event channel without blocking: broadcasts land in the
/// channel synchronously during `handle_event`, so nothing here waits.
pub(super) fn drained_events(rx: &mpsc::Receiver<CtrlOutbound>) -> Vec<CtrlEvent> {
    let mut events = Vec::new();
    while let Ok(outbound) = rx.try_recv() {
        if let CtrlOutbound::Event(json) = outbound {
            events.push(serde_json::from_str(&json).unwrap());
        }
    }
    events
}

/// Contract: row 5.1 review fix — a Cast jump whose in-flight transition
/// expires promotes the queued jump with its origin intact, so an uncastable
/// promoted target still reports the item to the client that issued the
/// queued jump (`expire_and_redispatch` must not clear the tracked origin
/// before `take_promoted_origin` reads it).
#[test]
fn an_expired_jump_promotes_the_queued_jump_with_its_origin_so_the_failure_is_reported() {
    let mut t = test_loop_with_queue(
        crate::DaemonRole::Local,
        vec![
            feed_qi_with_url("a", "https://feed/a.mp3"),
            feed_qi_with_url("b", "https://feed/b.mp3"),
            feed_qi_without_url("c"),
        ],
        0,
    );
    let (client_id, rx, _calls) = select_cast_output(&mut t, mbv_cast::CastStatus::idle());

    // First Next: in flight, loads slot b; the idle receiver never settles it.
    t.event_loop.handle_event(DaemonEvent::Ctrl(
        CtrlCmd::PlaybackIntent(PlaybackIntent {
            request_id: 10,
            generation: 10,
            action: PlaybackIntentAction::Next,
        }),
        client_id,
        mpsc::channel().0,
    ));
    pump(&mut t);
    let target = t.event_loop.owner.core.queue.slots()[2].slot_id;
    // A play-slot jump: queued behind the in-flight step, targeting
    // uncastable c, with the issuing client as its origin.
    t.event_loop.handle_event(DaemonEvent::Ctrl(
        CtrlCmd::UnifiedQueuePlaySlot {
            slot_id: mbv_ctrl::slot_id_to_u64(target),
            op: None,
        },
        client_id,
        mpsc::channel().0,
    ));
    assert!(
        t.event_loop
            .owner
            .core
            .transitions
            .queued_latest()
            .is_some(),
        "the play-slot jump queues behind the in-flight step"
    );

    // The bounded window lapses: `expire` takes the clock as a parameter, so
    // a first call with a backdated instant arms a deadline already in the
    // past, and the next real-clock expiry pass abandons the in-flight step
    // and promotes the queued jump.
    let backdated = std::time::Instant::now()
        .checked_sub(std::time::Duration::from_secs(10))
        .expect("the monotonic clock reaches back ten seconds");
    t.event_loop.owner.core.transitions.expire(backdated);
    t.event_loop.on_recv_timeout();
    assert!(
        t.event_loop.owner.core.transitions.in_flight().is_some(),
        "the queued jump is promoted into flight"
    );
    pump(&mut t);

    let rejected = drained_events(&rx).into_iter().any(|event| {
        matches!(event, CtrlEvent::CommandRejected(report) if report.contains("\"c\" has no media URL"))
    });
    assert!(
        rejected,
        "the promoted uncastable jump is reported to its origin client"
    );
    assert!(t.event_loop.owner.core.transitions.in_flight().is_none());
}

/// Contract: row 5.2 review fix — a receiver that refuses the load rejects
/// the current play intent for the expired transition, so the next identical
/// play is accepted and re-executed instead of coalescing into the dead
/// intent and doing nothing.
#[test]
fn a_failed_receiver_load_rejects_the_current_play_intent_so_a_retry_is_accepted() {
    let http = MockHttp::new();
    http.respond(
        200,
        r#"{"MediaSources":[{"Id":"ms-1","SupportsDirectPlay":true}],"PlaySessionId":"ps-1"}"#,
    );
    let mut client = EmbyClient::new(Config {
        // An IP literal: the mock transport ignores the host, but the default
        // resolver must never attempt DNS for it (see tests/emby/client.rs).
        server_url: "http://127.0.0.1:1".into(),
        ..Config::default()
    })
    .with_test_agent(http.agent());
    client.token = "test-token".to_string();
    client.user_id = "test-user".to_string();
    let mut t = test_loop_with_role(crate::DaemonRole::Local);
    t.event_loop.client = Arc::new(Mutex::new(client));
    let (client_id, rx, _calls) = select_cast_output(&mut t, mbv_cast::CastStatus::idle());

    let intent = PlaybackIntent {
        request_id: 7,
        generation: 7,
        action: PlaybackIntentAction::Play {
            item_ids: vec!["movie".into()],
            start_idx: 0,
            start_ticks: 0,
            source: QueueSource::Remote,
        },
    };
    t.event_loop
        .owner
        .intents
        .accept(client_id, intent.clone(), false);
    t.event_loop.handle_event(DaemonEvent::PlaybackResolved {
        start_idx: 0,
        start_ticks: 0,
        source: QueueSource::Remote,
        client_id,
        request_id: 7,
        generation: 7,
        fetched: Ok(vec![item("movie", "Video", "Movie")]),
    });
    pump(&mut t);
    wait_for_worker_jobs(&t.event_loop.owner.output);
    let slot = {
        let PlaybackOutput::Cast(cast) = &t.event_loop.owner.output else {
            panic!("the receiver is the output");
        };
        cast.loaded.as_ref().expect("the play loaded its slot").slot
    };
    assert!(t.event_loop.owner.core.transitions.in_flight().is_some());

    // The receiver refuses the load.
    t.event_loop
        .handle_event(DaemonEvent::Cast(CastOwnerEvent::LoadFailed {
            slot,
            error: "receiver refused".into(),
        }));

    assert!(
        t.event_loop.owner.intents.current.is_none(),
        "the failed load abandons the guarded play intent"
    );
    let rejected = drained_events(&rx).into_iter().any(|event| {
        matches!(
            event,
            CtrlEvent::PlaybackIntent(ev)
                if ev.request_id == 7
                    && matches!(
                        ev.outcome,
                        PlaybackIntentOutcome::Rejected {
                            reason: mbv_ctrl::PlaybackIntentRejection::Unavailable,
                        }
                    )
        )
    });
    assert!(rejected, "the failed load rejects the play intent");

    let outcomes = t.event_loop.owner.intents.accept(client_id, intent, false);
    assert!(
        matches!(outcomes[0].outcome, PlaybackIntentOutcome::Accepted),
        "the identical retry is accepted, not coalesced into the dead intent"
    );
}

/// Contract: row 5.2 review fix — a Cast resolve that lands after its
/// transition expired in the bounded window loads nothing: the owner no
/// longer guards that transition, so a load would never settle and would
/// leave the receiver silently on the old slot.
#[test]
fn a_resolve_that_lands_after_its_transition_expired_loads_nothing() {
    let mut t = test_loop_with_queue(
        crate::DaemonRole::Local,
        vec![feed_qi_with_url("a", "https://feed/a.mp3")],
        0,
    );
    let (client_id, _rx, calls) = select_cast_output(&mut t, mbv_cast::CastStatus::idle());
    let slot = t.event_loop.owner.core.queue.slots()[0].slot_id;

    t.event_loop.handle_event(DaemonEvent::Ctrl(
        CtrlCmd::UnifiedQueuePlaySlot {
            slot_id: mbv_ctrl::slot_id_to_u64(slot),
            op: None,
        },
        client_id,
        mpsc::channel().0,
    ));
    assert!(
        t.event_loop.owner.pending_cast_resolve.is_some(),
        "the jump resolves its target's cast source"
    );
    assert!(t.event_loop.owner.core.transitions.in_flight().is_some());

    // The resolve's result is still queued, unpumped; the transition's
    // bounded window lapses first: `expire` takes the clock as a parameter,
    // so a first call with a backdated instant arms a deadline already in
    // the past, and the next real-clock expiry pass abandons the transition.
    let backdated = std::time::Instant::now()
        .checked_sub(std::time::Duration::from_secs(10))
        .expect("the monotonic clock reaches back ten seconds");
    t.event_loop.owner.core.transitions.expire(backdated);
    t.event_loop.on_recv_timeout();
    assert!(t.event_loop.owner.core.transitions.in_flight().is_none());

    pump(&mut t);
    let PlaybackOutput::Cast(cast) = &t.event_loop.owner.output else {
        panic!("the receiver is the output");
    };
    assert!(
        cast.loaded.is_none(),
        "the late resolve loads nothing once its transition expired"
    );
    assert!(
        !calls
            .lock()
            .unwrap()
            .iter()
            .any(|call| call.starts_with("load(")),
        "the receiver is given no load for the expired transition"
    );
}

/// A receiver status that confirms `content_id` is playing at `position_seconds`.
pub(super) fn playing_status_at(content_id: &str, position_seconds: f32) -> CastStatus {
    CastStatus {
        position_seconds: Some(position_seconds),
        ..playing_status(content_id)
    }
}
