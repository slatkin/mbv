use super::*;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;

fn start_ctrl_auth_test_peer(
    control_credential: Option<&str>,
    audio_only: bool,
    role: DaemonRole,
) -> (UnixStream, std::sync::mpsc::Receiver<DaemonEvent>) {
    let (client, peer) = UnixStream::pair().unwrap();
    let (merged_tx, merged_rx) = std::sync::mpsc::channel();
    let clients = std::sync::Arc::new(std::sync::Mutex::new(CtrlClients::default()));
    let player = cold_player();

    spawn_ctrl_client(
        SocketStream::Unix(peer),
        CtrlTransport::Local,
        merged_tx,
        clients,
        control_credential.map(str::to_owned),
        player.status,
        shared_queue_state(),
        audio_only,
        role,
        crate::owner_settings::fixed_reader(false),
    );
    (client, merged_rx)
}

#[derive(Clone, Copy)]
enum AdmissionClients {
    Empty,
    PlayerAttached,
}

#[derive(Clone, Copy)]
enum AdmissionShutdown {
    Running,
    ShuttingDown,
}

#[derive(Clone, Copy)]
enum AdmissionKind {
    Player,
    ServiceSetupAdmin,
}

fn admission_result(
    role: DaemonRole,
    stay_alive: bool,
    clients_state: AdmissionClients,
    shutdown: AdmissionShutdown,
    kind: AdmissionKind,
) -> CtrlEvent {
    let (client, peer) = UnixStream::pair().unwrap();
    client
        .set_read_timeout(Some(std::time::Duration::from_secs(1)))
        .unwrap();
    let (merged_tx, _merged_rx) = std::sync::mpsc::channel();
    let clients = std::sync::Arc::new(std::sync::Mutex::new(CtrlClients::new(merged_tx.clone())));
    if matches!(clients_state, AdmissionClients::PlayerAttached) {
        let (existing_tx, _existing_rx) = std::sync::mpsc::channel();
        clients.lock().unwrap().connect(
            existing_tx,
            CtrlTransport::Local,
            mbv_ctrl::CtrlAudiobookshelfCapabilities::default(),
            false,
        );
    }
    clients.lock().unwrap().shutting_down = matches!(shutdown, AdmissionShutdown::ShuttingDown);
    let player = cold_player();
    spawn_ctrl_client(
        SocketStream::Unix(peer),
        CtrlTransport::Local,
        merged_tx,
        clients,
        None,
        player.status,
        shared_queue_state(),
        false,
        role,
        crate::owner_settings::fixed_reader(stay_alive),
    );
    let mut reader = BufReader::new(client.try_clone().unwrap());
    assert!(matches!(read_ctrl_event(&mut reader), CtrlEvent::Hello(_)));
    let mut writer = reader.get_ref().try_clone().unwrap();
    writeln!(
        writer,
        "{}",
        serde_json::to_string(&CtrlCmd::Hello({
            let mut hello = CtrlHello::current();
            if matches!(kind, AdmissionKind::ServiceSetupAdmin) {
                hello
                    .capabilities
                    .push(mbv_ctrl::CTRL_CAP_SERVICE_SETUP_ADMIN.to_string());
            }
            hello
        }))
        .unwrap()
    )
    .unwrap();
    read_ctrl_event(&mut reader)
}

fn read_ctrl_event(reader: &mut BufReader<UnixStream>) -> CtrlEvent {
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    serde_json::from_str(line.trim_end()).unwrap()
}

#[test]
fn local_stay_alive_off_admits_one_client_and_refuses_a_second_without_queue_state() {
    let event = admission_result(
        DaemonRole::Local,
        false,
        AdmissionClients::PlayerAttached,
        AdmissionShutdown::Running,
        AdmissionKind::Player,
    );
    assert!(!matches!(event, CtrlEvent::UnifiedQueueState(_)));
    assert!(matches!(
        event,
        CtrlEvent::Disconnected {
            reason: DisconnectReason::ExclusiveOwner { pid }
        } if pid == std::process::id()
    ));
}

#[test]
fn stay_alive_on_admits_another_client() {
    assert!(matches!(
        admission_result(
            DaemonRole::Local,
            true,
            AdmissionClients::PlayerAttached,
            AdmissionShutdown::Running,
            AdmissionKind::Player,
        ),
        CtrlEvent::UnifiedQueueState(_)
    ));
}

#[test]
fn packaged_owner_admits_another_client_with_stay_alive_off() {
    assert!(matches!(
        admission_result(
            DaemonRole::Packaged,
            false,
            AdmissionClients::PlayerAttached,
            AdmissionShutdown::Running,
            AdmissionKind::Player,
        ),
        CtrlEvent::UnifiedQueueState(_)
    ));
}

#[test]
fn service_setup_admin_connection_is_admitted_while_local_owner_is_exclusive() {
    assert!(matches!(
        admission_result(
            DaemonRole::Local,
            false,
            AdmissionClients::PlayerAttached,
            AdmissionShutdown::Running,
            AdmissionKind::ServiceSetupAdmin,
        ),
        CtrlEvent::UnifiedQueueState(_)
    ));
}

#[test]
fn a_shutting_down_daemon_admits_no_client() {
    assert!(matches!(
        admission_result(
            DaemonRole::Local,
            true,
            AdmissionClients::Empty,
            AdmissionShutdown::ShuttingDown,
            AdmissionKind::Player,
        ),
        CtrlEvent::Disconnected {
            reason: DisconnectReason::OwnerShuttingDown
        }
    ));
}

#[test]
fn local_ctrl_socket_accepts_valid_control_credential_without_emby() {
    let (client, _events) =
        start_ctrl_auth_test_peer(Some("owner-control"), false, DaemonRole::Local);
    client
        .set_read_timeout(Some(std::time::Duration::from_secs(1)))
        .unwrap();
    let mut reader = BufReader::new(client.try_clone().unwrap());
    let hello = read_ctrl_event(&mut reader);
    let CtrlEvent::Hello(hello) = hello else {
        panic!("expected daemon hello");
    };
    assert!(hello.supports_control_auth());

    let mut writer = reader.get_mut().try_clone().unwrap();
    let client_hello = CtrlCmd::Hello(CtrlHello::current_control_client(
        "owner-control".to_string(),
    ));
    writeln!(writer, "{}", serde_json::to_string(&client_hello).unwrap()).unwrap();

    assert!(matches!(
        read_ctrl_event(&mut reader),
        CtrlEvent::UnifiedQueueState(_)
    ));
}

#[test]
fn local_ctrl_socket_rejects_wrong_control_credential_without_emby_fallback() {
    let (client, _events) =
        start_ctrl_auth_test_peer(Some("owner-control"), false, DaemonRole::Local);
    client
        .set_read_timeout(Some(std::time::Duration::from_secs(1)))
        .unwrap();
    let mut reader = BufReader::new(client.try_clone().unwrap());
    assert!(matches!(read_ctrl_event(&mut reader), CtrlEvent::Hello(_)));

    let mut writer = reader.get_mut().try_clone().unwrap();
    let client_hello = CtrlHello::current_control_client("wrong-control".to_string());
    writeln!(
        writer,
        "{}",
        serde_json::to_string(&CtrlCmd::Hello(client_hello)).unwrap()
    )
    .unwrap();

    let mut line = String::new();
    assert_eq!(reader.read_line(&mut line).unwrap(), 0);
}

#[test]
fn audio_only_daemon_advertises_capability_in_ctrl_hello() {
    let (client, _events) = start_ctrl_auth_test_peer(None, true, DaemonRole::Local);
    client
        .set_read_timeout(Some(std::time::Duration::from_secs(1)))
        .unwrap();
    let mut reader = BufReader::new(client);
    let mut line = String::new();
    assert!(
        reader.read_line(&mut line).unwrap() > 0,
        "daemon hello was empty"
    );
    let CtrlEvent::Hello(hello) = serde_json::from_str(line.trim_end()).unwrap() else {
        panic!("expected daemon hello");
    };
    assert!(hello.supports_audio_only());
}

#[test]
fn packaged_ctrl_socket_accepts_compatible_client_without_credentials() {
    let (client, _events) = start_ctrl_auth_test_peer(None, false, DaemonRole::Packaged);
    client
        .set_read_timeout(Some(std::time::Duration::from_secs(1)))
        .unwrap();
    let mut reader = BufReader::new(client.try_clone().unwrap());
    let hello = read_ctrl_event(&mut reader);
    let CtrlEvent::Hello(hello) = hello else {
        panic!("expected daemon hello");
    };
    assert!(!hello.supports_control_auth());
    assert!(!hello.supports_audio_only());

    let mut writer = reader.get_mut().try_clone().unwrap();
    writeln!(
        writer,
        "{}",
        serde_json::to_string(&CtrlCmd::Hello(CtrlHello::current())).unwrap()
    )
    .unwrap();
    assert!(matches!(
        read_ctrl_event(&mut reader),
        CtrlEvent::UnifiedQueueState(_)
    ));
}
