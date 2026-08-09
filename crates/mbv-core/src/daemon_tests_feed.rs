fn feed_entry(guid: &str) -> FeedEntry {
    FeedEntry {
        guid: guid.into(),
        title: guid.into(),
        enclosure_url: None,
        link: None,
        mime_type: None,
        duration_ticks: None,
        pub_date_secs: None,
    }
}

/// Connects a client with an explicit `feed-playback` capability, unlike
/// `connect_client` (which always connects a capable client). Used to
/// exercise the #5.1 per-client gating of the Feed tail.
fn connect_client_with_capability(
    clients: &mut CtrlClients,
    supports_feed_playback: bool,
) -> (u64, mpsc::Receiver<CtrlOutbound>) {
    let (tx, rx) = mpsc::channel();
    let id = clients.connect(tx, CtrlTransport::Local, supports_feed_playback);
    (id, rx)
}

#[test]
fn broadcast_gates_feed_items_by_peer_capability() {
    // #5.1: a capable peer's broadcast carries the real Feed tail; a legacy
    // peer (Hello without feed-playback) must receive an empty one.
    let player = cold_player();
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (capable_rx, legacy_rx) = {
        let mut clients = registry.lock().unwrap();
        let (_capable_id, capable_rx) = connect_client_with_capability(&mut clients, true);
        let (_legacy_id, legacy_rx) = connect_client_with_capability(&mut clients, false);
        (capable_rx, legacy_rx)
    };
    let shared_queue = shared_queue_state();
    let feed_items = vec![feed_entry("feed-1")];

    super::broadcast_queue_state(
        &registry,
        &player,
        &shared_queue,
        &[],
        0,
        &QueueSource::Unknown,
        &feed_items,
    );

    match recv_event(&capable_rx) {
        CtrlEvent::State(state) => {
            assert_eq!(
                state
                    .feed_items
                    .iter()
                    .map(|e| e.guid.as_str())
                    .collect::<Vec<_>>(),
                vec!["feed-1"]
            );
        }
        _ => panic!("expected queue state update"),
    }
    match recv_event(&legacy_rx) {
        CtrlEvent::State(state) => {
            assert!(state.feed_items.is_empty());
        }
        _ => panic!("expected queue state update"),
    }
}

#[test]
fn adopt_queue_rejected_when_feed_tail_active() {
    // #5.3: a nonempty Feed tail makes AdoptQueue ambiguous for capable
    // clients addressing the mixed Emby-then-Feed queue, so it's rejected.
    let player = cold_player();
    let player_cmd_rx = player.spy_on_commands();
    let client = Arc::new(Mutex::new(crate::api::EmbyClient::new(Config::default())));
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (_sender_id, _sender_rx) = {
        let mut clients = registry.lock().unwrap();
        connect_client(&mut clients)
    };
    let (reply_tx, reply_rx) = mpsc::channel();
    let mut items = Vec::new();
    let mut cursor = 0;
    let mut source = QueueSource::Unknown;
    let mut feed_items = vec![feed_entry("feed-1")];
    let (dummy_merged_tx, _dummy_rx) = mpsc::channel::<DaemonEvent>();

    handle_ctrl(
        CtrlCmd::AdoptQueue {
            items: vec![item("adopted", "Video", "Movie")],
            cursor: 0,
            source: QueueSource::Unknown,
        },
        1,
        CtrlRequest {
            reply_tx: &reply_tx,
        },
        &client,
        &player,
        false,
        &mut items,
        &mut cursor,
        &mut source,
        &mut feed_items,
        &shared_queue_state(),
        &registry,
        &mut PlaybackIntentState::default(),
        None,
        &dummy_merged_tx,
    );

    assert!(items.is_empty());
    assert_eq!(
        feed_items
            .iter()
            .map(|e| e.guid.as_str())
            .collect::<Vec<_>>(),
        vec!["feed-1"]
    );
    assert!(player_cmd_rx.try_recv().is_err());
    match recv_event(&reply_rx) {
        CtrlEvent::CommandRejected(reason) => {
            assert_eq!(reason, "queue has an active Feed tail; adoption skipped");
        }
        _ => panic!("expected command rejection"),
    }
}

#[test]
fn ctrl_load_feed_records_entry_in_tail_and_starts_playback() {
    // Settled design decision (#5.2, no owner/availability rejection): the
    // daemon always records LoadFeed into its Feed tail and uses the
    // lifecycle-capable play_feed path (not a bare send_command) so a cold
    // daemon actually starts mpv.  The entry must have a playable source
    // for play_feed to proceed past validation.
    let player = cold_player();
    let client = Arc::new(Mutex::new(crate::api::EmbyClient::new(Config::default())));
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (_sender_id, sender_rx) = {
        let mut clients = registry.lock().unwrap();
        connect_client(&mut clients)
    };
    let (reply_tx, _reply_rx) = mpsc::channel();
    let shared_queue = shared_queue_state();
    let mut items = Vec::new();
    let mut cursor = 0;
    let mut source = QueueSource::Unknown;
    let mut feed_items = Vec::new();
    let entry = FeedEntry {
        guid: "feed-1".into(),
        title: "Episode 1".into(),
        enclosure_url: Some("https://example.com/ep1.mp3".into()),
        link: None,
        mime_type: Some("audio/mpeg".into()),
        duration_ticks: Some(3_600_000_000),
        pub_date_secs: None,
    };
    let (dummy_merged_tx, _dummy_rx) = mpsc::channel::<DaemonEvent>();

    handle_ctrl(
        CtrlCmd::PlayerCmd(WireCommand::from(PlayerCommand::LoadFeed {
            entry: entry.clone(),
        })),
        1,
        CtrlRequest {
            reply_tx: &reply_tx,
        },
        &client,
        &player,
        false,
        &mut items,
        &mut cursor,
        &mut source,
        &mut feed_items,
        &shared_queue,
        &registry,
        &mut PlaybackIntentState::default(),
        None,
        &dummy_merged_tx,
    );

    // play_feed (not send_command) was used: the player status reflects
    // the feed entry being loaded, proving the lifecycle path ran.
    {
        let st = player.status.lock().unwrap();
        assert!(st.active, "play_feed must activate the player");
        assert_eq!(st.title, "Episode 1");
        assert_eq!(st.current_idx, 0);
        assert_eq!(st.queue_len, 1);
    }
    assert_eq!(
        feed_items
            .iter()
            .map(|e| e.guid.as_str())
            .collect::<Vec<_>>(),
        vec!["feed-1"]
    );
    assert_eq!(
        shared_queue
            .feed_items
            .lock()
            .unwrap()
            .iter()
            .map(|e| e.guid.as_str())
            .collect::<Vec<_>>(),
        vec!["feed-1"]
    );
    match recv_event(&sender_rx) {
        CtrlEvent::State(state) => {
            assert_eq!(
                state
                    .feed_items
                    .iter()
                    .map(|e| e.guid.as_str())
                    .collect::<Vec<_>>(),
                vec!["feed-1"]
            );
        }
        _ => panic!("expected queue state update"),
    }
}

#[test]
fn feed_consumed_removes_matching_entry_from_tail_and_broadcasts_state() {
    // Mirrors design.md: "A player Feed-removal event updates the daemon's
    // tail and the reconnect snapshot before it broadcasts the next state."
    let player = cold_player();
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (_sender_id, sender_rx) = {
        let mut clients = registry.lock().unwrap();
        connect_client(&mut clients)
    };
    let shared_queue = shared_queue_state();
    let items = Vec::new();
    let cursor = 0;
    let source = QueueSource::Unknown;
    let mut feed_items = vec![feed_entry("feed-1"), feed_entry("feed-2")];

    handle_feed_consumed(
        "feed-1",
        &registry,
        &player,
        &shared_queue,
        &items,
        cursor,
        &source,
        &mut feed_items,
    );

    assert_eq!(
        feed_items
            .iter()
            .map(|e| e.guid.as_str())
            .collect::<Vec<_>>(),
        vec!["feed-2"]
    );
    assert_eq!(
        shared_queue
            .feed_items
            .lock()
            .unwrap()
            .iter()
            .map(|e| e.guid.as_str())
            .collect::<Vec<_>>(),
        vec!["feed-2"]
    );
    match recv_event(&sender_rx) {
        CtrlEvent::State(state) => {
            assert_eq!(
                state
                    .feed_items
                    .iter()
                    .map(|e| e.guid.as_str())
                    .collect::<Vec<_>>(),
                vec!["feed-2"]
            );
        }
        _ => panic!("expected queue state update"),
    }
}

#[test]
fn adopt_queue_succeeds_when_feed_tail_empty() {
    // Baseline for the #5.3 guard: with no Feed tail, AdoptQueue against a
    // cold daemon still succeeds exactly as before.
    let player = cold_player();
    let client = Arc::new(Mutex::new(crate::api::EmbyClient::new(Config::default())));
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (_sender_id, _sender_rx) = {
        let mut clients = registry.lock().unwrap();
        connect_client(&mut clients)
    };
    let (reply_tx, _reply_rx) = mpsc::channel();
    let mut items = Vec::new();
    let mut cursor = 0;
    let mut source = QueueSource::Unknown;
    let (dummy_merged_tx, _dummy_rx) = mpsc::channel::<DaemonEvent>();

    handle_ctrl(
        CtrlCmd::AdoptQueue {
            items: vec![item("adopted", "Video", "Movie")],
            cursor: 0,
            source: QueueSource::Remote,
        },
        1,
        CtrlRequest {
            reply_tx: &reply_tx,
        },
        &client,
        &player,
        false,
        &mut items,
        &mut cursor,
        &mut source,
        &mut Vec::new(),
        &shared_queue_state(),
        &registry,
        &mut PlaybackIntentState::default(),
        None,
        &dummy_merged_tx,
    );

    assert_eq!(
        items.iter().map(|i| i.id.as_str()).collect::<Vec<_>>(),
        vec!["adopted"]
    );
}

#[test]
fn replace_queue_rejected_when_feed_tail_active() {
    // #5.3: see the AdoptQueue guard rationale.
    let player = cold_player();
    let player_cmd_rx = player.spy_on_commands();
    let client = Arc::new(Mutex::new(crate::api::EmbyClient::new(Config::default())));
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (_sender_id, _sender_rx) = {
        let mut clients = registry.lock().unwrap();
        connect_client(&mut clients)
    };
    let (reply_tx, reply_rx) = mpsc::channel();
    let mut items = vec![item("existing", "Video", "Movie")];
    let mut cursor = 0;
    let mut source = QueueSource::Remote;
    let mut feed_items = vec![feed_entry("feed-1")];
    let (dummy_merged_tx, _dummy_rx) = mpsc::channel::<DaemonEvent>();

    handle_ctrl(
        CtrlCmd::PlayerCmd(WireCommand::from(PlayerCommand::ReplaceQueue {
            items: vec![item("replacement", "Video", "Movie")],
            start_idx: 0,
        })),
        1,
        CtrlRequest {
            reply_tx: &reply_tx,
        },
        &client,
        &player,
        false,
        &mut items,
        &mut cursor,
        &mut source,
        &mut feed_items,
        &shared_queue_state(),
        &registry,
        &mut PlaybackIntentState::default(),
        None,
        &dummy_merged_tx,
    );

    assert_eq!(
        items.iter().map(|i| i.id.as_str()).collect::<Vec<_>>(),
        vec!["existing"]
    );
    assert!(player_cmd_rx.try_recv().is_err());
    match recv_event(&reply_rx) {
        CtrlEvent::CommandRejected(reason) => {
            assert_eq!(reason, "queue has an active Feed tail; replace skipped");
        }
        _ => panic!("expected command rejection"),
    }
}

#[test]
fn replace_queue_succeeds_when_feed_tail_empty() {
    // Baseline for the #5.3 guard: with no Feed tail, ReplaceQueue succeeds
    // exactly as before.
    let player = cold_player();
    let player_cmd_rx = player.spy_on_commands();
    let client = Arc::new(Mutex::new(crate::api::EmbyClient::new(Config::default())));
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (_sender_id, _sender_rx) = {
        let mut clients = registry.lock().unwrap();
        connect_client(&mut clients)
    };
    let (reply_tx, _reply_rx) = mpsc::channel();
    let mut items = vec![item("existing", "Video", "Movie")];
    let mut cursor = 0;
    let mut source = QueueSource::Remote;
    let (dummy_merged_tx, _dummy_rx) = mpsc::channel::<DaemonEvent>();

    handle_ctrl(
        CtrlCmd::PlayerCmd(WireCommand::from(PlayerCommand::ReplaceQueue {
            items: vec![item("replacement", "Video", "Movie")],
            start_idx: 0,
        })),
        1,
        CtrlRequest {
            reply_tx: &reply_tx,
        },
        &client,
        &player,
        false,
        &mut items,
        &mut cursor,
        &mut source,
        &mut Vec::new(),
        &shared_queue_state(),
        &registry,
        &mut PlaybackIntentState::default(),
        None,
        &dummy_merged_tx,
    );

    assert_eq!(
        items.iter().map(|i| i.id.as_str()).collect::<Vec<_>>(),
        vec!["replacement"]
    );
    assert!(matches!(
        player_cmd_rx.try_recv(),
        Ok(PlayerCommand::ReplaceQueue { .. })
    ));
}

#[test]
fn queue_append_rejected_when_feed_tail_active() {
    // #5.3: see the AdoptQueue guard rationale.
    let player = cold_player();
    let player_cmd_rx = player.spy_on_commands();
    let client = Arc::new(Mutex::new(crate::api::EmbyClient::new(Config::default())));
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (_sender_id, _sender_rx) = {
        let mut clients = registry.lock().unwrap();
        connect_client(&mut clients)
    };
    let (reply_tx, reply_rx) = mpsc::channel();
    let mut items = vec![item("existing", "Video", "Movie")];
    let mut cursor = 0;
    let mut source = QueueSource::Remote;
    let mut feed_items = vec![feed_entry("feed-1")];
    let (dummy_merged_tx, _dummy_rx) = mpsc::channel::<DaemonEvent>();

    handle_ctrl(
        CtrlCmd::PlayerCmd(WireCommand::from(PlayerCommand::QueueAppend {
            items: vec![item("appended", "Video", "Movie")],
        })),
        1,
        CtrlRequest {
            reply_tx: &reply_tx,
        },
        &client,
        &player,
        false,
        &mut items,
        &mut cursor,
        &mut source,
        &mut feed_items,
        &shared_queue_state(),
        &registry,
        &mut PlaybackIntentState::default(),
        None,
        &dummy_merged_tx,
    );

    assert_eq!(
        items.iter().map(|i| i.id.as_str()).collect::<Vec<_>>(),
        vec!["existing"]
    );
    assert!(player_cmd_rx.try_recv().is_err());
    match recv_event(&reply_rx) {
        CtrlEvent::CommandRejected(reason) => {
            assert_eq!(reason, "queue has an active Feed tail; append skipped");
        }
        _ => panic!("expected command rejection"),
    }
}

#[test]
fn queue_move_rejected_when_feed_tail_active() {
    // #5.3: see the AdoptQueue guard rationale.
    let player = cold_player();
    let player_cmd_rx = player.spy_on_commands();
    let client = Arc::new(Mutex::new(crate::api::EmbyClient::new(Config::default())));
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (_sender_id, _sender_rx) = {
        let mut clients = registry.lock().unwrap();
        connect_client(&mut clients)
    };
    let (reply_tx, reply_rx) = mpsc::channel();
    let mut items = vec![
        item("item-0", "Video", "Movie"),
        item("item-1", "Video", "Movie"),
    ];
    let mut cursor = 0;
    let mut source = QueueSource::Remote;
    let mut feed_items = vec![feed_entry("feed-1")];
    let (dummy_merged_tx, _dummy_rx) = mpsc::channel::<DaemonEvent>();

    handle_ctrl(
        CtrlCmd::PlayerCmd(WireCommand::from(PlayerCommand::QueueMove(0, 1))),
        1,
        CtrlRequest {
            reply_tx: &reply_tx,
        },
        &client,
        &player,
        false,
        &mut items,
        &mut cursor,
        &mut source,
        &mut feed_items,
        &shared_queue_state(),
        &registry,
        &mut PlaybackIntentState::default(),
        None,
        &dummy_merged_tx,
    );

    assert_eq!(
        items.iter().map(|i| i.id.as_str()).collect::<Vec<_>>(),
        vec!["item-0", "item-1"]
    );
    assert!(player_cmd_rx.try_recv().is_err());
    match recv_event(&reply_rx) {
        CtrlEvent::CommandRejected(reason) => {
            assert_eq!(reason, "queue has an active Feed tail; move skipped");
        }
        _ => panic!("expected command rejection"),
    }
}
