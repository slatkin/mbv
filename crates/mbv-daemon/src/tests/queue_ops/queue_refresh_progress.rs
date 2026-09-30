use super::*;

#[test]
fn unified_playback_queue_refresh_is_answered_at_once() {
    let player = cold_player();
    let client = queue_op_client("test-token");
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (client_id, reply_tx, reply_rx) =
        super::queue_mutations::connect_op_requester(&mut registry.lock().unwrap());
    let mut owner = owner_with(Vec::new(), 0);

    run_queue_cmd(
        CtrlCmd::UnifiedQueueRefresh {
            op: mbv_ctrl::QueueOpId(46),
        },
        client_id,
        &reply_tx,
        &client,
        &player,
        &mut owner,
        &registry,
    );

    assert!(matches!(
        recv_event(&reply_rx),
        CtrlEvent::QueueOpResult {
            op: mbv_ctrl::QueueOpId(46),
            outcome: mbv_ctrl::QueueOpOutcome::Applied(state),
        } if state.slots.is_empty()
    ));
}

#[test]
fn unified_playback_queue_apply_progress_skips_active_slot() {
    let player = cold_player();
    let client = queue_op_client("test-token");
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (client_id, reply_tx, reply_rx) =
        super::queue_mutations::connect_op_requester(&mut registry.lock().unwrap());
    let (_other_id, other_rx) = connect_client(&mut registry.lock().unwrap());
    let mut owner = owner_with(vec![abs_qi("li_1", "ep_1"), abs_qi("li_1", "ep_1")], 0);
    let active_slot = owner.core.queue.slots()[0].slot_id;
    let inactive_slot = owner.core.queue.slots()[1].slot_id;
    let _ = owner.core.queue.apply_progress(active_slot, 11, false);

    run_queue_cmd(
        CtrlCmd::UnifiedQueueApplyProgress {
            op: mbv_ctrl::QueueOpId(47),
            updates: vec![mbv_ctrl::ProgressUpdate {
                content_id: mbv_queue::QueueItemContentId::Audiobookshelf {
                    library_item_id: "li_1".into(),
                    episode_id: "ep_1".into(),
                },
                position_ticks: 42,
                finished: true,
            }],
        },
        client_id,
        &reply_tx,
        &client,
        &player,
        &mut owner,
        &registry,
    );

    let active = owner
        .core
        .queue
        .slot(active_slot)
        .unwrap()
        .item
        .as_audiobookshelf()
        .unwrap();
    let inactive = owner
        .core
        .queue
        .slot(inactive_slot)
        .unwrap()
        .item
        .as_audiobookshelf()
        .unwrap();
    assert_eq!(active.position_ticks, 11);
    assert!(!active.is_finished);
    assert_eq!(inactive.position_ticks, 42);
    assert!(inactive.is_finished);
    assert!(matches!(
        recv_event(&reply_rx),
        CtrlEvent::QueueOpResult {
            op: mbv_ctrl::QueueOpId(47),
            outcome: mbv_ctrl::QueueOpOutcome::Applied(_),
        }
    ));
    assert!(matches!(
        recv_event(&other_rx),
        CtrlEvent::UnifiedQueueState(state)
            if state.slots[1].item.as_audiobookshelf().unwrap().position_ticks == 42
    ));
}
