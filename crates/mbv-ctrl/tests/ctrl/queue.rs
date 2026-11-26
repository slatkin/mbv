use super::*;
use mbv_queue::{QueueItem, QueueLineage, QueueSource};

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
// `lineage`) entirely, and a peer without `named-queues` omits `queue` and
// `start_slot`; they decode to their defaults — a decode tolerance no fixture
// row owns (gui-named-queues task 4.2).
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
        queue: None,
        start_slot: None,
    })
    .unwrap();
    let obj = legacy.as_object_mut().unwrap();
    obj.remove("in_flight_transition");
    obj.remove("queued_latest_transition");
    obj.remove("lineage");
    obj.remove("queue");
    obj.remove("start_slot");
    let decoded: UnifiedQueueStateData = serde_json::from_value(legacy).unwrap();
    assert_eq!(decoded.in_flight_transition, None);
    assert_eq!(decoded.queued_latest_transition, None);
    assert_eq!(decoded.lineage, QueueLineage::default());
    assert_eq!(decoded.queue, None);
    assert_eq!(decoded.start_slot, None);
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
        queue: None,
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
        queue: None,
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

// gui-hero-design-polish task 1.4: the Emby person `id` and `primary_image_tag`
// are additive on the unified queue item payload (design D3) — a person with
// image identity round-trips unchanged through a queue command, and a payload
// from a peer that predates the fields decodes both as empty.
#[test]
fn unified_queue_replace_round_trips_person_image_fields_and_a_stale_payload_decodes_empty() {
    let mut item = stub_media_item();
    item.people = vec![mbv_emby_model::EmbyPerson {
        name: "Jane Doe".into(),
        role: "Lead".into(),
        kind: "Actor".into(),
        id: "person-9".into(),
        primary_image_tag: "tag-9".into(),
    }];
    let cmd = CtrlCmd::UnifiedQueueReplace {
        items: vec![QueueItem::Emby(Box::new(item.clone()))],
        slots: vec![UnifiedQueueSlot {
            slot_id: 1,
            item: QueueItem::Emby(Box::new(item)),
        }],
        start_idx: Some(0),
        source: QueueSource::Unknown,
        start_position_secs: None,
        op: None,
        queue: None,
    };

    let wire = serde_json::to_string(&cmd).unwrap();
    let decoded: CtrlCmd = serde_json::from_str(&wire).unwrap();

    // The decode re-encodes to the same wire bytes, so no field was dropped.
    assert_eq!(wire, serde_json::to_string(&decoded).unwrap());
    let CtrlCmd::UnifiedQueueReplace { items, slots, .. } = decoded else {
        panic!("expected UnifiedQueueReplace")
    };
    for carried in [items, slots.into_iter().map(|slot| slot.item).collect()] {
        let [QueueItem::Emby(emby)] = carried.as_slice() else {
            panic!("expected one Emby item")
        };
        assert_eq!(emby.people.len(), 1);
        assert_eq!(emby.people[0].id, "person-9");
        assert_eq!(emby.people[0].primary_image_tag, "tag-9");
    }

    // A payload from a peer that predates the fields decodes both as empty.
    let mut stale: serde_json::Value = serde_json::from_str(&wire).unwrap();
    for item in stale["UnifiedQueueReplace"]["items"]
        .as_array_mut()
        .unwrap()
    {
        let person = &mut item["people"][0];
        person.as_object_mut().unwrap().remove("id");
        person.as_object_mut().unwrap().remove("primary_image_tag");
    }
    let CtrlCmd::UnifiedQueueReplace { items, .. } = serde_json::from_value(stale).unwrap() else {
        panic!("expected UnifiedQueueReplace")
    };
    let [QueueItem::Emby(emby)] = items.as_slice() else {
        panic!("expected one Emby item")
    };
    assert_eq!(emby.people[0].id, "");
    assert_eq!(emby.people[0].primary_image_tag, "");
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
        queue: None,
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
