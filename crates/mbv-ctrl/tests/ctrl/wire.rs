use super::*;

// ctrl-protocol-conformance: the `protocol/ctrl/v10` corpus owns every wire
// shape (tag pins, encode bytes, decode round-trips); the tests below keep
// only the contracts no fixture row owns — additive-decode tolerances,
// fail-closed refusals, and behavioral predicates.

// A command with no ctrl wire form is refused fail-closed (design A2): the
// caller receives the unencodable command back, nothing is delivered, and the
// calling process survives — no unreachable!() abort.
#[test]
fn local_only_command_is_refused_without_wire_encoding() {
    let cmd = PlayerCommand::JumpTo {
        slot_id: mbv_queue::QueueSlotId::from_raw(3),
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
}

// queue-content-shuffle task 2.1: the shuffle command carries an optional
// queue operation id, so a peer that sends none still decodes and the owner
// answers it like any other queue edit. The corpus pins the canonical shape;
// this pins the missing-field decode tolerance.
#[test]
fn queue_shuffle_payload_without_op_decodes() {
    let decoded: CtrlCmd = serde_json::from_str(r#"{"UnifiedQueueShuffle":{}}"#).unwrap();
    let CtrlCmd::UnifiedQueueShuffle { op, .. } = decoded else {
        panic!("decoded to another command")
    };
    assert_eq!(op, None);
}

// gui-named-queues task 4.1: `queue` is additive on every queue edit — a
// payload from a peer that predates the field decodes with `None`, which
// means the current queue. The corpus pins the populated shape.
#[test]
fn queue_edit_payload_without_queue_decodes_as_the_current_queue() {
    let decoded: CtrlCmd =
        serde_json::from_str(r#"{"UnifiedQueueAppend":{"items":[],"before":null}}"#).unwrap();
    let CtrlCmd::UnifiedQueueAppend { queue, .. } = decoded else {
        panic!("decoded to another command")
    };
    assert_eq!(queue, None);
}

// The playback order left the wire (queue-content-shuffle task 5.2): a
// payload from a peer built before the change that still carries
// `play_order` decodes with the unknown field ignored.
#[test]
fn player_status_with_legacy_play_order_field_still_decodes() {
    let mut value = serde_json::to_value(PlayerStatus::default()).unwrap();
    value
        .as_object_mut()
        .unwrap()
        .insert("play_order".to_string(), serde_json::json!("Shuffle"));

    let decoded: PlayerStatus = serde_json::from_value(value).unwrap();
    assert!(!decoded.active);
    assert_eq!(decoded.queue_len, 0);
}

// gui-hero-design-polish task 1.4: the Emby person `id` and `primary_image_tag`
// are additive on the shared queue item payload (design D3); the conformance
// fixture pins the populated shape, and this pins the decode tolerance — a
// payload from a peer that predates the fields decodes both as empty.
#[test]
fn queue_item_person_payload_without_image_fields_decodes_empty() {
    use mbv_queue::QueueItem;

    let mut payload = serde_json::to_value(QueueItem::Emby(Box::new(stub_media_item()))).unwrap();
    payload["people"] = serde_json::json!([{"name": "Jane Doe", "role": "Lead", "kind": "Actor"}]);

    let decoded: QueueItem = serde_json::from_value(payload).unwrap();

    let QueueItem::Emby(emby) = decoded else {
        panic!("expected an Emby item")
    };
    assert_eq!(emby.people[0].id, "");
    assert_eq!(emby.people[0].primary_image_tag, "");
}

#[rstest::rstest]
#[case::remove_slot(CtrlCmd::UnifiedQueueRemoveSlot { slot_id: 1, op: None, queue: None }, true)]
#[case::remove_slots(CtrlCmd::UnifiedQueueRemoveSlots { slot_ids: vec![1, 2], op: None, queue: None }, true)]
#[case::move_slot(CtrlCmd::UnifiedQueueMoveSlot { slot_id: 1, to_index: 0, op: None, queue: None }, true)]
#[case::shuffle(CtrlCmd::UnifiedQueueShuffle { op: None, queue: None }, true)]
#[case::clear(CtrlCmd::UnifiedQueueClear, true)]
#[case::clear_op(CtrlCmd::UnifiedQueueClearOp { op: QueueOpId(1), queue: None }, true)]
#[case::refresh(CtrlCmd::UnifiedQueueRefresh { op: QueueOpId(1) }, true)]
#[case::apply_progress(
    CtrlCmd::UnifiedQueueApplyProgress {
        op: QueueOpId(1),
        updates: vec![ProgressUpdate {
            content_id: mbv_queue::QueueItemContentId::Feed("feed-id".into()),
            position_ticks: 42,
            finished: false,
        }],
    },
    true
)]
#[case::play_slot(CtrlCmd::UnifiedQueuePlaySlot { slot_id: 1, op: None, queue: None }, false)]
#[case::stop(CtrlCmd::Stop, false)]
#[case::shutdown(CtrlCmd::RequestShutdown, false)]
#[case::named_queue_show(
    CtrlCmd::NamedQueueShow {
        queue: mbv_queue::NamedQueueId::from_raw(2),
    },
    false
)]
fn owner_persists_only_after_queue_edits(#[case] cmd: CtrlCmd, #[case] persists: bool) {
    assert_eq!(cmd.mutates_owner_queue(), persists);
}

// switch-output-without-hand-off task 1.3: the new command's wire tag is
// pinned by an explicit `serde(rename)`, so a Rust-side rename cannot move it.
#[test]
fn switch_playback_output_tag_is_pinned() {
    let wire = serde_json::to_string(&CtrlCmd::SwitchPlaybackOutput(OutputSelection::Local))
        .expect("the command encodes");
    assert_eq!(wire, r#"{"SwitchPlaybackOutput":"Local"}"#);
}
