#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;

use mbv_ctrl::player::{PlayerEvent, PlayerStatus};
use mbv_ctrl::{
    CtrlCmd, CtrlCompatibility, CtrlEvent, CtrlHello, DisconnectReason, PlaybackIntent,
    UnifiedQueueStateData,
};
use mbv_net::stream::SocketStream;

use crate::RemotePlayer;

// Hard wall-clock bound on the post-connect protocol handshake (hello
// exchange + initial state), independent of `endpoint::DAEMON_TCP_CONNECT_TIMEOUT`
// -- that constant only bounds the initial TCP-level connect, not the
// blocking `read_line` calls that follow it (issue #191 fix #5). A stalled
// daemon on localhost/LAN (user-configured, not a public/flaky server) is a
// rarer and more clearly-broken scenario than a slow Emby server, so this is
// tighter than `EmbyClient::AUTHENTICATE_HARD_BOUND`.
const DAEMON_HANDSHAKE_HARD_BOUND: Duration = Duration::from_secs(5);

mod endpoint;

pub use endpoint::{DaemonEndpoint, resolve_library_route};

/// Performs the daemon control-protocol handshake (hello exchange, then the
/// initial state) on `stream`, returning a reader ready for the long-running
/// event-reading loop plus the initial `UnifiedQueueState`. Split out of
/// `connect_endpoint` so it can run on a worker thread bounded by
/// `DAEMON_HANDSHAKE_HARD_BOUND` (issue #191 fix #5), and so it can be tested
/// directly against a real stalled `TcpListener` without going through
/// `connect_endpoint`'s full setup.
pub(crate) fn perform_handshake<F>(
    stream: SocketStream,
    load_control_token: F,
) -> Result<(BufReader<SocketStream>, CtrlEvent, CtrlCompatibility), crate::RemotePlayerError>
where
    F: FnOnce() -> Result<String, crate::RemotePlayerError>,
{
    perform_handshake_with_role(stream, load_control_token, false)
}

fn perform_service_setup_admin_handshake<F>(
    stream: SocketStream,
    load_control_token: F,
) -> Result<(BufReader<SocketStream>, CtrlEvent, CtrlCompatibility), crate::RemotePlayerError>
where
    F: FnOnce() -> Result<String, crate::RemotePlayerError>,
{
    perform_handshake_with_role(stream, load_control_token, true)
}

fn perform_handshake_with_role<F>(
    stream: SocketStream,
    load_control_token: F,
    service_setup_admin: bool,
) -> Result<(BufReader<SocketStream>, CtrlEvent, CtrlCompatibility), crate::RemotePlayerError>
where
    F: FnOnce() -> Result<String, crate::RemotePlayerError>,
{
    let mut reader = BufReader::new(stream);
    let ctrl_compatibility = read_server_hello(&mut reader)?;
    send_client_hello(
        &mut reader,
        &ctrl_compatibility,
        load_control_token,
        service_setup_admin,
    )?;
    let state_event = read_initial_state(&mut reader)?;

    Ok((reader, state_event, ctrl_compatibility))
}

fn read_server_hello(
    reader: &mut BufReader<SocketStream>,
) -> Result<CtrlCompatibility, crate::RemotePlayerError> {
    let mut first_line = String::new();
    reader.read_line(&mut first_line).map_err(|e| {
        crate::RemotePlayerError::protocol(format!("failed to read daemon protocol hello: {e}"))
    })?;
    if first_line.trim().is_empty() {
        return Err(crate::RemotePlayerError::protocol(
            "daemon closed connection before protocol hello",
        ));
    }
    let hello = serde_json::from_str::<CtrlEvent>(first_line.trim_end()).map_err(|e| {
        crate::RemotePlayerError::protocol(format!("invalid daemon protocol hello: {e}"))
    })?;
    let CtrlEvent::Hello(info) = hello else {
        return Err(crate::RemotePlayerError::protocol(
            "daemon did not send protocol hello",
        ));
    };
    info.validate_peer()?;
    let mut compatibility = info.compatibility()?;
    compatibility.supports_lifecycle_shutdown = info.supports_lifecycle_shutdown();
    compatibility.supports_audio_only = info.supports_audio_only();
    compatibility.supports_control_auth = info.supports_control_auth();
    compatibility.supports_owner_queue_load = info.supports_owner_queue_load();
    compatibility.supports_answered_queue_ops = info.supports_answered_queue_ops();
    tracing::info!(name: "remote.daemon_protocol_validation.succeeded", target: "remote", protocol_version = info.protocol_version, app_version = %info.app_version, capabilities = ?info.capabilities, "daemon protocol validated");
    Ok(compatibility)
}

fn send_client_hello<F>(
    reader: &mut BufReader<SocketStream>,
    compatibility: &CtrlCompatibility,
    load_control_token: F,
    service_setup_admin: bool,
) -> Result<(), crate::RemotePlayerError>
where
    F: FnOnce() -> Result<String, crate::RemotePlayerError>,
{
    let control_token = compatibility
        .supports_control_auth
        .then(load_control_token)
        .transpose()?;
    let mut client_hello = if service_setup_admin {
        CtrlHello::current_service_setup_admin(control_token)
    } else if let Some(control_token) = control_token {
        CtrlHello::current_control_client(control_token)
    } else {
        CtrlHello::current()
    };
    client_hello.protocol_version = compatibility.client_protocol_version;
    let client_hello = serde_json::to_string(&CtrlCmd::Hello(client_hello))
        .map_err(|e| crate::RemotePlayerError::protocol(e.to_string()))?;
    // Write via the same handle the `BufReader` wraps (`get_mut()`) rather
    // than a second `try_clone()`'d handle -- the handshake is strictly
    // sequential (read hello -> write client hello -> read state) with no
    // concurrent access from another thread during this phase, so there's
    // nothing a second handle buys here beyond an extra fallible call.
    writeln!(reader.get_mut(), "{client_hello}").map_err(|e| {
        crate::RemotePlayerError::protocol(format!("failed to send daemon protocol hello: {e}"))
    })
}

fn read_initial_state(
    reader: &mut BufReader<SocketStream>,
) -> Result<CtrlEvent, crate::RemotePlayerError> {
    let mut state_line = String::new();
    reader.read_line(&mut state_line).map_err(|e| {
        crate::RemotePlayerError::protocol(format!("failed to read daemon initial state: {e}"))
    })?;
    if state_line.trim().is_empty() {
        return Err(crate::RemotePlayerError::protocol(
            "daemon closed connection before initial state",
        ));
    }
    let event = serde_json::from_str::<CtrlEvent>(state_line.trim_end()).map_err(|e| {
        crate::RemotePlayerError::protocol(format!("invalid daemon initial state: {e}"))
    })?;
    match event {
        CtrlEvent::Disconnected {
            reason: DisconnectReason::ExclusiveOwner { pid },
        } => Err(crate::RemotePlayerError::exclusive_owner(pid)),
        CtrlEvent::Disconnected {
            reason: DisconnectReason::OwnerShuttingDown,
        } => Err(crate::RemotePlayerError::owner_shutting_down()),
        event => Ok(event),
    }
}

/// Best-effort signal to a running same-user Local daemon to reread its own
/// owner-local Service storage. A single non-blocking connect attempt is made;
/// when no Local daemon is reachable the call returns `Ok(())` so a bare-mode
/// commit proceeds without a daemon. When a daemon is reachable, the
/// control-auth handshake runs, `ApplyServiceSetup` is sent, and the
/// applied/rejected acknowledgement is awaited. Any failure after the connect
/// reports a restart requirement; the caller's durable commit is untouched.
pub fn signal_local_daemon_service_setup(
    kind: mbv_queue::ServiceKind,
    revision: u64,
) -> Result<(), crate::RemotePlayerError> {
    let path = PathBuf::from(mbv_config::control_socket_path());
    let Ok(stream) = UnixStream::connect(&path) else {
        return Ok(());
    };
    stream
        .set_read_timeout(Some(Duration::from_secs(6)))
        .map_err(|error| {
            crate::RemotePlayerError::restart_required(format!(
                "restart required (cannot read local daemon ctrl): {error}"
            ))
        })?;
    let (mut reader, _state, _compatibility) =
        perform_service_setup_admin_handshake(SocketStream::Unix(stream), || {
            Ok(mbv_config::load_or_create_control_credential()?)
        })
        .map_err(|error| {
            crate::RemotePlayerError::restart_required(format!(
                "restart required (local daemon handshake failed): {error}"
            ))
        })?;
    let request =
        serde_json::to_string(&CtrlCmd::ApplyServiceSetup { kind, revision }).map_err(|error| {
            crate::RemotePlayerError::restart_required(format!(
                "restart required (cannot serialize setup request): {error}"
            ))
        })?;
    writeln!(reader.get_mut(), "{request}")
        .and_then(|()| reader.get_mut().flush())
        .map_err(|error| {
            crate::RemotePlayerError::restart_required(format!(
                "restart required (cannot send setup request): {error}"
            ))
        })?;
    await_service_setup_acknowledgement(&mut reader)
}

fn await_service_setup_acknowledgement(
    reader: &mut BufReader<SocketStream>,
) -> Result<(), crate::RemotePlayerError> {
    for next in reader.lines() {
        let line = next.map_err(|_error| {
            crate::RemotePlayerError::restart_required(
                "restart required (setup acknowledgement unavailable)",
            )
        })?;
        let event = serde_json::from_str::<CtrlEvent>(&line).map_err(|_error| {
            crate::RemotePlayerError::restart_required(
                "restart required (invalid setup acknowledgement)",
            )
        })?;
        match event {
            CtrlEvent::ServiceSetupApplied { .. } => return Ok(()),
            CtrlEvent::ServiceSetupRejected { reason, .. } => {
                return Err(crate::RemotePlayerError::restart_required(format!(
                    "restart required (live setup rejected: {reason:?})"
                )));
            }
            _ => {}
        }
    }
    Err(crate::RemotePlayerError::restart_required(
        "restart required (setup acknowledgement unavailable)",
    ))
}

fn apply_ctrl_event(
    ev: CtrlEvent,
    status: &Arc<Mutex<PlayerStatus>>,
    unified_queue: &Arc<Mutex<Option<UnifiedQueueStateData>>>,
    event_tx: &mpsc::Sender<PlayerEvent>,
    pending_playback: &Arc<Mutex<HashMap<u64, PlaybackIntent>>>,
    notify: bool,
) {
    match ev {
        CtrlEvent::Hello(_) => {
            tracing::warn!(name: "remote.daemon_protocol_hello.unexpected", target: "remote", "unexpected daemon protocol hello after negotiation");
        }
        CtrlEvent::StatusOnly(s) => {
            let mut current = status.lock().unwrap();
            let current_idx = current.current_idx;
            let queue_len = current.queue_len;
            *current = s;
            current.current_idx = current_idx;
            current.queue_len = queue_len;
        }
        CtrlEvent::Player(pe) => apply_player_event(pe, status, event_tx, notify),
        CtrlEvent::CommandRejected(reason) => {
            send_if_notifying(notify, event_tx, PlayerEvent::CommandRejected(reason));
        }
        CtrlEvent::PlaybackIntent(event) => {
            // A coalesced request is terminal for that request identity too;
            // the canonical request remains tracked separately by the daemon.
            pending_playback.lock().unwrap().remove(&event.request_id);
            send_if_notifying(notify, event_tx, PlayerEvent::PlaybackIntent(event));
        }
        CtrlEvent::PipePlaybackStatus(status_event) => send_if_notifying(
            notify,
            event_tx,
            PlayerEvent::PipePlaybackStatus(status_event),
        ),
        CtrlEvent::QueueOpResult { op, outcome } => {
            if let mbv_ctrl::QueueOpOutcome::Applied(state) = &outcome {
                apply_unified_queue_state(
                    (**state).clone(),
                    status,
                    unified_queue,
                    event_tx,
                    notify,
                );
            }
            send_if_notifying(notify, event_tx, PlayerEvent::QueueOpResult { op, outcome });
        }
        CtrlEvent::ShutdownAccepted | CtrlEvent::ShutdownRejected { .. } => {
            // Shutdown replies are handled by RemotePlayer's request-completion path.
        }
        CtrlEvent::ServiceSetupApplied { .. } | CtrlEvent::ServiceSetupRejected { .. } => {
            tracing::debug!(name: "remote.service_reconciliation_event.ignored", target: "remote", "ignoring owner-service reconciliation event");
        }
        CtrlEvent::Disconnected { reason } => {
            apply_disconnected_event(reason, event_tx, notify);
        }
        CtrlEvent::UnifiedQueueLoadResult { request_id, result } => send_if_notifying(
            notify,
            event_tx,
            PlayerEvent::UnifiedQueueLoadResult { request_id, result },
        ),
        CtrlEvent::UnifiedQueueState(unified) => {
            // Retain the canonical snapshot for TUI and reconnect paths.
            apply_unified_queue_state(unified, status, unified_queue, event_tx, notify);
        }
        CtrlEvent::AudiobookshelfProgress(event) => {
            // Dormant: forwarded for a future browse-reconciliation consumer.
            // Does not touch `status` or `unified_queue`.
            send_if_notifying(notify, event_tx, PlayerEvent::AudiobookshelfProgress(event));
        }
        CtrlEvent::AudiobookshelfBookProgress(event) => send_if_notifying(
            notify,
            event_tx,
            PlayerEvent::AudiobookshelfBookProgress(event),
        ),
    }
}

fn send_if_notifying(notify: bool, event_tx: &mpsc::Sender<PlayerEvent>, event: PlayerEvent) {
    if notify {
        let _ = event_tx.send(event);
    }
}

fn apply_player_event(
    event: PlayerEvent,
    status: &Arc<Mutex<PlayerStatus>>,
    event_tx: &mpsc::Sender<PlayerEvent>,
    notify: bool,
) {
    match &event {
        PlayerEvent::Stopped { .. } => status.lock().unwrap().active = false,
        // `TrackChanged` now names a `QueueSlotId`, not an ordinal; this read
        // loop has no queue to resolve it against. The forwarded event reaches
        // the Client, whose queue coordinates are replaced by the next unified
        // snapshot rather than merged from this event.
        PlayerEvent::PausedChanged(paused) => status.lock().unwrap().paused = *paused,
        _ => {}
    }
    send_if_notifying(notify, event_tx, event);
}

fn apply_disconnected_event(
    reason: DisconnectReason,
    event_tx: &mpsc::Sender<PlayerEvent>,
    notify: bool,
) {
    if !notify {
        return;
    }
    let msg = disconnect_reason_message(reason).to_string();
    match reason {
        DisconnectReason::TakenOverByEmbyRemote => {
            let _ = event_tx.send(PlayerEvent::EmbyAuthorityTaken(msg));
        }
        // Handled by the reader thread's end-of-loop logic below
        // (`is_structured_disconnect`), which sends
        // `PlayerEvent::DaemonShutdownAnnounced` once the connection closes.
        DisconnectReason::DaemonShutdown
        | DisconnectReason::ExclusiveOwner { .. }
        | DisconnectReason::OwnerShuttingDown => {}
    }
}

fn disconnect_reason_message(reason: DisconnectReason) -> &'static str {
    match reason {
        DisconnectReason::TakenOverByEmbyRemote => {
            "Emby remote control took over — returned to local mode"
        }
        DisconnectReason::DaemonShutdown => "the daemon was stopped",
        DisconnectReason::ExclusiveOwner { .. } => "the owner already has a client",
        DisconnectReason::OwnerShuttingDown => "the owner is shutting down",
    }
}

/// Applies a unified-queue state snapshot to the status and canonical queue,
/// and — when `notify` is true — emits a `PlayerEvent::UnifiedQueueUpdated` carrying
/// tagged queue, slot identity, active slot, and revision so the TUI can
/// reconstruct the canonical queue without decomposing it into Emby-only
/// shapes.
fn apply_unified_queue_state(
    mut unified: UnifiedQueueStateData,
    status: &Arc<Mutex<PlayerStatus>>,
    unified_queue: &Arc<Mutex<Option<UnifiedQueueStateData>>>,
    event_tx: &mpsc::Sender<PlayerEvent>,
    notify: bool,
) {
    // Normalize the compatibility coordinates on the snapshot itself. Every
    // projection below, including the event sent to the Client, then observes
    // the same revision, slots, observed slot, and status.
    unified.status.queue_len = unified.slots.len();
    if let Some(active_index) = unified.active_slot.and_then(|slot_id| {
        unified
            .slots
            .iter()
            .position(|slot| slot.slot_id == slot_id)
    }) {
        unified.status.current_idx = active_index;
    }

    let next_status = unified.status.clone();

    *status.lock().unwrap() = next_status;
    *unified_queue.lock().unwrap() = Some(unified.clone());

    if notify {
        // Emit the full unified state so the TUI can reconstruct the
        // canonical queue (tagged QueueItems, slot identity, active slot,
        // revision) without losing Feed entries or canonical order.
        let _ = event_tx.send(PlayerEvent::UnifiedQueueUpdated(Box::new(unified)));
    }
}

pub(crate) fn connect_endpoint(
    endpoint: &DaemonEndpoint,
) -> Result<(RemotePlayer, mpsc::Receiver<PlayerEvent>), crate::RemotePlayerError> {
    let stream = endpoint.connect_stream()?;
    tracing::info!(name: "remote.daemon_connection.started", target: "remote", endpoint = %endpoint, "connecting to daemon endpoint");
    connect_stream(stream)
}

struct ReaderThreadState {
    status: Arc<Mutex<PlayerStatus>>,
    unified_queue: Arc<Mutex<Option<UnifiedQueueStateData>>>,
    pending_playback: Arc<Mutex<HashMap<u64, PlaybackIntent>>>,
    disconnected: Arc<AtomicBool>,
    disconnect_notified: Arc<AtomicBool>,
    shutdown_announced: Arc<AtomicBool>,
    shutdown_request: Arc<Mutex<Option<mpsc::Sender<crate::ShutdownResponse>>>>,
    event_tx: mpsc::Sender<PlayerEvent>,
}

/// Builds a `RemotePlayer` over an already-connected control stream.
///
/// Split out of `connect_endpoint` so tests can drive the swap/disconnect
/// bookkeeping over an in-memory `UnixStream` pair (`connect_stub_daemon_pair`)
/// instead of a real listener.
fn connect_stream(
    stream: SocketStream,
) -> Result<(RemotePlayer, mpsc::Receiver<PlayerEvent>), crate::RemotePlayerError> {
    // Kept aside for `disconnect()` (#233) -- taken before `stream` is
    // moved into the writer thread below.
    let disconnect_stream = stream.try_clone()?;

    // Correlation peer (D5 "Ctrl connections"): the owner joins its
    // `ctrl.client.connected` line to this side's
    // `ctrl.connection.established` line by an exact `peer=` match.
    // Unix endpoints peer by this process's pid; TCP endpoints by this side's
    // local `ip:port`, in the owner's format.
    let peer = match &stream {
        SocketStream::Unix(_) => std::process::id().to_string(),
        SocketStream::Tcp(tcp) => tcp
            .local_addr()
            .map_or_else(|_| "unknown".to_string(), |addr| addr.to_string()),
    };

    let status = Arc::new(Mutex::new(PlayerStatus::default()));
    let subtitle_prefs = Arc::new(Mutex::new(mbv_ctrl::player::SubtitlePrefs::default()));
    let unified_queue = Arc::new(Mutex::new(None));
    let disconnected = Arc::new(AtomicBool::new(false));
    let disconnect_notified = Arc::new(AtomicBool::new(false));
    let shutdown_announced = Arc::new(AtomicBool::new(false));
    let next_playback_id = Arc::new(std::sync::atomic::AtomicU64::new(1));
    let next_queue_op_id = Arc::new(std::sync::atomic::AtomicU64::new(1));
    let pending_playback = Arc::new(Mutex::new(HashMap::new()));
    let shutdown_request_tx: Arc<Mutex<Option<mpsc::Sender<crate::ShutdownResponse>>>> =
        Arc::new(Mutex::new(None));

    let (event_tx, event_rx) = mpsc::channel::<PlayerEvent>();
    let (cmd_tx, cmd_rx) = mpsc::channel::<CtrlCmd>();

    // The handshake (hello exchange + initial state) runs on a worker
    // thread bounded by `DAEMON_HANDSHAKE_HARD_BOUND`, independent of
    // `endpoint::DAEMON_TCP_CONNECT_TIMEOUT` -- that only bounds the initial
    // TCP-level connect, not these blocking reads (issue #191 fix #5).
    // `stream` itself is kept untouched on this thread for the writer
    // thread spawned below; a clone goes to the worker thread instead.
    let handshake_stream = stream.try_clone()?;
    let (reader, state_event, ctrl_compatibility) = mbv_net::bounded::run_with_hard_bound_or_error(
        move || {
            perform_handshake(handshake_stream, || {
                Ok(mbv_config::load_or_create_control_credential()?)
            })
        },
        || {
            crate::RemotePlayerError::connection(format!(
                "timed out after {}s",
                DAEMON_HANDSHAKE_HARD_BOUND.as_secs()
            ))
        },
        DAEMON_HANDSHAKE_HARD_BOUND,
    )?;
    tracing::info!(
        name: "ctrl.connection.established",
        target: "ctrl",
        peer = %peer,
        "ctrl connection established"
    );
    apply_ctrl_event(
        state_event,
        &status,
        &unified_queue,
        &event_tx,
        &pending_playback,
        false,
    );

    // Reader thread: deserializes CtrlEvent lines from daemon
    let reader_state = ReaderThreadState {
        status: Arc::clone(&status),
        unified_queue: Arc::clone(&unified_queue),
        pending_playback: Arc::clone(&pending_playback),
        disconnected: Arc::clone(&disconnected),
        disconnect_notified: Arc::clone(&disconnect_notified),
        shutdown_announced: Arc::clone(&shutdown_announced),
        shutdown_request: Arc::clone(&shutdown_request_tx),
        event_tx: event_tx.clone(),
    };
    std::thread::spawn(move || read_remote_events(reader, reader_state));

    // Writer thread: serializes CtrlCmd to daemon
    let disconnected_w = Arc::clone(&disconnected);
    std::thread::spawn(move || {
        write_remote_commands(
            stream,
            &cmd_rx,
            &disconnected_w,
            &disconnect_notified,
            &event_tx,
        );
    });

    Ok((
        RemotePlayer {
            status,
            subtitle_prefs,
            unified_queue,
            cmd_tx,
            disconnected,
            shutdown_announced,
            ctrl_compatibility,
            control_stream: Arc::new(Mutex::new(Some(disconnect_stream))),
            next_playback_id,
            next_queue_op_id,
            pending_playback,
            shutdown_request_tx,
        },
        event_rx,
    ))
}

fn read_remote_events(reader: BufReader<SocketStream>, state: ReaderThreadState) {
    let ReaderThreadState {
        status,
        unified_queue,
        pending_playback,
        disconnected,
        disconnect_notified,
        shutdown_announced,
        shutdown_request,
        event_tx,
    } = state;
    let mut expected_disconnect = false;
    for line in reader.lines() {
        match line {
            Err(_) => break,
            Ok(l) if l.is_empty() => {}
            Ok(l) => {
                let Ok(ev) = serde_json::from_str::<CtrlEvent>(&l) else {
                    tracing::warn!(name: "remote.daemon_event_parse.failed", target: "remote", bytes = l.len(), "unrecognized event from daemon");
                    continue;
                };

                // Handle shutdown request responses directly.
                match &ev {
                    CtrlEvent::ShutdownAccepted => {
                        if let Some(tx) = shutdown_request.lock().unwrap().take() {
                            let _ = tx.send(crate::ShutdownResponse::Accepted);
                        }
                    }
                    CtrlEvent::ShutdownRejected { reason } => {
                        if let Some(tx) = shutdown_request.lock().unwrap().take() {
                            let _ = tx.send(crate::ShutdownResponse::Rejected {
                                reason: reason.clone(),
                            });
                        }
                    }
                    _ => {}
                }

                // Under multi-connection (v5), `Disconnected { TakenOverByEmbyRemote }` is
                // a notification — the connection stays open. Only set expected_disconnect
                // for events that actually close the connection. Exhaustive match ensures
                // new DisconnectReason variants are evaluated.
                let is_structured_disconnect = match &ev {
                    CtrlEvent::Disconnected { reason } => {
                        matches!(reason, DisconnectReason::DaemonShutdown)
                    }
                    _ => false,
                };
                apply_ctrl_event(
                    ev,
                    &status,
                    &unified_queue,
                    &event_tx,
                    &pending_playback,
                    true,
                );
                expected_disconnect |= is_structured_disconnect;
            }
        }
    }
    disconnected.store(true, Ordering::SeqCst);
    pending_playback.lock().unwrap().clear();

    // Resolve any pending shutdown request with Disconnected.
    if let Some(tx) = shutdown_request.lock().unwrap().take() {
        let _ = tx.send(crate::ShutdownResponse::Disconnected);
    }

    tracing::info!(name: "remote.daemon_connection.ended", target: "remote", "daemon disconnected");
    if expected_disconnect {
        // An "expected"/structured disconnect (e.g. an Emby Remote
        // takeover, or a deliberate daemon shutdown) never sends a
        // Stopped PlayerEvent, so nothing else clears `status`.
        // Clear it here, at the source, so
        // every consumer of `status` (not just MPRIS's separate
        // `disconnected_flag()` check in mbv-desktop::mpris) sees an
        // inactive/no-track player immediately rather than stale
        // "still playing" data.
        if let Ok(mut s) = status.lock() {
            s.active = false;
            s.paused = false;
            s.clear_current_item_metadata();
        }
        // `TakenOverByEmbyRemote` never reaches this branch (it does not
        // close the connection), so this structured disconnect is a
        // deliberate daemon shutdown.
        shutdown_announced.store(true, Ordering::SeqCst);
        let _ = event_tx.send(PlayerEvent::DaemonShutdownAnnounced);
    } else if !disconnect_notified.swap(true, Ordering::SeqCst) {
        let _ = event_tx.send(PlayerEvent::RemoteDisconnected(
            mbv_ctrl::player::CONNECTION_LOST_MESSAGE.to_string(),
        ));
    }
}

fn write_remote_commands(
    mut stream: SocketStream,
    cmd_rx: &mpsc::Receiver<CtrlCmd>,
    disconnected: &Arc<AtomicBool>,
    disconnect_notified: &Arc<AtomicBool>,
    event_tx: &mpsc::Sender<PlayerEvent>,
) {
    while let Ok(cmd) = cmd_rx.recv() {
        let Ok(json) = serde_json::to_string(&cmd) else {
            continue;
        };
        if let Err(error) = writeln!(stream, "{json}") {
            tracing::warn!(name: "remote.daemon_command_write.failed", target: "remote", error = %error, "failed to write command to daemon");
            disconnected.store(true, Ordering::SeqCst);
            if !disconnect_notified.swap(true, Ordering::SeqCst) {
                let _ = event_tx.send(PlayerEvent::RemoteDisconnected(
                    mbv_ctrl::player::CONNECTION_LOST_MESSAGE.to_string(),
                ));
            }
            break;
        }
    }
}

/// Test-support: connects a `RemotePlayer` over an in-memory `UnixStream`
/// pair whose peer completes the control handshake and then holds the socket
/// open. The returned join handle completes only once the peer observes EOF,
/// i.e. once `RemotePlayer::disconnect()` (or dropping the owner) shuts the
/// client end down -- the hermetic oracle for "the previous remote was
/// disconnected" without a listener or spawned product process.
///
/// # Panics
///
/// The spawned peer thread panics if `UnixStream::try_clone` fails on the
/// daemon end of the pair, if writing or reading the stub handshake hits an IO
/// error, or if the handshake events cannot be serialized to JSON. The
/// returned `Result` covers only the client end of the pair.
#[cfg(any(test, feature = "test"))]
pub fn connect_stub_daemon_pair() -> Result<
    (
        RemotePlayer,
        mpsc::Receiver<PlayerEvent>,
        std::thread::JoinHandle<()>,
    ),
    String,
> {
    use std::io::Read;
    let (client, daemon) = UnixStream::pair().map_err(|e| e.to_string())?;
    let peer = std::thread::spawn(move || {
        let mut writer = daemon.try_clone().unwrap();
        let mut reader = BufReader::new(daemon);
        let hello = serde_json::to_string(&CtrlEvent::Hello(CtrlHello::current())).unwrap();
        writeln!(writer, "{hello}").unwrap();
        let mut client_hello = String::new();
        reader.read_line(&mut client_hello).unwrap();
        let state = serde_json::to_string(&CtrlEvent::UnifiedQueueState(UnifiedQueueStateData {
            status: PlayerStatus::default(),
            slots: Vec::new(),
            active_slot: None,
            revision: 0,
            source: mbv_queue::QueueSource::Unknown,
            lineage: mbv_queue::QueueLineage::default(),
            in_flight_transition: None,
            queued_latest_transition: None,
        }))
        .unwrap();
        writeln!(writer, "{state}").unwrap();
        let mut buf = [0u8; 256];
        while let Ok(n) = reader.get_mut().read(&mut buf) {
            if n == 0 {
                break;
            }
        }
    });
    let (player, rx) =
        connect_stream(SocketStream::Unix(client)).map_err(|error| error.to_string())?;
    Ok((player, rx, peer))
}
