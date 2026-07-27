use std::io::{BufRead, BufReader, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

use crate::api::{mbv_direct_tcp_port_command, EmbyClient, MediaItem};
use crate::ctrl::{CtrlCmd, CtrlEvent, CtrlHello, CtrlState, DisconnectReason};
use crate::player::{Player, PlayerCommand, PlayerEvent};
use crate::ws::WsEvent;

/// Shared by the startup registration and the periodic 10-minute
/// re-registration in the main loop below.
fn register_capabilities(client: &EmbyClient, direct_commands: &[String], audio_only: bool) {
    client.register_capabilities_with_options(direct_commands, audio_only);
}

fn bind_ctrl_listener() -> Option<UnixListener> {
    let path = crate::config::control_socket_path();
    let _ = std::fs::remove_file(&path);
    match UnixListener::bind(&path) {
        Ok(listener) => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
            }
            Some(listener)
        }
        Err(e) => {
            log::error!(
                target: "daemon",
                "ctrl socket bind failed ({e}), remote TUI unavailable"
            );
            None
        }
    }
}

enum DaemonEvent {
    Player(PlayerEvent),
    Ws(WsEvent),
    /// Carries the requesting client's own event sender alongside the
    /// command, so a rejection (see #90) can be replied to that one client
    /// instead of broadcast to every connected TUI.
    Ctrl(CtrlCmd, CtrlClientId, CtrlSender),
    CtrlDisconnected(CtrlClientId),
    /// A spectrum frame from the daemon's spectrum reader thread, carrying
    /// 64 normalized bar values (0.0–1.0) from `CavaWorker::take_latest_frame()`.
    Spectrum(Vec<f32>),
    /// The spectrum reader thread detected a CAVA failure.
    SpectrumFailed {
        reason: String,
    },
    Shutdown,
}

/// Daemon-local state for an active spectrum streaming session.
///
/// Holds the reader thread that converts frames from
/// `CavaWorker::take_latest_frame()` into `DaemonEvent::Spectrum` messages.
/// The `CavaWorker` is owned by the reader thread and dropped when it exits.
/// The `stop()` method is idempotent — safe to call from `StopSpectrum`,
/// `CtrlDisconnected`, playback stop, and shutdown paths.
pub(crate) struct SpectrumState {
    reader: Option<std::thread::JoinHandle<()>>,
    stop_tx: std::sync::mpsc::Sender<()>,
}

impl SpectrumState {
    fn start(worker: crate::visualizer::CavaWorker, merged_tx: mpsc::Sender<DaemonEvent>) -> Self {
        let (stop_tx, stop_rx) = mpsc::channel();
        let reader = std::thread::spawn(move || {
            loop {
                if stop_rx.try_recv().is_ok() {
                    break;
                }
                match worker.take_latest_frame() {
                    Ok(Some(frame)) => {
                        let _ = merged_tx.send(DaemonEvent::Spectrum(frame));
                    }
                    Ok(None) => {
                        std::thread::sleep(std::time::Duration::from_millis(16));
                    }
                    Err(()) => {
                        let _ = merged_tx.send(DaemonEvent::SpectrumFailed {
                            reason: "CAVA worker channel closed".to_string(),
                        });
                        break;
                    }
                }
            }
            drop(worker);
        });
        Self {
            reader: Some(reader),
            stop_tx,
        }
    }

    /// Idempotent stop: signals the reader thread to exit and joins it with
    /// a timeout. Safe to call multiple times.
    pub(crate) fn stop(&mut self) {
        let _ = self.stop_tx.send(());
        if let Some(handle) = self.reader.take() {
            let started = std::time::Instant::now();
            let join_timeout = std::time::Duration::from_millis(500);
            let (done_tx, done_rx) = mpsc::channel();
            std::thread::spawn(move || {
                let _ = handle.join();
                let _ = done_tx.send(());
            });
            if done_rx.recv_timeout(join_timeout).is_err() {
                log::warn!(
                    target: "daemon",
                    "spectrum reader thread did not join within {}ms; detaching",
                    join_timeout.as_millis()
                );
                return;
            }
            log::debug!(
                target: "daemon",
                "spectrum reader thread joined in {}ms",
                started.elapsed().as_millis()
            );
        }
    }
}

type CtrlClientId = u64;
type CtrlSender = mpsc::Sender<CtrlOutbound>;

enum CtrlOutbound {
    Event(String),
}

trait CtrlStream: std::io::Read + Write + Send + Sized + 'static {
    fn try_clone_stream(&self) -> std::io::Result<Self>;
    fn shutdown_stream(&self);
}

impl CtrlStream for UnixStream {
    fn try_clone_stream(&self) -> std::io::Result<Self> {
        self.try_clone()
    }

    fn shutdown_stream(&self) {
        let _ = self.shutdown(Shutdown::Both);
    }
}

impl CtrlStream for TcpStream {
    fn try_clone_stream(&self) -> std::io::Result<Self> {
        self.try_clone()
    }

    fn shutdown_stream(&self) {
        let _ = self.shutdown(Shutdown::Both);
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum AuthorityHolder {
    #[default]
    None,
    Ctrl,
    EmbyRemote,
}

#[derive(Default)]
struct CtrlClients {
    next_id: CtrlClientId,
    connection: Vec<CtrlClient>,
    authority: AuthorityHolder,
}

struct CtrlClient {
    id: CtrlClientId,
    tx: CtrlSender,
}

type ClientRegistry = Arc<Mutex<CtrlClients>>;

struct CtrlRequest<'a> {
    reply_tx: &'a CtrlSender,
}

#[derive(Clone)]
struct SharedQueueState {
    items: Arc<Mutex<Vec<MediaItem>>>,
    cursor: Arc<Mutex<usize>>,
    source: Arc<Mutex<crate::config::QueueSource>>,
}

pub struct DaemonPlayerHandle {
    pub status: Arc<Mutex<crate::player::PlayerStatus>>,
    pub command_tx: Arc<Mutex<Option<mpsc::Sender<PlayerCommand>>>>,
}

type OnPlayerReady = Box<dyn FnOnce(DaemonPlayerHandle)>;
type OnTrayReady = Box<dyn FnOnce(mpsc::SyncSender<()>) -> Option<Box<dyn Send>>>;

pub struct DaemonRuntimeHooks {
    pub on_player_ready: OnPlayerReady,
    pub on_tray_ready: OnTrayReady,
}

pub fn pid_file() -> std::path::PathBuf {
    let dir = crate::config::data_dir_system_or_local();
    let _ = std::fs::create_dir_all(&dir);
    dir.join("mbv.pid")
}

fn broadcast(clients: &ClientRegistry, event: &CtrlEvent) {
    let Some(json) = serialize_ctrl_event(event) else {
        return;
    };
    clients.lock().unwrap().broadcast_to_all(json);
}

/// Send an event to a single ctrl-socket client, rather than every connected
/// TUI. Used for per-request responses like a command rejection (#90).
fn send_to(client: &CtrlSender, event: &CtrlEvent) {
    if let Some(json) = serialize_ctrl_event(event) {
        let _ = client.send(CtrlOutbound::Event(json));
    }
}

/// Shared by `broadcast` and `send_to` so both go through one serialization
/// path instead of repeating `serde_json::to_string(event).ok()` inline.
fn serialize_ctrl_event(event: &CtrlEvent) -> Option<String> {
    serde_json::to_string(event).ok()
}

impl CtrlClients {
    /// Append `tx` as a new ctrl connection. Multiple clients may coexist.
    /// Does NOT override authority if it is currently `EmbyRemote` — the new
    /// client receives broadcasts but its commands are rejected until
    /// authority returns to `Ctrl`.
    fn connect(&mut self, tx: CtrlSender) -> CtrlClientId {
        let id = self.next_id;
        self.next_id += 1;
        self.connection.push(CtrlClient { id, tx });
        if self.authority == AuthorityHolder::None {
            self.authority = AuthorityHolder::Ctrl;
        }
        id
    }

    fn remove(&mut self, id: CtrlClientId) {
        self.connection.retain(|c| c.id != id);
        if self.connection.is_empty() && self.authority == AuthorityHolder::Ctrl {
            self.authority = AuthorityHolder::None;
        }
    }

    fn has_client(&self, id: CtrlClientId) -> bool {
        self.connection.iter().any(|c| c.id == id)
    }

    fn has_driver(&self) -> bool {
        !self.connection.is_empty()
    }

    /// Broadcast `json` to all connected ctrl clients. Removes any client
    /// whose channel has failed (broken pipe / disconnected).
    fn broadcast_to_all(&mut self, json: String) {
        self.connection
            .retain(|c| c.tx.send(CtrlOutbound::Event(json.clone())).is_ok());
    }

    /// Broadcast a `Disconnected` notification to all connected ctrl clients
    /// without closing their connections. Used for Emby remote authority
    /// transitions — clients observe the authority change but stay connected.
    fn notify_disconnected_all(&self, reason: DisconnectReason) {
        for client in &self.connection {
            send_to(
                &client.tx,
                &CtrlEvent::Disconnected {
                    reason: reason.clone(),
                },
            );
        }
    }

    fn take_authority_for_emby_remote(&mut self) {
        self.notify_disconnected_all(DisconnectReason::TakenOverByEmbyRemote);
        self.authority = AuthorityHolder::EmbyRemote;
    }
}

fn take_authority_for_emby_remote(ctrl_clients: &ClientRegistry) {
    ctrl_clients
        .lock()
        .unwrap()
        .take_authority_for_emby_remote();
}

/// A reason a ctrl-socket command is not acted on, computed server-side.
/// Currently the only case is audio-only mode rejecting a non-audio play
/// request; kept as a small pure function so it's testable without a live
/// `Player`/`EmbyClient`. Returns the bare reason (not a `CtrlEvent`) so the
/// same string can be reused for both the server-side log line and the wire
/// event the caller sends — one message, not two that can drift apart.
fn audio_only_rejection(audio_only: bool, fetched: &[MediaItem]) -> Option<String> {
    if audio_only && !all_audio(fetched) {
        Some("Daemon is running in audio-only mode; can't play video items".to_string())
    } else {
        None
    }
}

fn spawn_ctrl_client<S>(
    stream: S,
    merged_tx: mpsc::Sender<DaemonEvent>,
    ctrl_clients: ClientRegistry,
    client: Arc<Mutex<EmbyClient>>,
    player_status: Arc<Mutex<crate::player::PlayerStatus>>,
    shared_queue: SharedQueueState,
) where
    S: CtrlStream,
{
    let Ok(writer_stream) = stream.try_clone_stream() else {
        return;
    };
    let (ev_tx, ev_rx) = mpsc::channel::<CtrlOutbound>();

    let mut daemon_hello = CtrlHello::current();
    daemon_hello
        .capabilities
        .push(crate::ctrl::CTRL_CAP_SPECTRUM.to_string());
    if let Ok(hello_json) = serde_json::to_string(&CtrlEvent::Hello(daemon_hello)) {
        ev_tx.send(CtrlOutbound::Event(hello_json)).ok();
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
            }
        }
        w.shutdown_stream();
    });

    std::thread::spawn(move || {
        let reader = BufReader::new(stream);
        let mut lines = reader.lines();
        let Some(Ok(line)) = lines.next() else {
            return;
        };
        match serde_json::from_str::<CtrlCmd>(&line) {
            Ok(CtrlCmd::Hello(info)) => {
                if let Err(e) = info.validate_peer() {
                    log::warn!(target: "daemon", "rejecting ctrl client: {e}");
                    return;
                }
                let Some(auth_token) = info.auth_token.as_deref() else {
                    log::warn!(target: "daemon", "rejecting ctrl client: missing Emby auth token");
                    return;
                };
                let validate_client = client.lock().unwrap().clone();
                if let Err(e) = validate_client.validate_presented_token(auth_token) {
                    log::warn!(
                        target: "daemon",
                        "rejecting ctrl client: presented Emby token validation failed: {e}"
                    );
                    return;
                }
            }
            Ok(_) => {
                log::warn!(target: "daemon", "rejecting ctrl client: missing protocol hello");
                return;
            }
            Err(e) => {
                log::warn!(target: "daemon", "rejecting ctrl client: invalid protocol hello: {e}");
                return;
            }
        }

        if let Ok(init_json) = serde_json::to_string(&CtrlEvent::State(CtrlState {
            status: player_status.lock().unwrap().clone(),
            items: shared_queue.items.lock().unwrap().clone(),
            cursor: *shared_queue.cursor.lock().unwrap(),
            source: shared_queue.source.lock().unwrap().clone(),
        })) {
            ev_tx.send(CtrlOutbound::Event(init_json)).ok();
        }
        let reply_tx = ev_tx.clone();
        let client_id = ctrl_clients.lock().unwrap().connect(ev_tx);

        for line in lines {
            let Ok(line) = line else { break };
            if line.is_empty() {
                continue;
            }
            if let Ok(cmd) = serde_json::from_str::<CtrlCmd>(&line) {
                let _ = merged_tx.send(DaemonEvent::Ctrl(cmd, client_id, reply_tx.clone()));
            }
        }
        let _ = merged_tx.send(DaemonEvent::CtrlDisconnected(client_id));
    });
}
