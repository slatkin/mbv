pub(super) fn broadcast_player_event_if_not_replaced(
    ctrl_clients: &ClientRegistry,
    event: PlayerEvent,
    replacement_committed: bool,
) {
    if !replacement_committed {
        broadcast(ctrl_clients, &CtrlEvent::Player(event));
    }
}

pub(super) fn playback_run_identity_is_current(
    run_identity: (PlaybackRequestId, PlaybackGeneration),
    player: &Player,
) -> bool {
    run_identity == (0, player.status.lock().unwrap().sequence_generation)
}

fn apply_track_completed_observation(
    owner: &mut DaemonPlayerOwner,
    player: &Player,
    shared_queue: &SharedQueueState,
    run_identity: (PlaybackRequestId, PlaybackGeneration),
    slot_id: QueueSlotId,
    position_ticks: i64,
    played: bool,
    consume: bool,
    consume_videos: bool,
    consume_audio: bool,
) -> bool {
    if !playback_run_identity_is_current(run_identity, player) {
        return false;
    }
    if let Some(slot) = owner.core.queue.slot(slot_id) {
        let position = if played {
            0
        } else if position_ticks >= crate::api::MEANINGFUL_TRACK_COMPLETED_PROGRESS_TICKS
            && !slot.item.is_audio()
        {
            position_ticks
        } else {
            slot.item.playback_position_ticks()
        };
        owner.core.apply_completion_progress(slot_id, position, played);
    }
    if owner.core.consume_completed_slot(slot_id, consume, consume_videos, consume_audio) {
        log::info!(target: "consume", "TrackCompleted: consumed slot_id={slot_id:?}");
    }
    *shared_queue.observed_active_slot.lock().unwrap() = owner.core.observed_active_slot();
    true
}

pub(super) fn apply_stopped_observation(
    owner: &mut DaemonPlayerOwner,
    player: &Player,
    run_identity: (PlaybackRequestId, PlaybackGeneration),
    slot_id: Option<QueueSlotId>,
    position_ticks: i64,
    played: bool,
) -> Option<bool> {
    if !playback_run_identity_is_current(run_identity, player) {
        return None;
    }
    let Some(slot_id) = slot_id else {
        return Some(false);
    };
    let Some(slot) = owner.core.queue.slot(slot_id) else {
        return Some(false);
    };
    let position = if played {
        0
    } else if position_ticks > 0 && !slot.item.is_audio() {
        position_ticks
    } else {
        slot.item.playback_position_ticks()
    };
    owner.core.apply_completion_progress(slot_id, position, played);
    Some(true)
}

fn apply_queue_enriched(
    items: Vec<(QueueSlotId, EmbyItem)>,
    owner: &mut DaemonPlayerOwner,
    player: &Player,
    shared_queue: &SharedQueueState,
    ctrl_clients: &ClientRegistry,
) {
    let result = owner.core.queue.merge_refresh_for_slots(items);
    if !result.updated_slots.is_empty() {
        broadcast_queue_state(
            ctrl_clients,
            player,
            shared_queue,
            &owner.core.queue,
            &owner.core.source,
            &owner.core.transitions,
        );
    }
}

pub fn run_with_options(
    startup: DaemonStartupContext,
    audio_only: bool,
    hooks: DaemonRuntimeHooks,
) -> ! {
    let mut emby_runtime = startup.emby;
    let mut audiobookshelf_runtime = startup.audiobookshelf;
    let config = startup.config;
    let role = startup.role;
    std::fs::write(pid_file(), std::process::id().to_string())
        .expect("mbv daemon: failed to write PID file");

    let (shutdown_signal_tx, shutdown_signal_rx) = setup_shutdown_signal();
    let client = emby_runtime
        .as_ref()
        .map(|runtime| runtime.client.clone())
        .unwrap_or_else(|| Arc::new(Mutex::new(EmbyClient::new(config.clone()))));
    let control_credential = if role == DaemonRole::Packaged {
        None
    } else {
        Some(
            crate::config::load_or_create_control_credential()
                .expect("mbv daemon: failed to load Control credential"),
        )
    };

    let (player_tx, player_rx) = mpsc::channel();
    let (ws_tx_chan, ws_rx) = mpsc::channel();
    // ws::start() only spawns a background reconnect-loop thread and returns
    // immediately — it does not block on the connection actually completing
    // — so it's cheap enough to keep here, ahead of Player/mpris/tray.
    let mut ws_send_tx = emby_runtime
        .as_ref()
        .map(|_| crate::ws::start(client.lock().unwrap().ws_url(), ws_tx_chan));

    let mut client_locked = client.lock().unwrap().clone();
    // Daemon always runs headless — ignore user's show_audio_window setting.
    client_locked.config.show_audio_window = false;
    // always_play_next, always_skip_intro, and subtitle/audio-lang prefs are
    // controlling-client preferences, not daemon config — mbvd never reads
    // them from its own host config.toml, regardless of what's in it.
    // Clocked ALSA output is a packaged-mbvd default; the Local daemon
    // keeps its current (unforced) output regardless of `audio_device`.
    let audio_device =
        (role == DaemonRole::Packaged).then(|| client_locked.config.audio_device.clone());
    let player = Player::new(
        client_locked.config.server_url.clone(),
        client_locked.token.clone(),
        client_locked.config.show_audio_window,
        client_locked.config.use_mpv_config,
        client_locked.config.no_scripts,
        false,
        crate::player::SubtitlePrefs::default(),
        player_tx,
        ws_send_tx.clone(),
    )
    .with_video_cache(
        client_locked.config.video_cache_forward_mb,
        client_locked.config.video_cache_back_mb,
    )
    .with_audio_device(audio_device);
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
    if let Some(runtime) = &emby_runtime {
        let generation = runtime.generation;
        let tx = merged_tx.clone();
        std::thread::spawn(move || {
            for ev in ws_rx {
                let _ = tx.send(DaemonEvent::Ws {
                    generation,
                    event: ev,
                });
            }
        });
    }
    let tx = merged_tx.clone();
    std::thread::spawn(move || {
        if shutdown_signal_rx.recv().is_ok() {
            let _ = tx.send(DaemonEvent::Shutdown);
        }
    });

    // Install the owner's Audiobookshelf context on the daemon player so
    // admitted ABS slots reach `prepare_source`, and wire the player's
    // acknowledged-progress sender into the daemon event loop.
    install_daemon_audiobookshelf_context(&player, &audiobookshelf_runtime, &merged_tx);

    // Shared state for ctrl socket initial-state snapshots — stores the
    // canonical queue so all ctrl peers are seeded from one source.
    let owner_state = if role == DaemonRole::Local {
        let owner_path = crate::config::stay_alive_queue_state_path();
        crate::config::load_stay_alive_queue_state().or_else(|| {
            (!owner_path.exists()).then(crate::config::load_queue_state).flatten()
                .and_then(|queue| {
                    crate::config::legacy_queue_for_owner_if_absent(&owner_path, Some(queue))
                })
        })
    } else {
        None
    };
    let (initial_queue, initial_source, initial_lineage) = owner_state
        .map(|state| {
            let queue = PlaybackQueue::from_queue_items(
                state.queue.items,
                Some(state.queue.cursor),
            );
            (queue, state.queue.source, state.lineage)
        })
        .unwrap_or_else(|| {
            (
                PlaybackQueue::default(),
                crate::config::QueueSource::Unknown,
                crate::ctrl::QueueLineage::default(),
            )
        });
    if role == DaemonRole::Local {
        if let Err(error) = crate::config::save_stay_alive_queue_state(
            &crate::config::StayAliveQueueState {
                queue: project_queue_state(
                    &initial_queue,
                    &initial_source,
                    &player.status.lock().unwrap(),
                ),
                lineage: initial_lineage,
            },
        ) {
            log::error!(target: "queue", "failed to initialize Stay-alive queue state: {error}");
        }
        player.set_initial_queue(
            &initial_queue
                .slots()
                .iter()
                .map(|slot| slot.item.clone())
                .collect::<Vec<_>>(),
            initial_queue.active_index().unwrap_or(0),
        );
    }
    let shared_queue = SharedQueueState {
        queue: Arc::new(Mutex::new(initial_queue.clone())),
        source: Arc::new(Mutex::new(initial_source.clone())),
        lineage: Arc::new(Mutex::new(initial_lineage)),
        observed_active_slot: Arc::new(Mutex::new(None)),
    };
    let ctrl_clients: ClientRegistry = Arc::new(Mutex::new(CtrlClients::default()));

    // Bind and start the control socket only once the daemon can immediately
    // accept and speak the protocol, so local clients never connect and hang
    // waiting for the daemon hello.
    if let Some(listener) = bind_ctrl_listener() {
        let ctrl_clients = ctrl_clients.clone();
        let merged_tx2 = merged_tx.clone();
        let player_status = player.status.clone();
        let shared_queue = shared_queue.clone();
        let control_credential = control_credential.clone();

        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { continue };
                spawn_ctrl_client(
                    SocketStream::Unix(stream),
                    CtrlTransport::Local,
                    merged_tx2.clone(),
                    ctrl_clients.clone(),
                    control_credential.clone(),
                    player_status.clone(),
                    shared_queue.clone(),
                    audio_only,
                );
            }
        });
    }

    let mut direct_commands = Vec::new();

    // --- From here on: network/Emby-session-visibility setup (protocol
    // negotiation metadata, capability registration). Local control is
    // already up and serving connections above. ---

    let daemon_tcp_listen = config.daemon_server_tcp_listen.clone();
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
    if emby_runtime.is_some() {
        let client = client.lock().unwrap().clone();
        let direct_commands = direct_commands.clone();
        std::thread::spawn(move || {
            client.register_capabilities_with_options(&direct_commands, audio_only);
        });
    }

    if let Some(listener) = tcp_listener {
        let ctrl_clients = ctrl_clients.clone();
        let merged_tx2 = merged_tx.clone();
        let player_status = player.status.clone();
        let shared_queue = shared_queue.clone();
        let control_credential = control_credential.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { continue };
                spawn_ctrl_client(
                    SocketStream::Tcp(stream),
                    CtrlTransport::Tcp,
                    merged_tx2.clone(),
                    ctrl_clients.clone(),
                    control_credential.clone(),
                    player_status.clone(),
                    shared_queue.clone(),
                    audio_only,
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

    // ── Canonical queue authority — single source of truth ──────────────
    let mut owner = DaemonPlayerOwner {
        core: PlayerOwnerState::new(initial_queue, initial_source),
        ..Default::default()
    };
    let mut last_keepalive = Instant::now();
    let mut last_capabilities = Instant::now();

    loop {
        if emby_runtime.is_some() && last_keepalive.elapsed() >= Duration::from_secs(30) {
            if let Some(ws_send_tx) = &ws_send_tx {
                let _ = ws_send_tx.send_text("{\"MessageType\":\"KeepAlive\"}".to_string());
            }
            last_keepalive = Instant::now();
        }
        if emby_runtime.is_some() && last_capabilities.elapsed() >= Duration::from_secs(600) {
            let client = client.lock().unwrap().clone();
            let direct_commands = direct_commands.clone();
            std::thread::spawn(move || {
                client.register_capabilities_with_options(&direct_commands, audio_only)
            });
            last_capabilities = Instant::now();
        }

        let ev = match merged_rx.recv_timeout(Duration::from_millis(25)) {
            Ok(ev) => ev,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                cancel_pending_idle_queue_load_if_run_changed(&mut owner, &player);
                expire_pending_idle_queue_load(&mut owner, Instant::now());
                if let Some((connection_id, event)) = owner.intents.settle_buffering_if_due() {
                    log::info!(target: "pipe_latency", "request={} generation={} outcome=settled", event.request_id, event.generation);
                    let clients = ctrl_clients.lock().unwrap();
                    if clients.has_client(connection_id) {
                        clients.send_to_client(connection_id, &CtrlEvent::PlaybackIntent(event));
                    } else {
                        drop(clients);
                        owner.intents.invalidate_connection(connection_id);
                    }
                }
                expire_and_redispatch(&mut owner, &player, &ctrl_clients, &shared_queue);
                continue;
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                unreachable!("daemon event channel closed")
            }
        };

        // Set by any arm below that mutated the owner's canonical queue;
        // persisted once after the match instead of inline per mutation site.
        let mut owner_queue_dirty = false;

        match ev {
            DaemonEvent::Player(PlayerEvent::TrackChanged { slot_id, transition }) => {
                // Resolve the reported slot against the canonical queue. A
                // report naming a slot the daemon no longer holds carries no
                // evidence about which surviving slot was intended, so it is
                // discarded and logged without touching canonical queue or
                // observed slot (design D6).
                let Some((observed_idx, resolved_slot_id)) =
                    owner.core.observe_track_change(slot_id)
                else {
                    log::warn!(
                        target: "queue",
                        "discarding TrackChanged for unknown slot {slot_id:?}"
                    );
                    continue;
                };
                broadcast(
                    &ctrl_clients,
                    &CtrlEvent::Player(PlayerEvent::TrackChanged {
                        slot_id: resolved_slot_id,
                        transition,
                    }),
                );
                // Settle the desired transition before publishing so the
                // snapshot contains every owner change from this turn.
                if let Some((observed_request_id, _)) = transition {
                    settle_and_redispatch(
                        &mut owner,
                        &player,
                        observed_request_id,
                        resolved_slot_id,
                    );
                }
                *shared_queue.observed_active_slot.lock().unwrap() =
                    owner.core.observed_active_slot();
                broadcast_queue_state(
                    &ctrl_clients,
                    &player,
                    &shared_queue,
                    &owner.core.queue,
                    &owner.core.source,
                    &owner.core.transitions,
                );
                // Settle playback intent if the reported slot matches.
                if let Some((connection_id, request_id, generation)) = owner.intents
                    .current
                    .as_ref()
                    .filter(|current| match &current.action {
                        PlaybackIntentAction::Play { item_ids, .. } => owner.core.queue
                            .slots()
                            .get(observed_idx)
                            .is_some_and(|slot| item_ids.iter().any(|id| id == slot.item.id())),
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
                    if let Some(event) =
                        owner.intents.applied_if_current(connection_id, request_id, generation)
                    {
                        ctrl_clients
                            .lock()
                            .unwrap()
                            .send_to_client(connection_id, &CtrlEvent::PlaybackIntent(event));
                    }
                }
            }
            DaemonEvent::Player(PlayerEvent::NextUpThreshold {
                series_id,
                season,
                episode,
            }) => {
                let active_idx = owner.core.queue.active_index().unwrap_or(0);
                if let Some(slot) = owner.core.queue.slots().get(active_idx + 1) {
                    if let Some(emby) = slot.item.as_emby() {
                        player.send_command(PlayerCommand::NextUpShow {
                            item_id: emby.id.clone(),
                            show_title: emby.series_name.clone(),
                            ep_title: emby.name.clone(),
                            artist: emby.artist.clone(),
                        });
                    }
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
                if let Some(slot) = owner.core.queue.slots().get(next_idx) {
                    if let Some(emby) = slot.item.as_emby() {
                        player.send_command(PlayerCommand::NextUpShow {
                            item_id: emby.id.clone(),
                            show_title: emby.series_name.clone(),
                            ep_title: emby.name.clone(),
                            artist: emby.artist.clone(),
                        });
                    }
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
                    owner.intents.output_started_if_current(delay)
                {
                    log::info!(target: "pipe_latency", "request={} generation={} phase={:?} elapsed_ms={}", status.request_id, status.generation, status.phase, owner.intents.current.as_ref().map(|current| current.accepted_at.elapsed().as_millis()).unwrap_or_default());
                    ctrl_clients
                        .lock()
                        .unwrap()
                        .send_to_client(connection_id, &CtrlEvent::PipePlaybackStatus(status));
                    if delay.is_none() {
                        if let Some(current) = owner.intents.current.as_ref() {
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
                // A cold-started queue plays its first track with no
                // track-to-track transition, so clients never get a snapshot
                // reflecting `status.active` and the started slot. Push one
                // here so their now-playing highlight lands on the right row.
                broadcast_queue_state(
                    &ctrl_clients,
                    &player,
                    &shared_queue,
                    &owner.core.queue,
                    &owner.core.source,
                    &owner.core.transitions,
                );
            }
            DaemonEvent::Player(pe @ PlayerEvent::TrackCompleted {
                slot_id,
                run_identity,
                position_ticks,
                played,
                consume,
                ..
            }) => {
                let (consume_videos, consume_audio) = {
                    let cfg = client.lock().unwrap();
                    (cfg.config.consume_videos, cfg.config.consume_audio)
                };
                if !apply_track_completed_observation(
                    &mut owner,
                    &player,
                    &shared_queue,
                    run_identity,
                    slot_id,
                    position_ticks,
                    played,
                    consume,
                    consume_videos,
                    consume_audio,
                ) {
                    continue;
                }
                broadcast_queue_state(
                    &ctrl_clients,
                    &player,
                    &shared_queue,
                    &owner.core.queue,
                    &owner.core.source,
                    &owner.core.transitions,
                );
                broadcast(&ctrl_clients, &CtrlEvent::Player(pe));
                owner_queue_dirty = true;
            }
            DaemonEvent::Player(pe) => {
                if let PlayerEvent::Stopped { run_identity, .. } = &pe {
                    if owner.pending_idle_load.as_ref().is_some_and(|pending| {
                        pending.stopped_run != *run_identity
                    }) {
                        cancel_pending_idle_queue_load(
                            &mut owner,
                            "playback stopped for a different run during queue load",
                        );
                    }
                }
                let pending_idle_load_matches = match &pe {
                    PlayerEvent::Stopped { run_identity, .. } => owner
                        .pending_idle_load
                        .as_ref()
                        .is_some_and(|pending| pending.stopped_run == *run_identity),
                    _ => false,
                };
                let stopped_queue_updated = if let PlayerEvent::Stopped {
                    slot_id,
                    run_identity,
                    position_ticks,
                    played,
                    ..
                } = &pe
                {
                    let Some(updated) = apply_stopped_observation(
                        &mut owner,
                        &player,
                        *run_identity,
                        *slot_id,
                        *position_ticks,
                        *played,
                    ) else {
                        continue;
                    };
                    updated
                } else {
                    false
                };
                let replacement_committed = if pending_idle_load_matches {
                    let failure = match &pe {
                        PlayerEvent::Stopped { error, .. } => error.clone(),
                        _ => None,
                    };
                    complete_pending_idle_queue_load(
                        match &pe {
                            PlayerEvent::Stopped { run_identity, .. } => *run_identity,
                            _ => unreachable!(),
                        },
                        failure.clone(),
                        &mut owner,
                        &player,
                        &shared_queue,
                        &ctrl_clients,
                    ) && failure.is_none()
                } else {
                    false
                };
                if stopped_queue_updated && !replacement_committed {
                    // Unlike TrackCompleted (which broadcasts unconditionally
                    // below via the raw player event too), a full Stopped has
                    // no other broadcast carrying the corrected queue. The
                    // successful pending-load commit publishes the new stopped
                    // queue once instead of first publishing this old queue.
                    broadcast_queue_state(
                        &ctrl_clients,
                        &player,
                        &shared_queue,
                        &owner.core.queue,
                        &owner.core.source,
                        &owner.core.transitions,
                    );
                }
                if let PlayerEvent::PausedChanged(paused) = &pe {
                    if let Some((connection_id, request_id, generation)) = owner.intents
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
                        if let Some(event) = owner.intents.applied_if_current(
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
                    if let Some((connection_id, request_id, generation)) = owner.intents
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
                        if let Some(event) = owner.intents.applied_if_current(
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
                broadcast_player_event_if_not_replaced(&ctrl_clients, pe, replacement_committed);
                if stopped_queue_updated || replacement_committed {
                    owner_queue_dirty = true;
                }
            }
            DaemonEvent::Ws { generation, event } => {
                if emby_runtime
                    .as_ref()
                    .is_some_and(|runtime| runtime.generation == generation)
                {
                    handle_ws(
                        event,
                        Some(&client),
                        &player,
                        audio_only,
                        &mut owner.core.queue,
                        &mut owner.core.source,
                        &mut owner.core.transitions,
                        &shared_queue,
                        &ctrl_clients,
                    );
                    owner_queue_dirty = true;
                }
            }
            DaemonEvent::QueueEnriched(items) => {
                apply_queue_enriched(items, &mut owner, &player, &shared_queue, &ctrl_clients);
            }
            DaemonEvent::AudiobookshelfProgress(update) => {
                apply_audiobookshelf_progress(
                    update,
                    audiobookshelf_runtime
                        .as_ref()
                        .map(|runtime| runtime.generation),
                    &mut owner.core.queue,
                    &ctrl_clients,
                );
            }
            DaemonEvent::AudiobookshelfBookProgress(update) => {
                apply_audiobookshelf_book_progress(
                    update,
                    audiobookshelf_runtime
                        .as_ref()
                        .map(|runtime| runtime.generation),
                    &mut owner.core.queue,
                    &ctrl_clients,
                );
            }
            DaemonEvent::Ctrl(cmd, client_id, reply_tx) => {
                if !ctrl_clients.lock().unwrap().has_client(client_id) {
                    continue;
                }
                if let CtrlCmd::ApplyServiceSetup { kind, revision } = cmd {
                    let transport = ctrl_clients.lock().unwrap().transport(client_id);
                    let allowed = owner_admin_transport_allowed(role, kind, transport);
                    let result = if !allowed {
                        Err(crate::ctrl::ServiceSetupRejection::TransitionRejected)
                    } else {
                        match kind {
                            crate::config::ServiceKind::Emby => reconcile_packaged_emby(
                                revision,
                                &mut emby_runtime,
                                &mut ws_send_tx,
                                &client,
                                &player,
                                &mut owner.core.queue,
                                &mut owner.core.source,
                                &mut owner.core.transitions,
                                &shared_queue,
                                &ctrl_clients,
                                &merged_tx,
                                &direct_commands,
                                audio_only,
                            ),
                            crate::config::ServiceKind::Audiobookshelf => {
                                reconcile_packaged_audiobookshelf(
                                    revision,
                                    &mut audiobookshelf_runtime,
                                    &player,
                                    &mut owner.core.queue,
                                    &mut owner.core.source,
                                    &mut owner.core.transitions,
                                    &shared_queue,
                                    &ctrl_clients,
                                    &client,
                                )
                            }
                        }
                    };
                    match result {
                        Ok(()) => send_to(
                            &reply_tx,
                            &CtrlEvent::ServiceSetupApplied { kind, revision },
                        ),
                        Err(reason) => send_to(
                            &reply_tx,
                            &CtrlEvent::ServiceSetupRejected {
                                kind,
                                revision,
                                reason,
                            },
                        ),
                    }
                    if result.is_ok() && kind == crate::config::ServiceKind::Audiobookshelf {
                        install_daemon_audiobookshelf_context(
                            &player,
                            &audiobookshelf_runtime,
                            &merged_tx,
                        );
                    }
                    continue;
                }
                let persist_after_command = matches!(
                    &cmd,
                    CtrlCmd::UnifiedQueueLoadIdle { .. }
                        | CtrlCmd::UnifiedQueueSourceUpdate { .. }
                        | CtrlCmd::UnifiedQueueReplace { .. }
                        | CtrlCmd::UnifiedQueueAppend { .. }
                        | CtrlCmd::UnifiedQueueRemoveSlot { .. }
                        | CtrlCmd::UnifiedQueueRemoveSlots { .. }
                        | CtrlCmd::UnifiedQueueMoveSlot { .. }
                        | CtrlCmd::UnifiedQueueClear
                );
                handle_ctrl_for_role(
                    cmd,
                    client_id,
                    CtrlRequest {
                        reply_tx: &reply_tx,
                    },
                    &client,
                    &player,
                    audio_only,
                    &mut owner,
                    &shared_queue,
                    &ctrl_clients,
                    audiobookshelf_runtime.is_some(),
                    &merged_tx,
                    config.stay_alive,
                    role,
                );
                if persist_after_command && owner.pending_idle_load.is_none() {
                    owner_queue_dirty = true;
                }
            }
            DaemonEvent::PlaybackResolved {
                start_idx,
                start_ticks,
                source: new_source,
                client_id,
                request_id,
                generation,
                fetched,
            } => {
                if !ctrl_clients.lock().unwrap().has_client(client_id) {
                    owner.intents.invalidate_connection(client_id);
                    continue;
                }
                if !owner.intents.is_current(client_id, request_id, generation) {
                    continue;
                }
                if let Err(error) = &fetched {
                    if let Some(event) = owner.intents.rejected_if_current(
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
                    } else if audio_only_rejection(
                        audio_only,
                        &items_for_intent
                            .iter()
                            .cloned()
                            .map(|e| QueueItem::Emby(Box::new(e)))
                            .collect::<Vec<_>>(),
                    )
                    .is_some()
                    {
                        Some(crate::ctrl::PlaybackIntentRejection::AudioOnly)
                    } else {
                        None
                    };
                    if let Some(reason) = rejection {
                        if let Some(event) = owner.intents
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
                owner.intents.mark_starting(request_id);
                if let Some(status) = owner.intents.pipe_status() {
                    log::info!(target: "pipe_latency", "request={} generation={} phase={:?} elapsed_ms={}", status.request_id, status.generation, status.phase, owner.intents.current.as_ref().map(|current| current.accepted_at.elapsed().as_millis()).unwrap_or_default());
                    ctrl_clients
                        .lock()
                        .unwrap()
                        .send_to_client(client_id, &CtrlEvent::PipePlaybackStatus(status));
                }
                if let Ok(fetched_items) = fetched {
                    // A resolved Play replaces the queue: it deliberately
                    // interrupts any in-flight or queued slot jump.
                    reset_slot_jumps(
                        &mut owner.core.transitions,
                        &mut owner.queued_transition_origin,
                    );
                    play_resolved_items(
                        fetched_items,
                        start_idx,
                        start_ticks,
                        new_source,
                        &client,
                        &player,
                        &mut owner.core.queue,
                        &mut owner.core.source,
                        &shared_queue,
                        &ctrl_clients,
                        &owner.core.transitions,
                    );
                    owner_queue_dirty = true;
                }
            }
            DaemonEvent::CtrlDisconnected(client_id) => {
                ctrl_clients.lock().unwrap().remove(client_id);
                owner.intents.invalidate_connection(client_id);
            }
            DaemonEvent::Shutdown => {
                log::info!(target: "daemon", "graceful shutdown: stopping player");
                if role == DaemonRole::Local {
                    if let Err(error) = persist_stay_alive_owner_queue(&owner, &player, &shared_queue) {
                        log::error!(target: "queue", "failed to persist Stay-alive queue on shutdown: {error}");
                    }
                }
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

        if role == DaemonRole::Local && owner_queue_dirty {
            if let Err(error) = persist_stay_alive_owner_queue(&owner, &player, &shared_queue) {
                log::error!(target: "queue", "failed to persist Stay-alive queue: {error}");
            }
        }
    }
}
