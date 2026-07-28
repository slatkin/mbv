#[test]
fn connecting_ctrl_client_becomes_driver_immediately() {
    // Connecting *is* the takeover (ADR 0003): there is no window where a
    // client is attached but not yet driving, so a broadcast reaches a
    // freshly connected client with no separate "take over" step.
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
fn second_connect_evicts_first_and_becomes_sole_connection() {
    let mut clients = CtrlClients::default();
    let (old_id, old_rx) = connect_client(&mut clients);
    let (new_id, new_rx) = connect_client(&mut clients);

    match recv_event(&old_rx) {
        CtrlEvent::Disconnected { reason } => {
            assert_eq!(reason, DisconnectReason::TakenOverByCtrlClient);
        }
        _ => panic!("expected structured disconnect"),
    }
    assert_close(&old_rx);

    // Exactly one connection can ever exist: the evicted id is gone, the
    // new id is the sole driver. This is what makes the co-pending
    // AdoptQueue race from #119 structurally impossible — there is never
    // a second live connection to lose the cold-check.
    assert!(!clients.has_client(old_id));
    assert!(clients.has_client(new_id));
    assert!(clients.has_driver());

    let registry = Arc::new(Mutex::new(clients));
    broadcast(
        &registry,
        &CtrlEvent::StatusOnly(PlayerStatus {
            volume: 77,
            ..PlayerStatus::default()
        }),
    );

    match recv_event(&new_rx) {
        CtrlEvent::StatusOnly(status) => assert_eq!(status.volume, 77),
        _ => panic!("expected status update"),
    }
    assert!(old_rx.try_recv().is_err());
}

#[test]
fn emby_remote_takeover_disconnects_current_ctrl_driver() {
    // Scope boundary (ADR 0003 / issue #119 brief): the ctrl-vs-Emby-
    // remote-websocket authority axis is untouched by the exclusive-
    // connection collapse — a successful Emby remote command must still
    // fully evict the sole ctrl connection exactly as before.
    let mut clients = CtrlClients::default();
    let (driver_id, driver_rx) = connect_client(&mut clients);
    assert!(clients.has_client(driver_id));

    clients.take_authority_for_emby_remote();

    match recv_event(&driver_rx) {
        CtrlEvent::Disconnected { reason } => {
            assert_eq!(reason, DisconnectReason::TakenOverByEmbyRemote);
        }
        _ => panic!("expected structured disconnect"),
    }
    assert_close(&driver_rx);
    assert!(!clients.has_driver());
    assert_eq!(clients.authority, AuthorityHolder::EmbyRemote);
}

#[test]
fn ctrl_reconnect_after_emby_remote_takeover_becomes_driver_and_receives_broadcasts() {
    let mut clients = CtrlClients::default();
    let (_old_id, old_rx) = connect_client(&mut clients);
    clients.take_authority_for_emby_remote();

    match recv_event(&old_rx) {
        CtrlEvent::Disconnected { reason } => {
            assert_eq!(reason, DisconnectReason::TakenOverByEmbyRemote);
        }
        _ => panic!("expected structured disconnect"),
    }
    assert_close(&old_rx);
    assert_eq!(clients.authority, AuthorityHolder::EmbyRemote);

    let (new_id, new_rx) = connect_client(&mut clients);
    assert!(clients.has_client(new_id));
    assert_eq!(clients.authority, AuthorityHolder::Ctrl(new_id));

    let registry = Arc::new(Mutex::new(clients));
    broadcast(
        &registry,
        &CtrlEvent::StatusOnly(PlayerStatus {
            volume: 66,
            ..PlayerStatus::default()
        }),
    );

    match recv_event(&new_rx) {
        CtrlEvent::StatusOnly(status) => assert_eq!(status.volume, 66),
        _ => panic!("expected status update"),
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
    // Daemon contract (CONTEXT.md): disconnecting the sole ctrl
    // connection must not stop playback or drop the queue. `CtrlClients`
    // holds no player/queue state at all, so `remove` can only ever
    // affect the connection registry — this test pins that shape so a
    // future change can't accidentally couple the two.
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
    let mut items = Vec::new();
    let mut cursor = 0;
    let mut source = QueueSource::Unknown;

    handle_ctrl(
        CtrlCmd::PlayerCmd(WireCommand::from(PlayerCommand::TogglePause)),
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
        &shared_queue_state(),
        &registry,
    );

    // The connection was already the driver from connect-time, so a
    // no-op command on a cold player neither promotes nor evicts anyone.
    assert!(registry.lock().unwrap().has_driver());
    assert!(sender_rx.try_recv().is_err());
}
