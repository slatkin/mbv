// Transport and control commands routed to a Cast output: routed wire
// commands, track rejections, stop, teardown, tray transport, guarded
// SetPaused, and queue replacement (cast-owner-output rows 5.3 and 7.1 B1).

use super::*;

/// A receiver status paused at 10s.
fn paused_status() -> CastStatus {
    CastStatus {
        state: mbv_cast::CastPlaybackState::Paused,
        ..playing_status_at("https://feed/a.mp3", 10.0)
    }
}

/// Contract: cast-owner-output row 5.3 (design D4) — a routed transport
/// command on a Cast output lands on the receiver as the matching cast
/// operation and nothing else: `TogglePause` picks its direction from the
/// receiver's reported state, a relative Seek starts from the receiver's
/// reported position, the 0-100 wire volume scales to the receiver's 0-1
/// level, and the local run is never told.
#[rstest]
#[case::toggle_pause_pauses_a_playing_receiver(
    mbv_ctrl::WireCommand::TogglePause,
    playing_status_at("https://feed/a.mp3", 10.0),
    "pause"
)]
#[case::toggle_pause_plays_a_paused_receiver(
    mbv_ctrl::WireCommand::TogglePause,
    paused_status(),
    "play"
)]
#[case::relative_seek_seeks_from_the_reported_position(
    mbv_ctrl::WireCommand::Seek(5.0),
    playing_status_at("https://feed/a.mp3", 10.0),
    "seek(15)"
)]
#[case::absolute_seek_lands_as_a_receiver_seek(
    mbv_ctrl::WireCommand::SeekAbsolute(42.0),
    playing_status_at("https://feed/a.mp3", 10.0),
    "seek(42)"
)]
#[case::wire_volume_scales_to_the_receiver_level(
    mbv_ctrl::WireCommand::SetVolume(30),
    playing_status_at("https://feed/a.mp3", 10.0),
    "set_volume(0.3)"
)]
#[case::mute_lands_as_a_receiver_mute(
    mbv_ctrl::WireCommand::SetMute(true),
    playing_status_at("https://feed/a.mp3", 10.0),
    "set_muted(true)"
)]
fn a_routed_transport_command_lands_on_the_receiver(
    #[case] command: mbv_ctrl::WireCommand,
    #[case] receiver_status: CastStatus,
    #[case] expected_call: &str,
) {
    let mut t = test_loop_with_role(crate::DaemonRole::Local);
    let (client_id, _rx, calls) = select_cast_output(&mut t, receiver_status);
    // One tick runs the status poll, so the output holds the receiver's
    // reported state and position before the command routes.
    let now = std::time::Instant::now();
    t.event_loop.tick(now);
    pump(&mut t);
    let commands = t.event_loop.player.spy_on_commands();

    t.event_loop.handle_event(DaemonEvent::Ctrl(
        CtrlCmd::PlayerCmd(command),
        client_id,
        mpsc::channel().0,
    ));
    wait_for_worker_jobs(&t.event_loop.owner.output);

    // The poll job pongs before its status exchange, so the routed command's
    // call is the last one the transport recorded.
    let log = calls.lock().unwrap().clone();
    assert_eq!(
        log.last().map(String::as_str),
        Some(expected_call),
        "the routed command lands on the receiver: {log:?}"
    );
    assert!(
        commands.try_recv().is_err(),
        "the local run is never started or touched"
    );
}

/// A Cast connector whose receiver refuses every transport command; its status
/// is idle.
fn refusing_connector() -> CastConnector {
    let calls = Arc::new(Mutex::new(Vec::new()));
    Box::new(move |request: CastConnectRequest| {
        let transport = mbv_cast::FakeCastTransport {
            calls: Arc::clone(&calls),
            status: Some(mbv_cast::CastStatus::idle()),
            refuse_commands: true,
        };
        let (worker, _calls) = mbv_cast::spawn_fake_cast_worker(transport);
        let _ = request.reply.send(CastConnectOutcome::Connected(worker));
    })
}

/// Contract: cast-owner-output spec "Command is not supported by the receiver"
/// (row 5.3) — a transport command the receiver refuses reaches the Client as
/// `CommandRejected` carrying the failure, instead of being logged only.
#[test]
fn a_command_the_receiver_refuses_is_reported_to_the_client() {
    let mut t = test_loop_with_role(crate::DaemonRole::Local);
    t.event_loop.cast_connector = Some(refusing_connector());
    let (client_id, reply_tx, rx) = connect_output_client(&t.event_loop.ctrl_clients, true);
    select_output(&mut t, client_id, &reply_tx, cast_selection());
    pump_cast_event(&mut t);

    t.event_loop.handle_event(DaemonEvent::Ctrl(
        CtrlCmd::PlayerCmd(mbv_ctrl::WireCommand::TogglePause),
        client_id,
        mpsc::channel().0,
    ));
    wait_for_worker_jobs(&t.event_loop.owner.output);
    pump(&mut t);

    assert!(
        drained_events(&rx).into_iter().any(|event| matches!(
            event,
            CtrlEvent::CommandRejected(report) if report.contains("receiver refused")
        )),
        "the refused pause is reported to the client"
    );
}

/// Contract: cast-owner-output row 5.3 (design D4) — a track command has no
/// cast primitive: it is rejected with `CommandRejected` and lands nothing
/// on the receiver.
#[rstest]
#[case::set_audio_is_rejected(mbv_ctrl::WireCommand::SetAudio(1))]
#[case::set_sub_is_rejected(mbv_ctrl::WireCommand::SetSub(0))]
#[case::subtitle_prefs_are_rejected(mbv_ctrl::WireCommand::SetSubtitlePrefs {
    mode: "off".into(),
    subtitle_lang: String::new(),
    audio_lang: String::new(),
})]
fn a_track_command_on_cast_is_rejected_and_lands_nothing(#[case] command: mbv_ctrl::WireCommand) {
    let mut t = test_loop_with_role(crate::DaemonRole::Local);
    let (client_id, _rx, calls) = select_cast_output(&mut t, mbv_cast::CastStatus::idle());
    let (reply_tx, reply_rx) = mpsc::channel();

    t.event_loop.handle_event(DaemonEvent::Ctrl(
        CtrlCmd::PlayerCmd(command),
        client_id,
        reply_tx,
    ));
    wait_for_worker_jobs(&t.event_loop.owner.output);

    assert!(matches!(
        recv_event(&reply_rx),
        CtrlEvent::CommandRejected(_)
    ));
    assert!(
        calls.lock().unwrap().is_empty(),
        "the rejected track command lands nothing on the receiver"
    );
}

/// Contract: cast-owner-output row 5.3 — a stop on a Cast output stops the
/// receiver media and forgets the loaded item, so a later idle status can
/// neither settle nor advance anything, and the awaited load's transition is
/// reset like the local stop's.
#[test]
fn a_stop_stops_the_receiver_media_and_forgets_the_loaded_item() {
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
    pump(&mut t);
    wait_for_worker_jobs(&t.event_loop.owner.output);
    assert!(loaded_item_slot(&t).is_some(), "the play loaded its slot");

    t.event_loop.handle_event(DaemonEvent::Ctrl(
        CtrlCmd::Stop,
        client_id,
        mpsc::channel().0,
    ));
    wait_for_worker_jobs(&t.event_loop.owner.output);

    assert_eq!(
        *calls.lock().unwrap(),
        vec![
            // The load job pongs before its load exchange (row 5.1).
            "keep_alive".to_string(),
            "load(https://feed/a.mp3, 0)".to_string(),
            "stop".to_string()
        ],
        "the stop lands on the receiver after the load"
    );
    assert!(
        loaded_item_slot(&t).is_none(),
        "the loaded item is forgotten"
    );
    assert!(
        t.event_loop.owner.core.transitions.in_flight().is_none(),
        "the stop resets the awaited load's transition"
    );
}

/// Contract: cast-owner-output row 7.1 B1 / design D12 — the receiver-app
/// teardown is reachable from the Owner's Cast worker as a queued job, and
/// it runs FIFO after a media-stop job, so a deliberate output change can
/// stop the media before it tears down the app. A user Stop itself must not
/// tear down: only the queued teardown job the caller submits does.
#[test]
fn a_queued_teardown_runs_after_the_media_stop_on_the_worker() {
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
    pump(&mut t);
    wait_for_worker_jobs(&t.event_loop.owner.output);

    t.event_loop.handle_event(DaemonEvent::Ctrl(
        CtrlCmd::Stop,
        client_id,
        mpsc::channel().0,
    ));
    let PlaybackOutput::Cast(cast) = &t.event_loop.owner.output else {
        panic!("the receiver is the output");
    };
    crate::cast::queue_teardown_job(cast);
    wait_for_worker_jobs(&t.event_loop.owner.output);

    let calls = calls.lock().unwrap();
    assert_eq!(
        calls.iter().filter(|call| *call == "teardown").count(),
        1,
        "the teardown lands on the receiver exactly once: {calls:?}"
    );
    let stop = calls
        .iter()
        .position(|call| *call == "stop")
        .expect("the media stop lands on the receiver");
    let teardown = calls
        .iter()
        .position(|call| *call == "teardown")
        .expect("the teardown lands on the receiver");
    assert!(
        stop < teardown,
        "the teardown runs after the media stop, in worker FIFO order: {calls:?}"
    );
}

/// The loaded item's slot, when one is recorded.
fn loaded_item_slot(t: &TestLoop) -> Option<mbv_queue::QueueSlotId> {
    let PlaybackOutput::Cast(cast) = &t.event_loop.owner.output else {
        panic!("the receiver is the output");
    };
    cast.loaded.as_ref().map(|loaded| loaded.slot)
}

/// Contract: row 5.3 correction — a tray/MPRIS transport command on a Cast
/// output lands on the receiver, not on the local player: the transport
/// `Player` arm routes to the active output like the ctrl wire command.
#[test]
fn a_tray_transport_command_on_cast_lands_on_the_receiver_not_the_local_player() {
    let mut t = test_loop_with_role(crate::DaemonRole::Local);
    let (_client_id, _rx, calls) =
        select_cast_output(&mut t, playing_status_at("https://feed/a.mp3", 10.0));
    // One tick runs the status poll, so the output holds the receiver's
    // reported state before the command routes.
    let now = std::time::Instant::now();
    t.event_loop.tick(now);
    pump(&mut t);
    let commands = t.event_loop.player.spy_on_commands();

    t.event_loop
        .handle_event(DaemonEvent::Transport(mbv_ctrl::TransportCommand::Player(
            PlayerCommand::TogglePause,
        )));
    wait_for_worker_jobs(&t.event_loop.owner.output);

    // The poll job pongs before its status exchange, so the routed command's
    // call is the last one the transport recorded.
    let log = calls.lock().unwrap().clone();
    assert_eq!(
        log.last().map(String::as_str),
        Some("pause"),
        "the tray pause lands on the receiver: {log:?}"
    );
    assert!(
        commands.try_recv().is_err(),
        "the local run is never started or touched"
    );
}

/// Contract: row 5.3 correction (spec "play, pause, next, previous, and stop
/// from either Client act on that receiver") — a guarded `SetPaused` intent
/// on a Cast output pauses the receiver with an explicit pause (the intent
/// carries the desired state) and settles to Applied immediately: no run
/// event would settle it, and the local player is never told.
#[test]
fn a_set_paused_intent_on_cast_pauses_the_receiver_and_settles_the_intent() {
    let mut t = test_loop_with_role(crate::DaemonRole::Local);
    let (connector, calls) = status_connector(playing_status_at("https://feed/a.mp3", 10.0));
    t.event_loop.cast_connector = Some(connector);
    let (client_id, reply_tx, rx) = connect_output_client(&t.event_loop.ctrl_clients, true);
    select_output(&mut t, client_id, &reply_tx, cast_selection());
    pump_cast_event(&mut t);
    let now = std::time::Instant::now();
    t.event_loop.tick(now);
    pump(&mut t);

    t.event_loop.handle_event(DaemonEvent::Ctrl(
        CtrlCmd::PlaybackIntent(PlaybackIntent {
            request_id: 9,
            generation: 9,
            action: PlaybackIntentAction::SetPaused { paused: true },
        }),
        client_id,
        reply_tx,
    ));
    wait_for_worker_jobs(&t.event_loop.owner.output);

    let log = calls.lock().unwrap().clone();
    assert_eq!(
        log.last().map(String::as_str),
        Some("pause"),
        "the desired pause translates to an explicit receiver pause: {log:?}"
    );
    assert!(
        !t.event_loop.player.status.lock().unwrap().paused,
        "the local player is never told"
    );
    let events = drained_events(&rx);
    assert!(
        events.iter().any(|event| matches!(
            event,
            CtrlEvent::PlaybackIntent(ev)
                if ev.request_id == 9
                    && matches!(ev.outcome, PlaybackIntentOutcome::Accepted)
        )),
        "the intent is accepted"
    );
    assert!(
        events.iter().any(|event| matches!(
            event,
            CtrlEvent::PlaybackIntent(ev)
                if ev.request_id == 9
                    && matches!(ev.outcome, PlaybackIntentOutcome::Applied)
        )),
        "the guarded intent settles to Applied"
    );
}
