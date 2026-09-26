//! Minimal Engine.IO v4 / Socket.IO v4 client for Audiobookshelf progress
//! push, mirroring `ws.rs`'s background-thread/mpsc/backoff shape.
//!
//! Connects directly with `transport=websocket` — no polling handshake.
//! Handles Engine.IO framing (open/ping/pong/message) and, inside message
//! packets, Socket.IO v4 packet types (connect-ack/event).

use std::io::ErrorKind;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use serde::Deserialize;
use serde_json::Value;
use tungstenite::Message;

// ---------------------------------------------------------------------------
// Typed events
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum SocketEvent {
    /// Engine.IO `open` packet with server heartbeat parameters.
    Open {
        ping_interval: Duration,
        ping_timeout: Duration,
    },
    /// Socket.IO CONNECT acknowledgement (`40{...}`).
    ConnectAck,
    /// `42["init", {...}]` — auth accepted (ABS emits `init` on success).
    Authenticated,
    /// `42["auth_failed", {...}]` — auth rejected.
    InvalidToken,
    /// `42["user_item_progress_updated", {...}]`.
    ProgressUpdated(AudiobookshelfProgress),
}

// ---------------------------------------------------------------------------
// Wire types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct AudiobookshelfProgress {
    pub library_item_id: String,
    pub episode_id: String,
    pub current_time_seconds: f64,
    pub is_finished: bool,
}

#[derive(Debug, Deserialize)]
struct ProgressWire {
    #[serde(rename = "libraryItemId")]
    library_item_id: String,
    #[serde(rename = "episodeId")]
    episode_id: Option<String>,
    #[serde(rename = "currentTime")]
    current_time: Option<f64>,
    #[serde(rename = "isFinished")]
    is_finished: Option<bool>,
}

// ---------------------------------------------------------------------------
// URL helper
// ---------------------------------------------------------------------------

/// Build a `wss://` (or `ws://`) WebSocket URL for Audiobookshelf's
/// Engine.IO v4 endpoint from the configured server URL.
#[must_use]
pub fn socket_url(server_url: &str) -> Option<String> {
    let rest = server_url
        .strip_prefix("https://")
        .or_else(|| server_url.strip_prefix("http://"))?;
    let scheme = if server_url.starts_with("https://") {
        "wss://"
    } else {
        "ws://"
    };
    Some(format!(
        "{scheme}{rest}/socket.io/?EIO=4&transport=websocket"
    ))
}

// ---------------------------------------------------------------------------
// Framing parser — Engine.IO v4 / Socket.IO v4
// ---------------------------------------------------------------------------

fn parse(text: &str) -> Option<SocketEvent> {
    match text.as_bytes().first()? {
        // Engine.IO open packet: `0{...}`
        b'0' => parse_open(&text[1..]),
        // Engine.IO message packet: `4...` → decode Socket.IO packet type at text[1].
        b'4' => parse_socket_io(&text[1..]),
        // Close, ping, pong, and unknown packet types are handled or ignored
        // at the connection layer.
        _ => None,
    }
}

/// Engine.IO `open` packet body is a JSON object with `pingInterval`,
/// `pingTimeout`, `sid`, `upgrades`, `maxPayload`.
fn parse_open(body: &str) -> Option<SocketEvent> {
    let v: Value = serde_json::from_str(body).ok()?;
    Some(SocketEvent::Open {
        ping_interval: Duration::from_millis(v["pingInterval"].as_u64().unwrap_or(25_000)),
        ping_timeout: Duration::from_millis(v["pingTimeout"].as_u64().unwrap_or(20_000)),
    })
}

/// Socket.IO packet types inside an Engine.IO message (`4` prefix stripped).
fn parse_socket_io(payload: &str) -> Option<SocketEvent> {
    match payload.as_bytes().first()? {
        // 0 = CONNECT. Server sends `40{"sid":"..."}` as connect acknowledgement.
        b'0' => Some(SocketEvent::ConnectAck),
        // 2 = EVENT. Payload is a JSON array: `["event_name", ...]`.
        b'2' => parse_event(&payload[1..]),
        // DISCONNECT and every other packet type are not handled here.
        _ => None,
    }
}

/// Socket.IO EVENT packet payload is `["<name>", <data?>, <ack_id?>?]`.
fn parse_event(args_json: &str) -> Option<SocketEvent> {
    let v: Value = serde_json::from_str(args_json).ok()?;
    let args = v.as_array()?;
    let name = args.first()?.as_str()?;
    match name {
        // Current ABS server events (SocketAuthority.js): `init` after a
        // successful auth, `auth_failed` after a rejected token.
        "init" => Some(SocketEvent::Authenticated),
        "auth_failed" => Some(SocketEvent::InvalidToken),
        "user_item_progress_updated" => {
            let payload = args.get(1)?;
            let progress = decode_progress(payload)?;
            Some(SocketEvent::ProgressUpdated(progress))
        }
        // Every other event (stream_progress, user_online, etc.) is not
        // listening progress.
        _ => None,
    }
}

/// The `user_item_progress_updated` event payload is `{"id": ..., "data": {...}}`
/// where `data` is a full `MediaProgress` object matching `ProgressWire`.
fn decode_progress(payload: &Value) -> Option<AudiobookshelfProgress> {
    let data = &payload["data"];
    let wire: ProgressWire = serde_json::from_value(data.clone()).ok()?;
    let episode_id = wire.episode_id?;
    Some(AudiobookshelfProgress {
        library_item_id: wire.library_item_id,
        episode_id,
        current_time_seconds: wire.current_time.unwrap_or(0.0).max(0.0),
        is_finished: wire.is_finished.unwrap_or(false),
    })
}

// ---------------------------------------------------------------------------
// Background connection thread
// ---------------------------------------------------------------------------

/// Start the background WebSocket connection thread, sending parsed
/// [`SocketEvent`]s to `event_tx`. Sending on the returned sender signals
/// shutdown.
///
/// The thread handles Engine.IO ping/pong heartbeats, Socket.IO authentication,
/// and reconnects with exponential backoff capped at 60s.
#[must_use]
pub fn start(
    ws_url: String,
    token: String,
    event_tx: mpsc::Sender<SocketEvent>,
) -> mpsc::Sender<()> {
    let (shutdown_tx, shutdown_rx) = mpsc::channel::<()>();
    thread::spawn(move || run_socket_thread(&ws_url, &token, &event_tx, &shutdown_rx));
    shutdown_tx
}

fn run_socket_thread(
    ws_url: &str,
    token: &str,
    event_tx: &mpsc::Sender<SocketEvent>,
    shutdown_rx: &mpsc::Receiver<()>,
) {
    // Default heartbeat params from Engine.IO spec — overwritten by `open`
    // packet once received.
    let mut ping_interval = Duration::from_secs(25);
    let mut ping_timeout = Duration::from_secs(20);
    let mut backoff_secs: u64 = 1;
    let mut shutdown_requested = false;

    'reconnect: loop {
        log::info!(target: "audiobookshelf_socket", "connecting…");
        match tungstenite::connect(ws_url) {
            Ok((socket, _)) => {
                backoff_secs = 1;
                shutdown_requested = match run_connected(
                    socket,
                    token,
                    event_tx,
                    shutdown_rx,
                    &mut ping_interval,
                    &mut ping_timeout,
                ) {
                    Ok(shutdown_requested) => shutdown_requested,
                    Err(()) => return,
                };
            }
            Err(e) => log::warn!(target: "audiobookshelf_socket", "connect failed: {e}"),
        }

        if shutdown_requested {
            log::info!(
                target: "audiobookshelf_socket",
                "shutdown requested, exiting reconnect loop"
            );
            break 'reconnect;
        }

        // Exponential backoff with jitter, max 60s.
        mbv_net::reconnect_backoff_sleep(&mut backoff_secs, "audiobookshelf_socket");
    }
}

fn run_connected(
    mut socket: tungstenite::WebSocket<tungstenite::stream::MaybeTlsStream<std::net::TcpStream>>,
    token: &str,
    event_tx: &mpsc::Sender<SocketEvent>,
    shutdown_rx: &mpsc::Receiver<()>,
    ping_interval: &mut Duration,
    ping_timeout: &mut Duration,
) -> Result<bool, ()> {
    // Short read timeout so we can drain outbound messages between reads.
    let timeout = Some(Duration::from_millis(100));
    match socket.get_ref() {
        tungstenite::stream::MaybeTlsStream::Plain(tcp) => {
            let _ = tcp.set_read_timeout(timeout);
        }
        tungstenite::stream::MaybeTlsStream::NativeTls(tls) => {
            let _ = tls.get_ref().set_read_timeout(timeout);
        }
        _ => {}
    }

    // Socket.IO v4: open the default namespace. A send failure at the WebSocket
    // level will surface on the next read/send inside 'conn and trigger reconnect.
    let _ = socket.send(Message::Text("40".into()));
    log::info!(target: "audiobookshelf_socket", "connected");
    let mut last_activity = Instant::now();

    'conn: loop {
        if shutdown_rx.try_recv().is_ok() {
            return Ok(true);
        }

        // Engine.IO v4 heartbeat: the SERVER pings every `ping_interval` and
        // we reply pong below; a client-sent ping is a protocol error.
        if last_activity.elapsed() >= *ping_interval + *ping_timeout {
            log::warn!(
                target: "audiobookshelf_socket",
                "no data for {:.0}s, reconnecting",
                last_activity.elapsed().as_secs_f64()
            );
            break 'conn;
        }

        match socket.read() {
            Ok(Message::Text(txt)) => {
                last_activity = Instant::now();
                if !handle_text(
                    &mut socket,
                    &txt,
                    token,
                    event_tx,
                    ping_interval,
                    ping_timeout,
                ) {
                    return Err(());
                }
            }
            Ok(Message::Ping(data)) => {
                last_activity = Instant::now();
                let _ = socket.send(Message::Pong(data));
            }
            Ok(Message::Pong(_)) => last_activity = Instant::now(),
            Ok(Message::Close(_)) => {
                log::info!(target: "audiobookshelf_socket", "closed by server, reconnecting");
                break 'conn;
            }
            Err(tungstenite::Error::Io(e))
                if e.kind() == ErrorKind::WouldBlock || e.kind() == ErrorKind::TimedOut => {}
            Err(e) => {
                log::warn!(target: "audiobookshelf_socket", "error: {e}, reconnecting");
                break 'conn;
            }
            _ => {}
        }
    }
    Ok(false)
}

fn handle_text(
    socket: &mut tungstenite::WebSocket<tungstenite::stream::MaybeTlsStream<std::net::TcpStream>>,
    txt: &str,
    token: &str,
    event_tx: &mpsc::Sender<SocketEvent>,
    ping_interval: &mut Duration,
    ping_timeout: &mut Duration,
) -> bool {
    // Engine.IO ping/pong are handled at the connection layer.
    if txt == "2" {
        let _ = socket.send(Message::Text("3".into()));
        return true;
    }
    if txt == "3" {
        return true;
    }

    if let Some(ev) = parse(txt) {
        match ev {
            SocketEvent::Open {
                ping_interval: pi,
                ping_timeout: pt,
            } => {
                *ping_interval = pi;
                *ping_timeout = pt;
            }
            SocketEvent::ConnectAck => {
                // Server reads the token as the first event argument — a bare
                // string, not an object.
                let auth_payload = serde_json::json!(["auth", token]);
                let _ = socket.send(Message::Text(format!("42{auth_payload}").into()));
            }
            other => {
                if event_tx.send(other).is_err() {
                    // App dropped event receiver → stop the background thread.
                    return false;
                }
            }
        }
    }
    true
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    // -- Engine.IO open packet -----------------------------------------------

    #[test]
    fn open_packet_extracts_ping_interval_and_timeout() {
        let msg = r#"0{"sid":"abc","upgrades":[],"pingInterval":30000,"pingTimeout":10000}"#;
        match parse(msg) {
            Some(SocketEvent::Open {
                ping_interval,
                ping_timeout,
            }) => {
                assert_eq!(ping_interval, Duration::from_secs(30));
                assert_eq!(ping_timeout, Duration::from_millis(10_000));
            }
            other => panic!("expected Open, got {other:?}"),
        }
    }

    #[test]
    fn open_packet_defaults_when_missing() {
        let msg = r#"0{"sid":"abc"}"#;
        match parse(msg) {
            Some(SocketEvent::Open {
                ping_interval,
                ping_timeout,
            }) => {
                assert_eq!(ping_interval, Duration::from_secs(25));
                assert_eq!(ping_timeout, Duration::from_secs(20));
            }
            other => panic!("expected Open, got {other:?}"),
        }
    }

    // -- user_item_progress_updated decode -----------------------------------

    #[test]
    fn progress_updated_decodes_event() {
        let msg = r#"42["user_item_progress_updated",{"id":"abc","data":{"libraryItemId":"lib-1","episodeId":"ep-1","currentTime":120.0,"isFinished":false}}]"#;
        match parse(msg) {
            Some(SocketEvent::ProgressUpdated(progress)) => {
                assert_eq!(progress.library_item_id, "lib-1");
                assert_eq!(progress.episode_id, "ep-1");
                assert!((progress.current_time_seconds - 120.0).abs() < 1e-9);
                assert!(!progress.is_finished);
            }
            other => panic!("expected ProgressUpdated, got {other:?}"),
        }
    }

    #[test]
    fn progress_updated_finished() {
        let msg = r#"42["user_item_progress_updated",{"id":"abc","data":{"libraryItemId":"lib-1","episodeId":"ep-1","currentTime":3000.0,"isFinished":true}}]"#;
        match parse(msg) {
            Some(SocketEvent::ProgressUpdated(progress)) => {
                assert_eq!(progress.library_item_id, "lib-1");
                assert_eq!(progress.episode_id, "ep-1");
                assert!((progress.current_time_seconds - 3000.0).abs() < 1e-9);
                assert!(progress.is_finished);
            }
            other => panic!("expected ProgressUpdated, got {other:?}"),
        }
    }

    // -- Unknown events -------------------------------------------------------

    #[test]
    fn unknown_event_returns_none() {
        let msg = r#"42["user_online",{"userId":"u1"}]"#;
        assert_eq!(parse(msg), None);
    }

    // -- Authenticated / InvalidToken (ABS emits `init` / `auth_failed`) -----

    #[test]
    fn authenticated_event() {
        let msg = r#"42["init",{"userId":"u1","username":"slatkin"}]"#;
        assert_eq!(parse(msg), Some(SocketEvent::Authenticated));
    }

    #[test]
    fn invalid_token_event() {
        let msg = r#"42["auth_failed",{"message":"Invalid token"}]"#;
        assert_eq!(parse(msg), Some(SocketEvent::InvalidToken));
    }

    // -- Malformed / truncated / unknown framing -----------------------------

    #[rstest]
    #[case::malformed_json("not json")]
    #[case::truncated_open_packet("0{{{")]
    #[case::engine_ping("2")]
    #[case::engine_pong("3")]
    #[case::engine_close("1")]
    #[case::message_disconnect("41")]
    #[case::message_ack("43")]
    #[case::message_connect_error("44")]
    fn parse_rejects_input(#[case] msg: &str) {
        assert_eq!(parse(msg), None);
    }

    #[test]
    fn non_array_event_payload_returns_none() {
        let msg = r#"42"not-an-array""#;
        assert_eq!(parse(msg), None);
    }

    #[test]
    fn connect_ack_parses() {
        let msg = r#"40{"sid":"s-1"}"#;
        assert_eq!(parse(msg), Some(SocketEvent::ConnectAck));
    }

    // -- socket_url helper ---------------------------------------------------

    #[test]
    fn socket_url_https_to_wss() {
        let url = socket_url("https://media.example.test/path").unwrap();
        assert_eq!(
            url,
            "wss://media.example.test/path/socket.io/?EIO=4&transport=websocket"
        );
    }

    #[test]
    fn socket_url_http_to_ws() {
        let url = socket_url("http://192.168.1.100").unwrap();
        assert_eq!(
            url,
            "ws://192.168.1.100/socket.io/?EIO=4&transport=websocket"
        );
    }

    #[test]
    fn socket_url_invalid() {
        assert_eq!(socket_url("ftp://bad"), None);
    }
}
