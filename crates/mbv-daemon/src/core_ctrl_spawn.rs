use super::control_queue::unified_queue_state_for_peer;
use super::core::{DaemonEvent, SharedQueueState};
use crate::ctrl::{ClientRegistry, CtrlConnectionRole, CtrlOutbound, CtrlTransport, SwapSurface};
use crate::{DaemonRole, OwnerSettingsReader};
use mbv_ctrl::{CtrlAudiobookshelfCapabilities, CtrlCmd, CtrlEvent, CtrlHello};
use mbv_net::stream::SocketStream;
use std::io::{BufRead, BufReader, Write};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

/// What one decoded Hello classifies into for admission and registration
/// (tray-pin-swap design D7).
struct HelloClassification {
    audiobookshelf: CtrlAudiobookshelfCapabilities,
    supports_owner_queue_load: bool,
    role: CtrlConnectionRole,
    swap_surface: Option<SwapSurface>,
    swap_token: Option<String>,
}

fn ctrl_client_capabilities(
    line: &str,
    control_credential: Option<&str>,
) -> Option<HelloClassification> {
    match serde_json::from_str::<CtrlCmd>(line) {
        Ok(CtrlCmd::Hello(info)) => {
            if !hello_credentials_valid(&info, control_credential) {
                return None;
            }
            let role = connection_role_from_hello(&info)?;
            Some(HelloClassification {
                audiobookshelf: CtrlAudiobookshelfCapabilities {
                    queue: info.supports_abs_queue(),
                    progress: info.supports_abs_progress(),
                    book_queue: info.supports_abs_book_queue(),
                    book_progress: info.supports_abs_book_progress(),
                },
                supports_owner_queue_load: info.supports_owner_queue_load(),
                role,
                swap_surface: swap_surface_from_hello(&info),
                swap_token: info.swap_token,
            })
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

/// Validates a decoded hello's peer identity and, when a Control credential
/// is required, that credential — logging the specific rejection.
fn hello_credentials_valid(info: &CtrlHello, control_credential: Option<&str>) -> bool {
    if let Err(e) = info.validate_peer() {
        tracing::warn!(name: "daemon.ctrl_client.peer_validation_failed", target: "daemon", error = %e, "ctrl client rejected");
        return false;
    }
    match control_credential {
        None => true,
        Some(control_credential) => control_credential_is_valid(info, control_credential),
    }
}

/// The connection role a Hello advertises; a Hello claiming both admin
/// capabilities is rejected (tray-pin-swap design D7).
fn connection_role_from_hello(info: &CtrlHello) -> Option<CtrlConnectionRole> {
    let service_setup_admin = info.supports_service_setup_admin();
    let owner_action = info.supports_owner_action();
    if service_setup_admin && owner_action {
        tracing::warn!(name: "daemon.ctrl_client.hello_conflicting_admin_roles", target: "daemon", "ctrl client rejected: hello advertises both admin capabilities");
        return None;
    }
    Some(if service_setup_admin {
        CtrlConnectionRole::ServiceSetupAdmin
    } else if owner_action {
        CtrlConnectionRole::OwnerAction
    } else {
        CtrlConnectionRole::Client
    })
}

fn control_credential_is_valid(info: &CtrlHello, control_credential: &str) -> bool {
    if info.control_token.is_none() {
        tracing::warn!(name: "daemon.ctrl_client.control_credential_missing", target: "daemon", "ctrl client rejected: missing Control credential");
        return false;
    }
    if let Err(e) = info.validate_control_credential(control_credential) {
        tracing::warn!(name: "daemon.ctrl_client.control_credential_invalid", target: "daemon", error = %e, "ctrl client rejected");
        return false;
    }
    true
}

/// Which swap surface the Hello advertises, per tray-pin-swap design D2:
/// `Some` only when `pin-swap` is present, `Pinned` only then when
/// `pinned-surface` is also present.
fn swap_surface_from_hello(info: &CtrlHello) -> Option<SwapSurface> {
    let supports = |cap: &str| info.capabilities.iter().any(|advertised| advertised == cap);
    if !supports(mbv_ctrl::CTRL_CAP_PIN_SWAP) {
        return None;
    }
    Some(if supports(mbv_ctrl::CTRL_CAP_PINNED_SURFACE) {
        SwapSurface::Pinned
    } else {
        SwapSurface::Terminal
    })
}

/// What `send_admission_refusal` decided about one Hello.
enum AdmissionOutcome {
    /// The connection proceeds as a normal (or admin-role) connection.
    Admitted,
    /// The refusal event was sent; the connection ends.
    Refused,
    /// Admitted as the Pin swap's replacement Client: the pending one-shot
    /// token was consumed (design D4). The caller reports the attach to the
    /// loop, which completes the swap.
    AdmittedReplacement(String),
}

fn send_admission_refusal(
    clients: &crate::ctrl::CtrlClients,
    role: DaemonRole,
    transport: CtrlTransport,
    stay_alive: bool,
    connection_role: CtrlConnectionRole,
    hello_swap_token: Option<&str>,
    ev_tx: &crate::ctrl::CtrlSender,
) -> AdmissionOutcome {
    let admin_connection = connection_role != CtrlConnectionRole::Client;
    if role == DaemonRole::Local && clients.shutting_down {
        crate::send_to(
            ev_tx,
            &CtrlEvent::Disconnected {
                reason: mbv_ctrl::DisconnectReason::OwnerShuttingDown,
            },
        );
        return AdmissionOutcome::Refused;
    }
    if role == DaemonRole::Local && !stay_alive && clients.has_driver() && !admin_connection {
        // A Hello carrying the pending one-shot token is the replacement
        // Client the Owner itself started for a Pin swap (design D4): it is
        // admitted despite `ExclusiveOwner`, and the token is spent — a
        // second Hello presenting it finds nothing pending.
        if transport == CtrlTransport::Local
            && connection_role == CtrlConnectionRole::Client
            && let Some(token) = clients.pending_swap().take_matching(hello_swap_token)
        {
            return AdmissionOutcome::AdmittedReplacement(token);
        }
        crate::send_to(
            ev_tx,
            &CtrlEvent::Disconnected {
                reason: mbv_ctrl::DisconnectReason::ExclusiveOwner {
                    pid: std::process::id(),
                },
            },
        );
        return AdmissionOutcome::Refused;
    }
    // The token must be consumed whatever the admission outcome: under
    // Stay-alive on, or with no Client attached, nothing refuses the
    // replacement, but the swap still completes when it attaches (design
    // D4). The token travels only through the child's environment and the
    // local Unix socket.
    if transport == CtrlTransport::Local
        && connection_role == CtrlConnectionRole::Client
        && let Some(token) = clients.pending_swap().take_matching(hello_swap_token)
    {
        return AdmissionOutcome::AdmittedReplacement(token);
    }
    AdmissionOutcome::Admitted
}

/// Refuses an Owner-action Hello that arrived over TCP (spec owner-actions
/// "Over TCP"): the connection closes without a reply, like a failed
/// credential.
fn owner_action_refused_over_tcp(transport: CtrlTransport, role: CtrlConnectionRole) -> bool {
    let refused = role == CtrlConnectionRole::OwnerAction && transport == CtrlTransport::Tcp;
    if refused {
        tracing::warn!(name: "daemon.ctrl_client.owner_action_over_tcp", target: "daemon", "ctrl client rejected: owner actions are local-only");
    }
    refused
}

/// Whether a restricted-role connection may send `cmd`; `Some` is the
/// `CommandRejected` reason (design D7: each admin role allows only its own
/// command).
fn restricted_command_rejection(role: CtrlConnectionRole, cmd: &CtrlCmd) -> Option<String> {
    match role {
        CtrlConnectionRole::Client => None,
        CtrlConnectionRole::ServiceSetupAdmin
            if matches!(cmd, CtrlCmd::ApplyServiceSetup { .. }) =>
        {
            None
        }
        CtrlConnectionRole::ServiceSetupAdmin => {
            Some("service-setup admin connections cannot send other commands".to_string())
        }
        CtrlConnectionRole::OwnerAction if matches!(cmd, CtrlCmd::RunOwnerAction(_)) => None,
        CtrlConnectionRole::OwnerAction => {
            Some("owner action connections cannot send other commands".to_string())
        }
    }
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
        let Some(hello) = ctrl_client_capabilities(&line, control_credential.as_deref()) else {
            return;
        };
        if owner_action_refused_over_tcp(transport, hello.role) {
            return;
        }
        let stay_alive = role != DaemonRole::Local || (owner_settings)().stay_alive;
        let initial_state = initial_queue_state_json(
            &player_status,
            &shared_queue,
            hello.audiobookshelf.queue,
            hello.audiobookshelf.book_queue,
        );

        let mut clients = ctrl_clients.lock().unwrap();
        match send_admission_refusal(
            &clients,
            role,
            transport,
            stay_alive,
            hello.role,
            hello.swap_token.as_deref(),
            &ev_tx,
        ) {
            AdmissionOutcome::Refused => return,
            AdmissionOutcome::AdmittedReplacement(token) => {
                let _ = merged_tx.send(DaemonEvent::PinSwapAdmitted { token });
            }
            AdmissionOutcome::Admitted => {}
        }
        if let Some(initial_state) = initial_state {
            let _ = ev_tx.send(CtrlOutbound::Event(initial_state));
        }
        let reply_tx = ev_tx.clone();
        let client_id = clients.connect_with_role(
            ev_tx,
            transport,
            hello.audiobookshelf,
            hello.supports_owner_queue_load,
            hello.swap_surface,
            hello.role,
        );
        drop(clients);
        log_ctrl_client_connected(client_id, peer);

        for line in lines {
            let Ok(line) = line else { break };
            if line.is_empty() {
                continue;
            }
            match serde_json::from_str::<CtrlCmd>(&line) {
                Ok(cmd) => match restricted_command_rejection(hello.role, &cmd) {
                    Some(reason) => {
                        crate::send_to(&reply_tx, &CtrlEvent::CommandRejected(reason));
                    }
                    None => {
                        let _ = merged_tx.send(DaemonEvent::Ctrl(cmd, client_id, reply_tx.clone()));
                    }
                },
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ctrl::CtrlClients;
    use std::sync::atomic::AtomicBool;

    /// A registry with one local Client attached, so `has_driver` is true and
    /// the `ExclusiveOwner` refusal fires under Stay-alive off.
    fn registry_with_driver() -> (CtrlClients, mpsc::Receiver<CtrlOutbound>) {
        let (merged_tx, _merged_rx) = mpsc::channel::<DaemonEvent>();
        let mut clients = CtrlClients::new(
            merged_tx,
            Arc::new(AtomicBool::new(false)),
            Arc::new(crate::PendingSwapToken::default()),
        );
        let (client_tx, client_rx) = mpsc::channel::<CtrlOutbound>();
        clients.connect_with_role(
            client_tx,
            CtrlTransport::Local,
            CtrlAudiobookshelfCapabilities::default(),
            false,
            None,
            CtrlConnectionRole::Client,
        );
        (clients, client_rx)
    }

    /// Contract: spec daemon-lifecycle "Pin swap with Stay Alive off" /
    /// design D4 — a Hello carrying the pending one-shot swap token is the
    /// replacement Client the Owner itself started, so it is admitted
    /// despite `ExclusiveOwner`, and the refusal event is not sent.
    #[test]
    fn matching_swap_token_is_admitted_despite_exclusive_owner() {
        let (clients, _client_rx) = registry_with_driver();
        clients.pending_swap().publish("token-1".to_string());
        let (ev_tx, ev_rx) = mpsc::channel::<CtrlOutbound>();

        let outcome = send_admission_refusal(
            &clients,
            DaemonRole::Local,
            CtrlTransport::Local,
            false,
            CtrlConnectionRole::Client,
            Some("token-1"),
            &ev_tx,
        );

        assert!(matches!(outcome, AdmissionOutcome::AdmittedReplacement(_)));
        assert!(ev_rx.try_recv().is_err());
    }

    /// Contract: design D4 — the token is consumed at once, so it works for
    /// one connection only; a second Hello presenting the same token is
    /// refused with the exclusive-owner reason like any user-started Client.
    #[test]
    fn a_swap_token_is_spent_on_first_use() {
        let (clients, _client_rx) = registry_with_driver();
        clients.pending_swap().publish("token-1".to_string());
        let (ev_tx, ev_rx) = mpsc::channel::<CtrlOutbound>();

        let first = send_admission_refusal(
            &clients,
            DaemonRole::Local,
            CtrlTransport::Local,
            false,
            CtrlConnectionRole::Client,
            Some("token-1"),
            &ev_tx,
        );
        let second = send_admission_refusal(
            &clients,
            DaemonRole::Local,
            CtrlTransport::Local,
            false,
            CtrlConnectionRole::Client,
            Some("token-1"),
            &ev_tx,
        );

        assert!(matches!(first, AdmissionOutcome::AdmittedReplacement(_)));
        assert!(matches!(second, AdmissionOutcome::Refused));
        // The second Hello was refused with the exclusive-owner reason.
        ev_rx.try_recv().unwrap();
    }

    /// Contract: design D4 — the token is consumed whatever the admission
    /// outcome. Under Stay-alive on nothing refuses the replacement, but the
    /// swap must still complete when it attaches (it guards against the
    /// machine waiting out its deadline after a successful attach).
    #[test]
    fn the_swap_token_is_consumed_when_stay_alive_admits_normally() {
        let (clients, _client_rx) = registry_with_driver();
        clients.pending_swap().publish("token-1".to_string());
        let (ev_tx, _ev_rx) = mpsc::channel::<CtrlOutbound>();

        let outcome = send_admission_refusal(
            &clients,
            DaemonRole::Local,
            CtrlTransport::Local,
            true,
            CtrlConnectionRole::Client,
            Some("token-1"),
            &ev_tx,
        );

        assert!(matches!(outcome, AdmissionOutcome::AdmittedReplacement(_)));
        assert_eq!(
            clients.pending_swap().take_matching(Some("token-1")),
            None,
            "the first Hello spent the token"
        );
    }

    /// Contract: design D4 — the shutting-down refusal still wins, even for
    /// a Hello that carries the pending swap token.
    #[test]
    fn shutting_down_refusal_wins_over_the_swap_token() {
        let (mut clients, _client_rx) = registry_with_driver();
        clients.shutting_down = true;
        clients.pending_swap().publish("token-1".to_string());
        let (ev_tx, ev_rx) = mpsc::channel::<CtrlOutbound>();

        let outcome = send_admission_refusal(
            &clients,
            DaemonRole::Local,
            CtrlTransport::Local,
            false,
            CtrlConnectionRole::Client,
            Some("token-1"),
            &ev_tx,
        );

        assert!(matches!(outcome, AdmissionOutcome::Refused));
        // The shutting-down refusal event was sent to the Hello.
        ev_rx.try_recv().unwrap();
        // The token stays pending: a refused Hello does not spend it.
        assert_eq!(
            clients.pending_swap().take_matching(Some("token-1")),
            Some("token-1".to_string())
        );
    }
}
