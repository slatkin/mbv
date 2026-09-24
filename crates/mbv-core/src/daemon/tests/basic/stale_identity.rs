use super::*;

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
        &[
            item("stopped-a", "Video", "Movie"),
            item("stopped-b", "Video", "Movie"),
        ],
        0,
    );
    let mut stopped_owner = DaemonPlayerOwner {
        core: PlayerOwnerState::new(queue, QueueSource::Remote),
        ..Default::default()
    };
    let stopped_slot = stopped_owner.core.queue.slots()[0].slot_id;
    stopped_owner
        .core
        .note_observed_active_slot(Some(stopped_slot));
    *shared_queue.observed_active_slot.lock().unwrap() = Some(stopped_slot);
    let original_position = stopped_owner
        .core
        .queue
        .slot(stopped_slot)
        .unwrap()
        .item
        .playback_position_ticks();

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
    assert!(!stopped_owner
        .core
        .queue
        .slot(stopped_slot)
        .unwrap()
        .item
        .played());
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
        stopped_owner
            .core
            .queue
            .slot(stopped_slot)
            .unwrap()
            .item
            .playback_position_ticks(),
        900
    );
    assert_eq!(
        stopped_owner.core.observed_active_slot(),
        Some(stopped_slot)
    );
    assert_eq!(
        *shared_queue.observed_active_slot.lock().unwrap(),
        Some(stopped_slot)
    );
    assert_ne!(
        stopped_owner
            .core
            .queue
            .slot(stopped_slot)
            .unwrap()
            .item
            .playback_position_ticks(),
        original_position
    );

    let queue = queue_from_items(
        &[
            item("completed-a", "Video", "Movie"),
            item("completed-b", "Video", "Movie"),
        ],
        0,
    );
    let mut completed_owner = DaemonPlayerOwner {
        core: PlayerOwnerState::new(queue, QueueSource::Remote),
        ..Default::default()
    };
    let completed_slot = completed_owner.core.queue.slots()[0].slot_id;
    completed_owner
        .core
        .note_observed_active_slot(Some(completed_slot));
    *shared_queue.observed_active_slot.lock().unwrap() = Some(completed_slot);
    let original_len = completed_owner.core.queue.len();
    let original_position = completed_owner
        .core
        .queue
        .slot(completed_slot)
        .unwrap()
        .item
        .playback_position_ticks();

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
    assert_eq!(
        completed_owner.core.observed_active_slot(),
        Some(completed_slot)
    );
    assert_eq!(
        *shared_queue.observed_active_slot.lock().unwrap(),
        Some(completed_slot)
    );
    assert_eq!(
        completed_owner
            .core
            .queue
            .slot(completed_slot)
            .unwrap()
            .item
            .playback_position_ticks(),
        original_position
    );
    assert!(!completed_owner
        .core
        .queue
        .slot(completed_slot)
        .unwrap()
        .item
        .played());

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
        completed_owner
            .core
            .queue
            .slot(completed_slot)
            .unwrap()
            .item
            .playback_position_ticks(),
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
