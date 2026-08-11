use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;

use super::spawn_ctrl_client;
use crate::ctrl::CtrlHello;

fn start_ctrl_auth_test_peer(
    control_credential: Option<&str>,
) -> (UnixStream, std::sync::mpsc::Receiver<DaemonEvent>) {
    let (client, peer) = UnixStream::pair().unwrap();
    let (merged_tx, merged_rx) = std::sync::mpsc::channel();
    let clients = std::sync::Arc::new(std::sync::Mutex::new(CtrlClients::default()));
    let player = cold_player();

    spawn_ctrl_client(
        peer,
        CtrlTransport::Local,
        merged_tx,
        clients,
        std::sync::Arc::new(std::sync::Mutex::new(crate::api::EmbyClient::new(
            Config::default(),
        ))),
        control_credential.map(str::to_owned),
        player.status,
        shared_queue_state(),
    );
    (client, merged_rx)
}

fn read_ctrl_event(reader: &mut BufReader<UnixStream>) -> CtrlEvent {
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    serde_json::from_str(line.trim_end()).unwrap()
}

#[test]
fn local_ctrl_socket_accepts_valid_control_credential_without_emby() {
    let (client, _events) = start_ctrl_auth_test_peer(Some("owner-control"));
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
    let (client, _events) = start_ctrl_auth_test_peer(Some("owner-control"));
    client
        .set_read_timeout(Some(std::time::Duration::from_secs(1)))
        .unwrap();
    let mut reader = BufReader::new(client.try_clone().unwrap());
    assert!(matches!(read_ctrl_event(&mut reader), CtrlEvent::Hello(_)));

    let mut writer = reader.get_mut().try_clone().unwrap();
    let mut client_hello = CtrlHello::current_control_client("wrong-control".to_string());
    client_hello.auth_token = Some("must-not-fall-through-to-Emby".to_string());
    writeln!(
        writer,
        "{}",
        serde_json::to_string(&CtrlCmd::Hello(client_hello)).unwrap()
    )
    .unwrap();

    let mut line = String::new();
    assert_eq!(reader.read_line(&mut line).unwrap(), 0);
}
