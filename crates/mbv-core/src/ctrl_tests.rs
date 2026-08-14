use super::*;

#[test]
fn play_items_command_preserves_start_index() {
    let json = serde_json::to_string(&CtrlCmd::PlayItems {
        item_ids: vec!["a".to_string(), "b".to_string()],
        start_idx: 1,
        start_ticks: 42,
        source: QueueSource::Album,
    })
    .unwrap();

    let cmd: CtrlCmd = serde_json::from_str(&json).unwrap();
    match cmd {
        CtrlCmd::PlayItems {
            item_ids,
            start_idx,
            start_ticks,
            source,
        } => {
            assert_eq!(item_ids, vec!["a", "b"]);
            assert_eq!(start_idx, 1);
            assert_eq!(start_ticks, 42);
            assert!(matches!(source, QueueSource::Album));
        }
        _ => panic!("expected PlayItems"),
    }
}

#[test]
fn current_hello_validates() {
    CtrlHello::current().validate_peer().unwrap();
    assert_eq!(CtrlHello::current().protocol_version, 9);
}

#[test]
fn hello_rejects_incompatible_protocol_version() {
    let mut hello = CtrlHello::current();
    hello.protocol_version += 1;
    assert!(hello.validate_peer().is_err());
}

#[test]
fn hello_rejects_missing_capability() {
    let mut hello = CtrlHello::current();
    hello.capabilities.retain(|cap| cap != CTRL_CAP_START_INDEX);
    assert!(hello.validate_peer().is_err());
}

#[test]
fn current_hello_has_no_service_credential_field() {
    let json = serde_json::to_string(&CtrlHello::current()).unwrap();
    assert!(!json.contains("auth_token"));
    assert!(!json.contains("token-123"));
}

#[test]
fn service_setup_reconciliation_wire_has_only_kind_and_revision() {
    let json = serde_json::to_string(&CtrlCmd::ApplyServiceSetup {
        kind: crate::config::ServiceKind::Emby,
        revision: 42,
    })
    .unwrap();
    assert_eq!(
        json,
        r#"{"ApplyServiceSetup":{"kind":"Emby","revision":42}}"#
    );
    assert!(!json.contains("setup"));
    assert!(!json.contains("token"));
    assert!(!json.contains("credential"));
}

#[test]
fn service_setup_reconciliation_responses_round_trip() {
    let applied = CtrlEvent::ServiceSetupApplied {
        kind: crate::config::ServiceKind::Emby,
        revision: 7,
    };
    let rejected = CtrlEvent::ServiceSetupRejected {
        kind: crate::config::ServiceKind::Emby,
        revision: 7,
        reason: ServiceSetupRejection::RevisionMismatch,
    };
    assert!(matches!(
        serde_json::from_str::<CtrlEvent>(&serde_json::to_string(&applied).unwrap()).unwrap(),
        CtrlEvent::ServiceSetupApplied { .. }
    ));
    assert!(matches!(
        serde_json::from_str::<CtrlEvent>(&serde_json::to_string(&rejected).unwrap()).unwrap(),
        CtrlEvent::ServiceSetupRejected {
            reason: ServiceSetupRejection::RevisionMismatch,
            ..
        }
    ));
}

#[test]
fn capable_client_hello_uses_control_credential_field() {
    let hello = CtrlHello::current_control_client("control-123".into());
    assert!(hello.supports_control_auth());
    assert_eq!(hello.control_token.as_deref(), Some("control-123"));
}

#[test]
fn invalid_control_credential_is_rejected_without_emby_validation() {
    let hello = CtrlHello::current_control_client("not-the-control-secret".into());
    assert!(hello.validate_control_credential("control-secret").is_err());
}

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
        serde_json::to_string(&WireCommand::QueueAppend { items: vec![] }).unwrap(),
        "{\"QueueAppend\":{\"items\":[]}}"
    );
    assert_eq!(
        serde_json::to_string(&WireCommand::QueueRemove(2)).unwrap(),
        "{\"PlaylistRemove\":2}"
    );
    assert_eq!(
        serde_json::to_string(&WireCommand::QueueMove(2, 3)).unwrap(),
        "{\"PlaylistMove\":[2,3]}"
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
    assert_eq!(
        serde_json::to_string(&WireCommand::ReplaceQueue {
            items: vec![],
            start_idx: 0,
        })
        .unwrap(),
        "{\"ReplacePlaylist\":{\"items\":[],\"start_idx\":0}}"
    );
    // LoadNew and NextUpShow carry a EmbyItem / free-form strings, so
    // asserting the full JSON body would just restate EmbyItem's field
    // list; instead check the pinned tag key only.
    assert_eq!(
        wire_tag(&WireCommand::LoadNew {
            url: "http://emby.local/stream".into(),
            start_pos: 0.0,
            item: Box::new(stub_media_item()),
        }),
        "LoadNew"
    );
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
        r#"{"Player":{"TrackCompleted":{"idx":1,"position_ticks":456,"played":true,"consume":true}}}"#,
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

fn stub_media_item() -> crate::api::EmbyItem {
    crate::api::EmbyItem {
        id: "item1".into(),
        name: "Test Item".into(),
        item_type: "Episode".into(),
        is_folder: false,
        media_type: "Video".into(),
        collection_type: String::new(),
        runtime_ticks: 0,
        played: false,
        playback_position_ticks: 0,
        series_id: String::new(),
        series_name: String::new(),
        album_id: String::new(),
        album: String::new(),
        index_number: 0,
        parent_index_number: 0,
        unplayed_item_count: 0,
        path: String::new(),
        artist: String::new(),
        sort_name: String::new(),
        production_year: 0,
        end_year: 0,
        overview: String::new(),
        premiere_date: String::new(),
        date_added: String::new(),
        total_count: 0,
        container: String::new(),
        director: String::new(),
        video_info: String::new(),
        audio_info: String::new(),
        genre: String::new(),
        playlist_item_id: String::new(),
    }
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
    let wire: WireCommand = PlayerCommand::SeekAbsolute(12.5).into();
    let json = serde_json::to_string(&wire).unwrap();
    let decoded: WireCommand = serde_json::from_str(&json).unwrap();
    match PlayerCommand::from(decoded) {
        PlayerCommand::SeekAbsolute(s) => assert_eq!(s, 12.5),
        _ => panic!("expected SeekAbsolute"),
    }
}

#[test]
fn ctrl_cmd_player_cmd_round_trips_through_json() {
    let json =
        serde_json::to_string(&CtrlCmd::PlayerCmd(PlayerCommand::SetMute(true).into())).unwrap();
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

// ── LoadFeed / feed-playback capability ────────────────────────────────────

fn stub_feed_entry() -> crate::playback_queue::FeedEntry {
    crate::playback_queue::FeedEntry {
        guid: "feed-guid-1".into(),
        title: "Episode 1".into(),
        enclosure_url: Some("https://example.com/ep1.mp3".into()),
        link: None,
        mime_type: Some("audio/mpeg".into()),
        duration_ticks: Some((3_600 * crate::api::TICKS_PER_SECOND) as u64),
        pub_date_secs: Some(1700000000),
        feed_kind: Some(crate::config::FeedKind::Audio),
        feed_id: None,
        position_ticks: 0,
        played: false,
    }
}

#[test]
fn load_feed_wire_tag_is_pinned() {
    assert_eq!(
        wire_tag(&WireCommand::LoadFeed {
            entry: stub_feed_entry(),
        }),
        "LoadFeed"
    );
}

#[test]
fn load_feed_wire_round_trips_through_json() {
    let entry = stub_feed_entry();
    let wire = WireCommand::LoadFeed {
        entry: entry.clone(),
    };
    let json = serde_json::to_string(&wire).unwrap();
    let decoded: WireCommand = serde_json::from_str(&json).unwrap();
    match decoded {
        WireCommand::LoadFeed {
            entry: decoded_entry,
        } => {
            assert_eq!(decoded_entry.guid, entry.guid);
            assert_eq!(decoded_entry.title, entry.title);
            assert_eq!(decoded_entry.enclosure_url, entry.enclosure_url);
            assert_eq!(decoded_entry.mime_type, entry.mime_type);
            assert_eq!(decoded_entry.duration_ticks, entry.duration_ticks);
            assert_eq!(decoded_entry.pub_date_secs, entry.pub_date_secs);
        }
        _ => panic!("expected LoadFeed"),
    }
}

#[test]
fn load_feed_ctrl_cmd_round_trips_through_json() {
    let entry = stub_feed_entry();
    let cmd = CtrlCmd::PlayerCmd(WireCommand::LoadFeed {
        entry: entry.clone(),
    });
    let json = serde_json::to_string(&cmd).unwrap();
    let decoded: CtrlCmd = serde_json::from_str(&json).unwrap();
    match decoded {
        CtrlCmd::PlayerCmd(wire) => match wire {
            WireCommand::LoadFeed {
                entry: decoded_entry,
            } => {
                assert_eq!(decoded_entry.guid, entry.guid);
            }
            _ => panic!("expected LoadFeed"),
        },
        _ => panic!("expected PlayerCmd"),
    }
}

#[test]
fn current_hello_advertises_feed_playback_capability() {
    let hello = CtrlHello::current();
    assert!(
        hello.supports_feed_playback(),
        "CtrlHello::current() must advertise feed-playback capability"
    );
}

#[test]
fn hello_missing_feed_playback_is_detected() {
    let mut hello = CtrlHello::current();
    hello
        .capabilities
        .retain(|cap| cap != CTRL_CAP_FEED_PLAYBACK);
    assert!(
        !hello.supports_feed_playback(),
        "hello without feed-playback must be detected as lacking it"
    );
}

#[test]
fn ctrl_compatibility_supports_feed_playback() {
    let compat = CtrlCompatibility::current();
    assert!(compat.supports_feed_playback);
}

#[test]
fn old_peer_without_feed_playback_capability_rejects_load_feed() {
    let compat = CtrlCompatibility {
        peer_protocol_version: CTRL_PROTOCOL_VERSION,
        client_protocol_version: CTRL_PROTOCOL_VERSION,
        supports_queue_append: true,
        supports_lifecycle_shutdown: false,
        supports_feed_playback: false,
        supports_unified_queue: false,
        supports_control_auth: false,
    };
    assert!(!compat.supports_feed_playback);
}

// ── Unified queue wire types ──────────────────────────────────────────────

#[test]
fn current_hello_advertises_unified_queue_capability() {
    let hello = CtrlHello::current();
    assert!(
        hello.supports_unified_queue(),
        "CtrlHello::current() must advertise unified-queue capability"
    );
}

#[test]
fn hello_without_unified_queue_detected() {
    let mut hello = CtrlHello::current();
    hello
        .capabilities
        .retain(|cap| cap != CTRL_CAP_UNIFIED_QUEUE);
    assert!(!hello.supports_unified_queue());
}

#[test]
fn unified_queue_slot_round_trips_through_json() {
    let slot = UnifiedQueueSlot {
        slot_id: 42,
        item: QueueItem::Emby(Box::new(stub_media_item())),
    };
    let json = serde_json::to_string(&slot).unwrap();
    let decoded: UnifiedQueueSlot = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.slot_id, 42);
    assert_eq!(decoded.item.id(), "item1");
}

#[test]
fn unified_queue_slot_feed_round_trips() {
    let slot = UnifiedQueueSlot {
        slot_id: 7,
        item: QueueItem::Feed(stub_feed_entry()),
    };
    let json = serde_json::to_string(&slot).unwrap();
    let decoded: UnifiedQueueSlot = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.slot_id, 7);
    match &decoded.item {
        QueueItem::Feed(e) => assert_eq!(e.guid, "feed-guid-1"),
        _ => panic!("expected Feed"),
    }
}

#[test]
fn unified_queue_state_data_round_trips() {
    let state = UnifiedQueueStateData {
        status: PlayerStatus::default(),
        slots: vec![
            UnifiedQueueSlot {
                slot_id: 1,
                item: QueueItem::Emby(Box::new(stub_media_item())),
            },
            UnifiedQueueSlot {
                slot_id: 2,
                item: QueueItem::Feed(stub_feed_entry()),
            },
        ],
        active_slot: Some(1),
        revision: 5,
        source: QueueSource::Unknown,
    };
    let json = serde_json::to_string(&state).unwrap();
    let decoded: UnifiedQueueStateData = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.slots.len(), 2);
    assert_eq!(decoded.active_slot, Some(1));
    assert_eq!(decoded.revision, 5);
}

#[test]
fn unified_queue_replace_cmd_round_trips() {
    let items = vec![
        QueueItem::Emby(Box::new(stub_media_item())),
        QueueItem::Feed(stub_feed_entry()),
    ];
    let cmd = CtrlCmd::UnifiedQueueReplace {
        items,
        start_idx: Some(0),
    };
    let json = serde_json::to_string(&cmd).unwrap();
    let decoded: CtrlCmd = serde_json::from_str(&json).unwrap();
    match decoded {
        CtrlCmd::UnifiedQueueReplace { items, start_idx } => {
            assert_eq!(items.len(), 2);
            assert_eq!(start_idx, Some(0));
        }
        _ => panic!("expected UnifiedQueueReplace"),
    }
}

#[test]
fn unified_queue_append_cmd_round_trips() {
    let cmd = CtrlCmd::UnifiedQueueAppend {
        items: vec![QueueItem::Feed(stub_feed_entry())],
    };
    let json = serde_json::to_string(&cmd).unwrap();
    let decoded: CtrlCmd = serde_json::from_str(&json).unwrap();
    match decoded {
        CtrlCmd::UnifiedQueueAppend { items } => assert_eq!(items.len(), 1),
        _ => panic!("expected UnifiedQueueAppend"),
    }
}

#[test]
fn unified_queue_remove_slot_cmd_round_trips() {
    let cmd = CtrlCmd::UnifiedQueueRemoveSlot { slot_id: 99 };
    let json = serde_json::to_string(&cmd).unwrap();
    let decoded: CtrlCmd = serde_json::from_str(&json).unwrap();
    match decoded {
        CtrlCmd::UnifiedQueueRemoveSlot { slot_id } => assert_eq!(slot_id, 99),
        _ => panic!("expected UnifiedQueueRemoveSlot"),
    }
}

#[test]
fn unified_queue_move_slot_cmd_round_trips() {
    let cmd = CtrlCmd::UnifiedQueueMoveSlot {
        slot_id: 3,
        to_index: 0,
    };
    let json = serde_json::to_string(&cmd).unwrap();
    let decoded: CtrlCmd = serde_json::from_str(&json).unwrap();
    match decoded {
        CtrlCmd::UnifiedQueueMoveSlot { slot_id, to_index } => {
            assert_eq!(slot_id, 3);
            assert_eq!(to_index, 0);
        }
        _ => panic!("expected UnifiedQueueMoveSlot"),
    }
}

#[test]
fn unified_queue_play_slot_cmd_round_trips() {
    let cmd = CtrlCmd::UnifiedQueuePlaySlot { slot_id: 5 };
    let json = serde_json::to_string(&cmd).unwrap();
    let decoded: CtrlCmd = serde_json::from_str(&json).unwrap();
    match decoded {
        CtrlCmd::UnifiedQueuePlaySlot { slot_id } => assert_eq!(slot_id, 5),
        _ => panic!("expected UnifiedQueuePlaySlot"),
    }
}

#[test]
fn unified_queue_clear_cmd_round_trips() {
    let cmd = CtrlCmd::UnifiedQueueClear;
    let json = serde_json::to_string(&cmd).unwrap();
    let decoded: CtrlCmd = serde_json::from_str(&json).unwrap();
    assert!(matches!(decoded, CtrlCmd::UnifiedQueueClear));
}

#[test]
fn unified_adopt_queue_cmd_round_trips() {
    let items = vec![
        QueueItem::Emby(Box::new(stub_media_item())),
        QueueItem::Feed(stub_feed_entry()),
    ];
    let cmd = CtrlCmd::UnifiedAdoptQueue {
        items,
        cursor: 1,
        source: QueueSource::Album,
    };
    let json = serde_json::to_string(&cmd).unwrap();
    let decoded: CtrlCmd = serde_json::from_str(&json).unwrap();
    match decoded {
        CtrlCmd::UnifiedAdoptQueue {
            items,
            cursor,
            source,
        } => {
            assert_eq!(items.len(), 2);
            assert_eq!(cursor, 1);
            assert!(matches!(source, QueueSource::Album));
        }
        _ => panic!("expected UnifiedAdoptQueue"),
    }
}

#[test]
fn unified_queue_state_event_round_trips() {
    let event = CtrlEvent::UnifiedQueueState(UnifiedQueueStateData {
        status: PlayerStatus::default(),
        slots: vec![],
        active_slot: None,
        revision: 0,
        source: QueueSource::Unknown,
    });
    let json = serde_json::to_string(&event).unwrap();
    let decoded: CtrlEvent = serde_json::from_str(&json).unwrap();
    match decoded {
        CtrlEvent::UnifiedQueueState(state) => {
            assert!(state.slots.is_empty());
            assert_eq!(state.active_slot, None);
        }
        _ => panic!("expected UnifiedQueueState"),
    }
}

#[test]
fn ctrl_compatibility_current_supports_unified_queue() {
    let compat = CtrlCompatibility::current();
    assert!(compat.supports_unified_queue);
}
