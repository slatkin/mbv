fn feed_entry(guid: &str) -> FeedEntry {
    FeedEntry {
        guid: guid.into(),
        title: guid.into(),
        enclosure_url: None,
        link: None,
        mime_type: None,
        duration_ticks: None,
        pub_date_secs: None,
        feed_kind: Some(crate::config::FeedKind::Audio),
        feed_id: None,
        position_ticks: 0,
        played: false,
    }
}

#[test]
fn feed_slot_consumed_removes_from_canonical_queue_and_broadcasts() {
    let player = cold_player();
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let shared_queue = shared_queue_state();
    let queue = PlaybackQueue::from_queue_items(
        vec![
            QueueItem::Feed(feed_entry("feed-1")),
            QueueItem::Feed(feed_entry("feed-2")),
        ],
        Some(0),
    );
    let source = QueueSource::Unknown;

    // Complete feed-1 while no Client is attached. The owner, not a Client,
    // applies the configured consume policy to the canonical queue.
    let slot_id = queue
        .slots()
        .iter()
        .find(|s| s.item.id() == "feed-1")
        .map(|s| s.slot_id)
        .expect("feed-1 slot not found");
    let mut owner = PlayerOwnerState::new(queue, source.clone());
    assert!(owner.consume_completed_slot(slot_id, true, false, true));
    super::broadcast_queue_state(
        &registry,
        &player,
        &shared_queue,
        &owner.queue,
        &owner.source,
        &crate::playback_transition::OwnerTransitionState::default(),
    );

    // A later Client receives the shortened owner snapshot.
    let (_sender_id, sender_rx) = {
        let mut clients = registry.lock().unwrap();
        connect_client(&mut clients)
    };
    super::broadcast_queue_state(
        &registry,
        &player,
        &shared_queue,
        &owner.queue,
        &owner.source,
        &crate::playback_transition::OwnerTransitionState::default(),
    );
    assert_eq!(owner.queue.len(), 1);
    assert_eq!(owner.queue.slots()[0].item.id(), "feed-2");
    assert_eq!(shared_queue.queue.lock().unwrap().len(), 1);
    match recv_event(&sender_rx) {
        CtrlEvent::UnifiedQueueState(state) => {
            assert_eq!(state.slots.len(), 1);
            assert_eq!(state.slots[0].item.id(), "feed-2");
        }
        _ => panic!("expected unified queue state update"),
    }
}

#[test]
fn replace_queue_succeeds_unconditionally() {
    // With the canonical queue, there is no Feed tail guard — replace always
    // succeeds.
    let player = cold_player();
    let _player_cmd_rx = player.spy_on_commands();
    let client = Arc::new(Mutex::new(crate::api::EmbyClient::new(Config::default())));
    client.lock().unwrap().token = "test-token".into();
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (sender_id, _sender_rx) = {
        let mut clients = registry.lock().unwrap();
        connect_client(&mut clients)
    };
    let (reply_tx, _reply_rx) = mpsc::channel();
    let queue =
        PlaybackQueue::from_queue_items(vec![QueueItem::Feed(feed_entry("feed-1"))], Some(0));
    let source = QueueSource::Remote;
    let (dummy_merged_tx, _dummy_rx) = mpsc::channel::<DaemonEvent>();

    let mut owner = DaemonPlayerOwner { core: PlayerOwnerState::new(queue, source), ..Default::default() };
    handle_ctrl(
        CtrlCmd::UnifiedQueueReplace {
            items: vec![QueueItem::Emby(Box::new(item("replacement", "Video", "Movie")))],
            start_idx: Some(0),
        },
        sender_id,
        CtrlRequest {
            reply_tx: &reply_tx,
        },
        &client,
        &player,
        false,
        &mut owner,
        &shared_queue_state(),
        &registry,
        false,
        &dummy_merged_tx,
        false,
    );
    let queue = owner.core.queue;

    // Queue was replaced — Feed slot is gone, Emby item is present.
    assert_eq!(queue.len(), 1);
    assert_eq!(queue.slots()[0].item.id(), "replacement");
    assert!(matches!(queue.slots()[0].item, QueueItem::Emby(_)));
}
