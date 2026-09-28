use super::control_queue::unified_queue_state_for_peer;
use super::core::{DaemonEvent, SharedQueueState};
use crate::ctrl::{ClientRegistry, CtrlOutbound, CtrlTransport};
use mbv_ctrl::{CtrlAudiobookshelfCapabilities, CtrlCmd, CtrlEvent, CtrlHello};
use mbv_net::stream::SocketStream;
use std::io::{BufRead, BufReader, Write};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

fn ctrl_client_capabilities(
    line: &str,
    control_credential: Option<&str>,
) -> Option<(CtrlAudiobookshelfCapabilities, bool)> {
    match serde_json::from_str::<CtrlCmd>(line) {
        Ok(CtrlCmd::Hello(info)) => {
            if let Err(e) = info.validate_peer() {
                tracing::warn!(name: "daemon.ctrl_client.rejected", target: "daemon", error = %e, "ctrl client rejected");
                return None;
            }
            if let Some(control_credential) = control_credential {
                if info.control_token.is_none() {
                    tracing::warn!(name: "daemon.ctrl_client.rejected", target: "daemon", "ctrl client rejected: missing Control credential");
                    return None;
                }
                if let Err(e) = info.validate_control_credential(control_credential) {
                    tracing::warn!(name: "daemon.ctrl_client.rejected", target: "daemon", error = %e, "ctrl client rejected");
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
            ))
        }
        Ok(_) => {
            tracing::warn!(name: "daemon.ctrl_client.rejected", target: "daemon", "ctrl client rejected: missing protocol hello");
            None
        }
        Err(e) => {
            tracing::warn!(name: "daemon.ctrl_client_hello.invalid", target: "daemon", error = %e, "invalid ctrl protocol hello");
            None
        }
    }
}

fn send_initial_queue_state(
    player_status: &Arc<Mutex<mbv_ctrl::player::PlayerStatus>>,
    shared_queue: &SharedQueueState,
    ev_tx: &mpsc::Sender<CtrlOutbound>,
    supports_abs_queue: bool,
    supports_abs_book_queue: bool,
) {
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
    if let Ok(init_json) = serde_json::to_string(&init_event) {
        let _ = ev_tx.send(CtrlOutbound::Event(init_json));
    }
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

pub(crate) fn spawn_ctrl_client(
    stream: SocketStream,
    transport: CtrlTransport,
    merged_tx: mpsc::Sender<DaemonEvent>,
    ctrl_clients: ClientRegistry,
    control_credential: Option<String>,
    player_status: Arc<Mutex<mbv_ctrl::player::PlayerStatus>>,
    shared_queue: SharedQueueState,
    audio_only: bool,
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
    std::thread::spawn(move || {
        let reader = BufReader::new(stream);
        let mut lines = reader.lines();
        let Some(Ok(line)) = lines.next() else {
            return;
        };
        let Some((audiobookshelf, supports_owner_queue_load)) =
            ctrl_client_capabilities(&line, control_credential.as_deref())
        else {
            return;
        };

        send_initial_queue_state(
            &player_status,
            &shared_queue,
            &ev_tx,
            audiobookshelf.queue,
            audiobookshelf.book_queue,
        );
        let reply_tx = ev_tx.clone();
        let client_id = ctrl_clients.lock().unwrap().connect(
            ev_tx,
            transport,
            audiobookshelf,
            supports_owner_queue_load,
        );
        tracing::info!(
            name: "ctrl.client.connected",
            target: "ctrl",
            client = %client_id,
            peer = %peer,
            "ctrl client connected"
        );

        for line in lines {
            let Ok(line) = line else { break };
            if line.is_empty() {
                continue;
            }
            match serde_json::from_str::<CtrlCmd>(&line) {
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
    });
}
