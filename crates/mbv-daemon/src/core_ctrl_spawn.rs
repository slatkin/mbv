use super::control_queue::unified_queue_state_for_peer;
use super::core::{DaemonEvent, SharedQueueState};
use crate::ctrl::{ClientRegistry, CtrlClients, CtrlOutbound, CtrlTransport};
use crate::{DaemonRole, OwnerSettingsReader};
use mbv_ctrl::{CtrlAudiobookshelfCapabilities, CtrlCmd, CtrlEvent, CtrlHello};
use mbv_net::stream::SocketStream;
use std::io::{BufRead, BufReader, Write};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

fn ctrl_client_capabilities(
    line: &str,
    control_credential: Option<&str>,
) -> Option<(CtrlAudiobookshelfCapabilities, bool, bool)> {
    match serde_json::from_str::<CtrlCmd>(line) {
        Ok(CtrlCmd::Hello(info)) => {
            if let Err(e) = info.validate_peer() {
                tracing::warn!(name: "daemon.ctrl_client.peer_validation_failed", target: "daemon", error = %e, "ctrl client rejected");
                return None;
            }
            if let Some(control_credential) = control_credential {
                if info.control_token.is_none() {
                    tracing::warn!(name: "daemon.ctrl_client.control_credential_missing", target: "daemon", "ctrl client rejected: missing Control credential");
                    return None;
                }
                if let Err(e) = info.validate_control_credential(control_credential) {
                    tracing::warn!(name: "daemon.ctrl_client.control_credential_invalid", target: "daemon", error = %e, "ctrl client rejected");
                    return None;
                }
            }
            Some((
                CtrlAudiobookshelfCapabilities {
                    queue: info.supports_abs_queue(),
                    progress: info.supports_abs_progress(),
                    book_queue: info.supports_abs_book_queue(),
                    book_progress: info.supports_abs_book_progress(),
                },
                info.supports_owner_queue_load(),
                info.supports_service_setup_admin(),
            ))
        }
        Ok(_) => {
            tracing::warn!(name: "daemon.ctrl_client.hello_missing", target: "daemon", "ctrl client rejected: missing protocol hello");
            None
        }
        Err(e) => {
            tracing::warn!(name: "daemon.ctrl_client_hello.invalid", target: "daemon", error = %e, "invalid ctrl protocol hello");
            None
        }
    }
}

fn send_admission_refusal(
    clients: &crate::ctrl::CtrlClients,
    role: DaemonRole,
    transport: CtrlTransport,
    stay_alive: bool,
    service_setup_admin: bool,
    ev_tx: &crate::ctrl::CtrlSender,
) -> bool {
    let admin_local_connection = service_setup_admin && transport == CtrlTransport::Local;
    let reason = if role == DaemonRole::Local && clients.shutting_down {
        Some(mbv_ctrl::DisconnectReason::OwnerShuttingDown)
    } else if role == DaemonRole::Local
        && !stay_alive
        && clients.has_driver()
        && !admin_local_connection
    {
        Some(mbv_ctrl::DisconnectReason::ExclusiveOwner {
            pid: std::process::id(),
        })
    } else {
        None
    };
    let Some(reason) = reason else { return false };
    crate::send_to(ev_tx, &CtrlEvent::Disconnected { reason });
    true
}

fn initial_queue_state_json(
    player_status: &Arc<Mutex<mbv_ctrl::player::PlayerStatus>>,
    shared_queue: &SharedQueueState,
    supports_abs_queue: bool,
    supports_abs_book_queue: bool,
) -> Option<String> {
    let status = player_status.lock().unwrap().clone();
    let q = shared_queue.queue.lock().unwrap().clone();
    let source = shared_queue.source.lock().unwrap().clone();
    let observed_active_slot = *shared_queue.observed_active_slot.lock().unwrap();
    let lineage = *shared_queue.lineage.lock().unwrap();
    let init_event = unified_queue_state_for_peer(
        &status,
        &q,
        &source,
        lineage,
        observed_active_slot,
        None,
        None,
        supports_abs_queue,
        supports_abs_book_queue,
    );
    serde_json::to_string(&init_event).ok()
}

/// The peer identity for the ctrl connect line (design D5): the Unix peer's
/// process id from `SO_PEERCRED`, or the TCP peer address. Read before `stream`
/// moves into the reader thread.
fn ctrl_peer_identity(stream: &SocketStream) -> String {
    match stream {
        SocketStream::Unix(stream) => {
            nix::sys::socket::getsockopt(stream, nix::sys::socket::sockopt::PeerCredentials)
                .ok()
                .map_or_else(|| "unknown".to_string(), |cred| cred.pid().to_string())
        }
        SocketStream::Tcp(stream) => stream
            .peer_addr()
            .map_or_else(|_| "unknown".to_string(), |addr| addr.to_string()),
    }
}

struct CtrlClientSession {
    stream: SocketStream,
    transport: CtrlTransport,
    merged_tx: mpsc::Sender<DaemonEvent>,
    ctrl_clients: ClientRegistry,
    control_credential: Option<String>,
    player_status: Arc<Mutex<mbv_ctrl::player::PlayerStatus>>,
    shared_queue: SharedQueueState,
    role: DaemonRole,
    owner_settings: OwnerSettingsReader,
}

fn log_ctrl_client_connected(client_id: u64, peer: &str) {
    tracing::info!(
        name: "ctrl.client.connected",
        target: "ctrl",
        client = %client_id,
        peer = %peer,
        "ctrl client connected"
    );
}

impl CtrlClientSession {
    fn run(self, peer: &str, ev_tx: mpsc::Sender<CtrlOutbound>) {
        let Self {
            stream,
            transport,
            merged_tx,
            ctrl_clients,
            control_credential,
            player_status,
            shared_queue,
            role,
            owner_settings,
        } = self;
        let reader = BufReader::new(stream);
        let mut lines = reader.lines();
        let Some(Ok(line)) = lines.next() else {
            return;
        };
        let Some((audiobookshelf, supports_owner_queue_load, service_setup_admin)) =
            ctrl_client_capabilities(&line, control_credential.as_deref())
        else {
            return;
        };
        let stay_alive = role != DaemonRole::Local || (owner_settings)().stay_alive;
        let initial_state = initial_queue_state_json(
            &player_status,
            &shared_queue,
            audiobookshelf.queue,
            audiobookshelf.book_queue,
        );

        let mut clients = ctrl_clients.lock().unwrap();
        if send_admission_refusal(
            &clients,
            role,
            transport,
            stay_alive,
            service_setup_admin,
            &ev_tx,
        ) {
            return;
        }
        if let Some(initial_state) = initial_state {
            let _ = ev_tx.send(CtrlOutbound::Event(initial_state));
        }
        let reply_tx = ev_tx.clone();
        let connect = if service_setup_admin {
            CtrlClients::connect_admin
        } else {
            CtrlClients::connect
        };
        let client_id = connect(
            &mut clients,
            ev_tx,
            transport,
            audiobookshelf,
            supports_owner_queue_load,
        );
        drop(clients);
        log_ctrl_client_connected(client_id, peer);

        for line in lines {
            let Ok(line) = line else { break };
            if line.is_empty() {
                continue;
            }
            match serde_json::from_str::<CtrlCmd>(&line) {
                Ok(cmd)
                    if service_setup_admin
                        && !matches!(&cmd, CtrlCmd::ApplyServiceSetup { .. }) =>
                {
                    crate::send_to(
                        &reply_tx,
                        &CtrlEvent::CommandRejected(
                            "service-setup admin connections cannot send other commands"
                                .to_string(),
                        ),
                    );
                }
                Ok(cmd) => {
                    let _ = merged_tx.send(DaemonEvent::Ctrl(cmd, client_id, reply_tx.clone()));
                }
                Err(e) => {
                    // A drop here is silent playback loss for the client (e.g.
                    // a wire-shape drift this peer can't parse), so surface it
                    // on both ends: the log for operators, CommandRejected for
                    // the client's toast — the serde error names the
                    // field/variant that drifted.
                    tracing::warn!(
                        name: "daemon.ctrl_command_parse.failed",
                        target: "daemon",
                        client = %client_id,
                        bytes = line.len(),
                        error = %e,
                        "ctrl command parse failed"
                    );
                    if let Ok(json) = serde_json::to_string(&CtrlEvent::CommandRejected(format!(
                        "mbvd ignored an unparsable control command: {e}"
                    ))) {
                        let _ = reply_tx.send(CtrlOutbound::Event(json));
                    }
                }
            }
        }
        let _ = merged_tx.send(DaemonEvent::CtrlDisconnected(client_id));
    }
}

pub(crate) fn spawn_ctrl_client(
    stream: SocketStream,
    transport: CtrlTransport,
    merged_tx: mpsc::Sender<DaemonEvent>,
    ctrl_clients: ClientRegistry,
    control_credential: Option<String>,
    player_status: Arc<Mutex<mbv_ctrl::player::PlayerStatus>>,
    shared_queue: SharedQueueState,
    audio_only: bool,
    role: DaemonRole,
    owner_settings: OwnerSettingsReader,
) {
    let peer = ctrl_peer_identity(&stream);
    let Ok(writer_stream) = stream.try_clone() else {
        return;
    };
    let (ev_tx, ev_rx) = mpsc::channel::<CtrlOutbound>();

    let mut daemon_hello = CtrlHello::current();
    if control_credential.is_none() {
        daemon_hello
            .capabilities
            .retain(|cap| cap != mbv_ctrl::CTRL_CAP_CONTROL_AUTH);
    }
    if audio_only {
        daemon_hello
            .capabilities
            .push(mbv_ctrl::CTRL_CAP_AUDIO_ONLY.to_string());
    }
    if let Ok(hello_json) = serde_json::to_string(&CtrlEvent::Hello(daemon_hello)) {
        let _ = ev_tx.send(CtrlOutbound::Event(hello_json));
    }

    std::thread::spawn(move || {
        let mut w = writer_stream;
        for outbound in ev_rx {
            match outbound {
                CtrlOutbound::Event(line) => {
                    if writeln!(w, "{line}").is_err() {
                        break;
                    }
                }
                CtrlOutbound::Flush(ack) => {
                    let _ = w.flush();
                    let _ = ack.send(());
                }
            }
        }
        let _ = w.shutdown();
    });
    let session = CtrlClientSession {
        stream,
        transport,
        merged_tx,
        ctrl_clients,
        control_credential,
        player_status,
        shared_queue,
        role,
        owner_settings,
    };
    std::thread::spawn(move || session.run(&peer, ev_tx));
}
