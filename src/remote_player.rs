use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::sync::{Arc, Mutex, mpsc};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::api::{EmbyClient, MediaItem};
use crate::ctrl::{CtrlCmd, CtrlEvent};
use crate::player::{PlayerCommand, PlayerEvent, PlayerStatus};

pub struct RemotePlayer {
    pub status: Arc<Mutex<PlayerStatus>>,
    pub subs_off: Arc<AtomicBool>,
    pub items: Arc<Mutex<Vec<MediaItem>>>,
    cmd_tx: mpsc::Sender<CtrlCmd>,
    disconnected: Arc<AtomicBool>,
}

impl RemotePlayer {
    pub fn connect() -> Result<(Self, mpsc::Receiver<PlayerEvent>), String> {
        let path = crate::config::control_socket_path();
        let stream = UnixStream::connect(&path)
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
            audio_id: 0,
            audio_lang: String::new(),
            sub_id: 0,
            muted: false,
            video_height: 0,
        }));
        let subs_off = Arc::new(AtomicBool::new(true));
        let items: Arc<Mutex<Vec<MediaItem>>> = Arc::new(Mutex::new(Vec::new()));
        let disconnected = Arc::new(AtomicBool::new(false));

        let (event_tx, event_rx) = mpsc::channel::<PlayerEvent>();
        let (cmd_tx, cmd_rx) = mpsc::channel::<CtrlCmd>();

        // Reader thread: deserializes CtrlEvent lines from daemon
        let status_r = status.clone();
        let subs_off_r = subs_off.clone();
        let items_r = items.clone();
        let disconnected_r = disconnected.clone();
        let event_tx_r = event_tx;
        let stream_r = stream.try_clone().map_err(|e| e.to_string())?;
        std::thread::spawn(move || {
            let reader = BufReader::new(stream_r);
            for line in reader.lines() {
                match line {
                    Err(_) => break,
                    Ok(l) if l.is_empty() => continue,
                    Ok(l) => {
                        let Ok(ev) = serde_json::from_str::<CtrlEvent>(&l) else {
                            log::warn!(target: "remote", "unrecognized event from daemon: {l}");
                            continue;
                        };
                        match ev {
                            CtrlEvent::StatusOnly(s) => {
                                subs_off_r.store(s.sub_id == 0, Ordering::Relaxed);
                                *status_r.lock().unwrap() = s;
                            }
                            CtrlEvent::State(s) => {
                                subs_off_r.store(s.status.sub_id == 0, Ordering::Relaxed);
                                *status_r.lock().unwrap() = s.status;
                                *items_r.lock().unwrap() = s.items.clone();
                                let _ = event_tx_r.send(PlayerEvent::QueueUpdated {
                                    items: s.items,
                                    cursor: s.cursor,
                                });
                            }
                            CtrlEvent::Player(pe) => {
                                match &pe {
                                    PlayerEvent::Stopped { .. } => {
                                        status_r.lock().unwrap().active = false;
                                    }
                                    PlayerEvent::TrackChanged(idx) => {
                                        status_r.lock().unwrap().current_idx = *idx;
                                    }
                                    _ => {}
                                }
                                let _ = event_tx_r.send(pe);
                            }
                        }
                    }
                }
            }
            disconnected_r.store(true, Ordering::SeqCst);
            log::info!(target: "remote", "daemon disconnected");
            let _ = event_tx_r.send(PlayerEvent::Stopped { idx: 0, position_ticks: 0, played: false });
        });

        // Writer thread: serializes CtrlCmd to daemon
        let mut stream_w = stream;
        std::thread::spawn(move || {
            while let Ok(cmd) = cmd_rx.recv() {
                let Ok(json) = serde_json::to_string(&cmd) else { continue };
                if writeln!(stream_w, "{json}").is_err() {
                    break;
                }
            }
        });

        Ok((RemotePlayer { status, subs_off, items, cmd_tx, disconnected }, event_rx))
    }

    pub fn is_disconnected(&self) -> bool {
        self.disconnected.load(Ordering::SeqCst)
    }

    pub fn send_command(&self, cmd: PlayerCommand) {
        let _ = self.cmd_tx.send(CtrlCmd::PlayerCmd(cmd));
    }

    pub fn play(&self, item: &MediaItem, _client: Arc<EmbyClient>, _initial_volume: u8) {
        let _ = self.cmd_tx.send(CtrlCmd::PlayItems {
            item_ids: vec![item.id.clone()],
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
        let start_ticks = items.get(start_idx).map_or(0, |i| i.playback_position_ticks);
        let _ = self.cmd_tx.send(CtrlCmd::PlayItems { item_ids, start_ticks });
        *self.items.lock().unwrap() = items;
    }

    pub fn stop(&self) {
        let _ = self.cmd_tx.send(CtrlCmd::Stop);
    }

    pub fn join(&self) {
        // No thread to join; daemon keeps running when TUI exits.
    }
}
