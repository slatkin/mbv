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
    }

    #[test]
    fn hello_rejects_incompatible_protocol_version() {
        let mut hello = CtrlHello::current();
        hello.protocol_version += 1;
        assert!(hello.validate_peer().is_err());
    }

    #[test]
    fn hello_rejects_v2_protocol_version() {
        let mut hello = CtrlHello::current();
        hello.protocol_version = 2;
        assert!(hello.validate_peer().is_err());
    }

    #[test]
    fn hello_rejects_v3_protocol_version() {
        let mut hello = CtrlHello::current();
        hello.protocol_version = 3;
        assert!(hello.validate_peer().is_err());
    }

    #[test]
    fn hello_rejects_previous_v4_protocol_version() {
        let mut hello = CtrlHello::current();
        hello.protocol_version = 4;
        assert!(hello.validate_peer().is_err());
    }

    #[test]
    fn hello_rejects_previous_v5_protocol_version() {
        let mut hello = CtrlHello::current();
        hello.protocol_version = 5;
        assert!(hello.validate_peer().is_err());
    }

    #[test]
    fn hello_rejects_missing_capability() {
        let mut hello = CtrlHello::current();
        hello.capabilities.retain(|cap| cap != CTRL_CAP_START_INDEX);
        assert!(hello.validate_peer().is_err());
    }

    #[test]
    fn current_client_hello_carries_auth_token() {
        let hello = CtrlHello::current_client("token-123".into());
        assert_eq!(hello.auth_token.as_deref(), Some("token-123"));
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
        // LoadNew and NextUpShow carry a MediaItem / free-form strings, so
        // asserting the full JSON body would just restate MediaItem's field
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
        // v8 slot-aware queue command tags
        assert_eq!(
            wire_tag(&WireCommand::QueueRemoveBySlot {
                slot_id: QueueSlotId::new(1),
                revision: QueueRevision::new(5),
            }),
            "QueueRemoveBySlot"
        );
        assert_eq!(
            wire_tag(&WireCommand::QueueMoveBySlot {
                slot_id: QueueSlotId::new(2),
                to_position: 1,
                revision: QueueRevision::new(6),
            }),
            "QueueMoveBySlot"
        );
        assert_eq!(
            wire_tag(&WireCommand::JumpToSlot {
                slot_id: QueueSlotId::new(3),
            }),
            "JumpToSlot"
        );
        assert_eq!(
            wire_tag(&WireCommand::QueueInsertAt {
                item: stub_media_item(),
                position: 1,
                revision: QueueRevision::new(7),
            }),
            "QueueInsertAt"
        );
        assert_eq!(
            wire_tag(&WireCommand::QueueRemoveActive {
                revision: QueueRevision::new(8),
            }),
            "QueueRemoveActive"
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

    fn stub_media_item() -> crate::api::MediaItem {
        crate::api::MediaItem {
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
        match PlayerCommand::try_from(decoded).unwrap() {
            PlayerCommand::SetVolume(v) => assert_eq!(v, 77),
            _ => panic!("expected SetVolume"),
        }
    }

    #[test]
    fn player_command_round_trips_through_wire_command() {
        let wire: WireCommand = PlayerCommand::SeekAbsolute(12.5).into();
        let json = serde_json::to_string(&wire).unwrap();
        let decoded: WireCommand = serde_json::from_str(&json).unwrap();
        match PlayerCommand::try_from(decoded).unwrap() {
            PlayerCommand::SeekAbsolute(s) => assert_eq!(s, 12.5),
            _ => panic!("expected SeekAbsolute"),
        }
    }

    #[test]
    fn ctrl_cmd_player_cmd_round_trips_through_json() {
        let json = serde_json::to_string(&CtrlCmd::PlayerCmd(PlayerCommand::SetMute(true).into()))
            .unwrap();
        let cmd: CtrlCmd = serde_json::from_str(&json).unwrap();
        match cmd {
            CtrlCmd::PlayerCmd(wire) => match PlayerCommand::try_from(wire).unwrap() {
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

    #[test]
    fn hello_accepts_v7_peer() {
        let mut hello = CtrlHello::current();
        hello.protocol_version = 7;
        assert!(hello.validate_peer().is_ok());
    }

    #[test]
    fn hello_accepts_v8_peer() {
        let mut hello = CtrlHello::current();
        hello.protocol_version = 8;
        assert!(hello.validate_peer().is_ok());
    }

    #[test]
    fn hello_rejects_v6_peer() {
        let mut hello = CtrlHello::current();
        hello.protocol_version = 6;
        assert!(hello.validate_peer().is_err());
    }

    #[test]
    fn v7_compatibility_returns_v7_client_version() {
        let compat = CtrlCompatibility::for_peer(7).unwrap();
        assert_eq!(compat.peer_protocol_version, 7);
        assert_eq!(compat.client_protocol_version, 7);
        assert!(compat.supports_queue_append);
    }

    #[test]
    fn v8_compatibility_returns_v8_client_version() {
        let compat = CtrlCompatibility::for_peer(8).unwrap();
        assert_eq!(compat.peer_protocol_version, 8);
        assert_eq!(compat.client_protocol_version, 8);
        assert!(compat.supports_queue_append);
    }

    #[test]
    fn compatible_client_uses_explicit_compat_version() {
        let compat = CtrlCompatibility::for_peer(7).unwrap();
        let hello = CtrlHello::compatible_client("tok".into(), compat);
        assert_eq!(hello.protocol_version, 7);
        assert_eq!(hello.auth_token.as_deref(), Some("tok"));
    }

    #[test]
    fn v8_wire_command_round_trips_through_json() {
        let cmd = WireCommand::QueueRemoveBySlot {
            slot_id: QueueSlotId::new(42),
            revision: QueueRevision::new(3),
        };
        let json = serde_json::to_string(&cmd).unwrap();
        let decoded: WireCommand = serde_json::from_str(&json).unwrap();
        match decoded {
            WireCommand::QueueRemoveBySlot { slot_id, revision } => {
                assert_eq!(slot_id, QueueSlotId::new(42));
                assert_eq!(revision, QueueRevision::new(3));
            }
            other => panic!("unexpected decoded variant: {other:?}"),
        }

        let cmd = WireCommand::QueueMoveBySlot {
            slot_id: QueueSlotId::new(1),
            to_position: 2,
            revision: QueueRevision::new(4),
        };
        let json = serde_json::to_string(&cmd).unwrap();
        let decoded: WireCommand = serde_json::from_str(&json).unwrap();
        match decoded {
            WireCommand::QueueMoveBySlot {
                slot_id,
                to_position,
                revision,
            } => {
                assert_eq!(slot_id, QueueSlotId::new(1));
                assert_eq!(to_position, 2);
                assert_eq!(revision, QueueRevision::new(4));
            }
            other => panic!("unexpected decoded variant: {other:?}"),
        }

        let cmd = WireCommand::JumpToSlot {
            slot_id: QueueSlotId::new(99),
        };
        let json = serde_json::to_string(&cmd).unwrap();
        let decoded: WireCommand = serde_json::from_str(&json).unwrap();
        match decoded {
            WireCommand::JumpToSlot { slot_id } => {
                assert_eq!(slot_id, QueueSlotId::new(99));
            }
            other => panic!("unexpected decoded variant: {other:?}"),
        }

        let cmd = WireCommand::QueueInsertAt {
            item: stub_media_item(),
            position: 0,
            revision: QueueRevision::new(1),
        };
        let json = serde_json::to_string(&cmd).unwrap();
        let decoded: WireCommand = serde_json::from_str(&json).unwrap();
        assert!(matches!(decoded, WireCommand::QueueInsertAt { position: 0, .. }));

        let cmd = WireCommand::QueueRemoveActive {
            revision: QueueRevision::new(2),
        };
        let json = serde_json::to_string(&cmd).unwrap();
        let decoded: WireCommand = serde_json::from_str(&json).unwrap();
        match decoded {
            WireCommand::QueueRemoveActive { revision } => {
                assert_eq!(revision, QueueRevision::new(2));
            }
            other => panic!("unexpected decoded variant: {other:?}"),
        }
    }

    #[test]
    fn ctrl_state_omits_slot_fields_when_default() {
        let state = CtrlState {
            status: PlayerStatus::default(),
            items: vec![],
            cursor: 0,
            source: QueueSource::Album,
            slot_ids: vec![],
            revision: QueueRevision::default(),
            active_slot_id: None,
        };
        let json = serde_json::to_string(&state).unwrap();
        // Verify the new fields are absent (skip_serializing_if)
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        let obj = value.as_object().unwrap();
        assert!(!obj.contains_key("slot_ids"));
        assert!(!obj.contains_key("revision"));
        assert!(!obj.contains_key("active_slot_id"));
    }

    #[test]
    fn ctrl_state_includes_slot_fields_when_present() {
        let state = CtrlState {
            status: PlayerStatus::default(),
            items: vec![],
            cursor: 0,
            source: QueueSource::Album,
            slot_ids: vec![QueueSlotId::new(1), QueueSlotId::new(2)],
            revision: QueueRevision::new(5),
            active_slot_id: Some(QueueSlotId::new(1)),
        };
        let json = serde_json::to_string(&state).unwrap();
        let decoded: CtrlState = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.slot_ids, vec![QueueSlotId::new(1), QueueSlotId::new(2)]);
        assert_eq!(decoded.revision, QueueRevision::new(5));
        assert_eq!(decoded.active_slot_id, Some(QueueSlotId::new(1)));
    }

    #[test]
    fn ctrl_state_deserializes_with_missing_slot_fields() {
        // A v7 daemon or client won't send slot_ids/revision/active_slot_id.
        // serde(default) must fill them in.
        let json = r#"{"status":{"position_ticks":0,"last_valid_pos":0,"runtime_ticks":0,"paused":false,"volume":100,"volume_max":130,"current_idx":0,"queue_len":0,"active":false,"title":"","artist":"","album":"","art_item_id":"","art_album_id":"","audio_tracks":[],"sub_tracks":[],"sub_track_stream_indexes":[],"audio_id":0,"audio_lang":"","sub_id":0,"sub_lang":"","muted":false,"video_height":0,"audio_codec":"","video_is_image":false},"items":[],"cursor":0,"source":{"type":"unknown"}}"#;
        let state: CtrlState = serde_json::from_str(json).unwrap();
        assert!(state.slot_ids.is_empty());
        assert_eq!(state.revision, QueueRevision::default());
        assert_eq!(state.active_slot_id, None);
    }
