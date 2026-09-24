use super::*;
use crate::player::PlayerOwnerState;

pub fn item(name: &str, media_type: &str, item_type: &str) -> EmbyItem {
    EmbyItem {
        id: name.into(),
        name: name.into(),
        item_type: item_type.into(),
        is_folder: false,
        child_count: None,
        media_type: media_type.into(),
        collection_type: String::new(),
        runtime_ticks: 0,
        played: false,
        playback_position_ticks: 0,
        series_id: String::new(),
        series_name: String::new(),
        album_id: String::new(),
        album: String::new(),
        index_number: 0,
        parent_index_number: 0,
        unplayed_item_count: 0,
        path: String::new(),
        artist: String::new(),
        artist_items: Vec::new(),
        sort_name: String::new(),
        production_year: 0,
        end_year: 0,
        overview: String::new(),
        premiere_date: String::new(),
        date_added: String::new(),
        total_count: 0,
        container: String::new(),
        video_info: String::new(),
        audio_info: String::new(),
        genres: Vec::new(),
        people: Vec::new(),
        external_urls: Vec::new(),
        playlist_item_id: String::new(),
        image_tags: Default::default(),
    }
}

pub fn emby_qi(name: &str, media_type: &str, item_type: &str) -> QueueItem {
    QueueItem::Emby(Box::new(item(name, media_type, item_type)))
}

pub fn video_feed_qi(guid: &str) -> QueueItem {
    QueueItem::Feed(FeedEntry {
        guid: guid.into(),
        title: guid.into(),
        enclosure_url: None,
        link: None,
        mime_type: Some("video/mp4".into()),
        duration_ticks: None,
        pub_date_secs: None,
        feed_kind: Some(crate::config::FeedKind::Video),
        feed_id: None,
        position_ticks: 0,
        played: false,
    })
}
/// Connects a client the same way the accept thread does.
pub fn connect_client(clients: &mut CtrlClients) -> (u64, mpsc::Receiver<CtrlOutbound>) {
    let (tx, rx) = mpsc::channel();
    let id = clients.connect(tx, CtrlTransport::Local, true, true, true, true, true);
    (id, rx)
}

pub fn shared_queue_state() -> SharedQueueState {
    SharedQueueState {
        queue: Arc::new(Mutex::new(PlaybackQueue::default())),
        source: Arc::new(Mutex::new(QueueSource::Unknown)),
        lineage: Arc::new(Mutex::new(crate::ctrl::QueueLineage::default())),
        observed_active_slot: Arc::new(Mutex::new(None)),
    }
}

pub fn cold_player() -> Player {
    let (event_tx, _event_rx) = mpsc::channel::<PlayerEvent>();
    Player::new(
        String::new(),
        String::new(),
        false,
        false,
        true,
        false,
        SubtitlePrefs::default(),
        event_tx,
        None,
    )
}

pub fn recv_event(rx: &mpsc::Receiver<CtrlOutbound>) -> CtrlEvent {
    match rx.recv().unwrap() {
        CtrlOutbound::Event(json) => serde_json::from_str(&json).unwrap(),
        CtrlOutbound::Flush(_) => panic!("expected a control event"),
    }
}

/// Helper: builds a `PlaybackQueue` from a list of `EmbyItem`s with an active index.
pub fn queue_from_items(items: &[EmbyItem], active: usize) -> PlaybackQueue {
    let qi: Vec<QueueItem> = items
        .iter()
        .cloned()
        .map(|i| QueueItem::Emby(Box::new(i)))
        .collect();
    PlaybackQueue::from_queue_items(qi, Some(active))
}

#[test]
fn shutdown_notification_is_flushed_before_writers_are_released() {
    let mut clients = CtrlClients::default();
    let (_client_id, rx) = connect_client(&mut clients);
    let writer = std::thread::spawn(move || {
        match rx.recv().unwrap() {
            CtrlOutbound::Event(json) => {
                assert!(matches!(
                    serde_json::from_str::<CtrlEvent>(&json).unwrap(),
                    CtrlEvent::Disconnected {
                        reason: DisconnectReason::DaemonShutdown
                    }
                ));
            }
            CtrlOutbound::Flush(_) => panic!("shutdown event must precede the flush barrier"),
        }
        match rx.recv().unwrap() {
            CtrlOutbound::Flush(ack) => ack.send(()).unwrap(),
            CtrlOutbound::Event(_) => panic!("flush barrier must follow the shutdown event"),
        }
    });

    clients.notify_disconnected_all(DisconnectReason::DaemonShutdown);
    clients.flush_writers(Duration::from_secs(1));
    writer.join().unwrap();
}

#[test]
fn connecting_ctrl_client_becomes_driver_immediately() {
    let mut clients = CtrlClients::default();
    let (id, rx) = connect_client(&mut clients);
    assert!(clients.has_driver());
    assert!(clients.has_client(id));

    let registry = Arc::new(Mutex::new(clients));
    broadcast(
        &registry,
        &CtrlEvent::StatusOnly(PlayerStatus {
            volume: 55,
            ..PlayerStatus::default()
        }),
    );

    match recv_event(&rx) {
        CtrlEvent::StatusOnly(status) => assert_eq!(status.volume, 55),
        _ => panic!("expected status update"),
    }
}

#[test]
fn emby_remote_takeover_notifies_ctrl_client_and_keeps_connection() {
    let mut clients = CtrlClients::default();
    let (driver_id, driver_rx) = connect_client(&mut clients);
    assert!(clients.has_client(driver_id));

    clients.take_authority_for_emby_remote();

    match recv_event(&driver_rx) {
        CtrlEvent::Disconnected { reason } => {
            assert_eq!(reason, DisconnectReason::TakenOverByEmbyRemote);
        }
        _ => panic!("expected structured disconnect notification"),
    }
    assert!(clients.has_driver());
    assert_eq!(clients.authority, AuthorityHolder::EmbyRemote);
}

#[test]
fn ctrl_connect_during_emby_authority_does_not_override_authority() {
    let mut clients = CtrlClients::default();
    let (_old_id, old_rx) = connect_client(&mut clients);
    clients.take_authority_for_emby_remote();

    match recv_event(&old_rx) {
        CtrlEvent::Disconnected { reason } => {
            assert_eq!(reason, DisconnectReason::TakenOverByEmbyRemote);
        }
        _ => panic!("expected structured disconnect notification"),
    }
    assert_eq!(clients.authority, AuthorityHolder::EmbyRemote);

    let (new_id, new_rx) = connect_client(&mut clients);
    assert!(clients.has_client(new_id));
    assert_eq!(clients.authority, AuthorityHolder::EmbyRemote);

    let registry = Arc::new(Mutex::new(clients));
    broadcast(
        &registry,
        &CtrlEvent::StatusOnly(PlayerStatus {
            volume: 66,
            ..PlayerStatus::default()
        }),
    );

    match recv_event(&old_rx) {
        CtrlEvent::StatusOnly(status) => assert_eq!(status.volume, 66),
        _ => panic!("expected status update on old client"),
    }
    match recv_event(&new_rx) {
        CtrlEvent::StatusOnly(status) => assert_eq!(status.volume, 66),
        _ => panic!("expected status update on new client"),
    }
}

#[test]
fn emby_remote_takeover_without_ctrl_client_still_records_authority() {
    let mut clients = CtrlClients::default();

    clients.take_authority_for_emby_remote();

    assert!(!clients.has_driver());
    assert_eq!(clients.authority, AuthorityHolder::EmbyRemote);
}

#[test]
fn sole_client_disconnect_clears_registry_without_touching_playback() {
    let mut clients = CtrlClients::default();
    let (id, _rx) = connect_client(&mut clients);
    assert!(clients.has_driver());

    clients.remove(id);

    assert!(!clients.has_driver());
    assert!(!clients.has_client(id));
}

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

    let mut owner = DaemonPlayerOwner { core: PlayerOwnerState::new(queue, source), ..Default::default() };
    handle_ctrl_for_role(
        CtrlCmd::PlayerCmd(
            WireCommand::try_from_player_command(PlayerCommand::TogglePause).unwrap(),
        ),
        1,
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
        crate::daemon::DaemonRole::Local,
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

    let mut owner = DaemonPlayerOwner { core: PlayerOwnerState::new(queue, source), ..Default::default() };
    handle_ctrl_for_role(
        CtrlCmd::UnifiedAdoptQueue {
            items: vec![emby_qi("adopted", "Video", "Movie")],
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
        &mut owner,
        &shared_queue_state(),
        &registry,
        false,
        &dummy_merged_tx,
        false,
        crate::daemon::DaemonRole::Packaged,
    );
    assert!(
        dummy_merged_rx.try_recv().is_err(),
        "UnifiedAdoptQueue must return without waiting for enrichment"
    );
    let queue = owner.core.queue;

    assert_eq!(queue.len(), 1);
    assert_eq!(queue.slots()[0].item.id(), "adopted");
    assert!(!player.status.lock().unwrap().active);
    assert!(player_cmd_rx.try_recv().is_err());
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
        CtrlEvent::UnifiedQueueState(state) => state.slots[1]
            .item
            .as_emby()
            .unwrap()
            .playback_position_ticks,
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

    let mut stale_refresh = item("played", "Video", "Movie");
    stale_refresh.playback_position_ticks = 2;
    apply_queue_enriched(
        vec![(slot_id, stale_refresh)],
        &mut owner,
        &player,
        &shared_queue,
        &registry,
    );

    assert!(client_rx.try_recv().is_err());
    let played = owner.core.queue.slots()[0].item.as_emby().unwrap();
    assert_eq!(played.playback_position_ticks, 9);
    assert!(played.played);
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

    let mut owner = DaemonPlayerOwner { core: PlayerOwnerState::new(queue, source), ..Default::default() };
    handle_ctrl_for_role(
        CtrlCmd::UnifiedAdoptQueue {
            items: vec![emby_qi("stale", "Video", "Movie")],
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
        &mut owner,
        &shared_queue_state(),
        &registry,
        false,
        &dummy_merged_tx,
        false,
        crate::daemon::DaemonRole::Packaged,
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

#[test]
fn cold_websocket_noop_does_not_evict_ctrl_driver() {
    let player = cold_player();
    let client = Arc::new(Mutex::new(crate::api::EmbyClient::new(Config::default())));
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (driver_id, driver_rx) = {
        let mut clients = registry.lock().unwrap();
        connect_client(&mut clients)
    };
    let mut queue = PlaybackQueue::default();
    let mut source = QueueSource::Unknown;
    let mut transitions = crate::playback_transition::OwnerTransitionState::default();

    handle_ws(
        WsEvent::TogglePause,
        Some(&client),
        &player,
        false,
        &mut queue,
        &mut source,
        &mut transitions,
        &shared_queue_state(),
        &registry,
    );

    let clients = registry.lock().unwrap();
    assert!(clients.has_client(driver_id));
    drop(clients);
    assert!(driver_rx.try_recv().is_err());
}

// ── design D6: stale identity is rejected, never repaired by position ────

#[test]
fn stale_client_jump_to_index_is_rejected_visibly() {
    let player = cold_player();
    let client = Arc::new(Mutex::new(crate::api::EmbyClient::new(Config::default())));
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (reply_tx, reply_rx) = mpsc::channel();
    let queue = queue_from_items(
        &[item("a", "Video", "Movie"), item("b", "Video", "Movie")],
        0,
    );
    let mut owner = DaemonPlayerOwner {
        core: PlayerOwnerState::new(queue, QueueSource::Remote),
        ..Default::default()
    };
    let (dummy_merged_tx, _dummy_rx) = mpsc::channel::<DaemonEvent>();

    handle_ctrl_for_role(
        CtrlCmd::PlayerCmd(WireCommand::JumpTo(1)),
        1,
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
        crate::daemon::DaemonRole::Local,
    );

    match recv_event(&reply_rx) {
        CtrlEvent::CommandRejected(reason) => assert!(reason.contains("index-addressed")),
        _ => panic!("expected a visible CommandRejected for an index-addressed jump"),
    }
    // The stale command is never repaired by position.
    assert_eq!(owner.core.queue.active_index(), Some(0));
    assert_eq!(owner.core.observed_active_slot(), None);
}

#[test]
fn stale_stopped_and_completed_run_observations_are_rejected() {
    let player = cold_player();
    player.status.lock().unwrap().sequence_generation = 5;
    let current_run = (0, 5);
    let old_run = (0, 4);
    let shared_queue = shared_queue_state();

    let queue = queue_from_items(
        &[item("stopped-a", "Video", "Movie"), item("stopped-b", "Video", "Movie")],
        0,
    );
    let mut stopped_owner = DaemonPlayerOwner {
        core: PlayerOwnerState::new(queue, QueueSource::Remote),
        ..Default::default()
    };
    let stopped_slot = stopped_owner.core.queue.slots()[0].slot_id;
    stopped_owner.core.note_observed_active_slot(Some(stopped_slot));
    *shared_queue.observed_active_slot.lock().unwrap() = Some(stopped_slot);
    let original_position = stopped_owner.core.queue.slot(stopped_slot).unwrap().item.playback_position_ticks();

    assert_eq!(
        apply_stopped_observation(
            &mut stopped_owner,
            &player,
            old_run.into(),
            Some(stopped_slot),
            900,
            true,
        ),
        None
    );
    assert!(!stopped_owner.core.queue.slot(stopped_slot).unwrap().item.played());
    assert_eq!(
        apply_stopped_observation(
            &mut stopped_owner,
            &player,
            current_run.into(),
            Some(stopped_slot),
            900,
            false,
        ),
        Some(true)
    );
    assert_eq!(
        stopped_owner.core.queue.slot(stopped_slot).unwrap().item.playback_position_ticks(),
        900
    );
    assert_eq!(stopped_owner.core.observed_active_slot(), Some(stopped_slot));
    assert_eq!(
        *shared_queue.observed_active_slot.lock().unwrap(),
        Some(stopped_slot)
    );
    assert_ne!(
        stopped_owner.core.queue.slot(stopped_slot).unwrap().item.playback_position_ticks(),
        original_position
    );

    let queue = queue_from_items(
        &[item("completed-a", "Video", "Movie"), item("completed-b", "Video", "Movie")],
        0,
    );
    let mut completed_owner = DaemonPlayerOwner {
        core: PlayerOwnerState::new(queue, QueueSource::Remote),
        ..Default::default()
    };
    let completed_slot = completed_owner.core.queue.slots()[0].slot_id;
    completed_owner.core.note_observed_active_slot(Some(completed_slot));
    *shared_queue.observed_active_slot.lock().unwrap() = Some(completed_slot);
    let original_len = completed_owner.core.queue.len();
    let original_position = completed_owner.core.queue.slot(completed_slot).unwrap().item.playback_position_ticks();

    assert!(!apply_track_completed_observation(
        &mut completed_owner,
        &player,
        &shared_queue,
        old_run.into(),
        completed_slot,
        crate::api::MEANINGFUL_TRACK_COMPLETED_PROGRESS_TICKS + 1,
        true,
        true,
        true,
        false,
    ));
    assert_eq!(completed_owner.core.queue.len(), original_len);
    assert_eq!(completed_owner.core.observed_active_slot(), Some(completed_slot));
    assert_eq!(
        *shared_queue.observed_active_slot.lock().unwrap(),
        Some(completed_slot)
    );
    assert_eq!(
        completed_owner.core.queue.slot(completed_slot).unwrap().item.playback_position_ticks(),
        original_position
    );
    assert!(!completed_owner.core.queue.slot(completed_slot).unwrap().item.played());

    assert!(apply_track_completed_observation(
        &mut completed_owner,
        &player,
        &shared_queue,
        current_run.into(),
        completed_slot,
        crate::api::MEANINGFUL_TRACK_COMPLETED_PROGRESS_TICKS + 1,
        false,
        false,
        true,
        false,
    ));
    assert_eq!(
        completed_owner.core.queue.slot(completed_slot).unwrap().item.playback_position_ticks(),
        crate::api::MEANINGFUL_TRACK_COMPLETED_PROGRESS_TICKS + 1
    );
    assert!(apply_track_completed_observation(
        &mut completed_owner,
        &player,
        &shared_queue,
        current_run.into(),
        completed_slot,
        0,
        true,
        true,
        true,
        false,
    ));
    assert_eq!(completed_owner.core.queue.len(), original_len - 1);
}

#[test]
fn stale_track_changed_report_leaves_queue_and_observed_slot_unchanged() {
    let queue = queue_from_items(
        &[item("a", "Video", "Movie"), item("b", "Video", "Movie")],
        1,
    );
    let mut owner = DaemonPlayerOwner {
        core: PlayerOwnerState::new(queue, QueueSource::Remote),
        ..Default::default()
    };

    // A genuine observation advances the observed active slot.
    let real = owner.core.queue.slots()[1].slot_id;
    assert_eq!(owner.core.observe_track_change(real), Some((1, real)));
    assert_eq!(owner.core.observed_active_slot(), Some(real));

    // A report naming a slot the owner no longer holds is discarded: the
    // caller (daemon_run's TrackChanged arm) emits nothing and canonical
    // queue + observed active slot are untouched (design D6, no clamp, no
    // neighbour fallback).
    let stale = crate::playback_queue::QueueSlotId::from_raw(9_999_999);
    assert!(owner.core.observe_track_change(stale).is_none());
    assert_eq!(owner.core.queue.active_slot_id(), Some(real));
    assert_eq!(owner.core.observed_active_slot(), Some(real));
}

#[test]
fn websocket_takeover_helper_records_emby_remote_authority() {
    let registry = Arc::new(Mutex::new(CtrlClients::default()));

    take_authority_for_emby_remote(&registry);

    let clients = registry.lock().unwrap();
    assert!(!clients.has_driver());
    assert_eq!(clients.authority, AuthorityHolder::EmbyRemote);
}

// ── relative transport steps advance from the desired slot ──────────────

/// A Next intent while a jump to B is in flight steps from B (queues C
/// behind it), never from the published `current_idx` mirror — which still
/// names A until the run confirms and made a rapid second press re-target B
/// (the erratic Next report). Both presses must also act: the owner, not a
/// client's status mirror, bounds-checks the step.
#[test]
fn next_intent_while_a_jump_is_in_flight_steps_from_the_desired_slot() {
    let player = cold_player();
    let cmd_rx = player.spy_on_commands();
    let client = Arc::new(Mutex::new(crate::api::EmbyClient::new(Config::default())));
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (client_id, client_rx) = {
        let mut clients = registry.lock().unwrap();
        connect_client(&mut clients)
    };
    let queue = queue_from_items(
        &[
            item("a", "Video", "Movie"),
            item("b", "Video", "Movie"),
            item("c", "Video", "Movie"),
        ],
        0,
    );
    let slot_b = queue.slots()[1].slot_id;
    let slot_c = queue.slots()[2].slot_id;
    let mut owner = DaemonPlayerOwner {
        core: PlayerOwnerState::new(queue, QueueSource::Remote),
        ..Default::default()
    };
    let shared = shared_queue_state();
    let (dummy_merged_tx, _dummy_rx) = mpsc::channel::<DaemonEvent>();

    let next_intent = |request_id: u64| CtrlCmd::PlaybackIntent(PlaybackIntent {
        request_id,
        generation: request_id,
        action: PlaybackIntentAction::Next,
    });

    // First press: nothing in flight, observed slot A -> jump to B.
    handle_ctrl_for_role(
        next_intent(1),
        client_id,
        CtrlRequest {
            reply_tx: &(mpsc::channel().0),
        },
        &client,
        &player,
        false,
        &mut owner,
        &shared,
        &registry,
        false,
        &dummy_merged_tx,
        false,
        crate::daemon::DaemonRole::Local,
    );
    assert!(
        matches!(
            cmd_rx.recv().unwrap(),
            PlayerCommand::JumpTo { slot_id, .. } if slot_id == slot_b
        ),
        "the first Next dispatches a slot jump to B"
    );

    // Second rapid press, B still in flight: steps from B and queues C
    // behind it.
    handle_ctrl_for_role(
        next_intent(2),
        client_id,
        CtrlRequest {
            reply_tx: &(mpsc::channel().0),
        },
        &client,
        &player,
        false,
        &mut owner,
        &shared,
        &registry,
        false,
        &dummy_merged_tx,
        false,
        crate::daemon::DaemonRole::Local,
    );
    // The second press must not dispatch past the in-flight jump (one
    // in-flight at a time, design D4): C is held queued, not sent to the run.
    assert!(cmd_rx.try_recv().is_err(), "no second dispatch while B is in flight");
    // The owner snapshot names the true desired pair: B in flight, C queued.
    // (Stepping from the mirror would have queued B again.) Drain the
    // already-queued events without ever blocking on recv.
    let mut state = None;
    while let Ok(outbound) = client_rx.try_recv() {
        if let CtrlOutbound::Event(json) = outbound {
            if let CtrlEvent::UnifiedQueueState(s) = serde_json::from_str(&json).unwrap() {
                state = Some(s);
            }
        }
    }
    let state = state.expect("a UnifiedQueueState snapshot was broadcast");
    assert_eq!(
        state.in_flight_transition.map(|t| t.target_slot),
        Some(slot_b.raw()),
        "B remains in flight"
    );
    assert_eq!(
        state.queued_latest_transition.map(|t| t.target_slot),
        Some(slot_c.raw()),
        "the second press queues C behind the in-flight B"
    );
}

// ── active-file JumpTo confirms via TrackChanged (stay-alive D1) ─────────

/// Active-file mode has no mpv playlist move to observe, so the daemon's
/// queue tracking depends entirely on the run's TrackChanged response to a
/// JumpTo. The run side needs mpv and is exercised by design review; this
/// test pins the daemon half of the loop: dispatch a slot jump, mock the
/// Playback run's TrackChanged response exactly as the active-file JumpTo
/// handler now emits it (target slot + the jump's request identity), and
/// assert the observed active slot advances to the target and the in-flight
/// transition settles — the closed loop Next/Previous resolves from.
#[test]
fn active_file_jump_to_observed_slot_advances_when_the_run_confirms_via_track_changed() {
    let player = cold_player();
    let cmd_rx = player.spy_on_commands();
    let client = Arc::new(Mutex::new(crate::api::EmbyClient::new(Config::default())));
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (client_id, _client_rx) = {
        let mut clients = registry.lock().unwrap();
        connect_client(&mut clients)
    };
    let queue = queue_from_items(
        &[
            item("a", "Video", "Movie"),
            item("b", "Video", "Movie"),
            item("c", "Video", "Movie"),
        ],
        0,
    );
    let slot_b = queue.slots()[1].slot_id;
    let mut owner = DaemonPlayerOwner {
        core: PlayerOwnerState::new(queue, QueueSource::Remote),
        ..Default::default()
    };
    let shared = shared_queue_state();
    let (dummy_merged_tx, _dummy_rx) = mpsc::channel::<DaemonEvent>();

    // Nothing observed yet: the pre-D1 broken state this change fixes.
    assert_eq!(owner.core.observed_active_slot(), None);

    // Next press dispatches the slot jump the active-file run will execute.
    handle_ctrl_for_role(
        CtrlCmd::PlaybackIntent(PlaybackIntent {
            request_id: 1,
            generation: 1,
            action: PlaybackIntentAction::Next,
        }),
        client_id,
        CtrlRequest {
            reply_tx: &(mpsc::channel().0),
        },
        &client,
        &player,
        false,
        &mut owner,
        &shared,
        &registry,
        false,
        &dummy_merged_tx,
        false,
        crate::daemon::DaemonRole::Local,
    );
    let (jump_request_id, jump_generation) = match cmd_rx.recv().unwrap() {
        PlayerCommand::JumpTo {
            slot_id,
            request_id,
            generation,
            ..
        } => {
            assert_eq!(slot_id, slot_b, "the jump targets the next slot");
            (request_id, generation)
        }
        other => panic!("expected a JumpTo dispatch, got {other:?}"),
    };

    // Mock the active-file run's response (design D1): TrackChanged naming
    // the target slot and carrying the jump's request identity, then apply
    // the daemon loop's TrackChanged arm (observe → settle → publish).
    let run_response = crate::player::PlayerEvent::TrackChanged {
        slot_id: slot_b,
        transition: Some((jump_request_id, jump_generation)),
    };
    let observed = match run_response {
        crate::player::PlayerEvent::TrackChanged {
            slot_id,
            transition: Some((request_id, _generation)),
        } if slot_id == slot_b => {
            let (_, resolved) = owner
                .core
                .observe_track_change(slot_id)
                .expect("the run reports a slot the canonical queue still holds");
            super::settle_and_redispatch(&mut owner, &player, request_id, resolved);
            *shared.observed_active_slot.lock().unwrap() = owner.core.observed_active_slot();
            Some((1, resolved))
        }
        _ => panic!("expected a TrackChanged confirmation for slot B"),
    };
    assert_eq!(observed, Some((1, slot_b)));
    assert_eq!(
        *shared.observed_active_slot.lock().unwrap(),
        Some(slot_b),
        "the observed active slot advances to the jump target"
    );
    assert_eq!(
        owner.core.in_flight_transition_slot(),
        None,
        "the jump's transition settles on the run's confirmation"
    );
}
