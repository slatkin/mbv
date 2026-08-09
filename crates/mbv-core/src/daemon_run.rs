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

    let mut client_locked = client.lock().unwrap().clone();
    // Daemon always runs headless — ignore user's show_audio_window setting.
    client_locked.config.show_audio_window = false;
    // always_play_next, always_skip_intro, and subtitle/audio-lang prefs are
    // controlling-client preferences, not daemon config — mbvd never reads
    // them from its own host config.toml, regardless of what's in it.
    let player = Player::new(
        client_locked.config.server_url.clone(),
        client_locked.token.clone(),
        client_locked.config.show_audio_window,
        client_locked.config.use_mpv_config,
        client_locked.config.no_scripts,
        false,
        false,
        crate::player::SubtitlePrefs::default(),
        player_tx,
        Some(ws_send_tx.clone()),
    );

    player.pre_warm(
        client_locked.config.audio_pipe_target(),
        client_locked.config.audio_pipe_samplerate,
        client_locked.config.audio_pipe_bitdepth,
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
        feed_items: Arc::new(Mutex::new(Vec::new())),
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
                    CtrlTransport::Local,
                    merged_tx2.clone(),
                    ctrl_clients.clone(),
                    client2.clone(),
                    player_status.clone(),
                    shared_queue.clone(),
                );
            }
        });
    }

    let mut direct_commands = Vec::new();

    // Shared-data hosting is optional and starts only after the playback and
    // local ctrl listener are operational. A database failure disables this
    // feature without affecting daemon playback.
    {
        let shared_config = client.lock().unwrap().config.clone();
        if shared_config.shared_data_enabled {
            match crate::shared_store::open_shared_db() {
                Ok(db) => {
                    let store =
                        crate::shared_worker::spawn_shared_store_worker(Arc::new(Mutex::new(db)));
                    let shared_port = crate::shared_service::start_shared_service(
                        client.clone(),
                        store,
                        &shared_config,
                    );
                    if let Some(port) = shared_port {
                        if port > 0 {
                            direct_commands
                                .push(crate::api::mbv_shared_data_tcp_port_command(port));
                        }
                        log::info!(target: "shared_data", "shared-data hosting enabled");
                    } else {
                        log::warn!(target: "shared_data", "shared-data hosting unavailable; playback remains operational");
                    }
                }
                Err(error) => {
                    log::error!(
                        target: "shared_data",
                        "shared-data database unavailable; playback remains operational: {error}"
                    );
                }
            }
        }
    }

    // --- From here on: network/Emby-session-visibility setup (protocol
    // negotiation metadata, capability registration). Local control is
    // already up and serving connections above. ---

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

    // Register capabilities off the startup path so it doesn't block on the
    // Emby HTTP round trip.
    {
        let client = client.lock().unwrap().clone();
        let direct_commands = direct_commands.clone();
        std::thread::spawn(move || {
            register_capabilities(&client, &direct_commands, audio_only);
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
                    CtrlTransport::Tcp,
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

    let mut items: Vec<EmbyItem> = Vec::new();
    let mut cursor: usize = 0;
    let mut source = crate::config::QueueSource::Unknown;
    let mut feed_items: Vec<FeedEntry> = Vec::new();
    let mut playback_intents = PlaybackIntentState::default();
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

        let ev = match merged_rx.recv_timeout(Duration::from_millis(25)) {
            Ok(ev) => ev,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if let Some((connection_id, event)) = playback_intents.settle_buffering_if_due() {
                    log::info!(target: "pipe_latency", "request={} generation={} outcome=settled", event.request_id, event.generation);
                    let clients = ctrl_clients.lock().unwrap();
                    if clients.has_client(connection_id) {
                        clients.send_to_client(connection_id, &CtrlEvent::PlaybackIntent(event));
                    } else {
                        drop(clients);
                        playback_intents.invalidate_connection(connection_id);
                    }
                }
                continue;
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                unreachable!("daemon event channel closed")
            }
        };

        match ev {
            DaemonEvent::Player(PlayerEvent::TrackChanged(idx)) => {
                // The player's internal queue may lag behind the daemon's
                // items list when a QueueRemove / QueueMove was sent down
                // the player command channel but hasn't been processed yet.
                // Clamp the reported index to the current items length so
                // the cursor can never point past the validated list.
                cursor = if items.is_empty() {
                    0
                } else {
                    idx.min(items.len() - 1)
                };
                *shared_queue.cursor.lock().unwrap() = cursor;
                broadcast(
                    &ctrl_clients,
                    &CtrlEvent::Player(PlayerEvent::TrackChanged(cursor)),
                );
                // Broadcast full state so CtrlState reflects the
                // authoritative playback position. Without this, a
                // CtrlState from a daemon-side list mutation processed
                // before the player thread acted on it can carry a stale
                // cursor computed from index arithmetic, and the
                // TrackChanged that fires between them refers to the old
                // pre-mutation index.
                // Gated per-client the same way as `broadcast_queue_state`
                // (#5.1): legacy peers must never receive the Feed tail.
                let status = player.status.lock().unwrap().clone();
                let capable_json = serialize_ctrl_event(&CtrlEvent::State(CtrlState {
                    status: status.clone(),
                    items: items.clone(),
                    cursor,
                    source: source.clone(),
                    feed_items: feed_items.clone(),
                }));
                let legacy_json = serialize_ctrl_event(&CtrlEvent::State(CtrlState {
                    status,
                    items: items.clone(),
                    cursor,
                    source: source.clone(),
                    feed_items: Vec::new(),
                }));
                if let (Some(capable_json), Some(legacy_json)) = (capable_json, legacy_json) {
                    ctrl_clients
                        .lock()
                        .unwrap()
                        .broadcast_state_gated(capable_json, legacy_json);
                }
                *shared_queue.items.lock().unwrap() = items.clone();
                *shared_queue.source.lock().unwrap() = source.clone();
                *shared_queue.feed_items.lock().unwrap() = feed_items.clone();
                if let Some((connection_id, request_id, generation)) = playback_intents
                    .current
                    .as_ref()
                    .filter(|current| match &current.action {
                        PlaybackIntentAction::Play { item_ids, .. } => items
                            .get(cursor)
                            .is_some_and(|item| item_ids.iter().any(|id| id == &item.id)),
                        PlaybackIntentAction::Next | PlaybackIntentAction::Previous => {
                            current.target_idx.is_some_and(|target| target == cursor)
                        }
                        _ => false,
                    })
                    .map(|current| {
                        (
                            current.connection_id,
                            current.request_id,
                            current.generation,
                        )
                    })
                {
                    if items.get(cursor).is_some() {
                        if let Some(event) = playback_intents.applied_if_current(
                            connection_id,
                            request_id,
                            generation,
                        ) {
                            ctrl_clients
                                .lock()
                                .unwrap()
                                .send_to_client(connection_id, &CtrlEvent::PlaybackIntent(event));
                        }
                    }
                }
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
            DaemonEvent::Player(PlayerEvent::OutputStarted) => {
                let delay = client
                    .lock()
                    .unwrap()
                    .config
                    .audio_pipe_playout_delay_ms
                    .map(Duration::from_millis);
                if let Some((connection_id, status)) =
                    playback_intents.output_started_if_current(delay)
                {
                    log::info!(target: "pipe_latency", "request={} generation={} phase={:?} elapsed_ms={}", status.request_id, status.generation, status.phase, playback_intents.current.as_ref().map(|current| current.accepted_at.elapsed().as_millis()).unwrap_or_default());
                    ctrl_clients
                        .lock()
                        .unwrap()
                        .send_to_client(connection_id, &CtrlEvent::PipePlaybackStatus(status));
                    if delay.is_none() {
                        if let Some(current) = playback_intents.current.as_ref() {
                            ctrl_clients.lock().unwrap().send_to_client(
                                current.connection_id,
                                &CtrlEvent::PlaybackIntent(PlaybackIntentEvent {
                                    request_id: current.request_id,
                                    generation: current.generation,
                                    outcome: PlaybackIntentOutcome::Applied,
                                }),
                            );
                        }
                    }
                }
                broadcast(
                    &ctrl_clients,
                    &CtrlEvent::Player(PlayerEvent::OutputStarted),
                );
            }
            DaemonEvent::Player(PlayerEvent::FeedConsumed { guid }) => {
                handle_feed_consumed(
                    &guid,
                    &ctrl_clients,
                    &player,
                    &shared_queue,
                    &items,
                    cursor,
                    &source,
                    &mut feed_items,
                );
            }
            DaemonEvent::Player(pe) => {
                if let PlayerEvent::PausedChanged(paused) = &pe {
                    if let Some((connection_id, request_id, generation)) = playback_intents
                        .current
                        .as_ref()
                        .and_then(|current| match &current.action {
                            PlaybackIntentAction::SetPaused { paused: desired }
                                if desired == paused =>
                            {
                                Some((
                                    current.connection_id,
                                    current.request_id,
                                    current.generation,
                                ))
                            }
                            _ => None,
                        })
                    {
                        if let Some(event) = playback_intents.applied_if_current(
                            connection_id,
                            request_id,
                            generation,
                        ) {
                            ctrl_clients
                                .lock()
                                .unwrap()
                                .send_to_client(connection_id, &CtrlEvent::PlaybackIntent(event));
                        }
                    }
                }
                if matches!(pe, PlayerEvent::Stopped { .. }) {
                    if let Some((connection_id, request_id, generation)) = playback_intents
                        .current
                        .as_ref()
                        .filter(|current| matches!(current.action, PlaybackIntentAction::Stop))
                        .map(|current| {
                            (
                                current.connection_id,
                                current.request_id,
                                current.generation,
                            )
                        })
                    {
                        if let Some(event) = playback_intents.applied_if_current(
                            connection_id,
                            request_id,
                            generation,
                        ) {
                            ctrl_clients
                                .lock()
                                .unwrap()
                                .send_to_client(connection_id, &CtrlEvent::PlaybackIntent(event));
                        }
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
                    client_id,
                    CtrlRequest {
                        reply_tx: &reply_tx,
                    },
                    &client,
                    &player,
                    audio_only,
                    &mut items,
                    &mut cursor,
                    &mut source,
                    &mut feed_items,
                    &shared_queue,
                    &ctrl_clients,
                    &mut playback_intents,
                    None,
                    &merged_tx,
                );
            }
            DaemonEvent::PlaybackResolved {
                command,
                client_id,
                reply_tx,
                request_id,
                generation,
                fetched,
            } => {
                if !ctrl_clients.lock().unwrap().has_client(client_id) {
                    playback_intents.invalidate_connection(client_id);
                    continue;
                }
                if !playback_intents.is_current(client_id, request_id, generation) {
                    continue;
                }
                if let Err(error) = &fetched {
                    if let Some(event) = playback_intents.rejected_if_current(
                        client_id,
                        request_id,
                        generation,
                        crate::ctrl::PlaybackIntentRejection::ResolutionFailed,
                    ) {
                        ctrl_clients
                            .lock()
                            .unwrap()
                            .send_to_client(client_id, &CtrlEvent::PlaybackIntent(event));
                    }
                    log::warn!(target: "daemon", "ctrl play resolution failed: {error}");
                    continue;
                }
                if let Ok(items_for_intent) = &fetched {
                    let rejection = if items_for_intent.is_empty() {
                        Some(crate::ctrl::PlaybackIntentRejection::EmptyTarget)
                    } else if audio_only_rejection(audio_only, items_for_intent).is_some() {
                        Some(crate::ctrl::PlaybackIntentRejection::AudioOnly)
                    } else {
                        None
                    };
                    if let Some(reason) = rejection {
                        if let Some(event) = playback_intents
                            .rejected_if_current(client_id, request_id, generation, reason)
                        {
                            ctrl_clients
                                .lock()
                                .unwrap()
                                .send_to_client(client_id, &CtrlEvent::PlaybackIntent(event));
                        }
                        continue;
                    }
                }
                playback_intents.mark_starting(request_id);
                if let Some(status) = playback_intents.pipe_status() {
                    log::info!(target: "pipe_latency", "request={} generation={} phase={:?} elapsed_ms={}", status.request_id, status.generation, status.phase, playback_intents.current.as_ref().map(|current| current.accepted_at.elapsed().as_millis()).unwrap_or_default());
                    ctrl_clients
                        .lock()
                        .unwrap()
                        .send_to_client(client_id, &CtrlEvent::PipePlaybackStatus(status));
                }
                handle_ctrl(
                    command,
                    client_id,
                    CtrlRequest {
                        reply_tx: &reply_tx,
                    },
                    &client,
                    &player,
                    audio_only,
                    &mut items,
                    &mut cursor,
                    &mut source,
                    &mut feed_items,
                    &shared_queue,
                    &ctrl_clients,
                    &mut playback_intents,
                    Some(fetched),
                    &merged_tx,
                );
            }
            DaemonEvent::CtrlDisconnected(client_id) => {
                ctrl_clients.lock().unwrap().remove(client_id);
                playback_intents.invalidate_connection(client_id);
            }
            DaemonEvent::Shutdown => {
                log::info!(target: "daemon", "graceful shutdown: stopping player");
                // Announce the deliberate shutdown to every connected client
                // before closing their connections, so they exit cleanly
                // instead of treating this as an unannounced crash.
                ctrl_clients
                    .lock()
                    .unwrap()
                    .notify_disconnected_all(DisconnectReason::DaemonShutdown);
                ctrl_clients
                    .lock()
                    .unwrap()
                    .flush_writers(std::time::Duration::from_secs(1));
                player.stop();
                player.join_or_timeout(std::time::Duration::from_secs(5));
                let _ = std::fs::remove_file(pid_file());
                std::process::exit(0);
            }
        }
    }
}
