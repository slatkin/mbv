use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::os::unix::net::UnixListener;
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

use crate::api::{mbv_direct_tcp_port_command, EmbyClient, MediaItem};
use crate::ctrl::{CtrlCmd, CtrlEvent, CtrlHello, CtrlState};
use crate::player::{Player, PlayerCommand, PlayerEvent};
use crate::ws::WsEvent;
use ksni::blocking::TrayMethods;

/// Shared by the startup registration and the periodic 10-minute
/// re-registration in the main loop below.
fn register_capabilities(client: &EmbyClient, direct_commands: &[String], audio_only: bool) {
    client.register_capabilities_with_options(direct_commands, audio_only);
}

fn bind_ctrl_listener() -> Option<UnixListener> {
    let path = crate::config::control_socket_path();
    let _ = std::fs::remove_file(&path);
    match UnixListener::bind(&path) {
        Ok(listener) => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
            }
            Some(listener)
        }
        Err(e) => {
            log::error!(
                target: "daemon",
                "ctrl socket bind failed ({e}), remote TUI unavailable"
            );
            None
        }
    }
}

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

fn spawn_ctrl_client<R, W>(
    reader_stream: R,
    writer_stream: W,
    merged_tx: mpsc::Sender<DaemonEvent>,
    ctrl_clients: ClientList,
    client: Arc<Mutex<EmbyClient>>,
    player_status: Arc<Mutex<crate::player::PlayerStatus>>,
    shared_items: Arc<Mutex<Vec<MediaItem>>>,
    shared_cursor: Arc<Mutex<usize>>,
) where
    R: std::io::Read + Send + 'static,
    W: Write + Send + 'static,
{
    let (ev_tx, ev_rx) = mpsc::channel::<String>();

    if let Ok(hello_json) = serde_json::to_string(&CtrlEvent::Hello(CtrlHello::current())) {
        ev_tx.send(hello_json).ok();
    }

    std::thread::spawn(move || {
        let mut w = writer_stream;
        for line in ev_rx {
            if writeln!(w, "{line}").is_err() {
                break;
            }
        }
    });

    std::thread::spawn(move || {
        let reader = BufReader::new(reader_stream);
        let mut lines = reader.lines();
        let Some(Ok(line)) = lines.next() else {
            return;
        };
        match serde_json::from_str::<CtrlCmd>(&line) {
            Ok(CtrlCmd::Hello(info)) => {
                if let Err(e) = info.validate_peer() {
                    log::warn!(target: "daemon", "rejecting ctrl client: {e}");
                    return;
                }
                let Some(auth_token) = info.auth_token.as_deref() else {
                    log::warn!(target: "daemon", "rejecting ctrl client: missing Emby auth token");
                    return;
                };
                let validate_client = client.lock().unwrap().clone();
                if let Err(e) = validate_client.validate_presented_token(auth_token) {
                    log::warn!(
                        target: "daemon",
                        "rejecting ctrl client: presented Emby token validation failed: {e}"
                    );
                    return;
                }
            }
            Ok(_) => {
                log::warn!(target: "daemon", "rejecting ctrl client: missing protocol hello");
                return;
            }
            Err(e) => {
                log::warn!(target: "daemon", "rejecting ctrl client: invalid protocol hello: {e}");
                return;
            }
        }

        if let Ok(init_json) = serde_json::to_string(&CtrlEvent::State(CtrlState {
            status: player_status.lock().unwrap().clone(),
            items: shared_items.lock().unwrap().clone(),
            cursor: *shared_cursor.lock().unwrap(),
        })) {
            ev_tx.send(init_json).ok();
        }
        ctrl_clients.lock().unwrap().push(ev_tx);

        for line in lines {
            let Ok(line) = line else { break };
            if line.is_empty() {
                continue;
            }
            if let Ok(cmd) = serde_json::from_str::<CtrlCmd>(&line) {
                let _ = merged_tx.send(DaemonEvent::Ctrl(cmd));
            }
        }
    });
}

pub fn run_with_options(client: EmbyClient, audio_only: bool) -> ! {
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

    let client = Arc::new(Mutex::new(client));

    let (player_tx, player_rx) = mpsc::channel();
    let (ws_tx_chan, ws_rx) = mpsc::channel();
    // ws::start() only spawns a background reconnect-loop thread and returns
    // immediately — it does not block on the connection actually completing
    // — so it's cheap enough to keep here, ahead of Player/mpris/tray.
    let ws_send_tx = crate::ws::start(client.lock().unwrap().ws_url(), ws_tx_chan);

    // Use client-config-only subtitle/audio-lang prefs (no network call) for
    // the player's initial state, so startup never blocks on an Emby round
    // trip. If the config doesn't pin these, the live user prefs are fetched
    // from Emby in the background further down and applied to the
    // already-running player once available.
    let subtitle_prefs_from_config = {
        let client = client.lock().unwrap();
        if client.config.subtitle_mode.is_empty()
            && client.config.subtitle_lang.is_empty()
            && client.config.audio_lang.is_empty()
        {
            None
        } else {
            Some(crate::player::SubtitlePrefs {
                mode: client.config.subtitle_mode.clone(),
                subtitle_lang: client.config.subtitle_lang.clone(),
                audio_lang: client.config.audio_lang.clone(),
            })
        }
    };
    let (has_config_subtitle_prefs, subtitle_prefs) = match subtitle_prefs_from_config {
        Some(prefs) => (true, prefs),
        None => (false, crate::player::SubtitlePrefs::default()),
    };
    let client_locked = client.lock().unwrap().clone();
    let player = Player::new(
        client_locked.config.server_url.clone(),
        client_locked.token.clone(),
        client_locked.config.show_audio_window,
        client_locked.config.use_mpv_config,
        client_locked.config.no_scripts,
        client_locked.config.always_play_next,
        client_locked.config.always_skip_intro,
        subtitle_prefs,
        player_tx,
        Some(ws_send_tx.clone()),
    );

    let player_status = player.status.clone();
    let player_cmd_tx = player.cmd_tx.clone();
    crate::mpris::start(player_status, move |cmd| {
        if let Some(tx) = player_cmd_tx.lock().unwrap().as_ref() {
            let _ = tx.send(cmd);
        }
    });

    let show_systray_icon = client.lock().unwrap().config.show_systray_icon;
    let _tray = if show_systray_icon {
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

    // Bind and start the control socket only once the daemon can immediately
    // accept and speak the protocol, so local clients never connect and hang
    // waiting for the daemon hello.
    if let Some(listener) = bind_ctrl_listener() {
        let ctrl_clients = ctrl_clients.clone();
        let merged_tx2 = merged_tx.clone();
        let client2 = client.clone();
        let player_status = player.status.clone();
        let shared_items = shared_items.clone();
        let shared_cursor = shared_cursor.clone();

        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { continue };
                let Ok(stream_w) = stream.try_clone() else {
                    continue;
                };
                spawn_ctrl_client(
                    stream,
                    stream_w,
                    merged_tx2.clone(),
                    ctrl_clients.clone(),
                    client2.clone(),
                    player_status.clone(),
                    shared_items.clone(),
                    shared_cursor.clone(),
                );
            }
        });
    }

    // --- From here on: network/Emby-session-visibility setup (protocol
    // negotiation metadata, capability registration, live subtitle-prefs
    // fetch). Local control is already up and serving connections above. ---

    let mut direct_commands = Vec::new();
    let daemon_tcp_listen = client
        .lock()
        .unwrap()
        .config
        .daemon_server_tcp_listen
        .clone();
    let tcp_listener = if daemon_tcp_listen.trim().is_empty() {
        None
    } else {
        match TcpListener::bind(daemon_tcp_listen.trim()) {
            Ok(listener) => {
                let port = listener.local_addr().map(|addr| addr.port()).unwrap_or(0);
                if port > 0 {
                    direct_commands.push(mbv_direct_tcp_port_command(port));
                    log::info!(
                        target: "daemon",
                        "daemon tcp control listening on {}",
                        listener
                            .local_addr()
                            .map(|addr| addr.to_string())
                            .unwrap_or_else(|_| daemon_tcp_listen.clone())
                    );
                }
                Some(listener)
            }
            Err(e) => {
                log::warn!(
                    target: "daemon",
                    "daemon tcp control bind failed for {}: {e}",
                    daemon_tcp_listen
                );
                None
            }
        }
    };

    // Register capabilities and, if the config didn't pin subtitle/audio
    // prefs, fetch the live user prefs — both are independent Emby HTTP
    // round trips, so run them concurrently and off the startup path
    // entirely rather than blocking one on the other.
    {
        let client = client.lock().unwrap().clone();
        let direct_commands = direct_commands.clone();
        std::thread::spawn(move || {
            register_capabilities(&client, &direct_commands, audio_only);
        });
    }
    if !has_config_subtitle_prefs {
        let client = client.lock().unwrap().clone();
        let player_cmd_tx = player.cmd_tx.clone();
        std::thread::spawn(move || {
            if let Ok(prefs) = client.get_user_subtitle_prefs() {
                if let Some(tx) = player_cmd_tx.lock().unwrap().as_ref() {
                    let _ = tx.send(PlayerCommand::SetSubtitlePrefs {
                        mode: prefs.mode,
                        subtitle_lang: prefs.subtitle_lang,
                        audio_lang: prefs.audio_lang,
                    });
                }
            }
        });
    }

    if let Some(listener) = tcp_listener {
        let ctrl_clients = ctrl_clients.clone();
        let merged_tx2 = merged_tx.clone();
        let client2 = client.clone();
        let player_status = player.status.clone();
        let shared_items = shared_items.clone();
        let shared_cursor = shared_cursor.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { continue };
                let Ok(stream_w) = stream.try_clone() else {
                    continue;
                };
                spawn_ctrl_client(
                    stream,
                    stream_w,
                    merged_tx2.clone(),
                    ctrl_clients.clone(),
                    client2.clone(),
                    player_status.clone(),
                    shared_items.clone(),
                    shared_cursor.clone(),
                );
            }
        });
    }

    // Broadcast current PlayerStatus to connected TUIs so the
    // seekbar and toggle state stay in sync without sending the full queue.
    {
        let broadcast_interval =
            std::time::Duration::from_millis(client.lock().unwrap().config.daemon_broadcast_ms);
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

    let mut items: Vec<MediaItem> = Vec::new();
    let mut cursor: usize = 0;
    let mut last_keepalive = Instant::now();
    let mut last_capabilities = Instant::now();

    loop {
        if last_keepalive.elapsed() >= Duration::from_secs(30) {
            let _ = ws_send_tx.send_text("{\"MessageType\":\"KeepAlive\"}".to_string());
            last_keepalive = Instant::now();
        }
        if last_capabilities.elapsed() >= Duration::from_secs(600) {
            let client = client.lock().unwrap().clone();
            let direct_commands = direct_commands.clone();
            std::thread::spawn(move || {
                register_capabilities(&client, &direct_commands, audio_only)
            });
            last_capabilities = Instant::now();
        }

        let ev = match merged_rx.recv_timeout(Duration::from_millis(250)) {
            Ok(ev) => ev,
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                unreachable!("daemon event channel closed")
            }
        };

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
                    audio_only,
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
                    audio_only,
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
}

fn handle_ctrl(
    cmd: CtrlCmd,
    client: &Arc<Mutex<EmbyClient>>,
    player: &Player,
    audio_only: bool,
    items: &mut Vec<MediaItem>,
    cursor: &mut usize,
    shared_items: &Arc<Mutex<Vec<MediaItem>>>,
    shared_cursor: &Arc<Mutex<usize>>,
    ctrl_clients: &ClientList,
) {
    match cmd {
        CtrlCmd::Hello(_) => {
            log::warn!(target: "daemon", "unexpected ctrl protocol hello after negotiation");
        }
        CtrlCmd::PlayerCmd(pc) => match PlayerCommand::from(pc) {
            PlayerCommand::ReplacePlaylist {
                items: new_items,
                start_idx,
            } => {
                let next_cursor = if new_items.is_empty() {
                    0
                } else {
                    start_idx.min(new_items.len().saturating_sub(1))
                };
                *items = new_items.clone();
                *cursor = next_cursor;
                *shared_items.lock().unwrap() = new_items.clone();
                *shared_cursor.lock().unwrap() = next_cursor;
                broadcast(
                    ctrl_clients,
                    &CtrlEvent::State(CtrlState {
                        status: player.status.lock().unwrap().clone(),
                        items: new_items.clone(),
                        cursor: next_cursor,
                    }),
                );
                player.send_command(PlayerCommand::ReplacePlaylist {
                    items: new_items,
                    start_idx,
                });
            }
            other => {
                player.send_command(other);
            }
        },
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
            if audio_only && !all_audio(&fetched) {
                log::warn!(
                    target: "daemon",
                    "rejecting ctrl play request in audio-only mode: non-audio items present"
                );
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
    audio_only: bool,
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
            if audio_only && !all_audio(&fetched) {
                log::warn!(
                    target: "daemon",
                    "rejecting websocket play request in audio-only mode: non-audio items present"
                );
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
            player.set_paused(true);
        }
        WsEvent::Unpause => {
            player.set_paused(false);
        }
        WsEvent::NextTrack => {
            player.next();
        }
        WsEvent::PreviousTrack => {
            player.previous();
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

fn all_audio(items: &[MediaItem]) -> bool {
    items.iter().all(MediaItem::is_audio)
}

#[cfg(test)]
mod tests {
    use super::all_audio;
    use crate::api::MediaItem;

    fn item(name: &str, media_type: &str, item_type: &str) -> MediaItem {
        MediaItem {
            id: name.into(),
            name: name.into(),
            item_type: item_type.into(),
            is_folder: false,
            media_type: media_type.into(),
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

    #[test]
    fn all_audio_accepts_audio_items() {
        assert!(all_audio(&[
            item("song1", "Audio", "Audio"),
            item("song2", "Audio", "Audio"),
        ]));
    }

    #[test]
    fn all_audio_rejects_video_items() {
        assert!(!all_audio(&[
            item("song", "Audio", "Audio"),
            item("movie", "Video", "Movie"),
        ]));
    }
}
