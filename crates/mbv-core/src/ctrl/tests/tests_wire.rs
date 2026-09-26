use super::super::*;

// The wire tags below are pinned via `#[serde(rename = "...")]` on
// `WireCommand` and must not change without a deliberate, explicit
// decision -- they are independent of whatever `PlayerCommand`'s Rust
// variant identifiers happen to be at any given time. If one of these
// assertions fails, the wire protocol just changed; that may be fine,
// but it should never happen as a side effect of an in-process rename.
#[test]
fn wire_command_tags_are_pinned() {
    assert_eq!(
        serde_json::to_string(&WireCommand::TogglePause).unwrap(),
        "\"TogglePause\""
    );
    assert_eq!(
        serde_json::to_string(&WireCommand::JumpTo(3)).unwrap(),
        "{\"JumpTo\":3}"
    );
    assert_eq!(
        serde_json::to_string(&WireCommand::SetVolume(50)).unwrap(),
        "{\"SetVolume\":50}"
    );
    assert_eq!(
        serde_json::to_string(&WireCommand::Seek(1.5)).unwrap(),
        "{\"Seek\":1.5}"
    );
    assert_eq!(
        serde_json::to_string(&WireCommand::SeekAbsolute(2.5)).unwrap(),
        "{\"SeekAbsolute\":2.5}"
    );
    assert_eq!(
        serde_json::to_string(&WireCommand::SetAudio(1)).unwrap(),
        "{\"SetAudio\":1}"
    );
    assert_eq!(
        serde_json::to_string(&WireCommand::SetSub(0)).unwrap(),
        "{\"SetSub\":0}"
    );
    assert_eq!(
        serde_json::to_string(&WireCommand::SetMute(true)).unwrap(),
        "{\"SetMute\":true}"
    );
    assert_eq!(
        serde_json::to_string(&WireCommand::NextUpDismiss).unwrap(),
        "\"NextUpDismiss\""
    );
    assert_eq!(
        serde_json::to_string(&WireCommand::SkipIntroDismiss).unwrap(),
        "\"SkipIntroDismiss\""
    );
    assert_eq!(
        serde_json::to_string(&WireCommand::SetSubtitlePrefs {
            mode: "auto".to_string(),
            subtitle_lang: "eng".to_string(),
            audio_lang: "jpn".to_string(),
        })
        .unwrap(),
        "{\"SetSubtitlePrefs\":{\"mode\":\"auto\",\"subtitle_lang\":\"eng\",\"audio_lang\":\"jpn\"}}"
    );
    // NextUpShow carries free-form strings, so asserting the full JSON body
    // would just restate the field list; instead check the pinned tag key
    // only.
    assert_eq!(
        wire_tag(&WireCommand::NextUpShow {
            item_id: "item1".into(),
            show_title: "Show".into(),
            ep_title: "Ep".into(),
            artist: String::new(),
        }),
        "NextUpShow"
    );
}

#[test]
fn old_stopped_player_event_defaults_progress_report_accepted() {
    let event: CtrlEvent = serde_json::from_str(
        r#"{"Player":{"Stopped":{"idx":0,"position_ticks":123,"played":false,"consume":false,"error":null}}}"#,
    )
    .unwrap();

    match event {
        CtrlEvent::Player(crate::player::PlayerEvent::Stopped {
            progress_report_accepted,
            ..
        }) => assert!(!progress_report_accepted),
        _ => panic!("expected stopped player event"),
    }
}

#[test]
fn old_track_completed_player_event_defaults_progress_report_accepted() {
    let event: CtrlEvent = serde_json::from_str(
        r#"{"Player":{"TrackCompleted":{"slot_id":1,"position_ticks":456,"played":true,"consume":true}}}"#,
    )
    .unwrap();

    match event {
        CtrlEvent::Player(crate::player::PlayerEvent::TrackCompleted {
            progress_report_accepted,
            ..
        }) => assert!(!progress_report_accepted),
        _ => panic!("expected track completed player event"),
    }
}

/// Returns the top-level (externally-tagged) JSON key for a serialized
/// `WireCommand`, i.e. the pinned wire tag.
fn wire_tag(cmd: &WireCommand) -> String {
    let json = serde_json::to_string(cmd).unwrap();
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    value
        .as_object()
        .and_then(|obj| obj.keys().next())
        .unwrap_or_else(|| panic!("expected a tagged object, got {json}"))
        .clone()
}

#[test]
fn wire_command_round_trips_through_json() {
    let json = serde_json::to_string(&WireCommand::SetVolume(77)).unwrap();
    let decoded: WireCommand = serde_json::from_str(&json).unwrap();
    match PlayerCommand::from(decoded) {
        PlayerCommand::SetVolume(v) => assert_eq!(v, 77),
        _ => panic!("expected SetVolume"),
    }
}

#[test]
fn player_command_round_trips_through_wire_command() {
    let wire = WireCommand::try_from_player_command(PlayerCommand::SeekAbsolute(12.5)).unwrap();
    let json = serde_json::to_string(&wire).unwrap();
    let decoded: WireCommand = serde_json::from_str(&json).unwrap();
    match PlayerCommand::from(decoded) {
        PlayerCommand::SeekAbsolute(s) => assert!((s - 12.5).abs() < f64::EPSILON),
        _ => panic!("expected SeekAbsolute"),
    }
}

#[test]
fn ctrl_cmd_player_cmd_round_trips_through_json() {
    let json = serde_json::to_string(&CtrlCmd::PlayerCmd(
        WireCommand::try_from_player_command(PlayerCommand::SetMute(true)).unwrap(),
    ))
    .unwrap();
    let cmd: CtrlCmd = serde_json::from_str(&json).unwrap();
    match cmd {
        CtrlCmd::PlayerCmd(wire) => match PlayerCommand::from(wire) {
            PlayerCommand::SetMute(m) => assert!(m),
            _ => panic!("expected SetMute"),
        },
        _ => panic!("expected PlayerCmd"),
    }
}

#[test]
fn playback_intent_round_trips_as_a_distinct_command() {
    let command = CtrlCmd::PlaybackIntent(PlaybackIntent {
        request_id: 7,
        generation: 3,
        action: PlaybackIntentAction::SetPaused { paused: true },
    });

    let json = serde_json::to_string(&command).unwrap();
    let decoded: CtrlCmd = serde_json::from_str(&json).unwrap();
    assert!(matches!(
        decoded,
        CtrlCmd::PlaybackIntent(PlaybackIntent {
            request_id: 7,
            generation: 3,
            action: PlaybackIntentAction::SetPaused { paused: true },
        })
    ));
}

#[test]
fn playback_intent_event_round_trips_structured_rejection() {
    let event = CtrlEvent::PlaybackIntent(PlaybackIntentEvent {
        request_id: 7,
        generation: 3,
        outcome: PlaybackIntentOutcome::Rejected {
            reason: PlaybackIntentRejection::AudioOnly,
        },
    });

    let json = serde_json::to_string(&event).unwrap();
    let decoded: CtrlEvent = serde_json::from_str(&json).unwrap();
    assert!(matches!(
        decoded,
        CtrlEvent::PlaybackIntent(PlaybackIntentEvent {
            outcome: PlaybackIntentOutcome::Rejected {
                reason: PlaybackIntentRejection::AudioOnly,
            },
            ..
        })
    ));
}

// A command with no ctrl wire form is refused fail-closed (design A2): the
// caller receives the unencodable command back, nothing is delivered, and the
// calling process survives — no unreachable!() abort.
#[test]
fn local_only_command_is_refused_without_delivery_or_termination() {
    let cmd = PlayerCommand::JumpTo {
        slot_id: crate::playback_queue::QueueSlotId::from_raw(3),
        request_id: 9,
        generation: 9,
        resume_ticks: None,
    };

    // The transport refuses to encode it, returning the command.
    assert!(
        matches!(
            WireCommand::try_from_player_command(cmd),
            Err(PlayerCommand::JumpTo { slot_id, .. }) if slot_id.raw() == 3
        ),
        "JumpTo has no wire form and must be refused with the command returned"
    );

    // End-to-end through the remote-player send path: the send reports the
    // refusal (`false`), delivers nothing, and the caller keeps running.
    let (remote, _event_rx, cmd_rx) =
        crate::remote_player::RemotePlayer::stub_with_command_rx(Vec::new(), 0);
    assert!(!remote.send_command(PlayerCommand::JumpTo {
        slot_id: crate::playback_queue::QueueSlotId::from_raw(3),
        request_id: 9,
        generation: 9,
        resume_ticks: None,
    }));
    assert!(
        cmd_rx.try_recv().is_err(),
        "refused command must not be delivered"
    );
}

#[rstest::rstest]
#[case::remove_slot(CtrlCmd::UnifiedQueueRemoveSlot { slot_id: 1 }, true)]
#[case::remove_slots(CtrlCmd::UnifiedQueueRemoveSlots { slot_ids: vec![1, 2] }, true)]
#[case::move_slot(CtrlCmd::UnifiedQueueMoveSlot { slot_id: 1, to_index: 0 }, true)]
#[case::clear(CtrlCmd::UnifiedQueueClear, true)]
#[case::play_slot(CtrlCmd::UnifiedQueuePlaySlot { slot_id: 1 }, false)]
#[case::stop(CtrlCmd::Stop, false)]
#[case::shutdown(CtrlCmd::RequestShutdown, false)]
fn owner_persists_only_after_queue_edits(#[case] cmd: CtrlCmd, #[case] persists: bool) {
    assert_eq!(cmd.mutates_owner_queue(), persists);
}
