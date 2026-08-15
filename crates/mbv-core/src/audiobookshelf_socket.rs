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

use rand::RngExt;
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
    /// `42["authenticated"]` — auth accepted.
    Authenticated,
    /// `42["invalid_token"]` — auth rejected.
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
        // Engine.IO close packet: `1` — handled at connection layer.
        b'1' => None,
        // Engine.IO ping/pong — handled at connection layer (reply pong / update activity).
        b'2' | b'3' => None,
        // Engine.IO message packet: `4...` → decode Socket.IO packet type at text[1].
        b'4' => parse_socket_io(&text[1..]),
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
        // 1 = DISCONNECT — server closing the namespace. Handled at connection layer.
        b'1' => None,
        // 2 = EVENT. Payload is a JSON array: `["event_name", ...]`.
        b'2' => parse_event(&payload[1..]),
        // 3 = ACK, 4 = CONNECT_ERROR, 5 = BINARY_EVENT, 6 = BINARY_ACK — not handled.
        _ => None,
    }
}

/// Socket.IO EVENT packet payload is `["<name>", <data?>, <ack_id?>?]`.
fn parse_event(args_json: &str) -> Option<SocketEvent> {
    let v: Value = serde_json::from_str(args_json).ok()?;
    let args = v.as_array()?;
    let name = args.first()?.as_str()?;
    match name {
        "authenticated" => Some(SocketEvent::Authenticated),
        "invalid_token" => Some(SocketEvent::InvalidToken),
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
/// where `data` is a full MediaProgress object matching `ProgressWire`.
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
/// The thread handles:
/// - Engine.IO ping/pong heartbeat using the server's declared `pingInterval`/
///   `pingTimeout` from the `open` packet.
/// - Socket.IO CONNECT (`40`) on initial connection and every reconnect.
/// - Auth emit (`42["auth", {"token": ...}]`) after each connect-ack.
/// - Reconnect with exponential backoff capped at 60s.
pub fn start(
    ws_url: String,
    token: String,
    event_tx: mpsc::Sender<SocketEvent>,
) -> mpsc::Sender<()> {
    let (shutdown_tx, shutdown_rx) = mpsc::channel::<()>();

    thread::spawn(move || {
        // Default heartbeat params from Engine.IO spec — overwritten by `open`
        // packet once received.
        let mut ping_interval = Duration::from_millis(25_000);
        let mut ping_timeout = Duration::from_millis(20_000);

        let mut backoff_secs: u64 = 1;
        let mut shutdown_requested = false;

        'reconnect: loop {
            log::info!(target: "audiobookshelf_socket", "connecting…");

            match tungstenite::connect(&ws_url) {
                Ok((mut socket, _)) => {
                    backoff_secs = 1;

                    // Short read timeout so we can drain outbound messages
                    // between reads.
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

                    // Socket.IO v4: open the default namespace. A send
                    // failure at the WebSocket level will surface on the
                    // next read/send inside 'conn and trigger reconnect.
                    let _ = socket.send(Message::Text("40".into()));

                    log::info!(target: "audiobookshelf_socket", "connected");

                    let mut last_activity = Instant::now();
                    let mut last_ping = Instant::now();

                    'conn: loop {
                        if shutdown_rx.try_recv().is_ok() {
                            shutdown_requested = true;
                            break 'conn;
                        }

                        // Engine.IO heartbeat: send a ping every ping_interval
                        // (the server also pings us; we reply in the read
                        // match below).
                        if last_ping.elapsed() >= ping_interval {
                            if socket.send(Message::Text("2".into())).is_err() {
                                log::warn!(
                                    target: "audiobookshelf_socket",
                                    "ping send failed, reconnecting"
                                );
                                break 'conn;
                            }
                            last_ping = Instant::now();
                        }

                        // Detect stale connection: no data for longer than
                        // ping_interval + ping_timeout.
                        if last_activity.elapsed() >= ping_interval + ping_timeout {
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

                                // Engine.IO ping → respond with pong.
                                if txt == "2" {
                                    let _ = socket.send(Message::Text("3".into()));
                                    continue;
                                }
                                // Engine.IO pong — activity is already updated above.
                                if txt == "3" {
                                    continue;
                                }

                                if let Some(ev) = parse(&txt) {
                                    match ev {
                                        SocketEvent::Open {
                                            ping_interval: pi,
                                            ping_timeout: pt,
                                        } => {
                                            ping_interval = pi;
                                            ping_timeout = pt;
                                        }
                                        SocketEvent::ConnectAck => {
                                            // Authenticate immediately after the
                                            // Socket.IO connect acknowledgement.
                                            let auth_payload = serde_json::json!([
                                                "auth",
                                                { "token": token }
                                            ]);
                                            let _ = socket.send(Message::Text(
                                                format!("42{auth_payload}").into(),
                                            ));
                                        }
                                        other => {
                                            if event_tx.send(other).is_err() {
                                                // App dropped event receiver → stop.
                                                return;
                                            }
                                        }
                                    }
                                }
                            }
                            Ok(Message::Ping(data)) => {
                                last_activity = Instant::now();
                                let _ = socket.send(Message::Pong(data));
                            }
                            Ok(Message::Pong(_)) => {
                                last_activity = Instant::now();
                            }
                            Ok(Message::Close(_)) => {
                                log::info!(
                                    target: "audiobookshelf_socket",
                                    "closed by server, reconnecting"
                                );
                                break 'conn;
                            }
                            Err(tungstenite::Error::Io(e))
                                if e.kind() == ErrorKind::WouldBlock
                                    || e.kind() == ErrorKind::TimedOut => {}
                            Err(e) => {
                                log::warn!(
                                    target: "audiobookshelf_socket",
                                    "error: {e}, reconnecting"
                                );
                                break 'conn;
                            }
                            _ => {}
                        }
                    }
                }
                Err(e) => {
                    log::warn!(
                        target: "audiobookshelf_socket",
                        "connect failed: {e}"
                    );
                }
            }

            if shutdown_requested {
                log::info!(
                    target: "audiobookshelf_socket",
                    "shutdown requested, exiting reconnect loop"
                );
                break 'reconnect;
            }

            // Exponential backoff with jitter, max 60s.
            let jitter: f64 = rand::rng().random_range(0.0..1.0);
            let delay = Duration::from_secs_f64(backoff_secs as f64 + jitter);
            log::info!(
                target: "audiobookshelf_socket",
                "reconnecting in {:.1}s (backoff={backoff_secs}s)",
                delay.as_secs_f64()
            );
            thread::sleep(delay);
            backoff_secs = (backoff_secs * 2).min(60);
        }
    });

    shutdown_tx
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- Engine.IO open packet -----------------------------------------------

    #[test]
    fn open_packet_extracts_ping_interval_and_timeout() {
        let msg = r#"0{"sid":"abc","upgrades":[],"pingInterval":30000,"pingTimeout":10000}"#;
        match parse(msg) {
            Some(SocketEvent::Open {
                ping_interval,
                ping_timeout,
            }) => {
                assert_eq!(ping_interval, Duration::from_millis(30_000));
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
                assert_eq!(ping_interval, Duration::from_millis(25_000));
                assert_eq!(ping_timeout, Duration::from_millis(20_000));
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

    // -- Authenticated / InvalidToken ----------------------------------------

    #[test]
    fn authenticated_event() {
        let msg = r#"42["authenticated"]"#;
        assert_eq!(parse(msg), Some(SocketEvent::Authenticated));
    }

    #[test]
    fn invalid_token_event() {
        let msg = r#"42["invalid_token"]"#;
        assert_eq!(parse(msg), Some(SocketEvent::InvalidToken));
    }

    // -- Malformed / truncated / unknown framing -----------------------------

    #[test]
    fn malformed_json_returns_none() {
        assert_eq!(parse("not json"), None);
    }

    #[test]
    fn truncated_open_packet_returns_none() {
        assert_eq!(parse("0{{{"), None);
    }

    #[test]
    fn non_array_event_payload_returns_none() {
        let msg = r#"42"not-an-array""#;
        assert_eq!(parse(msg), None);
    }

    #[test]
    fn engine_ping_returns_none() {
        assert_eq!(parse("2"), None);
    }

    #[test]
    fn engine_pong_returns_none() {
        assert_eq!(parse("3"), None);
    }

    #[test]
    fn engine_close_returns_none() {
        assert_eq!(parse("1"), None);
    }

    #[test]
    fn message_disconnect_returns_none() {
        assert_eq!(parse("41"), None);
    }

    #[test]
    fn message_ack_returns_none() {
        assert_eq!(parse("43"), None);
    }

    #[test]
    fn message_connect_error_returns_none() {
        assert_eq!(parse("44"), None);
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
