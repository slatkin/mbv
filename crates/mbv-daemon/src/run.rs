use super::core::{DaemonEvent, bind_ctrl_listener, broadcast};
use super::{
    AudiobookshelfOwnerContext, CtrlTransport, DaemonLoop, DaemonPlayerHandle, DaemonPlayerOwner,
    DaemonRole, DaemonRuntimeHooks, DaemonStartupContext, EmbyOwnerContext, LoopFlow,
    SharedQueueState, broadcast_queue_state, install_daemon_audiobookshelf_context, pid_file,
    project_queue_state, setup_shutdown_signal, spawn_ctrl_client,
};
use crate::{ClientRegistry, CtrlClients};
use mbv_ctrl::player::PlayerEvent;
use mbv_ctrl::{CtrlEvent, PlaybackGeneration};
use mbv_emby::{EmbyClient, mbv_direct_tcp_port_command};
use mbv_emby_model::EmbyItem;
use mbv_net::stream::SocketStream;
use mbv_player::{Player, PlayerOwnerState};
use mbv_queue::{PlaybackQueue, ProgressObservation, QueueSlotId, StopReportOutcome};
use std::net::TcpListener;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub(crate) fn broadcast_player_event_if_not_replaced(
    ctrl_clients: &ClientRegistry,
    event: PlayerEvent,
    replacement_committed: bool,
) {
    if !replacement_committed {
        broadcast(ctrl_clients, &CtrlEvent::Player(event));
    }
}

pub(crate) fn playback_run_identity_is_current(
    run_identity: PlaybackGeneration,
    player: &Player,
) -> bool {
    run_identity == player.status.lock().unwrap().sequence_generation
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ConsumePolicy {
    pub videos: bool,
    pub audio: bool,
}

pub(super) fn apply_track_completed_observation(
    owner: &mut DaemonPlayerOwner,
    player: &Player,
    shared_queue: &SharedQueueState,
    run_identity: PlaybackGeneration,
    slot_id: QueueSlotId,
    position_ticks: i64,
    was_played: bool,
    consume: bool,
    consume_policy: ConsumePolicy,
    outcome: StopReportOutcome,
) -> bool {
    if !playback_run_identity_is_current(run_identity, player) {
        return false;
    }
    if let Some(slot) = owner.core.queue.slot(slot_id) {
        let observation = ProgressObservation::Completed {
            position_ticks,
            played: was_played,
        };
        owner.core.apply_completion_progress(
            slot_id,
            observation.position_to_record(&slot.item),
            observation.played(),
            outcome,
        );
    }
    if owner.core.consume_completed_slot(
        slot_id,
        consume,
        consume_policy.videos,
        consume_policy.audio,
    ) {
        tracing::info!(name: "daemon.queue_slot.consumed", target: "consume", slot = ?slot_id, "completed track consumed");
    }
    shared_queue.publish_observed(&owner.core);
    true
}

pub(super) fn apply_stopped_observation(
    owner: &mut DaemonPlayerOwner,
    player: &Player,
    run_identity: PlaybackGeneration,
    slot_id: Option<QueueSlotId>,
    position_ticks: i64,
    was_played: bool,
    outcome: StopReportOutcome,
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
    let observation = ProgressObservation::Stopped {
        position_ticks,
        played: was_played,
    };
    owner.core.apply_completion_progress(
        slot_id,
        observation.position_to_record(&slot.item),
        observation.played(),
        outcome,
    );
    Some(true)
}

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
    _tray: Option<Box<dyn Send>>,
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

fn start_daemon(startup: DaemonStartupContext, hooks: DaemonRuntimeHooks) -> DaemonStarted {
    let role = startup.role;
    let config = startup.config;
    let emby_runtime = startup.emby;
    let audiobookshelf_runtime = startup.audiobookshelf;
    std::fs::write(pid_file(), std::process::id().to_string())
        .expect("mbv daemon: failed to write PID file");

    let (shutdown_signal_tx, shutdown_signal_rx) = setup_shutdown_signal();
    let client = emby_runtime.as_ref().map_or_else(
        || Arc::new(Mutex::new(EmbyClient::new(config.clone()))),
        |runtime| Arc::clone(&runtime.client),
    );
    let control_credential = if role == DaemonRole::Packaged {
        None
    } else {
        Some(
            mbv_config::load_or_create_control_credential()
                .expect("mbv daemon: failed to load Control credential"),
        )
    };

    let (player_tx, player_rx) = mpsc::channel();
    let (ws_tx_chan, ws_rx) = mpsc::channel();
    // ws::start() only spawns a background reconnect-loop thread and returns
    // immediately — it does not block on the connection actually completing
    // — so it's cheap enough to keep here, ahead of Player/mpris/tray.
    let ws_send_tx = emby_runtime
        .as_ref()
        .map(|_| mbv_ws::start(client.lock().unwrap().ws_url(), ws_tx_chan));

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
        mbv_ctrl::player::SubtitlePrefs::default(),
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
    let (merged_tx, merged_rx) = mpsc::channel::<DaemonEvent>();
    let player_status = Arc::clone(&player.status);
    let (transport_tx, transport_rx) = mpsc::channel();
    (hooks.on_player_ready)(DaemonPlayerHandle {
        status: player_status,
        transport_tx,
    });

    let tray = (hooks.on_tray_ready)(shutdown_signal_tx.clone());
    forward_transport(transport_rx, merged_tx.clone());

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
    install_daemon_audiobookshelf_context(&player, audiobookshelf_runtime.as_ref(), &merged_tx);

    DaemonStarted {
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
        _tray: tray,
    }
}

fn initialize_queue(role: DaemonRole, player: &Player) -> (DaemonPlayerOwner, SharedQueueState) {
    // Shared state for ctrl socket initial-state snapshots — stores the
    // canonical queue so all ctrl peers are seeded from one source.
    let owner_state = if role == DaemonRole::Local {
        let owner_path = mbv_config::stay_alive_queue_state_path();
        mbv_config::load_stay_alive_queue_state().or_else(|| {
            (!owner_path.exists())
                .then(mbv_config::load_queue_state)
                .flatten()
                .and_then(|queue| {
                    mbv_config::legacy_queue_for_owner_if_absent(&owner_path, Some(queue))
                })
        })
    } else {
        None
    };
    let revision_mint = Arc::new(mbv_queue::QueueRevisionMint::default());
    let (initial_queue, initial_source, initial_lineage) = owner_state.map_or_else(
        || {
            (
                PlaybackQueue::from_queue_items(Vec::new(), None, Arc::clone(&revision_mint)),
                mbv_queue::QueueSource::Unknown,
                mbv_queue::QueueLineage::default(),
            )
        },
        |state| {
            let queue = PlaybackQueue::from_queue_items(
                state.queue.items,
                Some(state.queue.cursor),
                Arc::clone(&revision_mint),
            );
            (queue, state.queue.source, state.lineage)
        },
    );
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
    };
    let owner = DaemonPlayerOwner {
        core: PlayerOwnerState::new(initial_queue, initial_source),
        ..Default::default()
    };
    (owner, shared_queue)
}

fn start_local_control_server(
    audio_only: bool,
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
                    audio_only,
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
    audio_only: bool,
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
                    audio_only,
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
) -> ! {
    let started = start_daemon(startup, hooks);
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
        _tray,
    } = started;
    let (owner, shared_queue) = initialize_queue(role, &player);
    let owner_settings = crate::owner_settings::reader(role, &config);
    let ctrl_clients: ClientRegistry = Arc::new(Mutex::new(CtrlClients::new(merged_tx.clone())));
    start_local_control_server(
        audio_only,
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
        audio_only,
        &ctrl_clients,
        &merged_tx,
        &player,
        &shared_queue,
        control_credential.as_ref(),
        Arc::clone(&owner_settings),
    );
    spawn_status_broadcast(&client, &player, &ctrl_clients);

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
        store: Box::new(|state| Ok(mbv_config::save_stay_alive_queue_state(state)?)),
    };
    run_daemon_loop(&mut daemon_loop, &merged_rx)
}

fn run_daemon_loop(daemon_loop: &mut DaemonLoop, merged_rx: &mpsc::Receiver<DaemonEvent>) -> ! {
    loop {
        daemon_loop.tick(Instant::now());
        match merged_rx.recv_timeout(Duration::from_millis(25)) {
            Ok(ev) => {
                if daemon_loop.handle_event(ev) == LoopFlow::Shutdown {
                    let _ = std::fs::remove_file(pid_file());
                    std::process::exit(0);
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                daemon_loop.on_recv_timeout(Instant::now());
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                unreachable!("daemon event channel closed")
            }
        }
    }
}
