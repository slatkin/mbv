use super::super::*;

fn stub_media_item() -> crate::api::EmbyItem {
    crate::api::EmbyItem {
        id: "item1".into(),
        name: "Test Item".into(),
        item_type: "Episode".into(),
        is_folder: false,
        child_count: None,
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
        artist_items: Vec::new(),
        sort_name: String::new(),
        production_year: 0,
        end_year: 0,
        overview: String::new(),
        premiere_date: String::new(),
        date_added: String::new(),
        total_count: 0,
        container: String::new(),
        video_info: String::new(),
        audio_info: String::new(),
        genres: Vec::new(),
        people: Vec::new(),
        external_urls: Vec::new(),
        playlist_item_id: String::new(),
        image_tags: Default::default(),
    }
}

// ── Unified queue wire types ──────────────────────────────────────────────

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
        lineage: QueueLineage(42),
        in_flight_transition: Some(crate::ctrl::TransitionSummary {
            request_id: 7,
            generation: 3,
            target_slot: 2,
        }),
        queued_latest_transition: Some(crate::ctrl::TransitionSummary {
            request_id: 8,
            generation: 3,
            target_slot: 1,
        }),
    };
    let json = serde_json::to_string(&state).unwrap();
    let decoded: UnifiedQueueStateData = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.slots.len(), 2);
    assert_eq!(decoded.active_slot, Some(1));
    assert_eq!(decoded.revision, 5);
    assert_eq!(decoded.lineage, QueueLineage(42));
    assert_eq!(decoded.in_flight_transition, state.in_flight_transition);
    assert_eq!(
        decoded.queued_latest_transition,
        state.queued_latest_transition
    );

    // Older payloads omit the transition fields entirely.
    let mut legacy: serde_json::Value = serde_json::from_str(&json).unwrap();
    let obj = legacy.as_object_mut().unwrap();
    obj.remove("in_flight_transition");
    obj.remove("queued_latest_transition");
    obj.remove("lineage");
    let decoded_legacy: UnifiedQueueStateData = serde_json::from_value(legacy).unwrap();
    assert_eq!(decoded_legacy.in_flight_transition, None);
    assert_eq!(decoded_legacy.queued_latest_transition, None);
    assert_eq!(decoded_legacy.lineage, QueueLineage::default());
}

#[test]
fn unified_queue_replace_cmd_round_trips() {
    let items = vec![
        QueueItem::Emby(Box::new(stub_media_item())),
        QueueItem::Feed(stub_feed_entry()),
    ];
    let cmd = CtrlCmd::UnifiedQueueReplace {
        slots: items
            .iter()
            .cloned()
            .enumerate()
            .map(|(index, item)| UnifiedQueueSlot {
                slot_id: (index + 7) as u64,
                item,
            })
            .collect(),
        items,
        start_idx: Some(0),
        source: QueueSource::Album,
    };
    let json = serde_json::to_string(&cmd).unwrap();
    let decoded: CtrlCmd = serde_json::from_str(&json).unwrap();
    match decoded {
        CtrlCmd::UnifiedQueueReplace {
            items,
            slots,
            start_idx,
            source,
        } => {
            assert_eq!(items.len(), 2);
            assert_eq!(source, QueueSource::Album);
            assert_eq!(
                slots.iter().map(|slot| slot.slot_id).collect::<Vec<_>>(),
                vec![7, 8]
            );
            assert_eq!(start_idx, Some(0));
        }
        _ => panic!("expected UnifiedQueueReplace"),
    }

    // A pre-slot-identity peer omitted the additive field and remains readable.
    let mut legacy: serde_json::Value = serde_json::from_str(&json).unwrap();
    legacy
        .get_mut("UnifiedQueueReplace")
        .and_then(serde_json::Value::as_object_mut)
        .unwrap()
        .remove("slots");
    legacy
        .get_mut("UnifiedQueueReplace")
        .and_then(serde_json::Value::as_object_mut)
        .unwrap()
        .remove("source");
    let CtrlCmd::UnifiedQueueReplace {
        items,
        slots,
        start_idx,
        source,
    } = serde_json::from_value(legacy).unwrap()
    else {
        panic!("expected legacy UnifiedQueueReplace")
    };
    assert_eq!(items.len(), 2);
    assert!(slots.is_empty());
    assert_eq!(start_idx, Some(0));
    assert_eq!(source, QueueSource::Unknown);
}

#[test]
fn idle_queue_load_and_result_round_trip_with_request_identity() {
    let command = CtrlCmd::UnifiedQueueLoadIdle {
        request_id: 77,
        slots: vec![UnifiedQueueSlot {
            slot_id: 12,
            item: QueueItem::Feed(stub_feed_entry()),
        }],
        cursor: 0,
        source: QueueSource::Album,
    };
    let decoded: CtrlCmd = serde_json::from_str(&serde_json::to_string(&command).unwrap()).unwrap();
    assert!(matches!(
        decoded,
        CtrlCmd::UnifiedQueueLoadIdle {
            request_id: 77,
            cursor: 0,
            source: QueueSource::Album,
            ..
        }
    ));

    let event = CtrlEvent::UnifiedQueueLoadResult {
        request_id: 77,
        result: QueueLoadResult::Rejected {
            reason: "unsupported".to_string(),
        },
    };
    let decoded: CtrlEvent = serde_json::from_str(&serde_json::to_string(&event).unwrap()).unwrap();
    assert!(
        matches!(decoded, CtrlEvent::UnifiedQueueLoadResult { request_id: 77, result: QueueLoadResult::Rejected { reason } } if reason == "unsupported")
    );
    let accepted = CtrlEvent::UnifiedQueueLoadResult {
        request_id: 78,
        result: QueueLoadResult::Accepted,
    };
    let decoded: CtrlEvent =
        serde_json::from_str(&serde_json::to_string(&accepted).unwrap()).unwrap();
    assert!(matches!(
        decoded,
        CtrlEvent::UnifiedQueueLoadResult {
            request_id: 78,
            result: QueueLoadResult::Accepted
        }
    ));
}

#[test]
fn source_only_update_round_trips_queue_lineage() {
    let command = CtrlCmd::UnifiedQueueSourceUpdate {
        source: QueueSource::Album,
        lineage: QueueLineage(42),
    };
    let decoded: CtrlCmd = serde_json::from_str(&serde_json::to_string(&command).unwrap()).unwrap();
    assert!(matches!(
        decoded,
        CtrlCmd::UnifiedQueueSourceUpdate {
            source: QueueSource::Album,
            lineage: QueueLineage(42)
        }
    ));
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
fn unified_queue_remove_slots_cmd_round_trips() {
    let cmd = CtrlCmd::UnifiedQueueRemoveSlots {
        slot_ids: vec![4, 9],
    };
    let json = serde_json::to_string(&cmd).unwrap();
    let decoded: CtrlCmd = serde_json::from_str(&json).unwrap();
    match decoded {
        CtrlCmd::UnifiedQueueRemoveSlots { slot_ids } => assert_eq!(slot_ids, vec![4, 9]),
        _ => panic!("expected UnifiedQueueRemoveSlots"),
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
        lineage: QueueLineage::default(),
        in_flight_transition: None,
        queued_latest_transition: None,
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
