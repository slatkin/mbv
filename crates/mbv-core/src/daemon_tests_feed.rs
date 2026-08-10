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

/// Connects a client with explicit capability flags.
fn connect_client_with_capability(
    clients: &mut CtrlClients,
    supports_feed_playback: bool,
    supports_unified_queue: bool,
) -> (u64, mpsc::Receiver<CtrlOutbound>) {
    let (tx, rx) = mpsc::channel();
    let id = clients.connect(
        tx,
        CtrlTransport::Local,
        supports_feed_playback,
        supports_unified_queue,
    );
    (id, rx)
}

#[test]
fn broadcast_gates_state_by_peer_capability() {
    // Unified-queue peers get `UnifiedQueueState`; legacy peers get `CtrlState`.
    let player = cold_player();
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (capable_rx, legacy_rx) = {
        let mut clients = registry.lock().unwrap();
        let (_capable_id, capable_rx) = connect_client_with_capability(&mut clients, true, true);
        let (_legacy_id, legacy_rx) = connect_client_with_capability(&mut clients, false, false);
        (capable_rx, legacy_rx)
    };
    let shared_queue = shared_queue_state();
    // Build a canonical queue with one Feed slot.
    let queue =
        PlaybackQueue::from_queue_items(vec![QueueItem::Feed(feed_entry("feed-1"))], Some(0));

    super::broadcast_queue_state(
        &registry,
        &player,
        &shared_queue,
        &queue,
        &QueueSource::Unknown,
    );

    // Unified-queue peer receives UnifiedQueueState.
    match recv_event(&capable_rx) {
        CtrlEvent::UnifiedQueueState(state) => {
            assert_eq!(state.slots.len(), 1);
            assert_eq!(state.slots[0].item.id(), "feed-1");
        }
        _other => panic!("expected UnifiedQueueState, got different variant"),
    }
    // Legacy peer receives CtrlState with feed_items empty (no feed-playback).
    match recv_event(&legacy_rx) {
        CtrlEvent::State(state) => {
            assert!(state.feed_items.is_empty());
            assert_eq!(state.items.len(), 0);
        }
        _ => panic!("expected queue state update"),
    }
}

#[test]
fn ctrl_load_feed_adds_to_canonical_queue_and_starts_playback() {
    let player = cold_player();
    let player_cmd_rx = player.spy_on_commands();
    let client = Arc::new(Mutex::new(crate::api::EmbyClient::new(Config::default())));
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (sender_id, _sender_rx) = {
        let mut clients = registry.lock().unwrap();
        connect_client(&mut clients)
    };
    let (reply_tx, _reply_rx) = mpsc::channel();
    let shared_queue = shared_queue_state();
    let mut queue = PlaybackQueue::default();
    let mut source = QueueSource::Unknown;
    let entry = FeedEntry {
        guid: "feed-1".into(),
        title: "Episode 1".into(),
        enclosure_url: Some("https://example.com/ep1.mp3".into()),
        link: None,
        mime_type: Some("audio/mpeg".into()),
        duration_ticks: Some(3_600_000_000),
        pub_date_secs: None,
        feed_kind: Some(crate::config::FeedKind::Audio),
        feed_id: None,
        position_ticks: 0,
        played: false,
    };
    let (dummy_merged_tx, _dummy_rx) = mpsc::channel::<DaemonEvent>();

    handle_ctrl(
        CtrlCmd::PlayerCmd(WireCommand::LoadFeed {
            entry: entry.clone(),
        }),
        sender_id,
        CtrlRequest {
            reply_tx: &reply_tx,
        },
        &client,
        &player,
        false,
        &mut queue,
        &mut source,
        &shared_queue,
        &registry,
        &mut PlaybackIntentState::default(),
        None,
        &dummy_merged_tx,
    );

    // Canonical queue has the Feed slot.
    assert_eq!(queue.len(), 1);
    assert!(matches!(queue.slots()[0].item, QueueItem::Feed(_)));
    assert_eq!(queue.slots()[0].item.id(), "feed-1");
    // SubmitQueue was sent to the player.
    assert!(matches!(
        player_cmd_rx.try_recv(),
        Ok(PlayerCommand::SubmitQueue { .. })
    ));
    // Reconnect snapshot reflects the canonical queue.
    {
        let q = shared_queue.queue.lock().unwrap();
        assert_eq!(q.len(), 1);
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
fn adopt_queue_succeeds_when_queue_empty() {
    let player = cold_player();
    let client = Arc::new(Mutex::new(crate::api::EmbyClient::new(Config::default())));
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (sender_id, _sender_rx) = {
        let mut clients = registry.lock().unwrap();
        connect_client(&mut clients)
    };
    let (reply_tx, _reply_rx) = mpsc::channel();
    let mut queue = PlaybackQueue::default();
    let mut source = QueueSource::Unknown;
    let (dummy_merged_tx, _dummy_rx) = mpsc::channel::<DaemonEvent>();

    handle_ctrl(
        CtrlCmd::AdoptQueue {
            items: vec![item("adopted", "Video", "Movie")],
            cursor: 0,
            source: QueueSource::Remote,
        },
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
        None,
        &dummy_merged_tx,
    );

    assert_eq!(queue.len(), 1);
    assert_eq!(queue.slots()[0].item.id(), "adopted");
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
        None,
        &dummy_merged_tx,
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
        None,
        &dummy_merged_tx,
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
        None,
        &dummy_merged_tx,
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
