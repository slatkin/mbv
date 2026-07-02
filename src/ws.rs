use std::io::ErrorKind;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc,
    Arc,
};
use std::thread;
use std::time::{Duration, Instant};

use rand::RngExt;

use serde_json::Value;
use tungstenite::Message;

pub enum OutboundMessage {
    Text(String),
    Flush(mpsc::Sender<()>),
}

#[derive(Clone)]
pub struct WsSender {
    tx: mpsc::Sender<OutboundMessage>,
    connected: Arc<AtomicBool>,
}

impl WsSender {
    pub fn send_text(&self, msg: String) -> Result<(), mpsc::SendError<OutboundMessage>> {
        self.tx.send(OutboundMessage::Text(msg))
    }

    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Relaxed)
    }

    pub fn flush(&self, timeout: Duration) -> bool {
        let (tx, rx) = mpsc::channel();
        if self.tx.send(OutboundMessage::Flush(tx)).is_err() {
            return false;
        }
        rx.recv_timeout(timeout).is_ok()
    }
}

fn drop_stale_outbound(out_rx: &mpsc::Receiver<OutboundMessage>) {
    while let Ok(msg) = out_rx.try_recv() {
        if let OutboundMessage::Flush(tx) = msg {
            let _ = tx.send(());
        }
    }
}

pub enum WsEvent {
    Play {
        item_ids: Vec<String>,
        play_now: bool,
        start_position_ticks: i64,
        /// Index into item_ids of the first item to play; preceding items are
        /// already-queued but not current.
        start_index: usize,
    },
    Stop,
    Pause,
    Unpause,
    TogglePause,
    NextTrack,
    PreviousTrack,
    Seek(i64),         // absolute, ticks
    SeekRelative(f64), // relative, seconds
    SetVolume(i64),
    VolumeUp,
    VolumeDown,
    SetMute(bool),
    ToggleMute,
    SetAudio(i64),
    SetSub(i64),
    UserDataChanged,
}

fn parse(text: &str) -> Option<WsEvent> {
    let v: Value = serde_json::from_str(text).ok()?;
    let msg_type = v["MessageType"].as_str()?;
    log::debug!(target: "ws", "inbound: {msg_type}");

    match msg_type {
        "Play" => {
            let data = &v["Data"];
            // Case-insensitive key search ("ItemIds", "itemIds", etc.)
            let ids_value = data.as_object().and_then(|obj| {
                obj.iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case("itemids"))
                    .map(|(_, v)| v)
            });
            let item_ids: Vec<String> = ids_value
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|v| {
                            v.as_str()
                                .map(str::to_string)
                                .or_else(|| v.as_i64().map(|n| n.to_string()))
                                .or_else(|| v.as_u64().map(|n| n.to_string()))
                        })
                        .collect()
                })
                .unwrap_or_default();
            if item_ids.is_empty() {
                log::warn!(target: "ws", "Play: no ItemIds — raw data: {data}");
                return None;
            }
            let play_now = data["PlayCommand"].as_str().unwrap_or("PlayNow") == "PlayNow";
            let start_position_ticks = data["StartPositionTicks"].as_i64().unwrap_or(0);
            let start_index = data["StartIndex"].as_u64().unwrap_or(0) as usize;
            Some(WsEvent::Play {
                item_ids,
                play_now,
                start_position_ticks,
                start_index,
            })
        }
        "Playstate" => {
            let cmd = v["Data"]["Command"].as_str().unwrap_or("");
            log::debug!(target: "ws", "Playstate cmd={cmd}");
            match cmd {
                "Stop" => Some(WsEvent::Stop),
                "Pause" => Some(WsEvent::Pause),
                "Unpause" => Some(WsEvent::Unpause),
                "PlayPause" => Some(WsEvent::TogglePause),
                "NextTrack" => Some(WsEvent::NextTrack),
                "PreviousTrack" => Some(WsEvent::PreviousTrack),
                "Seek" => Some(WsEvent::Seek(
                    v["Data"]["SeekPositionTicks"].as_i64().unwrap_or(0),
                )),
                "Rewind" => Some(WsEvent::SeekRelative(-10.0)),
                "FastForward" => Some(WsEvent::SeekRelative(10.0)),
                other => {
                    log::warn!(target: "ws", "Playstate: unhandled cmd={other}");
                    None
                }
            }
        }
        "GeneralCommand" => {
            let name = v["Data"]["Name"].as_str().unwrap_or("");
            log::debug!(target: "ws", "GeneralCommand name={name}");
            match name {
                "PlayPause" => Some(WsEvent::TogglePause),
                "SetVolume" => {
                    let vol = v["Data"]["Arguments"]["Volume"]
                        .as_str()
                        .and_then(|s| s.parse::<i64>().ok())
                        .or_else(|| v["Data"]["Arguments"]["Volume"].as_i64())
                        .unwrap_or(50);
                    Some(WsEvent::SetVolume(vol))
                }
                "VolumeUp" => Some(WsEvent::VolumeUp),
                "VolumeDown" => Some(WsEvent::VolumeDown),
                "Mute" => Some(WsEvent::SetMute(true)),
                "Unmute" => Some(WsEvent::SetMute(false)),
                "ToggleMute" => Some(WsEvent::ToggleMute),
                "SetAudioStreamIndex" => {
                    let idx = v["Data"]["Arguments"]["Index"]
                        .as_str()
                        .and_then(|s| s.parse::<i64>().ok())
                        .or_else(|| v["Data"]["Arguments"]["Index"].as_i64())
                        .unwrap_or(0);
                    Some(WsEvent::SetAudio(idx))
                }
                "SetSubtitleStreamIndex" => {
                    let idx = v["Data"]["Arguments"]["Index"]
                        .as_str()
                        .and_then(|s| s.parse::<i64>().ok())
                        .or_else(|| v["Data"]["Arguments"]["Index"].as_i64())
                        .unwrap_or(-1);
                    Some(WsEvent::SetSub(idx))
                }
                other => {
                    log::warn!(target: "ws", "GeneralCommand: unhandled name={other}");
                    None
                }
            }
        }
        "UserDataChanged" => Some(WsEvent::UserDataChanged),
        _ => None,
    }
}

pub fn start(ws_url: String, event_tx: mpsc::Sender<WsEvent>) -> WsSender {
    let (out_tx, out_rx) = mpsc::channel::<OutboundMessage>();
    let connected = Arc::new(AtomicBool::new(false));
    let connected_bg = connected.clone();
    thread::spawn(move || {
        let mut backoff_secs: u64 = 1;
        loop {
            connected_bg.store(false, Ordering::Relaxed);
            log::info!(target: "ws", "connecting…");
            match tungstenite::connect(&ws_url) {
                Ok((mut socket, _)) => {
                    // Successful connection — reset backoff.
                    backoff_secs = 1;

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
                    log::info!(target: "ws", "connected");

                    // Drop any stale outbound text messages buffered while disconnected so
                    // an old progress update is never replayed after reconnect.
                    drop_stale_outbound(&out_rx);
                    connected_bg.store(true, Ordering::Relaxed);

                    let mut last_activity = Instant::now();
                    let mut last_ping = Instant::now();
                    const PING_INTERVAL: Duration = Duration::from_secs(20);
                    const PONG_TIMEOUT: Duration = Duration::from_secs(45);

                    'conn: loop {
                        // Send outbound messages.
                        while let Ok(msg) = out_rx.try_recv() {
                            match msg {
                                OutboundMessage::Text(msg) => {
                                    if socket.send(Message::Text(msg)).is_err() {
                                        log::warn!(target: "ws", "send error, reconnecting");
                                        break 'conn;
                                    }
                                }
                                OutboundMessage::Flush(tx) => {
                                    let _ = tx.send(());
                                }
                            }
                        }

                        // M4: Send periodic heartbeat pings.
                        if last_ping.elapsed() >= PING_INTERVAL {
                            if socket.send(Message::Ping(vec![])).is_err() {
                                log::warn!(target: "ws", "ping send failed, reconnecting");
                                break 'conn;
                            }
                            last_ping = Instant::now();
                        }

                        // M4: Detect stale connection (no data for PONG_TIMEOUT).
                        if last_activity.elapsed() >= PONG_TIMEOUT {
                            log::warn!(target: "ws", "no response for {:.0}s, reconnecting",
                                last_activity.elapsed().as_secs_f64());
                            break 'conn;
                        }

                        match socket.read() {
                            Ok(Message::Text(txt)) => {
                                last_activity = Instant::now();
                                if let Some(ev) = parse(&txt) {
                                    if event_tx.send(ev).is_err() {
                                        return;
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
                                log::info!(target: "ws", "closed by server, reconnecting");
                                break 'conn;
                            }
                            Err(tungstenite::Error::Io(e))
                                if e.kind() == ErrorKind::WouldBlock
                                    || e.kind() == ErrorKind::TimedOut => {}
                            Err(e) => {
                                log::warn!(target: "ws", "error: {e}, reconnecting");
                                break 'conn;
                            }
                            _ => {}
                        }
                    }
                    connected_bg.store(false, Ordering::Relaxed);
                }
                Err(e) => {
                    log::warn!(target: "ws", "connect failed: {e}");
                }
            }
            // M3: Exponential backoff with jitter, max 60s.
            let jitter: f64 = rand::rng().random_range(0.0..1.0);
            let delay = Duration::from_secs_f64(backoff_secs as f64 + jitter);
            log::info!(target: "ws", "reconnecting in {:.1}s (backoff={backoff_secs}s)", delay.as_secs_f64());
            thread::sleep(delay);
            backoff_secs = (backoff_secs * 2).min(60);
        }
    });
    WsSender { tx: out_tx, connected }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;
    use std::time::Duration;

    fn parse_msg(text: &str) -> Option<WsEvent> {
        parse(text)
    }

    // ── Play ─────────────────────────────────────────────────────────────────

    #[test]
    fn play_parses_item_ids_and_play_now() {
        let msg = r#"{"MessageType":"Play","Data":{"ItemIds":["a","b"],"PlayCommand":"PlayNow","StartPositionTicks":0}}"#;
        let ev = parse_msg(msg).unwrap();
        if let WsEvent::Play {
            item_ids,
            play_now,
            start_position_ticks,
            start_index,
        } = ev
        {
            assert_eq!(item_ids, vec!["a", "b"]);
            assert!(play_now);
            assert_eq!(start_position_ticks, 0);
            assert_eq!(start_index, 0);
        } else {
            panic!("wrong variant");
        }
    }

    #[test]
    fn play_play_now_false_for_next() {
        let msg = r#"{"MessageType":"Play","Data":{"ItemIds":["x"],"PlayCommand":"PlayNext"}}"#;
        if let WsEvent::Play { play_now, .. } = parse_msg(msg).unwrap() {
            assert!(!play_now);
        } else {
            panic!();
        }
    }

    #[test]
    fn play_start_position_ticks() {
        let msg =
            r#"{"MessageType":"Play","Data":{"ItemIds":["x"],"StartPositionTicks":50000000}}"#;
        if let WsEvent::Play {
            start_position_ticks,
            ..
        } = parse_msg(msg).unwrap()
        {
            assert_eq!(start_position_ticks, 50000000);
        } else {
            panic!();
        }
    }

    #[test]
    fn play_start_index_parsed() {
        let msg = r#"{"MessageType":"Play","Data":{"ItemIds":["a","b","c"],"StartIndex":2}}"#;
        if let WsEvent::Play { start_index, .. } = parse_msg(msg).unwrap() {
            assert_eq!(start_index, 2);
        } else {
            panic!();
        }
    }

    #[test]
    fn play_empty_item_ids_returns_none() {
        let msg = r#"{"MessageType":"Play","Data":{"ItemIds":[]}}"#;
        assert!(parse_msg(msg).is_none());
    }

    #[test]
    fn play_item_ids_case_insensitive_key() {
        let msg = r#"{"MessageType":"Play","Data":{"itemIds":["z"]}}"#;
        if let Some(WsEvent::Play { item_ids, .. }) = parse_msg(msg) {
            assert_eq!(item_ids, vec!["z"]);
        } else {
            panic!();
        }
    }

    // ── Playstate ────────────────────────────────────────────────────────────

    #[test]
    fn playstate_seek_absolute() {
        let msg = r#"{"MessageType":"Playstate","Data":{"Command":"Seek","SeekPositionTicks":100000000}}"#;
        assert!(matches!(parse_msg(msg), Some(WsEvent::Seek(100000000))));
    }

    #[test]
    fn playstate_rewind() {
        let msg = r#"{"MessageType":"Playstate","Data":{"Command":"Rewind"}}"#;
        if let Some(WsEvent::SeekRelative(s)) = parse_msg(msg) {
            assert_eq!(s, -10.0);
        } else {
            panic!();
        }
    }

    #[test]
    fn playstate_fast_forward() {
        let msg = r#"{"MessageType":"Playstate","Data":{"Command":"FastForward"}}"#;
        if let Some(WsEvent::SeekRelative(s)) = parse_msg(msg) {
            assert_eq!(s, 10.0);
        } else {
            panic!();
        }
    }

    #[test]
    fn playstate_unknown_command_returns_none() {
        let msg = r#"{"MessageType":"Playstate","Data":{"Command":"FlyToMoon"}}"#;
        assert!(parse_msg(msg).is_none());
    }

    // ── GeneralCommand ───────────────────────────────────────────────────────

    #[test]
    fn general_command_set_volume_string() {
        let msg = r#"{"MessageType":"GeneralCommand","Data":{"Name":"SetVolume","Arguments":{"Volume":"80"}}}"#;
        assert!(matches!(parse_msg(msg), Some(WsEvent::SetVolume(80))));
    }

    #[test]
    fn general_command_set_volume_number() {
        let msg = r#"{"MessageType":"GeneralCommand","Data":{"Name":"SetVolume","Arguments":{"Volume":75}}}"#;
        assert!(matches!(parse_msg(msg), Some(WsEvent::SetVolume(75))));
    }

    #[test]
    fn general_command_volume_up() {
        let msg = r#"{"MessageType":"GeneralCommand","Data":{"Name":"VolumeUp"}}"#;
        assert!(matches!(parse_msg(msg), Some(WsEvent::VolumeUp)));
    }

    #[test]
    fn general_command_volume_down() {
        let msg = r#"{"MessageType":"GeneralCommand","Data":{"Name":"VolumeDown"}}"#;
        assert!(matches!(parse_msg(msg), Some(WsEvent::VolumeDown)));
    }

    #[test]
    fn general_command_mute_controls() {
        let mute = r#"{"MessageType":"GeneralCommand","Data":{"Name":"Mute"}}"#;
        let unmute = r#"{"MessageType":"GeneralCommand","Data":{"Name":"Unmute"}}"#;
        let toggle = r#"{"MessageType":"GeneralCommand","Data":{"Name":"ToggleMute"}}"#;
        assert!(matches!(parse_msg(mute), Some(WsEvent::SetMute(true))));
        assert!(matches!(parse_msg(unmute), Some(WsEvent::SetMute(false))));
        assert!(matches!(parse_msg(toggle), Some(WsEvent::ToggleMute)));
    }

    #[test]
    fn general_command_stream_indices() {
        let audio = r#"{"MessageType":"GeneralCommand","Data":{"Name":"SetAudioStreamIndex","Arguments":{"Index":"2"}}}"#;
        let sub = r#"{"MessageType":"GeneralCommand","Data":{"Name":"SetSubtitleStreamIndex","Arguments":{"Index":3}}}"#;
        assert!(matches!(parse_msg(audio), Some(WsEvent::SetAudio(2))));
        assert!(matches!(parse_msg(sub), Some(WsEvent::SetSub(3))));
    }

    #[test]
    fn general_command_negative_subtitle_index_disables_subtitles() {
        let msg = r#"{"MessageType":"GeneralCommand","Data":{"Name":"SetSubtitleStreamIndex","Arguments":{"Index":-1}}}"#;
        assert!(matches!(parse_msg(msg), Some(WsEvent::SetSub(-1))));
    }

    #[test]
    fn general_command_unknown_returns_none() {
        let msg = r#"{"MessageType":"GeneralCommand","Data":{"Name":"SomethingUnknown"}}"#;
        assert!(parse_msg(msg).is_none());
    }

    // ── Other message types ──────────────────────────────────────────────────

    #[test]
    fn unknown_message_type_returns_none() {
        let msg = r#"{"MessageType":"SomethingElse"}"#;
        assert!(parse_msg(msg).is_none());
    }

    #[test]
    fn malformed_json_returns_none() {
        assert!(parse_msg("not json").is_none());
    }

    #[test]
    fn missing_message_type_returns_none() {
        assert!(parse_msg(r#"{"Data":{}}"#).is_none());
    }

    #[test]
    fn flush_acknowledges_without_a_connection() {
        let (tx, rx) = mpsc::channel();
        let sender = WsSender {
            tx,
            connected: Arc::new(AtomicBool::new(false)),
        };
        std::thread::spawn(move || {
            if let Ok(OutboundMessage::Flush(done)) = rx.recv() {
                let _ = done.send(());
            }
        });
        assert!(sender.flush(Duration::from_millis(100)));
    }

    #[test]
    fn drop_stale_outbound_discards_text_but_preserves_flush() {
        let (tx, rx) = mpsc::channel();
        let (done_tx, done_rx) = mpsc::channel();
        tx.send(OutboundMessage::Text("stale".into())).unwrap();
        tx.send(OutboundMessage::Flush(done_tx)).unwrap();

        drop_stale_outbound(&rx);

        assert!(done_rx.recv_timeout(Duration::from_millis(100)).is_ok());
        assert!(rx.try_recv().is_err());
    }
}
