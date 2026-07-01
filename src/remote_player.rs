use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};

use crate::api::{EmbyClient, MediaItem};
use crate::ctrl::{CtrlCmd, CtrlEvent, CtrlHello};
use crate::player::{PlayerCommand, PlayerEvent, PlayerStatus};

pub struct RemotePlayer {
    pub status: Arc<Mutex<PlayerStatus>>,
    pub subtitle_prefs: Arc<Mutex<crate::player::SubtitlePrefs>>,
    pub items: Arc<Mutex<Vec<MediaItem>>>,
    cmd_tx: mpsc::Sender<CtrlCmd>,
    disconnected: Arc<AtomicBool>,
}

fn apply_ctrl_event(
    ev: CtrlEvent,
    status: &Arc<Mutex<PlayerStatus>>,
    items: &Arc<Mutex<Vec<MediaItem>>>,
    event_tx: &mpsc::Sender<PlayerEvent>,
) {
    match ev {
        CtrlEvent::Hello(_) => {
            log::warn!(target: "remote", "unexpected daemon protocol hello after negotiation");
        }
        CtrlEvent::StatusOnly(s) => {
            let mut current = status.lock().unwrap();
            let current_idx = current.current_idx;
            *current = s;
            current.current_idx = current_idx;
        }
        CtrlEvent::State(s) => {
            let mut next_status = s.status;
            next_status.current_idx = s.cursor;
            *status.lock().unwrap() = next_status;
            *items.lock().unwrap() = s.items.clone();
            let _ = event_tx.send(PlayerEvent::QueueUpdated {
                items: s.items,
                cursor: s.cursor,
            });
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
            let _ = event_tx.send(pe);
        }
    }
}

impl RemotePlayer {
    pub fn connect() -> Result<(Self, mpsc::Receiver<PlayerEvent>), String> {
        let path = crate::config::control_socket_path();
        let mut stream = UnixStream::connect(&path)
            .map_err(|e| format!("cannot connect to daemon socket {path}: {e}"))?;
        log::info!(target: "remote", "connected to daemon socket {path}");

        let status = Arc::new(Mutex::new(PlayerStatus {
            position_ticks: 0,
            last_valid_pos: 0,
            runtime_ticks: 0,
            paused: false,
            volume: 100,
            volume_max: 130,
            current_idx: 0,
            active: false,
            title: String::new(),
            audio_tracks: Vec::new(),
            sub_tracks: Vec::new(),
            sub_track_stream_indexes: Vec::new(),
            audio_id: 0,
            audio_lang: String::new(),
            sub_id: 0,
            sub_lang: String::new(),
            muted: false,
            video_height: 0,
            audio_codec: String::new(),
            video_is_image: false,
        }));
        let subtitle_prefs = Arc::new(Mutex::new(crate::player::SubtitlePrefs::default()));
        let items: Arc<Mutex<Vec<MediaItem>>> = Arc::new(Mutex::new(Vec::new()));
        let disconnected = Arc::new(AtomicBool::new(false));

        let (event_tx, event_rx) = mpsc::channel::<PlayerEvent>();
        let (cmd_tx, cmd_rx) = mpsc::channel::<CtrlCmd>();

        let stream_r = stream.try_clone().map_err(|e| e.to_string())?;
        let mut reader = BufReader::new(stream_r);
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
        let client_hello = serde_json::to_string(&CtrlCmd::Hello(CtrlHello::current()))
            .map_err(|e| e.to_string())?;
        writeln!(stream, "{client_hello}")
            .map_err(|e| format!("failed to send daemon protocol hello: {e}"))?;

        // Reader thread: deserializes CtrlEvent lines from daemon
        let status_r = status.clone();
        let items_r = items.clone();
        let disconnected_r = disconnected.clone();
        let event_tx_r = event_tx;
        std::thread::spawn(move || {
            for line in reader.lines() {
                match line {
                    Err(_) => break,
                    Ok(l) if l.is_empty() => continue,
                    Ok(l) => {
                        let Ok(ev) = serde_json::from_str::<CtrlEvent>(&l) else {
                            log::warn!(target: "remote", "unrecognized event from daemon: {l}");
                            continue;
                        };
                        apply_ctrl_event(ev, &status_r, &items_r, &event_tx_r);
                    }
                }
            }
            disconnected_r.store(true, Ordering::SeqCst);
            log::info!(target: "remote", "daemon disconnected");
            let _ = event_tx_r.send(PlayerEvent::Stopped {
                idx: 0,
                position_ticks: 0,
                played: false,
                error: None,
            });
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
                cmd_tx,
                disconnected,
            },
            event_rx,
        ))
    }

    pub fn is_disconnected(&self) -> bool {
        self.disconnected.load(Ordering::SeqCst)
    }

    pub fn send_command(&self, cmd: PlayerCommand) -> bool {
        self.cmd_tx.send(CtrlCmd::PlayerCmd(cmd)).is_ok()
    }

    pub fn play(&self, item: &MediaItem, _client: Arc<EmbyClient>, _initial_volume: u8) {
        let _ = self.cmd_tx.send(CtrlCmd::PlayItems {
            item_ids: vec![item.id.clone()],
            start_idx: 0,
            start_ticks: item.playback_position_ticks,
        });
        *self.items.lock().unwrap() = vec![item.clone()];
    }

    pub fn play_playlist(
        &self,
        items: Vec<MediaItem>,
        start_idx: usize,
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
        });
        *self.items.lock().unwrap() = items;
    }

    pub fn stop(&self) {
        let _ = self.cmd_tx.send(CtrlCmd::Stop);
    }

    pub fn join(&self) {
        // No thread to join; daemon keeps running when TUI exits.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ctrl::CtrlState;

    fn status_with_idx(current_idx: usize) -> PlayerStatus {
        PlayerStatus {
            position_ticks: 0,
            last_valid_pos: 0,
            runtime_ticks: 0,
            paused: false,
            volume: 100,
            volume_max: 130,
            current_idx,
            active: true,
            title: String::new(),
            audio_tracks: Vec::new(),
            sub_tracks: Vec::new(),
            sub_track_stream_indexes: Vec::new(),
            audio_id: 0,
            audio_lang: String::new(),
            sub_id: 0,
            sub_lang: String::new(),
            muted: false,
            video_height: 0,
            audio_codec: String::new(),
            video_is_image: false,
        }
    }

    #[test]
    fn status_only_preserves_event_confirmed_current_index() {
        let status = Arc::new(Mutex::new(status_with_idx(3)));
        let items = Arc::new(Mutex::new(Vec::new()));
        let (tx, _rx) = mpsc::channel();

        apply_ctrl_event(
            CtrlEvent::StatusOnly(status_with_idx(5)),
            &status,
            &items,
            &tx,
        );

        assert_eq!(status.lock().unwrap().current_idx, 3);
    }

    #[test]
    fn state_uses_cursor_as_current_index() {
        let status = Arc::new(Mutex::new(status_with_idx(0)));
        let items = Arc::new(Mutex::new(Vec::new()));
        let (tx, rx) = mpsc::channel();

        apply_ctrl_event(
            CtrlEvent::State(CtrlState {
                status: status_with_idx(5),
                items: Vec::new(),
                cursor: 3,
            }),
            &status,
            &items,
            &tx,
        );

        assert_eq!(status.lock().unwrap().current_idx, 3);
        assert!(matches!(
            rx.recv().unwrap(),
            PlayerEvent::QueueUpdated { cursor: 3, .. }
        ));
    }
}
