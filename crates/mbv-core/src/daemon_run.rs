pub fn run_with_options(client: EmbyClient, audio_only: bool, hooks: DaemonRuntimeHooks) -> ! {
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
    let mut client_locked = client.lock().unwrap().clone();
    // Daemon always runs headless — ignore user's show_audio_window setting.
    client_locked.config.show_audio_window = false;
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
    (hooks.on_player_ready)(DaemonPlayerHandle {
        status: player_status,
        command_tx: player_cmd_tx,
    });

    let _tray = (hooks.on_tray_ready)(shutdown_signal_tx.clone());

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
    let shared_queue = SharedQueueState {
        items: Arc::new(Mutex::new(Vec::new())),
        cursor: Arc::new(Mutex::new(0)),
        source: Arc::new(Mutex::new(crate::config::QueueSource::Unknown)),
    };
    let ctrl_clients: ClientRegistry = Arc::new(Mutex::new(CtrlClients::default()));

    // Bind and start the control socket only once the daemon can immediately
    // accept and speak the protocol, so local clients never connect and hang
    // waiting for the daemon hello.
    if let Some(listener) = bind_ctrl_listener() {
        let ctrl_clients = ctrl_clients.clone();
        let merged_tx2 = merged_tx.clone();
        let client2 = client.clone();
        let player_status = player.status.clone();
        let shared_queue = shared_queue.clone();

        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { continue };
                spawn_ctrl_client(
                    stream,
                    merged_tx2.clone(),
                    ctrl_clients.clone(),
                    client2.clone(),
                    player_status.clone(),
                    shared_queue.clone(),
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
        let shared_queue = shared_queue.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { continue };
                spawn_ctrl_client(
                    stream,
                    merged_tx2.clone(),
                    ctrl_clients.clone(),
                    client2.clone(),
                    player_status.clone(),
                    shared_queue.clone(),
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
            if !ctrl_clients.lock().unwrap().has_driver() {
                continue;
            }
            let status = player_status.lock().unwrap().clone();
            broadcast(&ctrl_clients, &CtrlEvent::StatusOnly(status));
        });
    }

    let mut items: Vec<MediaItem> = Vec::new();
    let mut cursor: usize = 0;
    let mut source = crate::config::QueueSource::Unknown;
    let mut last_keepalive = Instant::now();
    let mut last_capabilities = Instant::now();
    let mut spectrum_state: Option<SpectrumState> = None;

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
                *shared_queue.cursor.lock().unwrap() = idx;
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
            DaemonEvent::Player(PlayerEvent::QueueNextUp { next_idx }) => {
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
                    &CtrlEvent::Player(PlayerEvent::QueueNextUp { next_idx }),
                );
            }
            DaemonEvent::Player(pe) => {
                if let PlayerEvent::Stopped { .. } = &pe {
                    if let Some(mut state) = spectrum_state.take() {
                        log::info!(target: "daemon", "auto-stopping spectrum on playback stop");
                        broadcast(
                            &ctrl_clients,
                            &CtrlEvent::SpectrumFailed {
                                reason: "playback stopped".to_string(),
                            },
                        );
                        state.stop();
                    }
                }
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
                    &mut source,
                    &shared_queue,
                    &ctrl_clients,
                );
            }
            DaemonEvent::Ctrl(cmd, client_id, reply_tx) => {
                if !ctrl_clients.lock().unwrap().has_client(client_id) {
                    continue;
                }
                handle_ctrl(
                    cmd,
                    CtrlRequest {
                        reply_tx: &reply_tx,
                    },
                    &client,
                    &player,
                    audio_only,
                    &mut items,
                    &mut cursor,
                    &mut source,
                    &shared_queue,
                    &ctrl_clients,
                    &merged_tx,
                    &mut spectrum_state,
                );
            }
            // Phase 1: stop spectrum on any client disconnect (preserves
            // pre-multi-connection behavior). Phase 2 should track which
            // client started spectrum and stop only when that client
            // disconnects.
            DaemonEvent::CtrlDisconnected(client_id) => {
                if let Some(mut state) = spectrum_state.take() {
                    log::info!(target: "daemon", "stopping spectrum on client disconnect");
                    state.stop();
                }
                ctrl_clients.lock().unwrap().remove(client_id);
            }
            DaemonEvent::Spectrum(bars) => {
                broadcast(&ctrl_clients, &CtrlEvent::Spectrum { bars });
            }
            DaemonEvent::SpectrumFailed { reason } => {
                broadcast(&ctrl_clients, &CtrlEvent::SpectrumFailed { reason });
                if let Some(mut state) = spectrum_state.take() {
                    state.stop();
                }
            }
            DaemonEvent::Shutdown => {
                log::info!(target: "daemon", "graceful shutdown: stopping player");
                if let Some(mut state) = spectrum_state.take() {
                    state.stop();
                }
                player.stop();
                player.join_or_timeout(std::time::Duration::from_secs(5));
                let _ = std::fs::remove_file(pid_file());
                std::process::exit(0);
            }
        }
    }
}
