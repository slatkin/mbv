use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use crate::api::{EmbyClient, MediaItem};
use crate::ctrl::{CtrlCmd, CtrlEvent, CtrlHello, DisconnectReason};
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
) -> Result<(BufReader<ControlStream>, CtrlEvent), String> {
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
    match hello {
        CtrlEvent::Hello(info) => {
            info.validate_peer()?;
            log::info!(
                target: "remote",
                "daemon protocol ok: version={} app={} capabilities={:?}",
                info.protocol_version,
                info.app_version,
                info.capabilities
            );
        }
        _ => {
            return Err("daemon did not send protocol hello".to_string());
        }
    }
    let client_hello = serde_json::to_string(&CtrlCmd::Hello(CtrlHello::current_client(
        auth_token.into(),
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

    Ok((reader, state_event))
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
    }
}

fn disconnect_reason_message(reason: &DisconnectReason) -> &'static str {
    match reason {
        DisconnectReason::TakenOverByCtrlClient => {
            "Another controller took over — returned to local mode"
        }
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
        let (reader, state_event) = crate::bounded::run_with_hard_bound(
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

    pub fn send_command(&self, cmd: PlayerCommand) -> bool {
        self.cmd_tx.send(CtrlCmd::PlayerCmd(cmd.into())).is_ok()
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
        (
            RemotePlayer {
                status,
                subtitle_prefs,
                items,
                queue_source,
                cmd_tx,
                disconnected,
            },
            event_rx,
            cmd_rx,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::QueueSource;
    use crate::ctrl::CtrlState;

    fn make_media_item(id: &str) -> MediaItem {
        MediaItem {
            id: id.into(),
            name: "Test Item".into(),
            item_type: "Episode".into(),
            is_folder: false,
            media_type: "Video".into(),
            collection_type: String::new(),
            runtime_ticks: 0,
            played: false,
            playback_position_ticks: 0,
            series_id: String::new(),
            series_name: String::new(),
            album_id: String::new(),
            album: String::new(),
            index_number: 0,
            parent_index_number: 0,
            unplayed_item_count: 0,
            path: String::new(),
            artist: String::new(),
            sort_name: String::new(),
            production_year: 0,
            end_year: 0,
            overview: String::new(),
            premiere_date: String::new(),
            date_added: String::new(),
            total_count: 0,
            container: String::new(),
            director: String::new(),
            video_info: String::new(),
            audio_info: String::new(),
            genre: String::new(),
            playlist_item_id: String::new(),
        }
    }

    fn status_with_idx(current_idx: usize) -> PlayerStatus {
        status_with_idx_and_len(current_idx, 0)
    }

    fn status_with_idx_and_len(current_idx: usize, queue_len: usize) -> PlayerStatus {
        RemotePlayer::stub_status(current_idx, queue_len)
    }

    #[test]
    fn daemon_endpoint_parses_local_and_unix_paths() {
        assert_eq!(
            DaemonEndpoint::parse("local").unwrap(),
            DaemonEndpoint::Local
        );
        assert_eq!(DaemonEndpoint::parse("").unwrap(), DaemonEndpoint::Local);
        assert_eq!(
            DaemonEndpoint::parse("unix:///tmp/mbv.sock").unwrap(),
            DaemonEndpoint::Unix(PathBuf::from("/tmp/mbv.sock"))
        );
        assert_eq!(
            DaemonEndpoint::parse("/tmp/mbv.sock").unwrap(),
            DaemonEndpoint::Unix(PathBuf::from("/tmp/mbv.sock"))
        );
        assert_eq!(
            DaemonEndpoint::parse("tcp://localhost:1234").unwrap(),
            DaemonEndpoint::Tcp(SocketAddr::from(([127, 0, 0, 1], 1234)))
        );
        assert_eq!(
            DaemonEndpoint::parse("tcp://127.0.0.1:1234").unwrap(),
            DaemonEndpoint::Tcp(SocketAddr::from(([127, 0, 0, 1], 1234)))
        );
        assert_eq!(
            DaemonEndpoint::parse("tcp://127.0.0.2:1234").unwrap(),
            DaemonEndpoint::Tcp(SocketAddr::from(([127, 0, 0, 2], 1234)))
        );
    }

    #[test]
    fn daemon_endpoint_rejects_unsupported_schemes() {
        assert_eq!(
            DaemonEndpoint::parse("tcp://10.0.0.1:1234").unwrap(),
            DaemonEndpoint::Tcp(SocketAddr::from(([10, 0, 0, 1], 1234)))
        );
        assert!(DaemonEndpoint::parse("tcp://[::1]:4321").is_err());
        assert!(DaemonEndpoint::parse("unix://").is_err());
        assert!(DaemonEndpoint::parse("http://localhost:1234").is_err());
    }

    #[test]
    fn status_only_preserves_event_confirmed_current_index() {
        let status = Arc::new(Mutex::new(status_with_idx(3)));
        let items = Arc::new(Mutex::new(Vec::new()));
        let queue_source = Arc::new(Mutex::new(QueueSource::Unknown));
        let (tx, _rx) = mpsc::channel();

        apply_ctrl_event(
            CtrlEvent::StatusOnly(status_with_idx(5)),
            &status,
            &items,
            &queue_source,
            &tx,
            true,
        );

        assert_eq!(status.lock().unwrap().current_idx, 3);
    }

    #[test]
    fn state_uses_cursor_as_current_index() {
        let status = Arc::new(Mutex::new(status_with_idx(0)));
        let items = Arc::new(Mutex::new(Vec::new()));
        let queue_source = Arc::new(Mutex::new(QueueSource::Unknown));
        let (tx, rx) = mpsc::channel();

        apply_ctrl_event(
            CtrlEvent::State(CtrlState {
                status: status_with_idx(5),
                items: Vec::new(),
                cursor: 3,
                source: QueueSource::Unknown,
            }),
            &status,
            &items,
            &queue_source,
            &tx,
            true,
        );

        assert_eq!(status.lock().unwrap().current_idx, 3);
        assert!(matches!(
            rx.recv().unwrap(),
            PlayerEvent::QueueUpdated { cursor: 3, .. }
        ));
    }

    #[test]
    fn status_only_preserves_current_idx_and_queue_len() {
        let status = Arc::new(Mutex::new(status_with_idx_and_len(3, 7)));
        let items = Arc::new(Mutex::new(Vec::new()));
        let queue_source = Arc::new(Mutex::new(QueueSource::Unknown));
        let (tx, _rx) = mpsc::channel();

        apply_ctrl_event(
            CtrlEvent::StatusOnly(status_with_idx_and_len(5, 2)),
            &status,
            &items,
            &queue_source,
            &tx,
            true,
        );

        let s = status.lock().unwrap();
        assert_eq!(s.current_idx, 3);
        assert_eq!(s.queue_len, 7);
    }

    #[test]
    fn state_derives_queue_len_from_items_not_status() {
        let status = Arc::new(Mutex::new(status_with_idx_and_len(0, 0)));
        let items = Arc::new(Mutex::new(Vec::new()));
        let queue_source = Arc::new(Mutex::new(QueueSource::Unknown));
        let (tx, _rx) = mpsc::channel();

        // s.status.queue_len (99) is stale relative to s.items.len() (2) — the
        // daemon broadcasts CtrlState before calling play_queue(...), so
        // items/cursor are authoritative over status at broadcast time.
        apply_ctrl_event(
            CtrlEvent::State(CtrlState {
                status: status_with_idx_and_len(5, 99),
                items: vec![make_media_item("a"), make_media_item("b")],
                cursor: 1,
                source: QueueSource::Unknown,
            }),
            &status,
            &items,
            &queue_source,
            &tx,
            true,
        );

        assert_eq!(status.lock().unwrap().queue_len, 2);
    }

    #[test]
    fn track_changed_updates_current_idx_but_not_queue_len() {
        let status = Arc::new(Mutex::new(status_with_idx_and_len(0, 5)));
        let items = Arc::new(Mutex::new(Vec::new()));
        let queue_source = Arc::new(Mutex::new(QueueSource::Unknown));
        let (tx, _rx) = mpsc::channel();

        apply_ctrl_event(
            CtrlEvent::Player(PlayerEvent::TrackChanged(2)),
            &status,
            &items,
            &queue_source,
            &tx,
            true,
        );

        let s = status.lock().unwrap();
        assert_eq!(s.current_idx, 2);
        assert_eq!(s.queue_len, 5);
    }

    #[test]
    fn command_rejected_forwards_reason_as_player_event() {
        let status = Arc::new(Mutex::new(status_with_idx(0)));
        let items = Arc::new(Mutex::new(Vec::new()));
        let queue_source = Arc::new(Mutex::new(QueueSource::Unknown));
        let (tx, rx) = mpsc::channel();

        apply_ctrl_event(
            CtrlEvent::CommandRejected("daemon is audio-only".to_string()),
            &status,
            &items,
            &queue_source,
            &tx,
            true,
        );

        match rx.recv().unwrap() {
            PlayerEvent::CommandRejected(reason) => {
                assert_eq!(reason, "daemon is audio-only");
            }
            _ => panic!("expected CommandRejected"),
        }
    }

    #[test]
    fn adopt_queue_returns_false_when_ctrl_socket_is_dead() {
        // #119 task 5: `adopt_queue`'s return value is the only signal that
        // the daemon never actually received the adoption — the call site
        // must not discard it and silently carry on with optimistic state.
        let (remote, _event_rx, cmd_rx) = RemotePlayer::stub_with_command_rx(Vec::new(), 0);
        drop(cmd_rx);

        let adopted = remote.adopt_queue(vec![make_media_item("1")], 0, QueueSource::Unknown);

        assert!(!adopted);
    }

    #[test]
    fn connect_endpoint_propagates_active_remote_playback_status() {
        // #175: a local `mbv` connected as the ctrl client of a remote
        // `mbvd` must mirror the daemon's active playback into
        // `RemotePlayer.status` -- that's the shared `Arc<Mutex<PlayerStatus>>`
        // MPRIS polls directly (see `src/mpris.rs::start`). This drives the
        // *real* TCP protocol path (hello exchange, initial `State`, then a
        // `StatusOnly` push) end-to-end, rather than calling
        // `apply_ctrl_event` directly, so it catches propagation bugs in the
        // reader thread / connect handshake that a unit-level test of
        // `apply_ctrl_event` alone would miss.
        use std::io::{BufRead, BufReader};
        use std::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        let daemon = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut writer = stream.try_clone().unwrap();
            let mut reader = BufReader::new(stream);

            // Protocol hello.
            let hello = serde_json::to_string(&CtrlEvent::Hello(CtrlHello::current())).unwrap();
            writeln!(writer, "{hello}").unwrap();

            // Read the client's hello back (unused beyond draining the line).
            let mut client_hello = String::new();
            reader.read_line(&mut client_hello).unwrap();

            // Initial baseline state: idle, nothing playing yet.
            let initial_state = serde_json::to_string(&CtrlEvent::State(CtrlState {
                status: PlayerStatus::default(),
                items: Vec::new(),
                cursor: 0,
                source: crate::config::QueueSource::Unknown,
            }))
            .unwrap();
            writeln!(writer, "{initial_state}").unwrap();

            // Now the daemon reports active playback, exactly like the #175
            // repro: an active `StatusOnly` push after the initial handshake.
            let active_status = serde_json::to_string(&CtrlEvent::StatusOnly(PlayerStatus {
                active: true,
                paused: false,
                title: "Song".to_string(),
                position_ticks: 5_000_000,
                runtime_ticks: 100_000_000,
                ..PlayerStatus::default()
            }))
            .unwrap();
            writeln!(writer, "{active_status}").unwrap();

            // Keep the connection open until the test thread is done with it.
            std::thread::sleep(Duration::from_millis(500));
        });

        let (remote, _event_rx) =
            RemotePlayer::connect_endpoint(&DaemonEndpoint::Tcp(addr), "token").unwrap();

        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        loop {
            if remote.status.lock().unwrap().active {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "remote status never reflected the daemon's active playback"
            );
            std::thread::sleep(Duration::from_millis(20));
        }

        let status = remote.status.lock().unwrap().clone();
        assert!(status.active);
        assert!(!status.paused);
        assert_eq!(status.title, "Song");

        daemon.join().unwrap();
    }

    #[test]
    fn perform_handshake_times_out_when_daemon_never_sends_hello() {
        // #191 fix 5: a daemon that accepts a TCP connection but never
        // speaks (never sends the protocol hello) must not hang the caller
        // forever -- the hard bound wrapping `perform_handshake` inside
        // `connect_endpoint` must kick in. This drives the real bounded
        // handshake path (real socket, real thread) rather than asserting
        // on config values, since ureq-style timeout knobs don't have
        // getters and a real stalled-listener test is the only way to prove
        // the join-timeout logic actually fires.
        use std::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        let daemon = std::thread::spawn(move || {
            // Accept the connection and then say nothing -- long enough to
            // outlive the test's much shorter hard bound below.
            let (_stream, _) = listener.accept().unwrap();
            std::thread::sleep(Duration::from_secs(2));
        });

        let stream = ControlStream::Tcp(TcpStream::connect(addr).unwrap());
        let result = crate::bounded::run_with_hard_bound(
            move || perform_handshake(stream, "token"),
            Duration::from_millis(50),
        );

        match result {
            Err(e) => assert_eq!(e, "timed out after 0s"),
            Ok(_) => panic!("expected perform_handshake to time out, got Ok"),
        }

        daemon.join().unwrap();
    }

    #[test]
    fn perform_handshake_succeeds_promptly_when_daemon_responds() {
        // Companion to the timeout test above: the fast/success path must
        // still work end-to-end through the same bounded wrapper used in
        // `connect_endpoint`, not just in isolation.
        use std::io::BufRead;
        use std::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        let daemon = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut writer = stream.try_clone().unwrap();
            let mut reader = BufReader::new(stream);

            let hello = serde_json::to_string(&CtrlEvent::Hello(CtrlHello::current())).unwrap();
            writeln!(writer, "{hello}").unwrap();

            let mut client_hello = String::new();
            reader.read_line(&mut client_hello).unwrap();

            let initial_state = serde_json::to_string(&CtrlEvent::State(CtrlState {
                status: PlayerStatus::default(),
                items: Vec::new(),
                cursor: 0,
                source: crate::config::QueueSource::Unknown,
            }))
            .unwrap();
            writeln!(writer, "{initial_state}").unwrap();
        });

        let stream = ControlStream::Tcp(TcpStream::connect(addr).unwrap());
        let result = crate::bounded::run_with_hard_bound(
            move || perform_handshake(stream, "token"),
            Duration::from_secs(5),
        );

        let (_reader, state_event) = match result {
            Ok(v) => v,
            Err(e) => panic!("expected Ok, got Err({e})"),
        };
        assert!(matches!(state_event, CtrlEvent::State(_)));

        daemon.join().unwrap();
    }
}
