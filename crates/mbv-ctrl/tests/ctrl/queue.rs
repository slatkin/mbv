use super::*;
use mbv_queue::{QueueItem, QueueLineage, QueueSource};

fn stub_media_item() -> mbv_emby_model::EmbyItem {
    mbv_emby_model::EmbyItem {
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
        parent_id: String::new(),
        path: String::new(),
        artist: String::new(),
        track_artist: String::new(),
        artist_items: Vec::new(),
        sort_name: String::new(),
        production_year: 0,
        end_year: 0,
        studios: Vec::new(),
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
        image_tags: mbv_emby_model::EmbyImageTags::default(),
    }
}

// ── Unified queue wire types ──────────────────────────────────────────────

fn stub_feed_entry() -> mbv_queue::FeedEntry {
    mbv_queue::FeedEntry {
        guid: "feed-guid-1".into(),
        title: "Episode 1".into(),
        enclosure_url: Some("https://example.com/ep1.mp3".into()),
        link: None,
        mime_type: Some("audio/mpeg".into()),
        duration_ticks: Some((3_600 * mbv_emby_model::TICKS_PER_SECOND) as u64),
        pub_date_secs: Some(1_700_000_000),
        feed_kind: Some(mbv_queue::FeedKind::Audio),
        feed_id: None,
        position_ticks: 0,
        played: false,
    }
}

// Contract: answered-queue-op support is additive — legacy peers' queue
// commands must keep deserializing, and the current hello advertises it. The
// corpus pins each command's canonical shape; this pins the missing-field
// decode tolerance no fixture row owns.
#[test]
fn unified_ctrl_behavior_is_capability_gated_and_additive() {
    assert!(CtrlHello::current().supports_answered_queue_ops());

    for legacy_json in [
        r#"{"UnifiedQueueReplace":{"items":[],"slots":[],"start_idx":null,"source":{"type":"unknown"}}}"#,
        r#"{"UnifiedQueueAppend":{"items":[]}}"#,
        r#"{"UnifiedQueueRemoveSlot":{"slot_id":1}}"#,
        r#"{"UnifiedQueueRemoveSlots":{"slot_ids":[1]}}"#,
        r#"{"UnifiedQueueMoveSlot":{"slot_id":1,"to_index":0}}"#,
        r#"{"UnifiedQueuePlaySlot":{"slot_id":1}}"#,
        r#"{"UnifiedQueueSourceUpdate":{"source":{"type":"unknown"},"lineage":1}}"#,
        r#""UnifiedQueueClear""#,
    ] {
        serde_json::from_str::<CtrlCmd>(legacy_json).unwrap();
    }
}

// A pre-transition peer's snapshot omits the transition fields (and
// `lineage`) entirely; they decode to their defaults — a decode tolerance no
// fixture row owns.
#[test]
fn unified_queue_state_data_decodes_without_transition_fields() {
    let mut legacy = serde_json::to_value(UnifiedQueueStateData {
        status: PlayerStatus::default(),
        slots: vec![UnifiedQueueSlot {
            slot_id: 1,
            item: QueueItem::Emby(Box::new(stub_media_item())),
        }],
        active_slot: Some(1),
        revision: 5,
        source: QueueSource::Unknown,
        lineage: QueueLineage(42),
        in_flight_transition: Some(mbv_ctrl::TransitionSummary {
            request_id: 7,
            generation: 3,
            target_slot: 2,
        }),
        queued_latest_transition: None,
    })
    .unwrap();
    let obj = legacy.as_object_mut().unwrap();
    obj.remove("in_flight_transition");
    obj.remove("queued_latest_transition");
    obj.remove("lineage");
    let decoded: UnifiedQueueStateData = serde_json::from_value(legacy).unwrap();
    assert_eq!(decoded.in_flight_transition, None);
    assert_eq!(decoded.queued_latest_transition, None);
    assert_eq!(decoded.lineage, QueueLineage::default());
}

// A pre-slot-identity peer omitted the additive `slots` and `source` fields;
// the command remains readable and both default — a decode tolerance no
// fixture row owns.
#[test]
fn unified_queue_replace_decodes_without_slot_identity() {
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
        start_position_secs: None,
        op: None,
    };
    let mut legacy: serde_json::Value =
        serde_json::from_str(&serde_json::to_string(&cmd).unwrap()).unwrap();
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
        op,
        ..
    } = serde_json::from_value(legacy).unwrap()
    else {
        panic!("expected legacy UnifiedQueueReplace")
    };
    assert_eq!(items.len(), 2);
    assert!(slots.is_empty());
    assert_eq!(start_idx, Some(0));
    assert_eq!(source, QueueSource::Unknown);
    assert_eq!(op, None);
}

// twitch-url-playback task 4.1: a Twitch item rides the existing unified
// queue shapes unchanged; the round trip pins the `Twitch` serde tag and the
// item's field shape on the command path (the conformance fixture pins the
// same shape as a corpus row).
#[test]
fn unified_queue_replace_round_trips_a_twitch_item() {
    let item = QueueItem::Twitch(
        mbv_queue::TwitchItem::from_url("https://twitch.tv/somechannel").unwrap(),
    );
    let cmd = CtrlCmd::UnifiedQueueReplace {
        items: vec![item.clone()],
        slots: vec![UnifiedQueueSlot {
            slot_id: 1,
            item: item.clone(),
        }],
        start_idx: Some(0),
        source: QueueSource::Unknown,
        start_position_secs: None,
        op: None,
    };

    let wire = serde_json::to_string(&cmd).unwrap();
    let decoded: CtrlCmd = serde_json::from_str(&wire).unwrap();

    // The decode re-encodes to the same wire bytes, so no field was dropped.
    assert_eq!(wire, serde_json::to_string(&decoded).unwrap());

    let CtrlCmd::UnifiedQueueReplace {
        items,
        slots,
        start_idx,
        ..
    } = decoded
    else {
        panic!("expected UnifiedQueueReplace")
    };
    assert_eq!(start_idx, Some(0));
    let [item] = items.as_slice() else {
        panic!("expected one item")
    };
    let QueueItem::Twitch(twitch) = item else {
        panic!("expected a Twitch item")
    };
    assert_eq!(twitch.url, "https://twitch.tv/somechannel");
    assert_eq!(twitch.title, "somechannel");
    assert_eq!(slots.len(), 1);
    assert_eq!(slots[0].slot_id, 1);
    assert!(matches!(&slots[0].item, QueueItem::Twitch(_)));
}

// queue-handoff-and-take task 2.1: `start_position_secs` is additive — a
// carried position round-trips, and a payload from a peer that predates the
// field decodes with `None` (design D4). The corpus pins the canonical
// populated shape; this pins the missing-field decode tolerance.
#[test]
fn unified_queue_replace_start_position_round_trips_and_a_missing_field_decodes_none() {
    let cmd = CtrlCmd::UnifiedQueueReplace {
        items: vec![],
        slots: vec![],
        start_idx: Some(2),
        source: QueueSource::Unknown,
        start_position_secs: Some(95.5),
        op: None,
    };
    let wire = serde_json::to_string(&cmd).unwrap();
    let CtrlCmd::UnifiedQueueReplace {
        start_idx,
        start_position_secs,
        ..
    } = serde_json::from_str(&wire).unwrap()
    else {
        panic!("expected UnifiedQueueReplace")
    };
    assert_eq!(start_idx, Some(2));
    assert_eq!(start_position_secs, Some(95.5));

    let stale: CtrlCmd = serde_json::from_str(
        r#"{"UnifiedQueueReplace":{"items":[],"slots":[],"start_idx":2,"source":{"type":"unknown"}}}"#,
    )
    .unwrap();
    let CtrlCmd::UnifiedQueueReplace {
        start_idx,
        start_position_secs,
        ..
    } = stale
    else {
        panic!("expected UnifiedQueueReplace")
    };
    assert_eq!(start_idx, Some(2));
    assert_eq!(start_position_secs, None);
}
