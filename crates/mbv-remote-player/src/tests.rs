use super::RemotePlayer;
use mbv_ctrl::player::PlayerCommand;

#[test]
fn local_only_command_is_refused_without_delivery_or_termination() {
    // End-to-end through the remote-player send path: the send reports the
    // refusal (`false`), delivers nothing, and the caller keeps running.
    let (remote, _event_rx, cmd_rx) = RemotePlayer::stub_with_command_rx(Vec::new(), 0);
    assert!(!remote.send_command(PlayerCommand::JumpTo {
        slot_id: mbv_queue::QueueSlotId::from_raw(3),
        request_id: 9,
        generation: 9,
        resume_ticks: None,
    }));
    assert!(
        cmd_rx.try_recv().is_err(),
        "refused command must not be delivered"
    );
}

#[test]
fn send_transport_step_next_sends_a_playback_intent() {
    let (remote, _, commands) = RemotePlayer::stub_with_command_rx(Vec::new(), 0);
    remote.send_transport(mbv_ctrl::TransportCommand::Step(mbv_ctrl::Direction::Next));
    assert!(matches!(
        commands.try_recv().unwrap(),
        mbv_ctrl::CtrlCmd::PlaybackIntent(intent)
            if intent.action == mbv_ctrl::PlaybackIntentAction::Next
    ));
}

#[test]
fn send_transport_stop_sends_the_ctrl_stop() {
    // An MPRIS/tray stop crosses as the uncorrelated wire stop, not as a
    // playback intent: stop has no resolve/open lifecycle to correlate.
    let (remote, _, commands) = RemotePlayer::stub_with_command_rx(Vec::new(), 0);
    remote.send_transport(mbv_ctrl::TransportCommand::Stop);
    assert!(matches!(
        commands.try_recv().unwrap(),
        mbv_ctrl::CtrlCmd::Stop
    ));
}

/// A daemon-side peer for the stop-ack seam test: handshakes like
/// `connect_stub_daemon_pair`'s peer, then answers every `PlaybackIntent`
/// with its correlated reply and reads on until the client closes.
#[test]
fn a_stop_intent_is_delivered_before_the_connection_closes() {
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::UnixListener;

    use mbv_ctrl::{
        CtrlEvent, CtrlHello, PlaybackIntentAction, PlaybackIntentEvent, PlaybackIntentOutcome,
        UnifiedQueueStateData,
    };

    // Review P1 (queue-handoff-and-take row 8.1): the settled Take closes
    // the source link right after the source Stop. `stop_and_await_ack`
    // must return only once the peer actually read the Stop, so the
    // teardown cannot outrun the write.
    let path = std::env::temp_dir().join(format!("mbv-stop-ack-{}.sock", std::process::id()));
    let _ = std::fs::remove_file(&path);
    let listener = UnixListener::bind(&path).unwrap();
    let peer = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut writer = stream.try_clone().unwrap();
        let mut reader = BufReader::new(stream);
        let hello = serde_json::to_string(&CtrlEvent::Hello(CtrlHello::current())).unwrap();
        writeln!(writer, "{hello}").unwrap();
        let mut client_hello = String::new();
        reader.read_line(&mut client_hello).unwrap();
        let state = serde_json::to_string(&CtrlEvent::UnifiedQueueState(UnifiedQueueStateData {
            status: mbv_ctrl::player::PlayerStatus::default(),
            slots: Vec::new(),
            active_slot: None,
            revision: 0,
            source: mbv_queue::QueueSource::Unknown,
            lineage: mbv_queue::QueueLineage::default(),
            in_flight_transition: None,
            queued_latest_transition: None,
        }))
        .unwrap();
        writeln!(writer, "{state}").unwrap();
        let mut saw_stop_before_eof = false;
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
            if let Ok(mbv_ctrl::CtrlCmd::PlaybackIntent(intent)) =
                serde_json::from_str::<mbv_ctrl::CtrlCmd>(&line)
            {
                if intent.action == PlaybackIntentAction::Stop {
                    saw_stop_before_eof = true;
                }
                let reply =
                    serde_json::to_string(&CtrlEvent::PlaybackIntent(PlaybackIntentEvent {
                        request_id: intent.request_id,
                        generation: intent.generation,
                        outcome: PlaybackIntentOutcome::Accepted,
                    }))
                    .unwrap();
                writeln!(writer, "{reply}").unwrap();
            }
        }
        assert!(
            saw_stop_before_eof,
            "the peer must read the Stop before the client closes the connection"
        );
    });
    let (player, _event_rx) =
        RemotePlayer::connect_endpoint(&super::DaemonEndpoint::Unix(path.clone()), false, None)
            .unwrap();

    player.stop_and_await_ack(std::time::Duration::from_secs(2));
    player.disconnect();
    peer.join().unwrap();
    let _ = std::fs::remove_file(&path);
}

#[test]
fn the_stop_ack_wait_times_out_without_a_reply_but_the_stop_is_enqueued() {
    let (remote, _event_rx, cmd_rx) = RemotePlayer::stub_with_command_rx(Vec::new(), 0);
    remote.stop_and_await_ack(std::time::Duration::ZERO);
    assert!(matches!(
        cmd_rx.try_recv().unwrap(),
        mbv_ctrl::CtrlCmd::PlaybackIntent(intent) if intent.action == mbv_ctrl::PlaybackIntentAction::Stop
    ));
}

#[test]
fn the_stop_ack_wait_reports_a_lost_connection_at_once() {
    let (remote, _event_rx, _cmd_rx) = RemotePlayer::stub_with_command_rx(Vec::new(), 0);
    remote.set_disconnected_for_test(true);
    assert!(!remote.await_playback_intent(1, std::time::Duration::from_secs(2)));
}
