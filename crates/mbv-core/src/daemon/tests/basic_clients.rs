use super::*;

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
