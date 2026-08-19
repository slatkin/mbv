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
    let (_sender_id, sender_rx) = {
        let mut clients = registry.lock().unwrap();
        connect_client(&mut clients)
    };
    let shared_queue = shared_queue_state();
    let mut queue = PlaybackQueue::from_queue_items(
        vec![
            QueueItem::Feed(feed_entry("feed-1")),
            QueueItem::Feed(feed_entry("feed-2")),
        ],
        Some(0),
    );
    let source = QueueSource::Unknown;

    // Find and consume the "feed-1" slot.
    let slot_id = queue
        .slots()
        .iter()
        .find(|s| s.item.id() == "feed-1")
        .map(|s| s.slot_id)
        .expect("feed-1 slot not found");
    match queue.consume_slot(slot_id) {
        crate::playback_queue::QueueMutationResult::Applied(_) => {}
        other => panic!("expected slot consumed, got {other:?}"),
    }
    super::broadcast_queue_state(&registry, &player, &shared_queue, &queue, &source);

    // Only the matching slot was removed.
    assert_eq!(queue.len(), 1);
    assert_eq!(queue.slots()[0].item.id(), "feed-2");
    // Reconnect snapshot updated.
    {
        let q = shared_queue.queue.lock().unwrap();
        assert_eq!(q.len(), 1);
    }
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
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (sender_id, _sender_rx) = {
        let mut clients = registry.lock().unwrap();
        connect_client(&mut clients)
    };
    let (reply_tx, _reply_rx) = mpsc::channel();
    let mut queue =
        PlaybackQueue::from_queue_items(vec![QueueItem::Feed(feed_entry("feed-1"))], Some(0));
    let mut source = QueueSource::Remote;
    let (dummy_merged_tx, _dummy_rx) = mpsc::channel::<DaemonEvent>();

    handle_ctrl(
        CtrlCmd::PlayerCmd(WireCommand::from(PlayerCommand::ReplaceQueue {
            items: vec![item("replacement", "Video", "Movie")],
            start_idx: 0,
        })),
        sender_id,
        CtrlRequest {
            reply_tx: &reply_tx,
        },
        &client,
        &player,
        false,
        &mut queue,
        &mut source,
        &shared_queue_state(),
        &registry,
        &mut PlaybackIntentState::default(),
        false,
        &dummy_merged_tx,
        false,
    );

    // Queue was replaced — Feed slot is gone, Emby item is present.
    assert_eq!(queue.len(), 1);
    assert_eq!(queue.slots()[0].item.id(), "replacement");
    assert!(matches!(queue.slots()[0].item, QueueItem::Emby(_)));
}

#[test]
fn queue_append_succeeds_unconditionally() {
    let player = cold_player();
    let client = Arc::new(Mutex::new(crate::api::EmbyClient::new(Config::default())));
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (sender_id, _sender_rx) = {
        let mut clients = registry.lock().unwrap();
        connect_client(&mut clients)
    };
    let (reply_tx, _reply_rx) = mpsc::channel();
    let mut queue =
        PlaybackQueue::from_queue_items(vec![QueueItem::Feed(feed_entry("feed-1"))], Some(0));
    let mut source = QueueSource::Remote;
    let (dummy_merged_tx, _dummy_rx) = mpsc::channel::<DaemonEvent>();

    handle_ctrl(
        CtrlCmd::PlayerCmd(WireCommand::from(PlayerCommand::QueueAppend {
            items: vec![QueueItem::Emby(Box::new(item(
                "appended", "Video", "Movie",
            )))],
        })),
        sender_id,
        CtrlRequest {
            reply_tx: &reply_tx,
        },
        &client,
        &player,
        false,
        &mut queue,
        &mut source,
        &shared_queue_state(),
        &registry,
        &mut PlaybackIntentState::default(),
        false,
        &dummy_merged_tx,
        false,
    );

    // Feed slot remains, Emby item appended.
    assert_eq!(queue.len(), 2);
    assert_eq!(queue.slots()[0].item.id(), "feed-1");
    assert_eq!(queue.slots()[1].item.id(), "appended");
}

#[test]
fn queue_move_succeeds_unconditionally() {
    let player = cold_player();
    let client = Arc::new(Mutex::new(crate::api::EmbyClient::new(Config::default())));
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (sender_id, _sender_rx) = {
        let mut clients = registry.lock().unwrap();
        connect_client(&mut clients)
    };
    let (reply_tx, _reply_rx) = mpsc::channel();
    let mut queue = PlaybackQueue::from_queue_items(
        vec![
            QueueItem::Feed(feed_entry("feed-1")),
            QueueItem::Feed(feed_entry("feed-2")),
        ],
        Some(0),
    );
    let mut source = QueueSource::Remote;
    let (dummy_merged_tx, _dummy_rx) = mpsc::channel::<DaemonEvent>();

    handle_ctrl(
        CtrlCmd::PlayerCmd(WireCommand::from(PlayerCommand::QueueMove(0, 1))),
        sender_id,
        CtrlRequest {
            reply_tx: &reply_tx,
        },
        &client,
        &player,
        false,
        &mut queue,
        &mut source,
        &shared_queue_state(),
        &registry,
        &mut PlaybackIntentState::default(),
        false,
        &dummy_merged_tx,
        false,
    );

    assert_eq!(
        queue
            .slots()
            .iter()
            .map(|s| s.item.id())
            .collect::<Vec<_>>(),
        vec!["feed-2", "feed-1"]
    );
}
