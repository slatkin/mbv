use super::*;
use super::core::{broadcast, bind_ctrl_listener, DaemonEvent};
use std::sync::{Arc, Mutex};
use std::sync::mpsc;
use std::net::TcpListener;
use crate::api::{EmbyClient, EmbyItem, mbv_direct_tcp_port_command};
use crate::daemon::ctrl::{ClientRegistry, CtrlClients};
use crate::player::{Player, PlayerEvent, PlayerOwnerState};
use crate::playback_queue::QueueSlotId;
use crate::playback::PlaybackQueue;
use crate::stream::SocketStream;
use crate::ctrl::{CtrlEvent, PlaybackRequestId, PlaybackGeneration};

pub(crate) fn broadcast_player_event_if_not_replaced(
    ctrl_clients: &ClientRegistry,
    event: PlayerEvent,
    replacement_committed: bool,
) {
    if !replacement_committed {
        broadcast(ctrl_clients, &CtrlEvent::Player(event));
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PlaybackRunIdentity {
    request_id: PlaybackRequestId,
    generation: PlaybackGeneration,
}

impl From<(PlaybackRequestId, PlaybackGeneration)> for PlaybackRunIdentity {
    fn from((request_id, generation): (PlaybackRequestId, PlaybackGeneration)) -> Self {
        Self {
            request_id,
            generation,
        }
    }
}

pub(crate) fn playback_run_identity_is_current(
    run_identity: PlaybackRunIdentity,
    player: &Player,
) -> bool {
    run_identity
        == PlaybackRunIdentity {
            request_id: 0,
            generation: player.status.lock().unwrap().sequence_generation,
        }
}

pub(crate) fn apply_track_completed_observation(
    owner: &mut DaemonPlayerOwner,
    player: &Player,
    shared_queue: &SharedQueueState,
    run_identity: PlaybackRunIdentity,
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
        // Completion observations ignore small progress changes; stopped observations below
        // retain any positive position so an interrupted item can resume precisely.
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

pub(crate) fn apply_stopped_observation(
    owner: &mut DaemonPlayerOwner,
    player: &Player,
    run_identity: PlaybackRunIdentity,
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

pub(crate) fn apply_queue_enriched(
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
    let emby_runtime = startup.emby;
    let audiobookshelf_runtime = startup.audiobookshelf;
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
    let ws_send_tx = emby_runtime
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
    let owner = DaemonPlayerOwner {
        core: PlayerOwnerState::new(initial_queue, initial_source),
        ..Default::default()
    };

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
        stay_alive: config.stay_alive,
        role,
        audio_only,
        last_keepalive: Instant::now(),
        last_capabilities: Instant::now(),
        store: Box::new(crate::config::save_stay_alive_queue_state),
    };

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
