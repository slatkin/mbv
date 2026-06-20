use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use std::io::ErrorKind;

use serde_json::Value;
use tungstenite::Message;

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
    Seek(i64),       // absolute, ticks
    SeekRelative(f64), // relative, seconds
    SetVolume(i64),
    VolumeUp,
    VolumeDown,
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
            let ids_value = data.as_object()
                .and_then(|obj| obj.iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case("itemids"))
                    .map(|(_, v)| v));
            let item_ids: Vec<String> = ids_value
                .and_then(|v| v.as_array())
                .map(|a| a.iter().filter_map(|v| {
                    v.as_str().map(str::to_string)
                        .or_else(|| v.as_i64().map(|n| n.to_string()))
                        .or_else(|| v.as_u64().map(|n| n.to_string()))
                }).collect())
                .unwrap_or_default();
            if item_ids.is_empty() {
                log::warn!(target: "ws", "Play: no ItemIds — raw data: {data}");
                return None;
            }
            let play_now = data["PlayCommand"].as_str().unwrap_or("PlayNow") == "PlayNow";
            let start_position_ticks = data["StartPositionTicks"].as_i64().unwrap_or(0);
            let start_index = data["StartIndex"].as_u64().unwrap_or(0) as usize;
            Some(WsEvent::Play { item_ids, play_now, start_position_ticks, start_index })
        }
        "Playstate" => {
            let cmd = v["Data"]["Command"].as_str().unwrap_or("");
            log::debug!(target: "ws", "Playstate cmd={cmd}");
            match cmd {
                "Stop"          => Some(WsEvent::Stop),
                "Pause"         => Some(WsEvent::Pause),
                "Unpause"       => Some(WsEvent::Unpause),
                "PlayPause"     => Some(WsEvent::TogglePause),
                "NextTrack"     => Some(WsEvent::NextTrack),
                "PreviousTrack" => Some(WsEvent::PreviousTrack),
                "Seek"          => Some(WsEvent::Seek(v["Data"]["SeekPositionTicks"].as_i64().unwrap_or(0))),
                "Rewind"        => Some(WsEvent::SeekRelative(-10.0)),
                "FastForward"   => Some(WsEvent::SeekRelative(10.0)),
                other           => { log::warn!(target: "ws", "Playstate: unhandled cmd={other}"); None }
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
                "VolumeUp"   => Some(WsEvent::VolumeUp),
                "VolumeDown" => Some(WsEvent::VolumeDown),
                other        => { log::warn!(target: "ws", "GeneralCommand: unhandled name={other}"); None }
            }
        }
        "UserDataChanged" => Some(WsEvent::UserDataChanged),
        _ => None,
    }
}

pub fn start(ws_url: String, event_tx: mpsc::Sender<WsEvent>) -> mpsc::Sender<String> {
    let (out_tx, out_rx) = mpsc::channel::<String>();
    thread::spawn(move || {
        loop {
            log::info!(target: "ws", "connecting…");
            match tungstenite::connect(&ws_url) {
                Ok((mut socket, _)) => {
                    // Short read timeout so we can drain outbound messages between reads.
                    let timeout = Some(Duration::from_millis(100));
                    match socket.get_ref() {
                        tungstenite::stream::MaybeTlsStream::Plain(tcp) => { let _ = tcp.set_read_timeout(timeout); }
                        tungstenite::stream::MaybeTlsStream::NativeTls(tls) => { let _ = tls.get_ref().set_read_timeout(timeout); }
                        _ => {}
                    }
                    log::info!(target: "ws", "connected");
                    'conn: loop {
                        while let Ok(msg) = out_rx.try_recv() {
                            if socket.send(Message::Text(msg)).is_err() {
                                log::warn!(target: "ws", "send error, reconnecting");
                                break 'conn;
                            }
                        }
                        match socket.read() {
                            Ok(Message::Text(txt)) => {
                                if let Some(ev) = parse(&txt) {
                                    if event_tx.send(ev).is_err() {
                                        return;
                                    }
                                }
                            }
                            Ok(Message::Ping(data)) => {
                                let _ = socket.send(Message::Pong(data));
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
                }
                Err(e) => {
                    log::warn!(target: "ws", "connect failed: {e}");
                }
            }
            thread::sleep(Duration::from_secs(5));
        }
    });
    out_tx
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_msg(text: &str) -> Option<WsEvent> {
        parse(text)
    }

    // ── Play ─────────────────────────────────────────────────────────────────

    #[test]
    fn play_parses_item_ids_and_play_now() {
        let msg = r#"{"MessageType":"Play","Data":{"ItemIds":["a","b"],"PlayCommand":"PlayNow","StartPositionTicks":0}}"#;
        let ev = parse_msg(msg).unwrap();
        if let WsEvent::Play { item_ids, play_now, start_position_ticks, start_index } = ev {
            assert_eq!(item_ids, vec!["a", "b"]);
            assert!(play_now);
            assert_eq!(start_position_ticks, 0);
            assert_eq!(start_index, 0);
        } else { panic!("wrong variant"); }
    }

    #[test]
    fn play_play_now_false_for_next() {
        let msg = r#"{"MessageType":"Play","Data":{"ItemIds":["x"],"PlayCommand":"PlayNext"}}"#;
        if let WsEvent::Play { play_now, .. } = parse_msg(msg).unwrap() {
            assert!(!play_now);
        } else { panic!(); }
    }

    #[test]
    fn play_start_position_ticks() {
        let msg = r#"{"MessageType":"Play","Data":{"ItemIds":["x"],"StartPositionTicks":50000000}}"#;
        if let WsEvent::Play { start_position_ticks, .. } = parse_msg(msg).unwrap() {
            assert_eq!(start_position_ticks, 50000000);
        } else { panic!(); }
    }

    #[test]
    fn play_start_index_parsed() {
        let msg = r#"{"MessageType":"Play","Data":{"ItemIds":["a","b","c"],"StartIndex":2}}"#;
        if let WsEvent::Play { start_index, .. } = parse_msg(msg).unwrap() {
            assert_eq!(start_index, 2);
        } else { panic!(); }
    }

    #[test]
    fn play_start_index_defaults_to_zero() {
        let msg = r#"{"MessageType":"Play","Data":{"ItemIds":["a","b"]}}"#;
        if let WsEvent::Play { start_index, .. } = parse_msg(msg).unwrap() {
            assert_eq!(start_index, 0);
        } else { panic!(); }
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
        } else { panic!(); }
    }

    // ── Playstate ────────────────────────────────────────────────────────────

    #[test]
    fn playstate_stop() {
        let msg = r#"{"MessageType":"Playstate","Data":{"Command":"Stop"}}"#;
        assert!(matches!(parse_msg(msg), Some(WsEvent::Stop)));
    }

    #[test]
    fn playstate_pause() {
        let msg = r#"{"MessageType":"Playstate","Data":{"Command":"Pause"}}"#;
        assert!(matches!(parse_msg(msg), Some(WsEvent::Pause)));
    }

    #[test]
    fn playstate_unpause() {
        let msg = r#"{"MessageType":"Playstate","Data":{"Command":"Unpause"}}"#;
        assert!(matches!(parse_msg(msg), Some(WsEvent::Unpause)));
    }

    #[test]
    fn playstate_toggle_pause() {
        let msg = r#"{"MessageType":"Playstate","Data":{"Command":"PlayPause"}}"#;
        assert!(matches!(parse_msg(msg), Some(WsEvent::TogglePause)));
    }

    #[test]
    fn playstate_next_track() {
        let msg = r#"{"MessageType":"Playstate","Data":{"Command":"NextTrack"}}"#;
        assert!(matches!(parse_msg(msg), Some(WsEvent::NextTrack)));
    }

    #[test]
    fn playstate_previous_track() {
        let msg = r#"{"MessageType":"Playstate","Data":{"Command":"PreviousTrack"}}"#;
        assert!(matches!(parse_msg(msg), Some(WsEvent::PreviousTrack)));
    }

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
        } else { panic!(); }
    }

    #[test]
    fn playstate_fast_forward() {
        let msg = r#"{"MessageType":"Playstate","Data":{"Command":"FastForward"}}"#;
        if let Some(WsEvent::SeekRelative(s)) = parse_msg(msg) {
            assert_eq!(s, 10.0);
        } else { panic!(); }
    }

    #[test]
    fn playstate_unknown_command_returns_none() {
        let msg = r#"{"MessageType":"Playstate","Data":{"Command":"FlyToMoon"}}"#;
        assert!(parse_msg(msg).is_none());
    }

    // ── GeneralCommand ───────────────────────────────────────────────────────

    #[test]
    fn general_command_play_pause() {
        let msg = r#"{"MessageType":"GeneralCommand","Data":{"Name":"PlayPause"}}"#;
        assert!(matches!(parse_msg(msg), Some(WsEvent::TogglePause)));
    }

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
    fn general_command_unknown_returns_none() {
        let msg = r#"{"MessageType":"GeneralCommand","Data":{"Name":"SomethingUnknown"}}"#;
        assert!(parse_msg(msg).is_none());
    }

    // ── Other message types ──────────────────────────────────────────────────

    #[test]
    fn user_data_changed() {
        let msg = r#"{"MessageType":"UserDataChanged"}"#;
        assert!(matches!(parse_msg(msg), Some(WsEvent::UserDataChanged)));
    }

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
}
