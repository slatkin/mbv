use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use crate::api::{EmbyClient, MediaItem};
use crate::ctrl::{CtrlCmd, CtrlCompatibility, CtrlEvent, CtrlHello, DisconnectReason};
use crate::player::{PlayerCommand, PlayerEvent, PlayerStatus};

const DAEMON_TCP_CONNECT_TIMEOUT: Duration = Duration::from_millis(750);

// Hard wall-clock bound on the post-connect protocol handshake (hello
// exchange + initial state), independent of `DAEMON_TCP_CONNECT_TIMEOUT`
// above -- that constant only bounds the initial TCP-level connect, not the
// blocking `read_line` calls that follow it (issue #191 fix #5). A stalled
// daemon on localhost/LAN (user-configured, not a public/flaky server) is a
// rarer and more clearly-broken scenario than a slow Emby server, so this is
// tighter than `EmbyClient::AUTHENTICATE_HARD_BOUND`.
const DAEMON_HANDSHAKE_HARD_BOUND: Duration = Duration::from_secs(5);

// A local daemon that was *just* launched (`mbv -d`) may have written its
// PID file (which is what makes it "detected") slightly before its ctrl
// socket is bound. Retry briefly rather than immediately falling back to
// standalone. Explicit remote endpoints (`Unix(path)` / `Tcp`) are not
// retried this way — they represent an already-running, user-specified
// target, not a same-machine process that might still be starting up.
const LOCAL_DAEMON_CONNECT_RETRY_TIMEOUT: Duration = Duration::from_secs(1);
const LOCAL_DAEMON_CONNECT_RETRY_INTERVAL: Duration = Duration::from_millis(50);

#[derive(Clone)]
pub struct RemotePlayer {
    pub status: Arc<Mutex<PlayerStatus>>,
    pub subtitle_prefs: Arc<Mutex<crate::player::SubtitlePrefs>>,
    pub items: Arc<Mutex<Vec<MediaItem>>>,
    pub queue_source: Arc<Mutex<crate::config::QueueSource>>,
    cmd_tx: mpsc::Sender<CtrlCmd>,
    disconnected: Arc<AtomicBool>,
    ctrl_compatibility: CtrlCompatibility,
    /// A kept clone of the control socket, used only by `disconnect()`
    /// (#233) to shut the connection down on demand rather than relying
    /// on `Drop` -- which only closes this clone's own fd duplicate, not
    /// the reader/writer threads' separate duplicates of the same
    /// underlying socket. `Arc<Mutex<..>>` so every `RemotePlayer` clone
    /// shares one handle and `disconnect()` is safe to call from any of
    /// them; `Option` so a second call is a no-op instead of a double
    /// shutdown.
    control_stream: Arc<Mutex<Option<ControlStream>>>,
}

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

    fn connect_stream(&self) -> Result<ControlStream, String> {
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

#[cfg(test)]
#[path = "remote_player_tests.rs"]
mod tests;

impl std::fmt::Display for DaemonEndpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Local => write!(f, "local ({})", crate::config::control_socket_path()),
            Self::Unix(path) => write!(f, "unix://{}", path.display()),
            Self::Tcp(addr) => write!(f, "tcp://{addr}"),
        }
    }
}

enum ControlStream {
    Unix(UnixStream),
    Tcp(TcpStream),
}

impl ControlStream {
    fn try_clone(&self) -> io::Result<Self> {
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
    fn shutdown(&self) -> io::Result<()> {
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

/// Performs the daemon control-protocol handshake (hello exchange, then the
/// initial state) on `stream`, returning a reader ready for the long-running
/// event-reading loop plus the initial `CtrlEvent::State`. Split out of
/// `connect_endpoint` so it can run on a worker thread bounded by
/// `DAEMON_HANDSHAKE_HARD_BOUND` (issue #191 fix #5), and so it can be tested
/// directly against a real stalled `TcpListener` without going through
/// `connect_endpoint`'s full setup.
fn perform_handshake(
    stream: ControlStream,
    auth_token: &str,
) -> Result<(BufReader<ControlStream>, CtrlEvent, CtrlCompatibility), String> {
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
            compatibility.supports_spectrum = info
                .capabilities
                .iter()
                .any(|cap| cap == crate::ctrl::CTRL_CAP_SPECTRUM);
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
    let client_hello = serde_json::to_string(&CtrlCmd::Hello(CtrlHello::compatible_client(
        auth_token.into(),
        ctrl_compatibility,
    )))
    .map_err(|e| e.to_string())?;
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

fn apply_ctrl_event(
    ev: CtrlEvent,
    status: &Arc<Mutex<PlayerStatus>>,
    items: &Arc<Mutex<Vec<MediaItem>>>,
    queue_source: &Arc<Mutex<crate::config::QueueSource>>,
    event_tx: &mpsc::Sender<PlayerEvent>,
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
        CtrlEvent::State(s) => {
            let mut next_status = s.status;
            next_status.current_idx = s.cursor;
            next_status.queue_len = s.items.len();
            *status.lock().unwrap() = next_status;
            *items.lock().unwrap() = s.items.clone();
            *queue_source.lock().unwrap() = s.source.clone();
            // The very first State snapshot read synchronously during connect()
            // establishes baseline state before the App (and its event loop)
            // exists; it must not be queued, or it would be applied *after* a
            // local-daemon queue adoption that happens between connect() and
            // App construction, transiently wiping the just-adopted queue.
            if notify {
                let _ = event_tx.send(PlayerEvent::QueueUpdated {
                    items: s.items,
                    cursor: s.cursor,
                    source: s.source,
                });
            }
        }
        CtrlEvent::Player(pe) => {
            match &pe {
                PlayerEvent::Stopped { .. } => {
                    status.lock().unwrap().active = false;
                }
                PlayerEvent::TrackChanged(idx) => {
                    status.lock().unwrap().current_idx = *idx;
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
        CtrlEvent::Disconnected { reason } => {
            if notify {
                let _ = event_tx.send(PlayerEvent::RemoteDisconnected(
                    disconnect_reason_message(&reason).to_string(),
                ));
            }
        }
        CtrlEvent::Spectrum { bars } => {
            if notify {
                let _ = event_tx.send(PlayerEvent::Spectrum(bars));
            }
        }
        CtrlEvent::SpectrumFailed { reason } => {
            if notify {
                let _ = event_tx.send(PlayerEvent::SpectrumFailed(reason));
            }
        }
    }
}

fn disconnect_reason_message(reason: &DisconnectReason) -> &'static str {
    match reason {
        DisconnectReason::TakenOverByEmbyRemote => {
            "Emby remote control took over — returned to local mode"
        }
    }
}

impl RemotePlayer {
    pub fn connect_endpoint(
        endpoint: &DaemonEndpoint,
        auth_token: &str,
    ) -> Result<(Self, mpsc::Receiver<PlayerEvent>), String> {
        let stream = endpoint.connect_stream()?;
        log::info!(target: "remote", "connected to daemon endpoint {endpoint}");

        // Kept aside for `disconnect()` (#233) -- taken before `stream` is
        // moved into the writer thread below.
        let disconnect_stream = stream.try_clone().map_err(|e| e.to_string())?;

        let status = Arc::new(Mutex::new(PlayerStatus::default()));
        let subtitle_prefs = Arc::new(Mutex::new(crate::player::SubtitlePrefs::default()));
        let items: Arc<Mutex<Vec<MediaItem>>> = Arc::new(Mutex::new(Vec::new()));
        let queue_source = Arc::new(Mutex::new(crate::config::QueueSource::Unknown));
        let disconnected = Arc::new(AtomicBool::new(false));

        let (event_tx, event_rx) = mpsc::channel::<PlayerEvent>();
        let (cmd_tx, cmd_rx) = mpsc::channel::<CtrlCmd>();

        // The handshake (hello exchange + initial state) runs on a worker
        // thread bounded by `DAEMON_HANDSHAKE_HARD_BOUND`, independent of
        // `DAEMON_TCP_CONNECT_TIMEOUT` above -- that only bounds the initial
        // TCP-level connect, not these blocking reads (issue #191 fix #5).
        // `stream` itself is kept untouched on this thread for the writer
        // thread spawned below; a clone goes to the worker thread instead.
        let handshake_stream = stream.try_clone().map_err(|e| e.to_string())?;
        let auth_token_owned = auth_token.to_string();
        let (reader, state_event, ctrl_compatibility) = crate::bounded::run_with_hard_bound(
            move || perform_handshake(handshake_stream, &auth_token_owned),
            DAEMON_HANDSHAKE_HARD_BOUND,
        )?;
        apply_ctrl_event(
            state_event,
            &status,
            &items,
            &queue_source,
            &event_tx,
            false,
        );

        // Reader thread: deserializes CtrlEvent lines from daemon
        let status_r = status.clone();
        let items_r = items.clone();
        let queue_source_r = queue_source.clone();
        let disconnected_r = disconnected.clone();
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
                        let is_structured_disconnect = matches!(ev, CtrlEvent::Disconnected { .. });
                        apply_ctrl_event(
                            ev,
                            &status_r,
                            &items_r,
                            &queue_source_r,
                            &event_tx_r,
                            true,
                        );
                        expected_disconnect |= is_structured_disconnect;
                    }
                }
            }
            disconnected_r.store(true, Ordering::SeqCst);
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
                // takeover) never sends a Stopped PlayerEvent, so nothing
                // else clears `status`. Clear it here, at the source, so
                // every consumer of `status` (not just MPRIS's separate
                // `disconnected_flag()` check in src/mpris.rs) sees an
                // inactive/no-track player immediately rather than stale
                // "still playing" data.
                if let Ok(mut s) = status_r.lock() {
                    s.active = false;
                    s.paused = false;
                    s.clear_current_item_metadata();
                }
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
                queue_source,
                cmd_tx,
                disconnected,
                ctrl_compatibility,
                control_stream: Arc::new(Mutex::new(Some(disconnect_stream))),
            },
            event_rx,
        ))
    }

    pub fn is_disconnected(&self) -> bool {
        self.disconnected.load(Ordering::SeqCst)
    }

    /// Shared handle to the disconnect flag, cloneable independent of the
    /// rest of `RemotePlayer` (#160): the root `mbv` crate's MPRIS polling
    /// loop holds this alongside `status` so it can stop advertising a
    /// live/active track the moment the daemon connection drops, even
    /// though `status` itself isn't guaranteed to be updated synchronously
    /// with the disconnect (an "expected" disconnect, e.g. an Emby Remote
    /// takeover, never sends a `Stopped` event -- see the reader thread in
    /// `connect_endpoint`).
    pub fn disconnected_flag(&self) -> Arc<AtomicBool> {
        self.disconnected.clone()
    }

    pub fn send_ctrl_cmd(&self, cmd: CtrlCmd) -> bool {
        self.cmd_tx.send(cmd).is_ok()
    }

    pub fn send_command(&self, cmd: PlayerCommand) -> bool {
        let wire_cmd = match cmd {
            PlayerCommand::QueueAppend { items }
                if !self.ctrl_compatibility.supports_queue_append =>
            {
                log::warn!(
                    target: "remote",
                    "remote ctrl peer protocol v{} does not support QueueAppend for {} item(s)",
                    self.ctrl_compatibility.peer_protocol_version,
                    items.len()
                );
                return false;
            }
            cmd => cmd.into(),
        };
        self.cmd_tx.send(CtrlCmd::PlayerCmd(wire_cmd)).is_ok()
    }

    pub fn adopt_queue(
        &self,
        items: Vec<MediaItem>,
        cursor: usize,
        source: crate::config::QueueSource,
    ) -> bool {
        let cursor = cursor.min(items.len().saturating_sub(1));
        {
            let mut status = self.status.lock().unwrap();
            status.current_idx = cursor;
            status.queue_len = items.len();
            status.active = false;
        }
        *self.items.lock().unwrap() = items.clone();
        *self.queue_source.lock().unwrap() = source.clone();
        self.cmd_tx
            .send(CtrlCmd::AdoptQueue {
                items,
                cursor,
                source,
            })
            .is_ok()
    }

    pub fn play(
        &self,
        item: &MediaItem,
        source: crate::config::QueueSource,
        _client: Arc<EmbyClient>,
        _initial_volume: u8,
    ) {
        let _ = self.cmd_tx.send(CtrlCmd::PlayItems {
            item_ids: vec![item.id.clone()],
            start_idx: 0,
            start_ticks: item.playback_position_ticks,
            source: source.clone(),
        });
        *self.items.lock().unwrap() = vec![item.clone()];
        *self.queue_source.lock().unwrap() = source;
    }

    pub fn play_queue(
        &self,
        items: Vec<MediaItem>,
        start_idx: usize,
        source: crate::config::QueueSource,
        _client: Arc<EmbyClient>,
        _initial_volume: u8,
    ) {
        let item_ids: Vec<String> = items.iter().map(|i| i.id.clone()).collect();
        let start_ticks = items
            .get(start_idx)
            .map_or(0, |i| i.playback_position_ticks);
        let _ = self.cmd_tx.send(CtrlCmd::PlayItems {
            item_ids,
            start_idx,
            start_ticks,
            source: source.clone(),
        });
        *self.items.lock().unwrap() = items;
        *self.queue_source.lock().unwrap() = source;
    }

    pub fn stop(&self) {
        let _ = self.cmd_tx.send(CtrlCmd::Stop);
    }

    pub fn join(&self) {
        // No thread to join; daemon keeps running when TUI exits.
    }

    /// Actively tears down the control-socket connection (#233): shuts
    /// down the shared underlying socket so the reader thread's blocking
    /// `read()` (inside `reader.lines()` in `connect_endpoint`) observes
    /// EOF/an error and exits, instead of leaking forever the way it did
    /// when the only teardown was an implicit `Drop` of one fd duplicate.
    /// Idempotent: the stored handle is taken out on first use, so a
    /// second call is a no-op rather than a double `shutdown()`.
    pub fn disconnect(&self) {
        if let Some(stream) = self.control_stream.lock().unwrap().take() {
            if let Err(e) = stream.shutdown() {
                log::warn!(target: "remote", "control-socket shutdown failed: {e}");
            }
        }
    }

    pub fn supports_queue_append(&self) -> bool {
        self.ctrl_compatibility.supports_queue_append
    }

    pub fn supports_spectrum(&self) -> bool {
        self.ctrl_compatibility.supports_spectrum
    }

    fn stub_status(current_idx: usize, queue_len: usize) -> PlayerStatus {
        PlayerStatus {
            current_idx,
            queue_len,
            active: true,
            ..Default::default()
        }
    }

    /// Test helper for root-crate integration tests that need a remote-player
    /// stand-in without a live daemon connection.
    pub fn stub(items: Vec<MediaItem>, current_idx: usize) -> (Self, mpsc::Receiver<PlayerEvent>) {
        let (remote, event_rx, _cmd_rx) = Self::stub_with_command_rx(items, current_idx);
        (remote, event_rx)
    }

    /// Test helper variant that also exposes commands sent to the daemon.
    pub fn stub_with_command_rx(
        items: Vec<MediaItem>,
        current_idx: usize,
    ) -> (Self, mpsc::Receiver<PlayerEvent>, mpsc::Receiver<CtrlCmd>) {
        let queue_len = items.len();
        let status = Arc::new(Mutex::new(Self::stub_status(current_idx, queue_len)));
        let subtitle_prefs = Arc::new(Mutex::new(crate::player::SubtitlePrefs::default()));
        let items = Arc::new(Mutex::new(items));
        let queue_source = Arc::new(Mutex::new(crate::config::QueueSource::Unknown));
        let disconnected = Arc::new(AtomicBool::new(false));
        let (cmd_tx, cmd_rx) = mpsc::channel::<CtrlCmd>();
        let (_event_tx, event_rx) = mpsc::channel::<PlayerEvent>();
        let mut compat = CtrlCompatibility::current();
        compat.supports_spectrum = true;
        (
            RemotePlayer {
                status,
                subtitle_prefs,
                items,
                queue_source,
                cmd_tx,
                disconnected,
                ctrl_compatibility: compat,
                control_stream: Arc::new(Mutex::new(None)),
            },
            event_rx,
            cmd_rx,
        )
    }

    /// Test helper variant for callers that need a protocol-v2 remote handle.
    pub fn stub_v2_with_command_rx(
        items: Vec<MediaItem>,
        current_idx: usize,
    ) -> (Self, mpsc::Receiver<PlayerEvent>, mpsc::Receiver<CtrlCmd>) {
        let (mut remote, event_rx, cmd_rx) = Self::stub_with_command_rx(items, current_idx);
        remote.ctrl_compatibility = CtrlCompatibility::for_peer(2).unwrap();
        (remote, event_rx, cmd_rx)
    }
}
