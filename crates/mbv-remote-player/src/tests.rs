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
