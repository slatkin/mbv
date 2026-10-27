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
    let CtrlCmd::UnifiedQueueShuffle { op } = decoded else {
        panic!("decoded to another command")
    };
    assert_eq!(op, None);
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

#[rstest::rstest]
#[case::remove_slot(CtrlCmd::UnifiedQueueRemoveSlot { slot_id: 1, op: None }, true)]
#[case::remove_slots(CtrlCmd::UnifiedQueueRemoveSlots { slot_ids: vec![1, 2], op: None }, true)]
#[case::move_slot(CtrlCmd::UnifiedQueueMoveSlot { slot_id: 1, to_index: 0, op: None }, true)]
#[case::shuffle(CtrlCmd::UnifiedQueueShuffle { op: None }, true)]
#[case::clear(CtrlCmd::UnifiedQueueClear, true)]
#[case::clear_op(CtrlCmd::UnifiedQueueClearOp { op: QueueOpId(1) }, true)]
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
#[case::play_slot(CtrlCmd::UnifiedQueuePlaySlot { slot_id: 1, op: None }, false)]
#[case::stop(CtrlCmd::Stop, false)]
#[case::shutdown(CtrlCmd::RequestShutdown, false)]
fn owner_persists_only_after_queue_edits(#[case] cmd: CtrlCmd, #[case] persists: bool) {
    assert_eq!(cmd.mutates_owner_queue(), persists);
}
