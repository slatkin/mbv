use std::sync::{Arc, Mutex, mpsc};

use ksni::blocking::TrayMethods;

use crate::api::{EmbyClient, MediaItem};
use crate::applog::{AppLog, Level};
use crate::player::{Player, PlayerCommand, PlayerEvent};
use crate::ws::WsEvent;

enum DaemonEvent {
    Player(PlayerEvent),
    Ws(WsEvent),
}

struct MbyTray;

impl ksni::Tray for MbyTray {
    fn id(&self) -> String {
        "mby".into()
    }
    fn icon_name(&self) -> String {
        "applications-multimedia".into()
    }
    fn title(&self) -> String {
        "mby".into()
    }
    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::*;
        vec![
            StandardItem {
                label: "Quit".into(),
                icon_name: "application-exit".into(),
                activate: Box::new(|_| std::process::exit(0)),
                ..Default::default()
            }
            .into(),
        ]
    }
}

pub fn pid_file() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    let dir = std::path::PathBuf::from(home).join(".local/share/mby");
    let _ = std::fs::create_dir_all(&dir);
    dir.join("mby.pid")
}

pub fn run(client: EmbyClient) -> ! {
    std::fs::write(pid_file(), std::process::id().to_string())
        .expect("mby daemon: failed to write PID file");

    let (player_tx, player_rx) = mpsc::channel();
    let (ws_tx_chan, ws_rx)    = mpsc::channel();
    let log = AppLog::new(50);
    let ws_send_tx = crate::ws::start(client.ws_url(), ws_tx_chan, log.clone());
    let player = Player::new(
        client.config.server_url.clone(),
        client.token.clone(),
        client.config.show_audio_window,
        client.config.use_mpv_config,
        client.config.no_scripts,
        client.config.always_play_next,
        player_tx,
        Some(ws_send_tx),
    );

    let player_status  = player.status.clone();
    let player_cmd_tx  = player.cmd_tx.clone();
    crate::mpris::start(player_status, move |cmd| {
        if let Some(tx) = player_cmd_tx.lock().unwrap().as_ref() {
            let _ = tx.send(cmd);
        }
    });

    let _tray = if client.config.show_systray_icon {
        MbyTray.spawn()
            .map_err(|e| log.push(Level::Warn, "tray", format!("not available: {e}")))
            .ok()
    } else {
        None
    };

    let (merged_tx, merged_rx) = mpsc::channel::<DaemonEvent>();

    let tx = merged_tx.clone();
    std::thread::spawn(move || {
        for ev in player_rx { let _ = tx.send(DaemonEvent::Player(ev)); }
    });
    let tx = merged_tx;
    std::thread::spawn(move || {
        for ev in ws_rx { let _ = tx.send(DaemonEvent::Ws(ev)); }
    });

    let client = Arc::new(Mutex::new(client));
    let mut items: Vec<MediaItem> = Vec::new();
    let mut cursor: usize = 0;

    for ev in merged_rx {
        match ev {
            DaemonEvent::Player(PlayerEvent::TrackChanged(idx)) => { cursor = idx; }
            DaemonEvent::Player(_) => {}
            DaemonEvent::Ws(ws_ev) => {
                handle_ws(ws_ev, &client, &player, &mut items, &mut cursor, &log);
            }
        }
    }
    unreachable!("daemon event channel closed")
}

fn handle_ws(
    ev: WsEvent,
    client: &Arc<Mutex<EmbyClient>>,
    player: &Player,
    items: &mut Vec<MediaItem>,
    cursor: &mut usize,
    log: &AppLog,
) {
    match ev {
        WsEvent::Play { item_ids, play_now, start_position_ticks } => {
            if !play_now { return; }
            let fetched = {
                let c = client.lock().unwrap();
                match c.get_items_by_ids(&item_ids) {
                    Ok(v) => v,
                    Err(e) => { log.push(Level::Warn, "daemon", format!("play error: {e}")); return; }
                }
            };
            if fetched.is_empty() { return; }
            *items  = fetched.clone();
            *cursor = 0;
            let c = Arc::new(client.lock().unwrap().clone());
            if fetched.len() == 1 {
                let mut item = fetched[0].clone();
                if start_position_ticks > 0 {
                    item.playback_position_ticks = start_position_ticks;
                }
                player.play(&item, c, log.clone());
            } else {
                let active = player.status.lock().unwrap().active;
                if active {
                    player.play(&fetched[0], c, log.clone());
                } else {
                    player.play_playlist(fetched, 0, c, log.clone());
                }
            }
        }
        WsEvent::Stop   => { player.stop(); }
        WsEvent::Pause  => {
            if !player.status.lock().unwrap().paused {
                player.send_command(PlayerCommand::TogglePause);
            }
        }
        WsEvent::Unpause => {
            if player.status.lock().unwrap().paused {
                player.send_command(PlayerCommand::TogglePause);
            }
        }
        WsEvent::NextTrack => {
            let idx = player.status.lock().unwrap().current_idx;
            if idx + 1 < items.len() {
                player.send_command(PlayerCommand::JumpTo(idx + 1));
            }
        }
        WsEvent::PreviousTrack => {
            let idx = player.status.lock().unwrap().current_idx;
            if idx > 0 { player.send_command(PlayerCommand::JumpTo(idx - 1)); }
        }
        WsEvent::Seek(ticks) => {
            use crate::api::TICKS_PER_SECOND;
            player.send_command(PlayerCommand::SeekAbsolute(
                ticks as f64 / TICKS_PER_SECOND as f64,
            ));
        }
        WsEvent::TogglePause => {
            player.send_command(PlayerCommand::TogglePause);
        }
        WsEvent::SeekRelative(secs) => {
            player.send_command(PlayerCommand::Seek(secs));
        }
        WsEvent::SetVolume(v) => {
            let vol_max = player.status.lock().unwrap().volume_max;
            player.send_command(PlayerCommand::SetVolume(v.clamp(0, vol_max)));
        }
        WsEvent::VolumeUp => {
            let st = player.status.lock().unwrap();
            let v = (st.volume + 5).min(st.volume_max);
            drop(st);
            player.send_command(PlayerCommand::SetVolume(v));
        }
        WsEvent::VolumeDown => {
            let v = (player.status.lock().unwrap().volume - 5).max(0);
            player.send_command(PlayerCommand::SetVolume(v));
        }
        WsEvent::UserDataChanged => {}
    }
}
