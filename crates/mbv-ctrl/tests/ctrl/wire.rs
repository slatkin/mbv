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

// gui-queue-panel-redesign task 2.1: `buffered_ticks` is additive (design
// D6) — a status that carries it round-trips the value.
#[test]
fn player_status_buffered_ticks_round_trips() {
    let status = PlayerStatus {
        buffered_ticks: 90_000_000,
        ..PlayerStatus::default()
    };

    let wire = serde_json::to_string(&status).unwrap();
    let decoded: PlayerStatus = serde_json::from_str(&wire).unwrap();
    assert_eq!(decoded.buffered_ticks, 90_000_000);
}

// The same delta: a status from an Owner that predates the field decodes with
// no buffered position known.
#[test]
fn player_status_without_buffered_ticks_decodes_as_unknown() {
    let mut value = serde_json::to_value(PlayerStatus::default()).unwrap();
    value
        .as_object_mut()
        .unwrap()
        .remove("buffered_ticks")
        .expect("a PlayerStatus serializes buffered_ticks");

    let decoded: PlayerStatus = serde_json::from_value(value).unwrap();
    assert_eq!(decoded.buffered_ticks, 0);
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
