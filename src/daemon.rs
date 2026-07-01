use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixListener;
use std::sync::{mpsc, Arc, Mutex};

use ksni::blocking::TrayMethods;

use crate::api::{EmbyClient, MediaItem};
use crate::ctrl::{CtrlCmd, CtrlEvent, CtrlState};
use crate::player::{Player, PlayerCommand, PlayerEvent};
use crate::ws::WsEvent;

enum DaemonEvent {
    Player(PlayerEvent),
    Ws(WsEvent),
    Ctrl(CtrlCmd),
    Shutdown,
}

type ClientList = Arc<Mutex<Vec<mpsc::Sender<String>>>>;

const TRAY_ICON: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/tray_icon.bin"));

struct MbyTray {
    shutdown_tx: mpsc::SyncSender<()>,
}

impl ksni::Tray for MbyTray {
    fn id(&self) -> String {
        "mbv".into()
    }
    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        vec![ksni::Icon {
            width: 24,
            height: 24,
            data: TRAY_ICON.to_vec(),
        }]
    }
    fn title(&self) -> String {
        "mbv".into()
    }
    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::*;
        vec![StandardItem {
            label: "Quit".into(),
            icon_name: "application-exit".into(),
            activate: Box::new(|tray: &mut Self| {
                let _ = tray.shutdown_tx.try_send(());
            }),
            ..Default::default()
        }
        .into()]
    }
}

pub fn pid_file() -> std::path::PathBuf {
    let dir = crate::config::data_dir_system_or_local();
    let _ = std::fs::create_dir_all(&dir);
    dir.join("mbv.pid")
}

fn broadcast(clients: &ClientList, event: &CtrlEvent) {
    if let Ok(json) = serde_json::to_string(event) {
        clients
            .lock()
            .unwrap()
            .retain(|tx| tx.send(json.clone()).is_ok());
    }
}

pub fn run(client: EmbyClient) -> ! {
    std::fs::write(pid_file(), std::process::id().to_string())
        .expect("mbv daemon: failed to write PID file");

    // Shared shutdown channel — written by SIGTERM thread and tray Quit item.
    let (shutdown_signal_tx, shutdown_signal_rx) = mpsc::sync_channel::<()>(1);

    // Block SIGTERM in all threads so sigwait() owns it exclusively.
    unsafe {
        let mut mask = std::mem::zeroed::<libc::sigset_t>();
        libc::sigemptyset(&mut mask);
        libc::sigaddset(&mut mask, libc::SIGTERM);
        libc::pthread_sigmask(libc::SIG_BLOCK, &mask, std::ptr::null_mut());
    }

    // Thread that blocks on SIGTERM and forwards it as a graceful shutdown.
    {
        let tx = shutdown_signal_tx.clone();
        std::thread::spawn(move || {
            let mut sig: libc::c_int = 0;
            let mut mask = unsafe { std::mem::zeroed::<libc::sigset_t>() };
            unsafe {
                libc::sigemptyset(&mut mask);
                libc::sigaddset(&mut mask, libc::SIGTERM);
                libc::sigwait(&mask, &mut sig);
            }
            let _ = tx.try_send(());
        });
    }

    let (player_tx, player_rx) = mpsc::channel();
    let (ws_tx_chan, ws_rx) = mpsc::channel();
    let ws_send_tx = crate::ws::start(client.ws_url(), ws_tx_chan);
    let subtitle_prefs = if client.config.subtitle_mode.is_empty()
        && client.config.subtitle_lang.is_empty()
        && client.config.audio_lang.is_empty()
    {
        client.get_user_subtitle_prefs().unwrap_or_default()
    } else {
        crate::player::SubtitlePrefs {
            mode: client.config.subtitle_mode.clone(),
            subtitle_lang: client.config.subtitle_lang.clone(),
            audio_lang: client.config.audio_lang.clone(),
        }
    };
    let player = Player::new(
        client.config.server_url.clone(),
        client.token.clone(),
        client.config.show_audio_window,
        client.config.use_mpv_config,
        client.config.no_scripts,
        client.config.always_play_next,
        client.config.always_skip_intro,
        subtitle_prefs,
        player_tx,
        Some(ws_send_tx),
    );

    let player_status = player.status.clone();
    let player_cmd_tx = player.cmd_tx.clone();
    crate::mpris::start(player_status, move |cmd| {
        if let Some(tx) = player_cmd_tx.lock().unwrap().as_ref() {
            let _ = tx.send(cmd);
        }
    });

    let _tray = if client.config.show_systray_icon {
        MbyTray {
            shutdown_tx: shutdown_signal_tx,
        }
        .spawn()
        .map_err(|e| {
            log::warn!(target: "tray", "not available: {e}");
        })
        .ok()
    } else {
        None
    };

    let (merged_tx, merged_rx) = mpsc::channel::<DaemonEvent>();

    let tx = merged_tx.clone();
    std::thread::spawn(move || {
        for ev in player_rx {
            let _ = tx.send(DaemonEvent::Player(ev));
        }
    });
    let tx = merged_tx.clone();
    std::thread::spawn(move || {
        for ev in ws_rx {
            let _ = tx.send(DaemonEvent::Ws(ev));
        }
    });
    let tx = merged_tx.clone();
    std::thread::spawn(move || {
        if shutdown_signal_rx.recv().is_ok() {
            let _ = tx.send(DaemonEvent::Shutdown);
        }
    });

    // Shared state for ctrl socket initial-state snapshots
    let shared_items: Arc<Mutex<Vec<MediaItem>>> = Arc::new(Mutex::new(Vec::new()));
    let shared_cursor: Arc<Mutex<usize>> = Arc::new(Mutex::new(0));
    let ctrl_clients: ClientList = Arc::new(Mutex::new(Vec::new()));

    // Spawn control socket listener
    {
        let ctrl_clients = ctrl_clients.clone();
        let merged_tx2 = merged_tx;
        let player_status = player.status.clone();
        let shared_items = shared_items.clone();
        let shared_cursor = shared_cursor.clone();

        std::thread::spawn(move || {
            let path = crate::config::control_socket_path();
            let _ = std::fs::remove_file(&path);
            let listener = match UnixListener::bind(&path) {
                Ok(l) => l,
                Err(e) => {
                    eprintln!("mbv daemon: ctrl socket bind failed ({e}), remote TUI unavailable");
                    return;
                }
            };

            // Restrict socket permissions to owner-only (prevents other users
            // on multi-user systems from controlling playback).
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
            }

            for stream in listener.incoming() {
                let Ok(stream) = stream else { continue };
                let Ok(stream_w) = stream.try_clone() else {
                    continue;
                };

                let (ev_tx, ev_rx) = mpsc::channel::<String>();

                // Build and enqueue initial state before registering, so it's
                // the first thing the writer thread sends.
                if let Ok(init_json) = serde_json::to_string(&CtrlEvent::State(CtrlState {
                    status: player_status.lock().unwrap().clone(),
                    items: shared_items.lock().unwrap().clone(),
                    cursor: *shared_cursor.lock().unwrap(),
                })) {
                    ev_tx.send(init_json).ok();
                }

                ctrl_clients.lock().unwrap().push(ev_tx);

                // Writer thread: drains ev_rx → socket
                std::thread::spawn(move || {
                    let mut w = stream_w;
                    for line in ev_rx {
                        if writeln!(w, "{line}").is_err() {
                            break;
                        }
                    }
                });

                // Reader thread: socket → merged_tx as DaemonEvent::Ctrl
                let ctrl_tx = merged_tx2.clone();
                std::thread::spawn(move || {
                    let reader = BufReader::new(stream);
                    for line in reader.lines() {
                        let Ok(line) = line else { break };
                        if line.is_empty() {
                            continue;
                        }
                        if let Ok(cmd) = serde_json::from_str::<CtrlCmd>(&line) {
                            let _ = ctrl_tx.send(DaemonEvent::Ctrl(cmd));
                        }
                    }
                });
            }
        });
    }

    // Broadcast current PlayerStatus to connected TUIs so the
    // seekbar and toggle state stay in sync without sending the full queue.
    {
        let broadcast_interval =
            std::time::Duration::from_millis(client.config.daemon_broadcast_ms);
        let player_status = player.status.clone();
        let ctrl_clients = ctrl_clients.clone();
        std::thread::spawn(move || loop {
            std::thread::sleep(broadcast_interval);
            if ctrl_clients.lock().unwrap().is_empty() {
                continue;
            }
            let status = player_status.lock().unwrap().clone();
            broadcast(&ctrl_clients, &CtrlEvent::StatusOnly(status));
        });
    }

    let client = Arc::new(Mutex::new(client));
    let mut items: Vec<MediaItem> = Vec::new();
    let mut cursor: usize = 0;

    for ev in merged_rx {
        match ev {
            DaemonEvent::Player(PlayerEvent::TrackChanged(idx)) => {
                cursor = idx;
                *shared_cursor.lock().unwrap() = idx;
                broadcast(
                    &ctrl_clients,
                    &CtrlEvent::Player(PlayerEvent::TrackChanged(idx)),
                );
            }
            DaemonEvent::Player(PlayerEvent::NextUpThreshold {
                series_id,
                season,
                episode,
            }) => {
                if let Some(item) = items.get(cursor + 1) {
                    player.send_command(PlayerCommand::NextUpShow {
                        item_id: item.id.clone(),
                        show_title: item.series_name.clone(),
                        ep_title: item.name.clone(),
                        artist: item.artist.clone(),
                    });
                }
                broadcast(
                    &ctrl_clients,
                    &CtrlEvent::Player(PlayerEvent::NextUpThreshold {
                        series_id,
                        season,
                        episode,
                    }),
                );
            }
            DaemonEvent::Player(PlayerEvent::PlaylistNextUp { next_idx }) => {
                if let Some(item) = items.get(next_idx) {
                    player.send_command(PlayerCommand::NextUpShow {
                        item_id: item.id.clone(),
                        show_title: item.series_name.clone(),
                        ep_title: item.name.clone(),
                        artist: item.artist.clone(),
                    });
                }
                broadcast(
                    &ctrl_clients,
                    &CtrlEvent::Player(PlayerEvent::PlaylistNextUp { next_idx }),
                );
            }
            DaemonEvent::Player(pe) => {
                broadcast(&ctrl_clients, &CtrlEvent::Player(pe));
            }
            DaemonEvent::Ws(ws_ev) => {
                handle_ws(
                    ws_ev,
                    &client,
                    &player,
                    &mut items,
                    &mut cursor,
                    &shared_items,
                    &shared_cursor,
                    &ctrl_clients,
                );
            }
            DaemonEvent::Ctrl(cmd) => {
                handle_ctrl(
                    cmd,
                    &client,
                    &player,
                    &mut items,
                    &mut cursor,
                    &shared_items,
                    &shared_cursor,
                    &ctrl_clients,
                );
            }
            DaemonEvent::Shutdown => {
                log::info!(target: "daemon", "graceful shutdown: stopping player");
                player.stop();
                player.join_or_timeout(std::time::Duration::from_secs(5));
                let _ = std::fs::remove_file(pid_file());
                std::process::exit(0);
            }
        }
    }
    unreachable!("daemon event channel closed")
}

fn handle_ctrl(
    cmd: CtrlCmd,
    client: &Arc<Mutex<EmbyClient>>,
    player: &Player,
    items: &mut Vec<MediaItem>,
    cursor: &mut usize,
    shared_items: &Arc<Mutex<Vec<MediaItem>>>,
    shared_cursor: &Arc<Mutex<usize>>,
    ctrl_clients: &ClientList,
) {
    match cmd {
        CtrlCmd::PlayerCmd(pc) => {
            player.send_command(pc);
        }
        CtrlCmd::PlayItems {
            item_ids,
            start_idx,
            start_ticks,
        } => {
            let fetched = {
                let c = client.lock().unwrap();
                match c.get_items_by_ids(&item_ids) {
                    Ok(v) => v,
                    Err(e) => {
                        log::warn!(target: "daemon", "ctrl play error: {e}");
                        return;
                    }
                }
            };
            if fetched.is_empty() {
                return;
            }
            if fetched.len() == 1 {
                let item = fetched[0].clone();
                if !item.series_id.is_empty() && player.always_play_next {
                    let queue = client
                        .lock()
                        .unwrap()
                        .get_episodes_from(&item.series_id, &item.id);
                    if queue.len() > 1 {
                        *items = queue.clone();
                        *cursor = 0;
                        *shared_items.lock().unwrap() = queue.clone();
                        *shared_cursor.lock().unwrap() = 0;
                        broadcast(
                            ctrl_clients,
                            &CtrlEvent::State(CtrlState {
                                status: player.status.lock().unwrap().clone(),
                                items: queue.clone(),
                                cursor: 0,
                            }),
                        );
                        let c = Arc::new(client.lock().unwrap().clone());
                        player.play_playlist(queue, 0, c, 100);
                        return;
                    }
                }
                *items = vec![item.clone()];
                *cursor = 0;
                *shared_items.lock().unwrap() = items.clone();
                *shared_cursor.lock().unwrap() = 0;
                broadcast(
                    ctrl_clients,
                    &CtrlEvent::State(CtrlState {
                        status: player.status.lock().unwrap().clone(),
                        items: items.clone(),
                        cursor: 0,
                    }),
                );
                let mut play_item = item;
                if start_ticks > 0 {
                    play_item.playback_position_ticks = start_ticks;
                }
                let c = Arc::new(client.lock().unwrap().clone());
                player.play(&play_item, c, 100);
            } else {
                let start_idx = start_idx.min(fetched.len().saturating_sub(1));
                let mut play_items = fetched.clone();
                if start_ticks > 0 {
                    play_items[start_idx].playback_position_ticks = start_ticks;
                }
                *items = play_items.clone();
                *cursor = start_idx;
                *shared_items.lock().unwrap() = play_items.clone();
                *shared_cursor.lock().unwrap() = start_idx;
                broadcast(
                    ctrl_clients,
                    &CtrlEvent::State(CtrlState {
                        status: player.status.lock().unwrap().clone(),
                        items: play_items.clone(),
                        cursor: start_idx,
                    }),
                );
                let c = Arc::new(client.lock().unwrap().clone());
                player.play_playlist(play_items, start_idx, c, 100);
            }
        }
        CtrlCmd::Stop => {
            player.stop();
        }
    }
}

fn handle_ws(
    ev: WsEvent,
    client: &Arc<Mutex<EmbyClient>>,
    player: &Player,
    items: &mut Vec<MediaItem>,
    cursor: &mut usize,
    shared_items: &Arc<Mutex<Vec<MediaItem>>>,
    shared_cursor: &Arc<Mutex<usize>>,
    ctrl_clients: &ClientList,
) {
    match ev {
        WsEvent::Play {
            item_ids,
            play_now,
            start_position_ticks,
            start_index,
        } => {
            if !play_now {
                return;
            }
            let fetched = {
                let c = client.lock().unwrap();
                match c.get_items_by_ids(&item_ids) {
                    Ok(v) => v,
                    Err(e) => {
                        log::warn!(target: "daemon", "play error: {e}");
                        return;
                    }
                }
            };
            if fetched.is_empty() {
                return;
            }
            // Clamp start_index in case the server sends an out-of-range value
            let start_idx = start_index.min(fetched.len().saturating_sub(1));
            *items = fetched.clone();
            *cursor = start_idx;
            *shared_items.lock().unwrap() = fetched.clone();
            *shared_cursor.lock().unwrap() = start_idx;
            // Broadcast updated playlist to connected TUIs
            if let Ok(json) = serde_json::to_string(&CtrlEvent::State(CtrlState {
                status: player.status.lock().unwrap().clone(),
                items: fetched.clone(),
                cursor: start_idx,
            })) {
                ctrl_clients
                    .lock()
                    .unwrap()
                    .retain(|tx| tx.send(json.clone()).is_ok());
            }
            if fetched.len() == 1 {
                let mut play_item = fetched[0].clone();
                if start_position_ticks > 0 {
                    play_item.playback_position_ticks = start_position_ticks;
                }
                let c = Arc::new(client.lock().unwrap().clone());
                player.play(&play_item, c, 100);
            } else {
                // Apply StartPositionTicks to the starting item
                let mut start_item = fetched[start_idx].clone();
                if start_position_ticks > 0 {
                    start_item.playback_position_ticks = start_position_ticks;
                }
                let mut items_with_pos = fetched.clone();
                items_with_pos[start_idx] = start_item;
                let c = Arc::new(client.lock().unwrap().clone());
                player.play_playlist(items_with_pos, start_idx, c, 100);
            }
        }
        WsEvent::Stop => {
            player.stop();
        }
        WsEvent::Pause => {
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
            if idx > 0 {
                player.send_command(PlayerCommand::JumpTo(idx - 1));
            }
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
        WsEvent::SetMute(muted) => {
            player.send_command(PlayerCommand::SetMute(muted));
        }
        WsEvent::ToggleMute => {
            let muted = !player.status.lock().unwrap().muted;
            player.send_command(PlayerCommand::SetMute(muted));
        }
        WsEvent::SetAudio(index) => {
            player.send_command(PlayerCommand::SetAudio(index));
        }
        WsEvent::SetSub(index) => {
            let sid = player
                .status
                .lock()
                .unwrap()
                .subtitle_stream_index_to_mpv_id(index);
            if let Some(sid) = sid {
                player.send_command(PlayerCommand::SetSub(sid));
            } else {
                log::warn!(target: "daemon", "subtitle stream index {index} did not match any mpv subtitle track");
            }
        }
        WsEvent::UserDataChanged => {}
    }
}
