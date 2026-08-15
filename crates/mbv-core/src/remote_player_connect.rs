#[cfg(test)]
mod tests {
    include!("remote_player_tests.rs");
    include!("remote_player_tests_socket.rs");
}

use std::collections::HashMap;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use crate::api::EmbyItem;
use crate::ctrl::{
    CtrlCmd, CtrlCompatibility, CtrlEvent, CtrlHello, DisconnectReason, PlaybackIntent,
    UnifiedQueueStateData,
};
use crate::player::{PlayerEvent, PlayerStatus};

use crate::remote_player::RemotePlayer;

pub(crate) enum ControlStream {
    Unix(UnixStream),
    Tcp(TcpStream),
}

impl ControlStream {
    pub(crate) fn try_clone(&self) -> io::Result<Self> {
        match self {
            Self::Unix(stream) => stream.try_clone().map(Self::Unix),
            Self::Tcp(stream) => stream.try_clone().map(Self::Tcp),
        }
    }

    /// Shuts down the underlying socket for both reads and writes (#233).
    /// Unlike dropping a `ControlStream` clone -- which only closes *that*
    /// clone's fd duplicate -- `shutdown` acts on the shared underlying
    /// socket in the kernel, so it unblocks a concurrent blocking `read()`
    /// on any other clone of the same connection immediately.
    pub(crate) fn shutdown(&self) -> io::Result<()> {
        match self {
            Self::Unix(stream) => stream.shutdown(std::net::Shutdown::Both),
            Self::Tcp(stream) => stream.shutdown(std::net::Shutdown::Both),
        }
    }
}

impl Read for ControlStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            Self::Unix(stream) => stream.read(buf),
            Self::Tcp(stream) => stream.read(buf),
        }
    }
}

impl Write for ControlStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            Self::Unix(stream) => stream.write(buf),
            Self::Tcp(stream) => stream.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            Self::Unix(stream) => stream.flush(),
            Self::Tcp(stream) => stream.flush(),
        }
    }
}

const DAEMON_TCP_CONNECT_TIMEOUT: Duration = Duration::from_millis(750);

// Hard wall-clock bound on the post-connect protocol handshake (hello
// exchange + initial state), independent of `DAEMON_TCP_CONNECT_TIMEOUT`
// above -- that constant only bounds the initial TCP-level connect, not the
// blocking `read_line` calls that follow it (issue #191 fix #5). A stalled
// daemon on localhost/LAN (user-configured, not a public/flaky server) is a
// rarer and more clearly-broken scenario than a slow Emby server, so this is
// tighter than `EmbyClient::AUTHENTICATE_HARD_BOUND`.
const DAEMON_HANDSHAKE_HARD_BOUND: Duration = Duration::from_secs(5);

// A local daemon that was *just* launched (via `stay_alive` or auto-detect)
// may have written its PID file (which is what makes it "detected") slightly
// before its ctrl socket is bound. Retry briefly rather than immediately
// falling back to standalone. Explicit remote endpoints (`Unix(path)` /
// `Tcp`) are not retried this way — they represent an already-running,
// user-specified target, not a same-machine process that might still be
// starting up.
const LOCAL_DAEMON_CONNECT_RETRY_TIMEOUT: Duration = Duration::from_secs(1);
const LOCAL_DAEMON_CONNECT_RETRY_INTERVAL: Duration = Duration::from_millis(50);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DaemonEndpoint {
    Local,
    Unix(PathBuf),
    Tcp(SocketAddr),
}

impl DaemonEndpoint {
    pub fn parse(value: &str) -> Result<Self, String> {
        let value = value.trim();
        if value.is_empty() || value == "local" {
            return Ok(Self::Local);
        }
        if let Some(path) = value.strip_prefix("unix://") {
            if path.is_empty() {
                return Err("daemon endpoint unix:// requires a socket path".to_string());
            }
            return Ok(Self::Unix(PathBuf::from(path)));
        }
        if let Some(value) = value.strip_prefix("tcp://") {
            return Self::parse_tcp(value);
        }
        if value.contains("://") {
            return Err(format!(
                "daemon endpoint scheme is not supported yet: {value} (use local, unix:///path, tcp://127.0.0.1:port, or a plain socket path)"
            ));
        }
        Ok(Self::Unix(PathBuf::from(value)))
    }

    fn parse_tcp(value: &str) -> Result<Self, String> {
        let value = value.trim();
        if value.is_empty() {
            return Err("daemon endpoint tcp:// requires a host and port".to_string());
        }

        let (host, port) = value
            .rsplit_once(':')
            .ok_or_else(|| format!("daemon endpoint tcp:// requires host:port: {value}"))?;

        let port: u16 = port
            .parse()
            .map_err(|_| format!("daemon endpoint tcp:// requires a numeric port: {value}"))?;

        let ip = if host.eq_ignore_ascii_case("localhost") {
            Ipv4Addr::LOCALHOST
        } else {
            host.parse()
                .map_err(|_| format!("daemon endpoint tcp:// requires an IPv4 host: {value}"))?
        };

        Ok(Self::Tcp(SocketAddr::from((ip, port))))
    }

    pub(crate) fn connect_stream(&self) -> Result<ControlStream, String> {
        match self {
            Self::Local => {
                let path = PathBuf::from(crate::config::control_socket_path());
                let start = std::time::Instant::now();
                loop {
                    match UnixStream::connect(&path) {
                        Ok(stream) => return Ok(ControlStream::Unix(stream)),
                        Err(e) => {
                            if start.elapsed() >= LOCAL_DAEMON_CONNECT_RETRY_TIMEOUT {
                                return Err(format!(
                                    "cannot connect to daemon endpoint {self}: {e}"
                                ));
                            }
                            std::thread::sleep(LOCAL_DAEMON_CONNECT_RETRY_INTERVAL);
                        }
                    }
                }
            }
            Self::Unix(path) => UnixStream::connect(path)
                .map(ControlStream::Unix)
                .map_err(|e| format!("cannot connect to daemon endpoint {self}: {e}")),
            Self::Tcp(addr) => TcpStream::connect_timeout(addr, DAEMON_TCP_CONNECT_TIMEOUT)
                .map(ControlStream::Tcp)
                .map_err(|e| format!("cannot connect to daemon endpoint {self}: {e}")),
        }
    }

    /// Whether this endpoint is the same-machine daemon. Callers use this to
    /// decide connection behavior (e.g. `App::new_remote`'s `is_local_daemon`)
    /// so that distinction is derived from the endpoint itself rather than
    /// tracked separately and passed around as a disconnected bool.
    pub fn is_local(&self) -> bool {
        matches!(self, Self::Local)
    }
}

impl std::fmt::Display for DaemonEndpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Local => write!(f, "local ({})", crate::config::control_socket_path()),
            Self::Unix(path) => write!(f, "unix://{}", path.display()),
            Self::Tcp(addr) => write!(f, "tcp://{addr}"),
        }
    }
}

/// Performs the daemon control-protocol handshake (hello exchange, then the
/// initial state) on `stream`, returning a reader ready for the long-running
/// event-reading loop plus the initial `CtrlEvent::State`. Split out of
/// `connect_endpoint` so it can run on a worker thread bounded by
/// `DAEMON_HANDSHAKE_HARD_BOUND` (issue #191 fix #5), and so it can be tested
/// directly against a real stalled `TcpListener` without going through
/// `connect_endpoint`'s full setup.
pub(crate) fn perform_handshake<F>(
    stream: ControlStream,
    load_control_token: F,
) -> Result<(BufReader<ControlStream>, CtrlEvent, CtrlCompatibility), String>
where
    F: FnOnce() -> Result<String, String>,
{
    let mut reader = BufReader::new(stream);
    let mut first_line = String::new();
    reader
        .read_line(&mut first_line)
        .map_err(|e| format!("failed to read daemon protocol hello: {e}"))?;
    if first_line.trim().is_empty() {
        return Err("daemon closed connection before protocol hello".to_string());
    }
    let hello = serde_json::from_str::<CtrlEvent>(first_line.trim_end())
        .map_err(|e| format!("invalid daemon protocol hello: {e}"))?;
    let ctrl_compatibility = match hello {
        CtrlEvent::Hello(info) => {
            info.validate_peer()?;
            let mut compatibility = info.compatibility()?;
            compatibility.supports_lifecycle_shutdown = info.supports_lifecycle_shutdown();
            compatibility.supports_control_auth = info.supports_control_auth();
            log::info!(
                target: "remote",
                "daemon protocol ok: version={} app={} capabilities={:?}",
                info.protocol_version,
                info.app_version,
                info.capabilities
            );
            compatibility
        }
        _ => {
            return Err("daemon did not send protocol hello".to_string());
        }
    };
    let mut client_hello = if ctrl_compatibility.supports_control_auth {
        CtrlHello::current_control_client(load_control_token()?)
    } else {
        CtrlHello::current()
    };
    client_hello.protocol_version = ctrl_compatibility.client_protocol_version;
    let client_hello =
        serde_json::to_string(&CtrlCmd::Hello(client_hello)).map_err(|e| e.to_string())?;
    // Write via the same handle the `BufReader` wraps (`get_mut()`) rather
    // than a second `try_clone()`'d handle -- the handshake is strictly
    // sequential (read hello -> write client hello -> read state) with no
    // concurrent access from another thread during this phase, so there's
    // nothing a second handle buys here beyond an extra fallible call.
    writeln!(reader.get_mut(), "{client_hello}")
        .map_err(|e| format!("failed to send daemon protocol hello: {e}"))?;

    let mut state_line = String::new();
    reader
        .read_line(&mut state_line)
        .map_err(|e| format!("failed to read daemon initial state: {e}"))?;
    if state_line.trim().is_empty() {
        return Err("daemon closed connection before initial state".to_string());
    }
    let state_event = serde_json::from_str::<CtrlEvent>(state_line.trim_end())
        .map_err(|e| format!("invalid daemon initial state: {e}"))?;

    Ok((reader, state_event, ctrl_compatibility))
}

/// Best-effort signal to a running same-user Local daemon to reread its own
/// owner-local Service storage. A single non-blocking connect attempt is made;
/// when no Local daemon is reachable the call returns `Ok(())` so a bare-mode
/// commit proceeds without a daemon. When a daemon is reachable, the
/// control-auth handshake runs, `ApplyServiceSetup` is sent, and the
/// applied/rejected acknowledgement is awaited. Any failure after the connect
/// reports a restart requirement; the caller's durable commit is untouched.
pub fn signal_local_daemon_service_setup(
    kind: crate::config::ServiceKind,
    revision: u64,
) -> Result<(), String> {
    let path = PathBuf::from(crate::config::control_socket_path());
    let stream = match UnixStream::connect(&path) {
        Ok(stream) => stream,
        Err(_) => return Ok(()),
    };
    stream
        .set_read_timeout(Some(Duration::from_secs(6)))
        .map_err(|_| "restart required (cannot read local daemon ctrl)".to_string())?;
    let (mut reader, _state, _compatibility) =
        perform_handshake(ControlStream::Unix(stream), || {
            crate::config::load_or_create_control_credential()
        })
        .map_err(|_| "restart required (local daemon handshake failed)".to_string())?;
    let request = serde_json::to_string(&CtrlCmd::ApplyServiceSetup { kind, revision })
        .map_err(|_| "restart required (cannot serialize setup request)".to_string())?;
    writeln!(reader.get_mut(), "{request}")
        .and_then(|_| reader.get_mut().flush())
        .map_err(|_| "restart required (cannot send setup request)".to_string())?;
    for next in reader.lines() {
        let line =
            next.map_err(|_| "restart required (setup acknowledgement unavailable)".to_string())?;
        let event = serde_json::from_str::<CtrlEvent>(&line)
            .map_err(|_| "restart required (invalid setup acknowledgement)".to_string())?;
        match event {
            CtrlEvent::ServiceSetupApplied { .. } => return Ok(()),
            CtrlEvent::ServiceSetupRejected { reason, .. } => {
                return Err(format!(
                    "restart required (live setup rejected: {reason:?})"
                ))
            }
            _ => {}
        }
    }
    Err("restart required (setup acknowledgement unavailable)".into())
}

fn apply_ctrl_event(
    ev: CtrlEvent,
    status: &Arc<Mutex<PlayerStatus>>,
    items: &Arc<Mutex<Vec<EmbyItem>>>,
    unified_queue: &Arc<Mutex<Option<UnifiedQueueStateData>>>,
    queue_source: &Arc<Mutex<crate::config::QueueSource>>,
    event_tx: &mpsc::Sender<PlayerEvent>,
    pending_playback: &Arc<Mutex<HashMap<u64, PlaybackIntent>>>,
    notify: bool,
) {
    match ev {
        CtrlEvent::Hello(_) => {
            log::warn!(target: "remote", "unexpected daemon protocol hello after negotiation");
        }
        CtrlEvent::StatusOnly(s) => {
            let mut current = status.lock().unwrap();
            let current_idx = current.current_idx;
            let queue_len = current.queue_len;
            *current = s;
            current.current_idx = current_idx;
            current.queue_len = queue_len;
        }
        CtrlEvent::Player(pe) => {
            match &pe {
                PlayerEvent::Stopped { .. } => {
                    status.lock().unwrap().active = false;
                }
                PlayerEvent::TrackChanged(idx) => {
                    status.lock().unwrap().current_idx = *idx;
                }
                PlayerEvent::PausedChanged(paused) => {
                    status.lock().unwrap().paused = *paused;
                }
                _ => {}
            }
            if notify {
                let _ = event_tx.send(pe);
            }
        }
        CtrlEvent::CommandRejected(reason) => {
            if notify {
                let _ = event_tx.send(PlayerEvent::CommandRejected(reason));
            }
        }
        CtrlEvent::PlaybackIntent(event) => {
            // A coalesced request is terminal for that request identity too;
            // the canonical request remains tracked separately by the daemon.
            pending_playback.lock().unwrap().remove(&event.request_id);
            if notify {
                let _ = event_tx.send(PlayerEvent::PlaybackIntent(event));
            }
        }
        CtrlEvent::PipePlaybackStatus(status_event) => {
            if notify {
                let _ = event_tx.send(PlayerEvent::PipePlaybackStatus(status_event));
            }
        }
        CtrlEvent::ShutdownAccepted | CtrlEvent::ShutdownRejected { .. } => {
            // Handled by the request-completion path in RemotePlayer
            //, not by the general event loop.
        }
        CtrlEvent::ServiceSetupApplied { .. } | CtrlEvent::ServiceSetupRejected { .. } => {
            log::debug!(target: "remote", "ignoring owner-service reconciliation event");
        }
        CtrlEvent::Disconnected { reason } => {
            if notify {
                let msg = disconnect_reason_message(&reason).to_string();
                match reason {
                    DisconnectReason::TakenOverByEmbyRemote => {
                        let _ = event_tx.send(PlayerEvent::EmbyAuthorityTaken(msg));
                    }
                    // Handled by the reader thread's end-of-loop logic below
                    // (`is_structured_disconnect`), which sends
                    // `PlayerEvent::DaemonShutdownAnnounced` once the
                    // connection actually closes; nothing to do here.
                    DisconnectReason::DaemonShutdown => {}
                }
            }
        }
        CtrlEvent::UnifiedQueueState(unified) => {
            // Keep a compatibility projection for older status consumers,
            // but retain the canonical snapshot for TUI and reconnect paths.
            apply_unified_queue_state(
                unified,
                status,
                items,
                unified_queue,
                queue_source,
                event_tx,
                notify,
            );
        }
        CtrlEvent::AudiobookshelfProgress(event) => {
            // Dormant: forwarded for a future browse-reconciliation consumer.
            // Does not touch `status`, `items`, or `unified_queue`.
            if notify {
                let _ = event_tx.send(PlayerEvent::AudiobookshelfProgress(event));
            }
        }
    }
}

fn disconnect_reason_message(reason: &DisconnectReason) -> &'static str {
    match reason {
        DisconnectReason::TakenOverByEmbyRemote => {
            "Emby remote control took over — returned to local mode"
        }
        DisconnectReason::DaemonShutdown => "the daemon was stopped",
    }
}

/// Applies a unified-queue state snapshot.  Updates the legacy `status`
/// and `items` Arc values for backward-compatible status-bar consumers,
/// and — when `notify` is true — emits a `PlayerEvent::UnifiedQueueUpdated`
/// that carries the full tagged queue, slot identity, active slot, and
/// revision so the TUI can reconstruct the canonical queue without
/// decomposing it into Emby-only shapes.
fn apply_unified_queue_state(
    unified: UnifiedQueueStateData,
    status: &Arc<Mutex<PlayerStatus>>,
    items: &Arc<Mutex<Vec<EmbyItem>>>,
    unified_queue: &Arc<Mutex<Option<UnifiedQueueStateData>>>,
    queue_source: &Arc<Mutex<crate::config::QueueSource>>,
    event_tx: &mpsc::Sender<PlayerEvent>,
    notify: bool,
) {
    let mut next_status = unified.status.clone();
    next_status.queue_len = unified.slots.len();
    if let Some(active_index) = unified.active_slot.and_then(|slot_id| {
        unified
            .slots
            .iter()
            .position(|slot| slot.slot_id == slot_id)
    }) {
        next_status.current_idx = active_index;
    }

    // Project Emby-only items only for compatibility consumers. The status
    // coordinates remain canonical; capable consumers use the stored unified
    // snapshot, while legacy consumers continue to receive their projection.
    let emby_items: Vec<EmbyItem> = unified
        .slots
        .iter()
        .filter_map(|slot| slot.item.as_emby().cloned())
        .collect();

    *status.lock().unwrap() = next_status;
    *items.lock().unwrap() = emby_items;
    *unified_queue.lock().unwrap() = Some(unified.clone());

    // Carry the queue source from the unified state so saved-playlist
    // detection remains correct across reconnect.
    *queue_source.lock().unwrap() = unified.source.clone();

    if notify {
        // Emit the full unified state so the TUI can reconstruct the
        // canonical queue (tagged QueueItems, slot identity, active slot,
        // revision) without losing Feed entries or canonical order.
        let _ = event_tx.send(PlayerEvent::UnifiedQueueUpdated(Box::new(unified)));
    }
}

pub(crate) fn connect_endpoint(
    endpoint: &DaemonEndpoint,
) -> Result<(RemotePlayer, mpsc::Receiver<PlayerEvent>), String> {
    let stream = endpoint.connect_stream()?;
    log::info!(target: "remote", "connected to daemon endpoint {endpoint}");

    // Kept aside for `disconnect()` (#233) -- taken before `stream` is
    // moved into the writer thread below.
    let disconnect_stream = stream.try_clone().map_err(|e| e.to_string())?;

    let status = Arc::new(Mutex::new(PlayerStatus::default()));
    let subtitle_prefs = Arc::new(Mutex::new(crate::player::SubtitlePrefs::default()));
    let items: Arc<Mutex<Vec<EmbyItem>>> = Arc::new(Mutex::new(Vec::new()));
    let unified_queue = Arc::new(Mutex::new(None));
    let queue_source = Arc::new(Mutex::new(crate::config::QueueSource::Unknown));
    let disconnected = Arc::new(AtomicBool::new(false));
    let shutdown_announced = Arc::new(AtomicBool::new(false));
    let next_playback_id = Arc::new(std::sync::atomic::AtomicU64::new(1));
    let pending_playback = Arc::new(Mutex::new(HashMap::new()));
    let shutdown_request_tx: Arc<
        Mutex<Option<mpsc::Sender<crate::remote_player::ShutdownResponse>>>,
    > = Arc::new(Mutex::new(None));

    let (event_tx, event_rx) = mpsc::channel::<PlayerEvent>();
    let (cmd_tx, cmd_rx) = mpsc::channel::<CtrlCmd>();

    // The handshake (hello exchange + initial state) runs on a worker
    // thread bounded by `DAEMON_HANDSHAKE_HARD_BOUND`, independent of
    // `DAEMON_TCP_CONNECT_TIMEOUT` above -- that only bounds the initial
    // TCP-level connect, not these blocking reads (issue #191 fix #5).
    // `stream` itself is kept untouched on this thread for the writer
    // thread spawned below; a clone goes to the worker thread instead.
    let handshake_stream = stream.try_clone().map_err(|e| e.to_string())?;
    let (reader, state_event, ctrl_compatibility) = crate::bounded::run_with_hard_bound(
        move || {
            perform_handshake(handshake_stream, || {
                crate::config::load_or_create_control_credential()
            })
        },
        DAEMON_HANDSHAKE_HARD_BOUND,
    )?;
    apply_ctrl_event(
        state_event,
        &status,
        &items,
        &unified_queue,
        &queue_source,
        &event_tx,
        &pending_playback,
        false,
    );

    // Reader thread: deserializes CtrlEvent lines from daemon
    let status_r = status.clone();
    let items_r = items.clone();
    let unified_queue_r = unified_queue.clone();
    let queue_source_r = queue_source.clone();
    let pending_playback_r = pending_playback.clone();
    let disconnected_r = disconnected.clone();
    let shutdown_announced_r = shutdown_announced.clone();
    let shutdown_request_r = shutdown_request_tx.clone();
    let event_tx_r = event_tx;
    std::thread::spawn(move || {
        let mut expected_disconnect = false;
        for line in reader.lines() {
            match line {
                Err(_) => break,
                Ok(l) if l.is_empty() => continue,
                Ok(l) => {
                    let Ok(ev) = serde_json::from_str::<CtrlEvent>(&l) else {
                        log::warn!(target: "remote", "unrecognized event from daemon: {l}");
                        continue;
                    };

                    // Handle shutdown request responses directly.
                    match &ev {
                        CtrlEvent::ShutdownAccepted => {
                            if let Some(tx) = shutdown_request_r.lock().unwrap().take() {
                                let _ = tx.send(crate::remote_player::ShutdownResponse::Accepted);
                            }
                        }
                        CtrlEvent::ShutdownRejected { reason } => {
                            if let Some(tx) = shutdown_request_r.lock().unwrap().take() {
                                let _ = tx.send(crate::remote_player::ShutdownResponse::Rejected {
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
                        CtrlEvent::Disconnected { reason } => match reason {
                            DisconnectReason::TakenOverByEmbyRemote => false,
                            DisconnectReason::DaemonShutdown => true,
                        },
                        _ => false,
                    };
                    apply_ctrl_event(
                        ev,
                        &status_r,
                        &items_r,
                        &unified_queue_r,
                        &queue_source_r,
                        &event_tx_r,
                        &pending_playback_r,
                        true,
                    );
                    expected_disconnect |= is_structured_disconnect;
                }
            }
        }
        disconnected_r.store(true, Ordering::SeqCst);
        pending_playback_r.lock().unwrap().clear();

        // Resolve any pending shutdown request with Disconnected.
        if let Some(tx) = shutdown_request_r.lock().unwrap().take() {
            let _ = tx.send(crate::remote_player::ShutdownResponse::Disconnected);
        }

        log::info!(target: "remote", "daemon disconnected");
        if !expected_disconnect {
            let _ = event_tx_r.send(PlayerEvent::Stopped {
                idx: 0,
                position_ticks: 0,
                played: false,
                consume: false,
                progress_report_accepted: false,
                error: None,
            });
        } else {
            // An "expected"/structured disconnect (e.g. an Emby Remote
            // takeover, or a deliberate daemon shutdown) never sends a
            // Stopped PlayerEvent, so nothing else clears `status`.
            // Clear it here, at the source, so
            // every consumer of `status` (not just MPRIS's separate
            // `disconnected_flag()` check in src/mpris.rs) sees an
            // inactive/no-track player immediately rather than stale
            // "still playing" data.
            if let Ok(mut s) = status_r.lock() {
                s.active = false;
                s.paused = false;
                s.clear_current_item_metadata();
            }
            // `TakenOverByEmbyRemote` never reaches this branch (it does not
            // close the connection), so this structured disconnect is a
            // deliberate daemon shutdown.
            shutdown_announced_r.store(true, Ordering::SeqCst);
            let _ = event_tx_r.send(PlayerEvent::DaemonShutdownAnnounced);
        }
    });

    // Writer thread: serializes CtrlCmd to daemon
    let mut stream_w = stream;
    std::thread::spawn(move || {
        while let Ok(cmd) = cmd_rx.recv() {
            let Ok(json) = serde_json::to_string(&cmd) else {
                continue;
            };
            if writeln!(stream_w, "{json}").is_err() {
                break;
            }
        }
    });

    Ok((
        RemotePlayer {
            status,
            subtitle_prefs,
            items,
            unified_queue,
            queue_source,
            cmd_tx,
            disconnected,
            shutdown_announced,
            ctrl_compatibility,
            control_stream: Arc::new(Mutex::new(Some(disconnect_stream))),
            next_playback_id,
            pending_playback,
            shutdown_request_tx,
        },
        event_rx,
    ))
}
