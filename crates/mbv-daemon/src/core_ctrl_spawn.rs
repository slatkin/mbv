use super::control_queue::unified_queue_state_for_peer;
use super::core::{DaemonEvent, SharedQueueState};
use crate::ctrl::{ClientRegistry, CtrlConnectionRole, CtrlOutbound, CtrlTransport, SwapSurface};
use crate::{DaemonRole, OwnerSettingsReader};
use mbv_ctrl::{CtrlAudiobookshelfCapabilities, CtrlCmd, CtrlEvent, CtrlHello};
use mbv_net::stream::SocketStream;
use std::io::{BufRead, BufReader, Write};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Deadline for an accepted Client to deliver its protocol Hello.
const CTRL_HELLO_DEADLINE: Duration = Duration::from_secs(10);

/// Interval the accepted stream allows between individual command writes.
const CTRL_WRITE_TIMEOUT: Duration = Duration::from_secs(30);

/// Decode one inbound ctrl line as a `CtrlCmd` — the single decode entry the
/// Hello classification and the per-line command loop share. The
/// ctrl-protocol-conformance corpus check in `tests/` pins this exact
/// function (design D6).
pub fn decode_ctrl_cmd(line: &str) -> Result<CtrlCmd, serde_json::Error> {
    serde_json::from_str::<CtrlCmd>(line)
}

/// What one decoded Hello classifies into for admission and registration
/// (tray-pin-swap design D7).
struct HelloClassification {
    audiobookshelf: CtrlAudiobookshelfCapabilities,
    capabilities: crate::ctrl::PeerHelloCapabilities,
    role: CtrlConnectionRole,
    swap_surface: Option<SwapSurface>,
    swap_token: Option<String>,
}

/// Capabilities this runtime's daemon Hello advertises. Decided once at
/// startup and passed to every ctrl accept (cast-owner-output design D2/D3:
/// `playback-output` only when the runtime supplies a Cast connector;
/// emby-session-owner-output design D2: `emby-session-output` only when the
/// runtime hook enables it).
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct DaemonHelloCaps {
    pub(crate) audio_only: bool,
    pub(crate) playback_output: bool,
    pub(crate) emby_session_output: bool,
}

fn ctrl_client_capabilities(
    line: &str,
    control_credential: Option<&str>,
) -> Option<HelloClassification> {
    match decode_ctrl_cmd(line) {
        Ok(CtrlCmd::Hello(info)) => {
            if !hello_credentials_valid(&info, control_credential) {
                return None;
            }
            let role = connection_role_from_hello(&info)?;
            let audiobookshelf = CtrlAudiobookshelfCapabilities {
                queue: info.supports_abs_queue(),
                progress: info.supports_abs_progress(),
                book_queue: info.supports_abs_book_queue(),
                book_progress: info.supports_abs_book_progress(),
            };
            Some(HelloClassification {
                audiobookshelf,
                capabilities: crate::ctrl::PeerHelloCapabilities {
                    owner_queue_source_update: info.supports_owner_queue_source_update(),
                    playback_output: info.supports_playback_output(),
                    emby_session_output: info.supports_emby_session_output(),
                    named_queues: crate::ctrl::NamedQueues::of(info.supports_named_queues()),
                    queue_transport: crate::ctrl::QueueTransport {
                        abs_episode: audiobookshelf.queue,
                        abs_book: audiobookshelf.book_queue,
                        twitch: info.supports_twitch_queue(),
                    },
                },
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

/// Which swap surface the Hello advertises. `Pinned` comes from the
/// `pinned-surface` capability alone: a pinned Client that does not
/// advertise `pin-swap` (a pre-upgrade pinned Client) is still an Unpin
/// target — it never answers `SwapPrepared`, so the swap abandons at the
/// deadline (spec pin-swap "Attached pinned Client does not support Pin
/// swap"). A Terminal surface requires `pin-swap`, so an old terminal Client
/// is never a Pin target.
fn swap_surface_from_hello(info: &CtrlHello) -> Option<SwapSurface> {
    if info.supports_pinned_surface() {
        Some(SwapSurface::Pinned)
    } else if info.supports_pin_swap() {
        Some(SwapSurface::Terminal)
    } else {
        None
    }
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
    // A Hello carrying the pending one-shot token is the replacement Client
    // the Owner itself started for a Pin swap (design D4): it is admitted
    // despite `ExclusiveOwner`, and the token is spent whatever the
    // admission outcome — a second Hello presenting it finds nothing
    // pending. Under Stay-alive on, or with no Client attached, nothing
    // refuses the replacement, but the swap still completes when it
    // attaches. The token travels only through the child's environment and
    // the local Unix socket.
    if transport == CtrlTransport::Local
        && connection_role == CtrlConnectionRole::Client
        && clients.pending_swap().take_matching(hello_swap_token)
        && let Some(token) = hello_swap_token
    {
        return AdmissionOutcome::AdmittedReplacement(token.to_string());
    }
    if role == DaemonRole::Local && !stay_alive && clients.has_driver() && !admin_connection {
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
    hello: &HelloClassification,
) -> Option<String> {
    let status = player_status.lock().unwrap().clone();
    let q = shared_queue.queue.lock().unwrap().clone();
    let source = shared_queue.source.lock().unwrap().clone();
    let observed_active_slot = *shared_queue.observed_active_slot.lock().unwrap();
    let lineage = *shared_queue.lineage.lock().unwrap();
    let init_event = unified_queue_state_for_peer(
        None,
        &status,
        &q,
        &source,
        lineage,
        observed_active_slot,
        shared_queue.start_slot(),
        None,
        None,
        hello.capabilities.queue_transport,
    );
    serde_json::to_string(&init_event).ok()
}

/// A Client that negotiated `playback-output` receives the owner's active
/// output on connect, before any command (cast-owner-output design D3, spec
/// "Client connects to an owner that is casting"). A peer that did not
/// negotiate `emby-session-output` receives `Local` in place of an Emby
/// Session output (emby-session-owner-output design D3). Split from the
/// connect path so that path stays under the line-count lint.
fn seed_playback_output(
    shared_queue: &SharedQueueState,
    hello: &HelloClassification,
    ev_tx: &mpsc::Sender<CtrlOutbound>,
) {
    if !hello.capabilities.playback_output {
        return;
    }
    let state = shared_queue.playback_output.lock().unwrap().clone();
    let state = crate::ctrl::output_state_for_peer(&state, hello.capabilities.emby_session_output);
    if let Ok(json) = serde_json::to_string(&CtrlEvent::PlaybackOutput(state)) {
        let _ = ev_tx.send(CtrlOutbound::Event(json));
    }
}

/// A Client that negotiated `named-queues` receives the Owner's Named-queue
/// list on connect, before any command (gui-named-queues design D3, "sent on
/// attach"). The list is the one the Owner deposited in `SharedQueueState` at
/// startup and at every change, so the accept thread needs no access to the
/// Owner's store. A peer without the capability receives nothing, and a
/// packaged Owner, which holds one queue and publishes no list, has nothing to
/// send.
fn seed_named_queue_list(
    shared_queue: &SharedQueueState,
    hello: &HelloClassification,
    ev_tx: &mpsc::Sender<CtrlOutbound>,
) {
    if !hello.capabilities.named_queues.is_supported() {
        return;
    }
    let Some(mut seed) = shared_queue.named_queue_list() else {
        return;
    };
    // The current queue's slots live in this shared state, not in the seed, so
    // its length is read here instead of carried over from the last publish. A
    // resting queue's length can only change through a command that publishes
    // the list, so every other entry is current.
    let current_len = shared_queue.queue.lock().unwrap().len();
    if let Some(entry) = seed
        .queues
        .iter_mut()
        .find(|entry| entry.id == seed.current)
    {
        entry.len = current_len;
    }
    if let Ok(json) = serde_json::to_string(&seed.event()) {
        let _ = ev_tx.send(CtrlOutbound::Event(json));
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
        let mut reader = BufReader::new(stream);
        let mut hello_line = String::new();
        let Ok(read) = reader.read_line(&mut hello_line) else {
            return;
        };
        if read == 0 {
            return;
        }
        // Admitted Clients may be idle indefinitely.
        let _ = reader.get_ref().set_read_timeout(None);
        let Some(hello) =
            ctrl_client_capabilities(hello_line.trim(), control_credential.as_deref())
        else {
            return;
        };
        if owner_action_refused_over_tcp(transport, hello.role) {
            return;
        }
        let stay_alive = role != DaemonRole::Local || (owner_settings)().stay_alive;
        let initial_state = initial_queue_state_json(&player_status, &shared_queue, &hello);

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
        seed_playback_output(&shared_queue, &hello, &ev_tx);
        seed_named_queue_list(&shared_queue, &hello, &ev_tx);
        let reply_tx = ev_tx.clone();
        let client_id = clients.connect_with_role(
            ev_tx,
            transport,
            hello.audiobookshelf.progress,
            hello.audiobookshelf.book_progress,
            hello.capabilities,
            hello.swap_surface,
            hello.role,
        );
        drop(clients);
        log_ctrl_client_connected(client_id, peer);

        for line in reader.lines() {
            let Ok(line) = line else { break };
            if line.is_empty() {
                continue;
            }
            match decode_ctrl_cmd(&line) {
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

/// The daemon Hello this runtime advertises. Built from the runtime's
/// startup-decided capabilities and its role, so one function decides what a
/// Client can negotiate with the Owner it just found.
///
/// `named-queues` is advertised only by [`DaemonRole::Local`]: a packaged
/// `mbvd` keeps one queue and offers no create, rename, or close of its queues
/// (gui-named-queues design D3, spec `named-queues` "Named queues are
/// local-Owner only").
fn daemon_hello(
    caps: DaemonHelloCaps,
    role: DaemonRole,
    control_credential: Option<&str>,
) -> mbv_ctrl::CtrlHello {
    let mut daemon_hello = mbv_ctrl::CtrlHello::current();
    // The Owner runs Owner actions (tray-pin-swap design D7); a Client reads
    // this capability off the server Hello to report a restart requirement
    // instead of sending `RunOwnerAction` at an Owner that cannot run it.
    daemon_hello
        .capabilities
        .push(mbv_ctrl::CTRL_CAP_OWNER_ACTION.to_string());
    if control_credential.is_none() {
        daemon_hello
            .capabilities
            .retain(|cap| cap != mbv_ctrl::CTRL_CAP_CONTROL_AUTH);
    }
    if caps.audio_only {
        daemon_hello
            .capabilities
            .push(mbv_ctrl::CTRL_CAP_AUDIO_ONLY.to_string());
    }
    // Only a runtime that supplies a Cast connector offers the playback
    // output (cast-owner-output design D2).
    if caps.playback_output {
        daemon_hello
            .capabilities
            .push(mbv_ctrl::CTRL_CAP_PLAYBACK_OUTPUT.to_string());
    }
    // Only a runtime whose hook enables the Emby Session output offers it.
    if caps.emby_session_output {
        daemon_hello
            .capabilities
            .push(mbv_ctrl::CTRL_CAP_EMBY_SESSION_OUTPUT.to_string());
    }
    // A runtime that hosts a remote output can switch to it and hand nothing
    // off (switch-output-without-hand-off design D4). Packaged `mbvd` hosts
    // none, so it advertises neither capability.
    if caps.playback_output || caps.emby_session_output {
        daemon_hello
            .capabilities
            .push(mbv_ctrl::CTRL_CAP_OUTPUT_SWITCH.to_string());
    }
    // Only the Local Owner holds Named queues.
    if role == DaemonRole::Local {
        daemon_hello
            .capabilities
            .push(mbv_ctrl::CTRL_CAP_NAMED_QUEUES.to_string());
    }
    daemon_hello
}

pub(crate) fn spawn_ctrl_client(
    stream: SocketStream,
    transport: CtrlTransport,
    merged_tx: mpsc::Sender<DaemonEvent>,
    ctrl_clients: ClientRegistry,
    control_credential: Option<String>,
    player_status: Arc<Mutex<mbv_ctrl::player::PlayerStatus>>,
    shared_queue: SharedQueueState,
    caps: DaemonHelloCaps,
    role: DaemonRole,
    owner_settings: OwnerSettingsReader,
) {
    let peer = ctrl_peer_identity(&stream);
    let _ = stream.set_read_timeout(Some(CTRL_HELLO_DEADLINE));
    let _ = stream.set_write_timeout(Some(CTRL_WRITE_TIMEOUT));
    let Ok(writer_stream) = stream.try_clone() else {
        return;
    };
    let (ev_tx, ev_rx) = mpsc::channel::<CtrlOutbound>();

    let daemon_hello = daemon_hello(caps, role, control_credential.as_deref());
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
mod tests;
