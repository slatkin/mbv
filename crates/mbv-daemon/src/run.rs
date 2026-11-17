use super::core::{
    DaemonEvent, NamedQueueListSeed, QueuePersistenceRequest, bind_ctrl_listener, broadcast,
};
use super::{
    AudiobookshelfOwnerContext, CtrlTransport, DaemonLoop, DaemonPlayerHandle, DaemonPlayerOwner,
    DaemonRole, DaemonRuntimeHooks, DaemonStartupContext, EmbyOwnerContext, LocalOutputHooks,
    LoopFlow, SharedQueueState, TrayState, broadcast_queue_state,
    install_daemon_audiobookshelf_context, pid_file, project_queue_state, setup_shutdown_signal,
    spawn_ctrl_client, start_queue_enrichment,
};
use crate::LocalStartupError;
use crate::PinSwapState;
use crate::owner_lock::{OwnerLockError, PidFileLock, lock_pid_file};
use crate::{ClientRegistry, CtrlClients};
use mbv_ctrl::{AttachedClientsCell, CtrlEvent};
use mbv_emby::{EmbyClient, mbv_direct_tcp_port_command};
use mbv_emby_model::EmbyItem;
use mbv_net::stream::SocketStream;
use mbv_player::{Player, PlayerOwnerState};
use mbv_queue::QueueSlotId;
use std::convert::Infallible;
use std::net::TcpListener;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

mod observations;
pub(super) use observations::{
    ConsumePolicy, apply_stopped_observation, apply_track_completed_observation,
    record_completed_slot,
};
mod restore;
#[cfg(test)]
pub(crate) use restore::{StartupQueues, restored_state_at};

pub(super) fn apply_queue_enriched(
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
            None,
        );
    }
}

struct DaemonStarted {
    config: mbv_config::Config,
    role: DaemonRole,
    emby_runtime: Option<EmbyOwnerContext>,
    audiobookshelf_runtime: Option<AudiobookshelfOwnerContext>,
    client: Arc<Mutex<EmbyClient>>,
    control_credential: Option<String>,
    player: Player,
    merged_tx: mpsc::Sender<DaemonEvent>,
    merged_rx: mpsc::Receiver<DaemonEvent>,
    ws_send_tx: Option<mbv_ws::WsSender>,
    owner_settings: crate::OwnerSettingsReader,
    tray: TrayState,
    /// Signal-truth PID record lock, held for the daemon's whole run
    /// (design D2). `None` for the Local role, which owns no `mbv.pid`.
    pid_lock: Option<PidFileLock>,
    pin_swap: PinSwapState,
    /// Starts the pinned-panel toggle (tray-panel-toggle design D3).
    toggle_panel: crate::TogglePanelHook,
    /// Shared between the Pin-swap machine and the ctrl admission path
    /// (tray-pin-swap design D4).
    pending_swap: Arc<crate::PendingSwapToken>,
    /// Shared with the ctrl registry, which keeps it current (design D1).
    attached_clients: Arc<AttachedClientsCell>,
    /// The runtime's Cast connector, when the runtime supplies one
    /// (cast-owner-output design D2). Lives on the event loop. Filled only
    /// from the Local entry's `LocalOutputHooks`.
    cast_connector: Option<crate::CastConnector>,
    /// Whether the runtime hosts an Emby Session output (emby-session-owner-output
    /// design D2), advertised as the `emby-session-output` capability.
    emby_session_output: bool,
}

fn spawn_queue_persistence_worker(
    merged_tx: mpsc::Sender<DaemonEvent>,
) -> mpsc::Sender<QueuePersistenceRequest> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        while let Ok(request) = rx.recv() {
            match request {
                QueuePersistenceRequest::Save(state) => {
                    if let Err(error) = mbv_config::save_stay_alive_queue_state(&state) {
                        let _ =
                            merged_tx.send(DaemonEvent::QueuePersistenceFailed(error.to_string()));
                    }
                }
                QueuePersistenceRequest::SaveNamedQueues(state) => {
                    if let Err(error) = mbv_config::save_named_queues_state(*state) {
                        // Report the failure back, so the loop unsets its
                        // Named-queue capture and the next dirty event
                        // retries the write.
                        let _ = merged_tx
                            .send(DaemonEvent::NamedQueuesPersistFailed(error.to_string()));
                    }
                }
                QueuePersistenceRequest::Flush(ack) => {
                    let _ = ack.send(());
                }
            }
        }
    });
    tx
}

fn forward_transport(
    transport_rx: mpsc::Receiver<mbv_ctrl::TransportCommand>,
    tx: mpsc::Sender<DaemonEvent>,
) {
    std::thread::spawn(move || {
        for command in transport_rx {
            let _ = tx.send(DaemonEvent::Transport(command));
        }
    });
}

fn prewarm_player(player: &Player, config: &mbv_config::Config) {
    player.pre_warm(
        config.audio_pipe_target(),
        config.audio_pipe_samplerate,
        config.audio_pipe_bitdepth,
    );
}

fn start_daemon(
    startup: DaemonStartupContext,
    hooks: DaemonRuntimeHooks,
    local_outputs: Option<LocalOutputHooks>,
) -> Result<DaemonStarted, OwnerLockError> {
    let role = startup.role;
    let config = startup.config;
    let owner_settings = crate::owner_settings::reader(role, &config);
    let emby_runtime = startup.emby;
    let audiobookshelf_runtime = startup.audiobookshelf;
    let (cast_connector, emby_session_output) = local_outputs.map_or((None, false), |o| {
        (Some(o.cast_connector), o.emby_session_output)
    });
    // Only the packaged Owner records its PID in `mbv.pid`; the Local
    // role's PID record is its single-instance lock file (design D2).
    // The flock is taken first and the guard held for the whole run, so
    // whoever locks the file first is the daemon and the recorded PID is
    // always the live lock holder. A held lock refuses the start with an
    // error naming the record (harden-owner-process-boundaries 1.3).
    let pid_lock = match role {
        DaemonRole::Packaged => Some(lock_pid_file(&pid_file())?),
        DaemonRole::Local => None,
    };

    let (shutdown_signal_tx, shutdown_signal_rx) = setup_shutdown_signal();
    let client = emby_runtime.as_ref().map_or_else(
        || Arc::new(Mutex::new(EmbyClient::new(config.clone()))),
        |runtime| Arc::clone(&runtime.client),
    );
    let control_credential = (role == DaemonRole::Local)
        .then(mbv_config::load_or_create_control_credential)
        .transpose()
        .expect("mbv daemon: failed to load Control credential");

    let (player_tx, player_rx) = mpsc::channel();
    let (ws_tx_chan, ws_rx) = mpsc::channel();
    // ws::start() only spawns a background reconnect-loop thread and returns
    // immediately — it does not block on the connection actually completing
    // — so it's cheap enough to keep here, ahead of Player/mpris/tray.
    let ws_send_tx = emby_runtime
        .as_ref()
        .map(|_| mbv_ws::start(client.lock().unwrap().ws_url(), ws_tx_chan));

    let mut client_locked = client.lock().unwrap().clone();
    // Packaged mbvd stays headless; Local honors the user's audio-window setting.
    client_locked.config.show_audio_window = role == DaemonRole::Local && config.show_audio_window;
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
        mbv_ctrl::player::SubtitlePrefs::default(),
        player_tx,
        ws_send_tx.clone(),
    )
    .with_video_cache(
        client_locked.config.video_cache_forward_mb,
        client_locked.config.video_cache_back_mb,
    )
    .with_audio_device(audio_device);
    prewarm_player(&player, &client_locked.config);
    let (merged_tx, merged_rx) = mpsc::channel::<DaemonEvent>();
    let player_status = Arc::clone(&player.status);
    let (transport_tx, transport_rx) = mpsc::channel();
    // Created here, before the registry exists, so the handle and the
    // registry share one cell (design D1).
    let attached_clients = Arc::new(AttachedClientsCell::default());
    (hooks.on_player_ready)(DaemonPlayerHandle {
        status: player_status,
        transport_tx,
        attached_clients: Arc::clone(&attached_clients),
    });

    // Tray ownership lives in the loop (design D4): one startup reconcile in
    // place of the old one-shot `start_tray`, then the loop's 1 s poll keeps
    // it in step with the live settings.
    let mut tray = TrayState::new(hooks.on_tray_ready, shutdown_signal_tx.clone());
    tray.reconcile(owner_settings().tray_enabled);
    let pending_swap = Arc::new(crate::PendingSwapToken::default());
    let pin_swap = PinSwapState::new(
        hooks.swap_command,
        hooks.notify,
        merged_tx.clone(),
        Arc::clone(&pending_swap),
    );
    forward_transport(transport_rx, merged_tx.clone());
    forward_merged_event_sources(
        &merged_tx,
        player_rx,
        ws_rx,
        emby_runtime.as_ref().map(|runtime| runtime.generation),
        shutdown_signal_rx,
    );

    // Install the owner's Audiobookshelf context on the daemon player so
    // admitted ABS slots reach `prepare_source`, and wire the player's
    // acknowledged-progress sender into the daemon event loop.
    install_daemon_audiobookshelf_context(&player, audiobookshelf_runtime.as_ref(), &merged_tx);

    Ok(DaemonStarted {
        config,
        role,
        emby_runtime,
        audiobookshelf_runtime,
        client,
        control_credential,
        player,
        merged_tx,
        merged_rx,
        ws_send_tx,
        owner_settings,
        tray,
        pid_lock,
        pin_swap,
        pending_swap,
        attached_clients,
        cast_connector,
        emby_session_output,
        toggle_panel: hooks.toggle_panel,
    })
}

/// Forwards the player, Emby-ws, and shutdown-signal sources onto the
/// daemon's one merged event channel, so the event loop reads a single
/// channel.
fn forward_merged_event_sources(
    merged_tx: &mpsc::Sender<DaemonEvent>,
    player_rx: mpsc::Receiver<mbv_ctrl::player::PlayerEvent>,
    ws_rx: mpsc::Receiver<mbv_ws::WsEvent>,
    ws_generation: Option<mbv_core::service_runtime::SetupGeneration>,
    shutdown_signal_rx: mpsc::Receiver<()>,
) {
    let tx = merged_tx.clone();
    std::thread::spawn(move || {
        for ev in player_rx {
            let _ = tx.send(DaemonEvent::Player(ev));
        }
    });
    if let Some(generation) = ws_generation {
        let tx = merged_tx.clone();
        std::thread::spawn(move || {
            for event in ws_rx {
                let _ = tx.send(DaemonEvent::Ws { generation, event });
            }
        });
    }
    let tx = merged_tx.clone();
    std::thread::spawn(move || {
        if shutdown_signal_rx.recv().is_ok() {
            let _ = tx.send(DaemonEvent::Shutdown);
        }
    });
}

fn initialize_queue(role: DaemonRole, player: &Player) -> (DaemonPlayerOwner, SharedQueueState) {
    // The Named queues the Owner starts with (gui-named-queues design D6): a
    // Local Owner restores them from their state file before it serves its
    // first Client, and a packaged `mbvd` starts with one empty `Default`.
    let startup = restore::startup_queues(role, &Arc::new(mbv_queue::QueueRevisionMint::default()));
    let initial_queue = startup.queue;
    let initial_source = startup.source;
    let initial_lineage = startup.lineage;
    if role == DaemonRole::Local {
        if let Err(error) =
            mbv_config::save_stay_alive_queue_state(&mbv_config::StayAliveQueueState {
                queue: project_queue_state(
                    &initial_queue,
                    &initial_source,
                    &player.status.lock().unwrap(),
                ),
                lineage: initial_lineage,
            })
        {
            tracing::error!(name: "daemon.queue_state_initialization.failed", target: "queue", error = %error, "failed to initialize Stay-alive queue state");
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
        // A new owner starts on local playback (cast-owner-output spec
        // "Owner starts").
        playback_output: Arc::new(Mutex::new(mbv_ctrl::OutputState::Local)),
        // A restored queue resumes where it left off: the start slot its
        // earlier Owner saved becomes the queue's active slot, so the player
        // seeds its playlist there and a Play begins at it (gui-named-queues
        // design D4). A fresh queue has none, so a Play starts its first slot.
        start_slot: Arc::new(Mutex::new(initial_queue.active_slot_id())),
        // The Named-queue list is published below, once the Owner exists.
        named_queue_list: Arc::new(Mutex::new(None)),
    };
    let owner = DaemonPlayerOwner {
        core: PlayerOwnerState::new(initial_queue, initial_source),
        named_queues: startup.store,
        ..Default::default()
    };
    // The Named queues the Local Owner holds, stated once before it serves its
    // first Client (gui-named-queues design D1). A packaged `mbvd` holds none:
    // it advertises no `named-queues` and its store is never used.
    if role == DaemonRole::Local {
        // The list a newly-attaching Client is seeded with (design D3, "sent on
        // attach"): deposited before the control socket binds, and refreshed at
        // every later list change.
        shared_queue.note_named_queue_list(NamedQueueListSeed::new(
            &owner.named_queues,
            owner.core.queue.len(),
        ));
        tracing::info!(
            name: "daemon.named_queues.ready",
            target: "queue",
            current = ?owner.named_queues.current,
            current_name = %owner.named_queues.current_name,
            next_queue_id = ?owner.named_queues.next_queue_id,
            resting = owner.named_queues.resting.len(),
            "the local Owner holds its Named queues"
        );
    }
    (owner, shared_queue)
}

fn start_local_control_server(
    role: DaemonRole,
    caps: crate::DaemonHelloCaps,
    merged_tx: &mpsc::Sender<DaemonEvent>,
    ctrl_clients: &ClientRegistry,
    player: &Player,
    shared_queue: &SharedQueueState,
    control_credential: Option<&String>,
    owner_settings: crate::OwnerSettingsReader,
) {
    // Bind and start the control socket only once the daemon can immediately
    // accept and speak the protocol, so local clients never connect and hang
    // waiting for the daemon hello.
    if let Some(listener) = bind_ctrl_listener() {
        let ctrl_clients = Arc::clone(ctrl_clients);
        let merged_tx2 = merged_tx.clone();
        let player_status = Arc::clone(&player.status);
        let shared_queue = shared_queue.clone();
        let control_credential = control_credential.cloned();
        std::thread::spawn(move || {
            let settings_reader = owner_settings;
            for stream in listener.incoming() {
                let Ok(stream) = stream else { continue };
                spawn_ctrl_client(
                    SocketStream::Unix(stream),
                    CtrlTransport::Local,
                    merged_tx2.clone(),
                    Arc::clone(&ctrl_clients),
                    control_credential.clone(),
                    Arc::clone(&player_status),
                    shared_queue.clone(),
                    caps,
                    role,
                    Arc::clone(&settings_reader),
                );
            }
        });
    }
}

fn bind_tcp_control(listen: &str, direct_commands: &mut Vec<String>) -> Option<TcpListener> {
    if listen.trim().is_empty() {
        None
    } else {
        match TcpListener::bind(listen.trim()) {
            Ok(listener) => {
                let port = listener.local_addr().map_or(0, |addr| addr.port());
                if port > 0 {
                    direct_commands.push(mbv_direct_tcp_port_command(port));
                    tracing::info!(name: "daemon.ctrl_tcp_listen.started", target: "daemon", address = %listener.local_addr().map_or_else(|_| listen.to_string(), |addr| addr.to_string()), "daemon TCP control listening");
                }
                Some(listener)
            }
            Err(e) => {
                tracing::warn!(name: "daemon.ctrl_tcp_bind.failed", target: "daemon", address = %listen, error = %e, "daemon TCP control bind failed");
                None
            }
        }
    }
}

fn register_capabilities(
    client: &Arc<Mutex<EmbyClient>>,
    emby_runtime: Option<&EmbyOwnerContext>,
    direct_commands: &[String],
    audio_only: bool,
) {
    // Register capabilities off the startup path so it doesn't block on the
    // Emby HTTP round trip.
    if emby_runtime.is_some() {
        let client = client.lock().unwrap().clone();
        let direct_commands = direct_commands.to_vec();
        std::thread::spawn(move || {
            client.register_capabilities_with_options(&direct_commands, audio_only);
        });
    }
}

fn serve_tcp_control(
    listener: Option<TcpListener>,
    role: DaemonRole,
    caps: crate::DaemonHelloCaps,
    ctrl_clients: &ClientRegistry,
    merged_tx: &mpsc::Sender<DaemonEvent>,
    player: &Player,
    shared_queue: &SharedQueueState,
    control_credential: Option<&String>,
    owner_settings: crate::OwnerSettingsReader,
) {
    if let Some(listener) = listener {
        let ctrl_clients = Arc::clone(ctrl_clients);
        let merged_tx2 = merged_tx.clone();
        let player_status = Arc::clone(&player.status);
        let shared_queue = shared_queue.clone();
        let control_credential = control_credential.cloned();
        std::thread::spawn(move || {
            let settings_reader = owner_settings;
            for stream in listener.incoming() {
                let Ok(stream) = stream else { continue };
                spawn_ctrl_client(
                    SocketStream::Tcp(stream),
                    CtrlTransport::Tcp,
                    merged_tx2.clone(),
                    Arc::clone(&ctrl_clients),
                    control_credential.clone(),
                    Arc::clone(&player_status),
                    shared_queue.clone(),
                    caps,
                    role,
                    Arc::clone(&settings_reader),
                );
            }
        });
    }
}

fn spawn_status_broadcast(
    client: &Arc<Mutex<EmbyClient>>,
    player: &Player,
    clients: &ClientRegistry,
) {
    // Broadcast current PlayerStatus to connected TUIs so the
    // seekbar and toggle state stay in sync without sending the full queue.
    let broadcast_interval =
        std::time::Duration::from_millis(client.lock().unwrap().config.daemon_broadcast_ms);
    let player_status = Arc::clone(&player.status);
    let ctrl_clients = Arc::clone(clients);
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(broadcast_interval);
            if !ctrl_clients.lock().unwrap().has_driver() {
                continue;
            }
            let status = player_status.lock().unwrap().clone();
            broadcast(&ctrl_clients, &CtrlEvent::StatusOnly(status));
        }
    });
}

pub fn run_with_options(
    startup: DaemonStartupContext,
    audio_only: bool,
    hooks: DaemonRuntimeHooks,
) -> Result<Infallible, OwnerLockError> {
    run_with_owner_outputs(startup, audio_only, hooks, None)
}

/// The Local run entry point: the only entry that can carry a Cast
/// connector or an Emby Session output, through `LocalOutputHooks`. The
/// TUI's Local process calls it; the packaged `mbvd` cannot, because its
/// entry point takes no such argument. A startup whose role is not
/// [`DaemonRole::Local`] is refused, so the packaged role can never reach
/// the remote-output hooks.
pub fn run_local_with_options(
    startup: DaemonStartupContext,
    audio_only: bool,
    hooks: DaemonRuntimeHooks,
    local_outputs: LocalOutputHooks,
) -> Result<Infallible, LocalStartupError> {
    if startup.role != DaemonRole::Local {
        return Err(LocalStartupError::NotLocal(startup.role));
    }
    run_with_owner_outputs(startup, audio_only, hooks, Some(local_outputs))
        .map_err(LocalStartupError::OwnerLock)
}

fn run_with_owner_outputs(
    startup: DaemonStartupContext,
    audio_only: bool,
    hooks: DaemonRuntimeHooks,
    local_outputs: Option<LocalOutputHooks>,
) -> Result<Infallible, OwnerLockError> {
    let started = start_daemon(startup, hooks, local_outputs)?;
    let DaemonStarted {
        config,
        role,
        emby_runtime,
        audiobookshelf_runtime,
        client,
        control_credential,
        player,
        merged_tx,
        merged_rx,
        ws_send_tx,
        owner_settings,
        tray,
        // Held past every later move, so the owner's flock stays up for
        // the whole run (the loop below never returns).
        pid_lock: _pid_lock,
        pin_swap,
        pending_swap,
        attached_clients,
        cast_connector,
        emby_session_output,
        toggle_panel,
    } = started;
    let (owner, shared_queue) = initialize_queue(role, &player);
    let caps = crate::DaemonHelloCaps {
        audio_only,
        playback_output: cast_connector.is_some(),
        emby_session_output,
    };
    // Owner restore runs the same progress refresh as cold adoption and a
    // manual queue refresh: the restored queue carries no Service positions
    // (design D3), so Emby and Audiobookshelf slots get the server's values
    // before the loop starts serving clients.
    start_queue_enrichment(
        &owner.core.queue,
        &client,
        audiobookshelf_runtime.as_ref(),
        &merged_tx,
    );
    let ctrl_clients: ClientRegistry = Arc::new(Mutex::new(CtrlClients::new(
        merged_tx.clone(),
        attached_clients,
        pending_swap,
    )));
    start_local_control_server(
        role,
        caps,
        &merged_tx,
        &ctrl_clients,
        &player,
        &shared_queue,
        control_credential.as_ref(),
        Arc::clone(&owner_settings),
    );

    let mut direct_commands = Vec::new();
    // --- From here on: network/Emby-session-visibility setup (protocol
    // negotiation metadata, capability registration). Local control is
    // already up and serving connections above. ---
    let tcp_listener = bind_tcp_control(&config.daemon_server_tcp_listen, &mut direct_commands);
    register_capabilities(&client, emby_runtime.as_ref(), &direct_commands, audio_only);
    serve_tcp_control(
        tcp_listener,
        role,
        caps,
        &ctrl_clients,
        &merged_tx,
        &player,
        &shared_queue,
        control_credential.as_ref(),
        Arc::clone(&owner_settings),
    );
    spawn_status_broadcast(&client, &player, &ctrl_clients);

    let queue_persist_tx = spawn_queue_persistence_worker(merged_tx.clone());
    let mut daemon_loop = DaemonLoop {
        owner,
        player,
        shared_queue,
        ctrl_clients,
        client,
        emby_runtime,
        audiobookshelf_runtime,
        merged_tx,
        ws_send_tx,
        direct_commands,
        owner_settings,
        role,
        audio_only,
        last_keepalive: Instant::now(),
        last_capabilities: Instant::now(),
        last_queue_resync: Instant::now(),
        store: Box::new(|state| Ok(mbv_config::save_stay_alive_queue_state(state)?)),
        named_store: Box::new(|state| Ok(mbv_config::save_named_queues_state(state)?)),
        named_queues_saved: None,
        queue_persist_tx: Some(queue_persist_tx),
        tray,
        pin_swap,
        cast_connector,
        emby_session_output,
        emby_session_worker: None,
        toggle_panel,
    };
    run_daemon_loop(&mut daemon_loop, &merged_rx)
}

fn run_daemon_loop(daemon_loop: &mut DaemonLoop, merged_rx: &mpsc::Receiver<DaemonEvent>) -> ! {
    loop {
        daemon_loop.tick(Instant::now());
        match merged_rx.recv_timeout(Duration::from_millis(25)) {
            Ok(ev) => {
                if daemon_loop.handle_event(ev) == LoopFlow::Shutdown {
                    // Only the packaged Owner owns the `mbv.pid` record and
                    // only it may remove the file (design D2).
                    if daemon_loop.role == DaemonRole::Packaged {
                        let _ = std::fs::remove_file(pid_file());
                    }
                    std::process::exit(0);
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                daemon_loop.on_recv_timeout();
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                unreachable!("daemon event channel closed")
            }
        }
    }
}
