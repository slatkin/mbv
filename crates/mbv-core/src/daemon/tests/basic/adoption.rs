use super::*;

#[test]
fn cold_ctrl_player_command_keeps_connection_as_driver() {
    let player = cold_player();
    let client = Arc::new(Mutex::new(crate::api::EmbyClient::new(Config::default())));
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (_sender_id, sender_rx) = {
        let mut clients = registry.lock().unwrap();
        connect_client(&mut clients)
    };
    let (reply_tx, _reply_rx) = mpsc::channel();
    let queue = PlaybackQueue::default();
    let source = QueueSource::Unknown;
    let (dummy_merged_tx, _dummy_rx) = mpsc::channel::<DaemonEvent>();

    let mut owner = DaemonPlayerOwner {
        core: PlayerOwnerState::new(queue, source),
        ..Default::default()
    };
    handle_ctrl_for_role(
        CtrlCmd::PlayerCmd(
            WireCommand::try_from_player_command(PlayerCommand::TogglePause).unwrap(),
        ),
        CtrlContext {
            reply_tx: &reply_tx,
            client_id: 1,
            client: &client,
            player: &player,
            audio_only: false,
            owner: &mut owner,
            shared_queue: &shared_queue_state(),
            ctrl_clients: &registry,
            has_audiobookshelf: false,
            merged_tx: &dummy_merged_tx,
            stay_alive: false,
            role: crate::daemon::DaemonRole::Local,
        },
    );
    let _queue = owner.core.queue;

    assert!(registry.lock().unwrap().has_driver());
    assert!(sender_rx.try_recv().is_err());
}

#[test]
fn unified_adopt_queue_seeds_status_without_starting_playback_when_cold() {
    let player = cold_player();
    let player_cmd_rx = player.spy_on_commands();
    let http = MockHttp::new();
    http.fail(std::io::ErrorKind::ConnectionRefused);
    let mut client = EmbyClient::new(Config {
        server_url: "http://127.0.0.1:1".into(),
        ..Config::default()
    })
    .with_test_agent(http.agent());
    client.token = "test-token".to_string();
    client.user_id = "test-user".to_string();
    let client = Arc::new(Mutex::new(client));
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (reply_tx, _reply_rx) = mpsc::channel();
    let queue = PlaybackQueue::default();
    let source = QueueSource::Unknown;
    let (dummy_merged_tx, dummy_merged_rx) = mpsc::channel::<DaemonEvent>();

    let mut owner = DaemonPlayerOwner {
        core: PlayerOwnerState::new(queue, source),
        ..Default::default()
    };
    handle_ctrl_for_role(
        CtrlCmd::UnifiedAdoptQueue {
            items: vec![emby_qi("adopted", "Video", "Movie")],
            cursor: 0,
            source: QueueSource::Remote,
        },
        CtrlContext {
            reply_tx: &reply_tx,
            client_id: 1,
            client: &client,
            player: &player,
            audio_only: false,
            owner: &mut owner,
            shared_queue: &shared_queue_state(),
            ctrl_clients: &registry,
            has_audiobookshelf: false,
            merged_tx: &dummy_merged_tx,
            stay_alive: false,
            role: crate::daemon::DaemonRole::Packaged,
        },
    );
    assert!(
        dummy_merged_rx.try_recv().is_err(),
        "UnifiedAdoptQueue must return without waiting for enrichment"
    );
    let queue = owner.core.queue;

    assert_eq!(queue.len(), 1);
    assert_eq!(queue.slots()[0].item.id(), "adopted");
    assert!(!player.status.lock().unwrap().active);
    player_cmd_rx.try_recv().unwrap_err();
}

#[test]
fn adopted_queue_enrichment_updates_canonical_queue_and_broadcasts() {
    let player = cold_player();
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (_client_id, client_rx) = {
        let mut clients = registry.lock().unwrap();
        connect_client(&mut clients)
    };
    let shared_queue = shared_queue_state();
    let mut owner = owner_with(
        vec![
            emby_qi("adopted", "Video", "Movie"),
            emby_qi("next", "Video", "Movie"),
        ],
        0,
    );
    let adopted_slot = owner.core.queue.slots()[0].slot_id;
    let next_slot = owner.core.queue.slots()[1].slot_id;
    let mut adopted = item("adopted", "Video", "Movie");
    adopted.playback_position_ticks = 40_000_000;
    let mut next = item("next", "Video", "Movie");
    next.playback_position_ticks = 20_000_000;

    apply_queue_enriched(
        vec![(adopted_slot, adopted), (next_slot, next)],
        &mut owner,
        &player,
        &shared_queue,
        &registry,
    );

    let refreshed = recv_event(&client_rx);
    let position = match refreshed {
        CtrlEvent::UnifiedQueueState(state) => {
            state.slots[1]
                .item
                .as_emby()
                .unwrap()
                .playback_position_ticks
        }
        _ => panic!("expected refreshed queue state"),
    };
    assert_eq!(position, 20_000_000);
    assert_eq!(
        owner.core.queue.slots()[1]
            .item
            .as_emby()
            .unwrap()
            .playback_position_ticks,
        20_000_000
    );
}

fn apply_adopted_refresh_positions(
    stored_position: i64,
    fetched_position: i64,
    fetched_played: bool,
) -> (DaemonPlayerOwner, mpsc::Receiver<CtrlOutbound>) {
    let player = cold_player();
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (_client_id, client_rx) = {
        let mut clients = registry.lock().unwrap();
        connect_client(&mut clients)
    };
    let shared_queue = shared_queue_state();
    let mut owner = owner_with(
        vec![
            emby_qi("stored", "Video", "Movie"),
            emby_qi("broadcast", "Video", "Movie"),
            emby_qi("active", "Video", "Movie"),
        ],
        2,
    );
    let stored_slot = owner.core.queue.slots()[0].slot_id;
    let broadcast_slot = owner.core.queue.slots()[1].slot_id;
    let mut stored = item("stored", "Video", "Movie");
    stored.playback_position_ticks = stored_position;
    let _ = owner
        .core
        .queue
        .update_slot_item(stored_slot, QueueItem::Emby(Box::new(stored)));
    let mut fetched = item("stored", "Video", "Movie");
    fetched.playback_position_ticks = fetched_position;
    fetched.played = fetched_played;
    let mut broadcast = item("broadcast", "Video", "Movie");
    broadcast.playback_position_ticks = 1;

    apply_queue_enriched(
        vec![(stored_slot, fetched), (broadcast_slot, broadcast)],
        &mut owner,
        &player,
        &shared_queue,
        &registry,
    );

    (owner, client_rx)
}

#[rstest]
#[case::unplayed_lower_position(40_000_000, 0, false, 40_000_000, false)]
#[case::unplayed_newer_position(10_000_000, 20_000_000, false, 20_000_000, false)]
#[case::played_reset(40_000_000, 0, true, 0, true)]
fn adopted_refresh_merges_positions_with_played_reset_authority(
    #[case] stored_position: i64,
    #[case] fetched_position: i64,
    #[case] fetched_played: bool,
    #[case] expected_position: i64,
    #[case] expected_played: bool,
) {
    let (owner, client_rx) =
        apply_adopted_refresh_positions(stored_position, fetched_position, fetched_played);

    let refreshed = recv_event(&client_rx);
    let CtrlEvent::UnifiedQueueState(state) = refreshed else {
        panic!("expected refreshed queue state");
    };
    let slot = &owner.core.queue.slots()[0];
    let emby = slot.item.as_emby().unwrap();
    assert_eq!(
        state.slots[0]
            .item
            .as_emby()
            .unwrap()
            .playback_position_ticks,
        expected_position
    );
    assert_eq!(emby.playback_position_ticks, expected_position);
    assert_eq!(emby.played, expected_played);
    assert_eq!(
        slot.progress_state.local,
        crate::playback_queue::SlotProgress {
            position_ticks: expected_position,
            played: expected_played,
        }
    );
}

#[test]
fn adopted_refresh_does_not_prune_or_broadcast_after_queue_replacement() {
    let player = cold_player();
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (_client_id, client_rx) = {
        let mut clients = registry.lock().unwrap();
        connect_client(&mut clients)
    };
    let shared_queue = shared_queue_state();
    let mut owner = owner_with(vec![emby_qi("adopted", "Video", "Movie")], 0);
    let adopted_slot = owner.core.queue.slots()[0].slot_id;
    let mut stale = item("adopted", "Video", "Movie");
    stale.playback_position_ticks = 40_000_000;

    owner
        .core
        .queue
        .replace(vec![emby_qi("replacement", "Video", "Movie")]);
    owner.core.queue.append(emby_qi("added", "Video", "Movie"));

    apply_queue_enriched(
        vec![(adopted_slot, stale)],
        &mut owner,
        &player,
        &shared_queue,
        &registry,
    );

    assert_eq!(owner.core.queue.len(), 2);
    assert!(client_rx.try_recv().is_err());
}

#[test]
fn adopted_queue_refresh_does_not_overwrite_played_progress() {
    let player = cold_player();
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (_client_id, client_rx) = {
        let mut clients = registry.lock().unwrap();
        connect_client(&mut clients)
    };
    let shared_queue = shared_queue_state();
    let mut owner = owner_with(
        vec![
            emby_qi("played", "Video", "Movie"),
            emby_qi("next", "Video", "Movie"),
        ],
        1,
    );
    let slot_id = owner.core.queue.slots()[0].slot_id;
    owner.core.apply_completion_progress(slot_id, 9, true);

    let mut refresh_item = item("played", "Video", "Movie");
    refresh_item.playback_position_ticks = 2;
    apply_queue_enriched(
        vec![(slot_id, refresh_item)],
        &mut owner,
        &player,
        &shared_queue,
        &registry,
    );

    assert!(client_rx.try_recv().is_err());
    let refreshed_item = owner.core.queue.slots()[0].item.as_emby().unwrap();
    assert_eq!(refreshed_item.playback_position_ticks, 9);
    assert!(refreshed_item.played);
}

#[test]
fn unified_adopt_queue_rejection_sends_authoritative_state_to_sole_client() {
    let player = cold_player();
    let client = Arc::new(Mutex::new(crate::api::EmbyClient::new(Config::default())));
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (_sender_id, _sender_rx) = {
        let mut clients = registry.lock().unwrap();
        connect_client(&mut clients)
    };
    let (reply_tx, reply_rx) = mpsc::channel();
    let queue = queue_from_items(&[item("existing", "Video", "Movie")], 0);
    let source = QueueSource::Remote;
    let (dummy_merged_tx, _dummy_rx) = mpsc::channel::<DaemonEvent>();

    let mut owner = DaemonPlayerOwner {
        core: PlayerOwnerState::new(queue, source),
        ..Default::default()
    };
    handle_ctrl_for_role(
        CtrlCmd::UnifiedAdoptQueue {
            items: vec![emby_qi("stale", "Video", "Movie")],
            cursor: 0,
            source: QueueSource::Unknown,
        },
        CtrlContext {
            reply_tx: &reply_tx,
            client_id: 1,
            client: &client,
            player: &player,
            audio_only: false,
            owner: &mut owner,
            shared_queue: &shared_queue_state(),
            ctrl_clients: &registry,
            has_audiobookshelf: false,
            merged_tx: &dummy_merged_tx,
            stay_alive: false,
            role: crate::daemon::DaemonRole::Packaged,
        },
    );
    let queue = owner.core.queue;

    assert_eq!(queue.len(), 1);
    assert_eq!(queue.slots()[0].item.id(), "existing");
    match recv_event(&reply_rx) {
        CtrlEvent::CommandRejected(reason) => {
            assert_eq!(reason, "daemon already has a queue; adoption skipped");
        }
        _ => panic!("expected command rejection"),
    }
    match recv_event(&reply_rx) {
        CtrlEvent::UnifiedQueueState(state) => {
            assert_eq!(
                state.slots.iter().map(|s| s.item.id()).collect::<Vec<_>>(),
                vec!["existing"]
            );
            assert_eq!(state.active_slot, Some(state.slots[0].slot_id));
        }
        _ => panic!("expected authoritative state resync"),
    }
}
