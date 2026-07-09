use std::io::{BufRead, BufReader, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

use crate::api::{mbv_direct_tcp_port_command, EmbyClient, MediaItem};
use crate::ctrl::{CtrlCmd, CtrlEvent, CtrlHello, CtrlState, DisconnectReason};
use crate::player::{Player, PlayerCommand, PlayerEvent};
use crate::ws::WsEvent;

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
    /// Carries the requesting client's own event sender alongside the
    /// command, so a rejection (see #90) can be replied to that one client
    /// instead of broadcast to every connected TUI.
    Ctrl(CtrlCmd, CtrlClientId, CtrlSender),
    CtrlDisconnected(CtrlClientId),
    Shutdown,
}

type CtrlClientId = u64;
type CtrlSender = mpsc::Sender<CtrlOutbound>;

enum CtrlOutbound {
    Event(String),
    Close,
}

trait CtrlStream: std::io::Read + Write + Send + Sized + 'static {
    fn try_clone_stream(&self) -> std::io::Result<Self>;
    fn shutdown_stream(&self);
}

impl CtrlStream for UnixStream {
    fn try_clone_stream(&self) -> std::io::Result<Self> {
        self.try_clone()
    }

    fn shutdown_stream(&self) {
        let _ = self.shutdown(Shutdown::Both);
    }
}

impl CtrlStream for TcpStream {
    fn try_clone_stream(&self) -> std::io::Result<Self> {
        self.try_clone()
    }

    fn shutdown_stream(&self) {
        let _ = self.shutdown(Shutdown::Both);
    }
}

#[derive(Default)]
struct CtrlClients {
    next_id: CtrlClientId,
    driver: Option<CtrlClientId>,
    clients: Vec<CtrlClient>,
}

struct CtrlClient {
    id: CtrlClientId,
    tx: CtrlSender,
}

type ClientRegistry = Arc<Mutex<CtrlClients>>;

struct CtrlRequest<'a> {
    client_id: CtrlClientId,
    reply_tx: &'a CtrlSender,
}

#[derive(Clone)]
struct SharedQueueState {
    items: Arc<Mutex<Vec<MediaItem>>>,
    cursor: Arc<Mutex<usize>>,
    source: Arc<Mutex<crate::config::QueueSource>>,
}

pub struct DaemonPlayerHandle {
    pub status: Arc<Mutex<crate::player::PlayerStatus>>,
    pub command_tx: Arc<Mutex<Option<mpsc::Sender<PlayerCommand>>>>,
}

type OnPlayerReady = Box<dyn FnOnce(DaemonPlayerHandle)>;
type OnTrayReady = Box<dyn FnOnce(mpsc::SyncSender<()>) -> Option<Box<dyn Send>>>;

pub struct DaemonRuntimeHooks {
    pub on_player_ready: OnPlayerReady,
    pub on_tray_ready: OnTrayReady,
}

pub fn pid_file() -> std::path::PathBuf {
    let dir = crate::config::data_dir_system_or_local();
    let _ = std::fs::create_dir_all(&dir);
    dir.join("mbv.pid")
}

fn broadcast(clients: &ClientRegistry, event: &CtrlEvent) {
    let Some(json) = serialize_ctrl_event(event) else {
        return;
    };
    clients.lock().unwrap().send_to_driver(json);
}

/// Send an event to a single ctrl-socket client, rather than every connected
/// TUI. Used for per-request responses like a command rejection (#90).
fn send_to(client: &CtrlSender, event: &CtrlEvent) {
    if let Some(json) = serialize_ctrl_event(event) {
        let _ = client.send(CtrlOutbound::Event(json));
    }
}

/// Shared by `broadcast` and `send_to` so both go through one serialization
/// path instead of repeating `serde_json::to_string(event).ok()` inline.
fn serialize_ctrl_event(event: &CtrlEvent) -> Option<String> {
    serde_json::to_string(event).ok()
}

impl CtrlClients {
    fn add_pending(&mut self, tx: CtrlSender) -> CtrlClientId {
        let id = self.next_id;
        self.next_id += 1;
        self.clients.push(CtrlClient { id, tx });
        id
    }

    fn remove(&mut self, id: CtrlClientId) {
        self.clients.retain(|client| client.id != id);
        if self.driver == Some(id) {
            self.driver = None;
        }
    }

    fn has_client(&self, id: CtrlClientId) -> bool {
        self.clients.iter().any(|client| client.id == id)
    }

    fn has_driver(&self) -> bool {
        self.driver
            .is_some_and(|id| self.clients.iter().any(|client| client.id == id))
    }

    fn send_to_driver(&mut self, json: String) {
        let Some(driver_id) = self.driver else {
            return;
        };
        let mut stale = false;
        if let Some(driver) = self.clients.iter().find(|client| client.id == driver_id) {
            stale = driver.tx.send(CtrlOutbound::Event(json)).is_err();
        }
        if stale {
            self.remove(driver_id);
        }
    }

    fn take_over(&mut self, next_driver: CtrlClientId, reason: DisconnectReason) {
        if !self.has_client(next_driver) {
            return;
        }
        if self.driver == Some(next_driver) {
            return;
        }
        let previous_driver = self.driver;
        self.driver = Some(next_driver);
        if let Some(previous_driver) = previous_driver {
            self.disconnect(previous_driver, reason);
        }
    }

    fn disconnect(&mut self, id: CtrlClientId, reason: DisconnectReason) {
        let Some(pos) = self.clients.iter().position(|client| client.id == id) else {
            return;
        };
        let client = self.clients.remove(pos);
        if self.driver == Some(id) {
            self.driver = None;
        }
        send_to(&client.tx, &CtrlEvent::Disconnected { reason });
        let _ = client.tx.send(CtrlOutbound::Close);
    }

    fn disconnect_driver(&mut self, reason: DisconnectReason) {
        if let Some(driver) = self.driver {
            self.disconnect(driver, reason);
        }
    }
}

fn take_over_ctrl_driver(ctrl_clients: &ClientRegistry, client_id: CtrlClientId) {
    ctrl_clients
        .lock()
        .unwrap()
        .take_over(client_id, DisconnectReason::TakenOverByCtrlClient);
}

fn evict_ctrl_driver_for_emby_remote(ctrl_clients: &ClientRegistry) {
    ctrl_clients
        .lock()
        .unwrap()
        .disconnect_driver(DisconnectReason::TakenOverByEmbyRemote);
}

/// A reason a ctrl-socket command is not acted on, computed server-side.
/// Currently the only case is audio-only mode rejecting a non-audio play
/// request; kept as a small pure function so it's testable without a live
/// `Player`/`EmbyClient`. Returns the bare reason (not a `CtrlEvent`) so the
/// same string can be reused for both the server-side log line and the wire
/// event the caller sends — one message, not two that can drift apart.
fn audio_only_rejection(audio_only: bool, fetched: &[MediaItem]) -> Option<String> {
    if audio_only && !all_audio(fetched) {
        Some("Daemon is running in audio-only mode; can't play video items".to_string())
    } else {
        None
    }
}

fn spawn_ctrl_client<S>(
    stream: S,
    merged_tx: mpsc::Sender<DaemonEvent>,
    ctrl_clients: ClientRegistry,
    client: Arc<Mutex<EmbyClient>>,
    player_status: Arc<Mutex<crate::player::PlayerStatus>>,
    shared_queue: SharedQueueState,
) where
    S: CtrlStream,
{
    let Ok(writer_stream) = stream.try_clone_stream() else {
        return;
    };
    let (ev_tx, ev_rx) = mpsc::channel::<CtrlOutbound>();

    if let Ok(hello_json) = serde_json::to_string(&CtrlEvent::Hello(CtrlHello::current())) {
        ev_tx.send(CtrlOutbound::Event(hello_json)).ok();
    }

    std::thread::spawn(move || {
        let mut w = writer_stream;
        for outbound in ev_rx {
            match outbound {
                CtrlOutbound::Event(line) => {
                    if writeln!(w, "{line}").is_err() {
                        break;
                    }
                }
                CtrlOutbound::Close => break,
            }
        }
        w.shutdown_stream();
    });

    std::thread::spawn(move || {
        let reader = BufReader::new(stream);
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
            items: shared_queue.items.lock().unwrap().clone(),
            cursor: *shared_queue.cursor.lock().unwrap(),
            source: shared_queue.source.lock().unwrap().clone(),
        })) {
            ev_tx.send(CtrlOutbound::Event(init_json)).ok();
        }
        let reply_tx = ev_tx.clone();
        let client_id = ctrl_clients.lock().unwrap().add_pending(ev_tx);

        for line in lines {
            let Ok(line) = line else { break };
            if line.is_empty() {
                continue;
            }
            if let Ok(cmd) = serde_json::from_str::<CtrlCmd>(&line) {
                let _ = merged_tx.send(DaemonEvent::Ctrl(cmd, client_id, reply_tx.clone()));
            }
        }
        let _ = merged_tx.send(DaemonEvent::CtrlDisconnected(client_id));
    });
}

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
                        client_id,
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
                );
            }
            DaemonEvent::CtrlDisconnected(client_id) => {
                ctrl_clients.lock().unwrap().remove(client_id);
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

/// Applies a freshly-decided queue snapshot to the cross-thread shared state
/// (used to seed newly-connecting ctrl-socket clients) and broadcasts it to
/// every already-connected client. Centralizes what `CtrlState`'s fields
/// must always carry together, so a future field addition (like `source` in
/// #113) can't land in only some of the call sites — previously this exact
/// shape was hand-rolled inline at five separate command branches.
fn broadcast_queue_state(
    ctrl_clients: &ClientRegistry,
    player: &Player,
    shared_queue: &SharedQueueState,
    items: &[MediaItem],
    cursor: usize,
    source: &crate::config::QueueSource,
) {
    let event = CtrlEvent::State(CtrlState {
        status: player.status.lock().unwrap().clone(),
        items: items.to_vec(),
        cursor,
        source: source.clone(),
    });
    broadcast(ctrl_clients, &event);
    *shared_queue.cursor.lock().unwrap() = cursor;
    *shared_queue.source.lock().unwrap() = source.clone();
    if let CtrlEvent::State(state) = event {
        *shared_queue.items.lock().unwrap() = state.items;
    }
}

fn handle_ctrl(
    cmd: CtrlCmd,
    request: CtrlRequest<'_>,
    client: &Arc<Mutex<EmbyClient>>,
    player: &Player,
    audio_only: bool,
    items: &mut Vec<MediaItem>,
    cursor: &mut usize,
    source: &mut crate::config::QueueSource,
    shared_queue: &SharedQueueState,
    ctrl_clients: &ClientRegistry,
) {
    match cmd {
        CtrlCmd::Hello(_) => {
            log::warn!(target: "daemon", "unexpected ctrl protocol hello after negotiation");
        }
        CtrlCmd::AdoptQueue {
            items: new_items,
            cursor: new_cursor,
            source: new_source,
        } => {
            // Adoption only ever applies to a Cold daemon (see CONTEXT.md's
            // "Cold daemon" entry) — one with no queue yet. If another
            // client's command already gave this daemon a queue by the time
            // this one arrives (a concurrent first-connect race), the daemon
            // is no longer cold, and a stale saved snapshot must not be
            // allowed to silently clobber whatever is already authoritative.
            if !items.is_empty() {
                log::warn!(
                    target: "daemon",
                    "ignoring AdoptQueue: daemon already has a queue ({} item(s))",
                    items.len()
                );
                send_to(
                    request.reply_tx,
                    &CtrlEvent::CommandRejected(
                        "daemon already has a queue; adoption skipped".to_string(),
                    ),
                );
                return;
            }
            let next_cursor = if new_items.is_empty() {
                0
            } else {
                new_cursor.min(new_items.len().saturating_sub(1))
            };
            player.set_initial_queue(&new_items, next_cursor);
            if !new_items.is_empty() {
                take_over_ctrl_driver(ctrl_clients, request.client_id);
            }
            broadcast_queue_state(
                ctrl_clients,
                player,
                shared_queue,
                &new_items,
                next_cursor,
                &new_source,
            );
            *items = new_items;
            *cursor = next_cursor;
            *source = new_source;
        }
        CtrlCmd::PlayerCmd(pc) => match PlayerCommand::from(pc) {
            PlayerCommand::ReplaceQueue {
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
                take_over_ctrl_driver(ctrl_clients, request.client_id);
                broadcast_queue_state(
                    ctrl_clients,
                    player,
                    shared_queue,
                    &new_items,
                    next_cursor,
                    source,
                );
                player.send_command(PlayerCommand::ReplaceQueue {
                    items: new_items,
                    start_idx,
                });
            }
            other => {
                if player.send_command(other) {
                    take_over_ctrl_driver(ctrl_clients, request.client_id);
                }
            }
        },
        CtrlCmd::PlayItems {
            item_ids,
            start_idx,
            start_ticks,
            source: new_source,
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
            if let Some(reason) = audio_only_rejection(audio_only, &fetched) {
                log::warn!(target: "daemon", "rejecting ctrl play request: {reason}");
                send_to(request.reply_tx, &CtrlEvent::CommandRejected(reason));
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
                        *source = new_source;
                        take_over_ctrl_driver(ctrl_clients, request.client_id);
                        broadcast_queue_state(
                            ctrl_clients,
                            player,
                            shared_queue,
                            &queue,
                            0,
                            source,
                        );
                        let c = Arc::new(client.lock().unwrap().clone());
                        player.play_queue(queue, 0, c, 100);
                        return;
                    }
                }
                *items = vec![item.clone()];
                *cursor = 0;
                *source = new_source;
                take_over_ctrl_driver(ctrl_clients, request.client_id);
                broadcast_queue_state(ctrl_clients, player, shared_queue, items, 0, source);
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
                *source = new_source;
                take_over_ctrl_driver(ctrl_clients, request.client_id);
                broadcast_queue_state(
                    ctrl_clients,
                    player,
                    shared_queue,
                    &play_items,
                    start_idx,
                    source,
                );
                let c = Arc::new(client.lock().unwrap().clone());
                player.play_queue(play_items, start_idx, c, 100);
            }
        }
        CtrlCmd::Stop => {
            player.stop();
            if !items.is_empty() {
                take_over_ctrl_driver(ctrl_clients, request.client_id);
            }
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
    source: &mut crate::config::QueueSource,
    shared_queue: &SharedQueueState,
    ctrl_clients: &ClientRegistry,
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
            if let Some(reason) = audio_only_rejection(audio_only, &fetched) {
                // Emby-websocket-driven remote control has no TUI on the other end
                // to show a rejection to — log only, per #90's scope.
                log::warn!(target: "daemon", "rejecting websocket play request: {reason}");
                return;
            }
            // Clamp start_index in case the server sends an out-of-range value
            let start_idx = start_index.min(fetched.len().saturating_sub(1));
            *items = fetched.clone();
            *cursor = start_idx;
            *source = crate::config::QueueSource::Remote;
            evict_ctrl_driver_for_emby_remote(ctrl_clients);
            broadcast_queue_state(
                ctrl_clients,
                player,
                shared_queue,
                &fetched,
                start_idx,
                source,
            );
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
                player.play_queue(items_with_pos, start_idx, c, 100);
            }
        }
        WsEvent::Stop => {
            player.stop();
            if !items.is_empty() {
                evict_ctrl_driver_for_emby_remote(ctrl_clients);
            }
        }
        WsEvent::Pause => {
            if player.set_paused(true) {
                evict_ctrl_driver_for_emby_remote(ctrl_clients);
            }
        }
        WsEvent::Unpause => {
            if player.set_paused(false) {
                evict_ctrl_driver_for_emby_remote(ctrl_clients);
            }
        }
        WsEvent::NextTrack => {
            if player.next() {
                evict_ctrl_driver_for_emby_remote(ctrl_clients);
            }
        }
        WsEvent::PreviousTrack => {
            if player.previous() {
                evict_ctrl_driver_for_emby_remote(ctrl_clients);
            }
        }
        WsEvent::Seek(ticks) => {
            use crate::api::TICKS_PER_SECOND;
            if player.send_command(PlayerCommand::SeekAbsolute(
                ticks as f64 / TICKS_PER_SECOND as f64,
            )) {
                evict_ctrl_driver_for_emby_remote(ctrl_clients);
            }
        }
        WsEvent::TogglePause => {
            if player.send_command(PlayerCommand::TogglePause) {
                evict_ctrl_driver_for_emby_remote(ctrl_clients);
            }
        }
        WsEvent::SeekRelative(secs) => {
            if player.send_command(PlayerCommand::Seek(secs)) {
                evict_ctrl_driver_for_emby_remote(ctrl_clients);
            }
        }
        WsEvent::SetVolume(v) => {
            let vol_max = player.status.lock().unwrap().volume_max;
            if player.send_command(PlayerCommand::SetVolume(v.clamp(0, vol_max))) {
                evict_ctrl_driver_for_emby_remote(ctrl_clients);
            }
        }
        WsEvent::VolumeUp => {
            let st = player.status.lock().unwrap();
            let v = (st.volume + 5).min(st.volume_max);
            drop(st);
            if player.send_command(PlayerCommand::SetVolume(v)) {
                evict_ctrl_driver_for_emby_remote(ctrl_clients);
            }
        }
        WsEvent::VolumeDown => {
            let v = (player.status.lock().unwrap().volume - 5).max(0);
            if player.send_command(PlayerCommand::SetVolume(v)) {
                evict_ctrl_driver_for_emby_remote(ctrl_clients);
            }
        }
        WsEvent::SetMute(muted) => {
            if player.send_command(PlayerCommand::SetMute(muted)) {
                evict_ctrl_driver_for_emby_remote(ctrl_clients);
            }
        }
        WsEvent::ToggleMute => {
            let muted = !player.status.lock().unwrap().muted;
            if player.send_command(PlayerCommand::SetMute(muted)) {
                evict_ctrl_driver_for_emby_remote(ctrl_clients);
            }
        }
        WsEvent::SetAudio(index) => {
            if player.send_command(PlayerCommand::SetAudio(index)) {
                evict_ctrl_driver_for_emby_remote(ctrl_clients);
            }
        }
        WsEvent::SetSub(index) => {
            let sid = player
                .status
                .lock()
                .unwrap()
                .subtitle_stream_index_to_mpv_id(index);
            if let Some(sid) = sid {
                if player.send_command(PlayerCommand::SetSub(sid)) {
                    evict_ctrl_driver_for_emby_remote(ctrl_clients);
                }
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
    use super::{
        all_audio, audio_only_rejection, broadcast, handle_ctrl, handle_ws, send_to, CtrlClients,
        CtrlEvent, CtrlOutbound, CtrlRequest, SharedQueueState,
    };
    use crate::api::MediaItem;
    use crate::config::{Config, QueueSource};
    use crate::ctrl::DisconnectReason;
    use crate::ctrl::{CtrlCmd, WireCommand};
    use crate::player::{Player, PlayerCommand, PlayerEvent, PlayerStatus, SubtitlePrefs};
    use crate::ws::WsEvent;
    use std::sync::{mpsc, Arc, Mutex};

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

    #[test]
    fn audio_only_daemon_rejects_non_audio_play_request() {
        let fetched = [item("movie", "Video", "Movie")];
        let rejection = audio_only_rejection(true, &fetched);
        assert!(rejection.is_some_and(|r| !r.is_empty()));
    }

    #[test]
    fn audio_only_daemon_accepts_audio_play_request() {
        let fetched = [item("song", "Audio", "Audio")];
        assert!(audio_only_rejection(true, &fetched).is_none());
    }

    #[test]
    fn non_audio_only_daemon_never_rejects() {
        let fetched = [item("movie", "Video", "Movie")];
        assert!(audio_only_rejection(false, &fetched).is_none());
    }

    fn add_client(clients: &mut CtrlClients) -> (u64, mpsc::Receiver<CtrlOutbound>) {
        let (tx, rx) = mpsc::channel();
        let id = clients.add_pending(tx);
        (id, rx)
    }

    fn shared_queue_state() -> SharedQueueState {
        SharedQueueState {
            items: Arc::new(Mutex::new(Vec::new())),
            cursor: Arc::new(Mutex::new(0)),
            source: Arc::new(Mutex::new(QueueSource::Unknown)),
        }
    }

    fn cold_player() -> Player {
        let (event_tx, _event_rx) = mpsc::channel::<PlayerEvent>();
        Player::new(
            String::new(),
            String::new(),
            false,
            false,
            true,
            false,
            false,
            SubtitlePrefs::default(),
            event_tx,
            None,
        )
    }

    fn recv_event(rx: &mpsc::Receiver<CtrlOutbound>) -> CtrlEvent {
        match rx.recv().unwrap() {
            CtrlOutbound::Event(json) => serde_json::from_str(&json).unwrap(),
            CtrlOutbound::Close => panic!("expected event, got close"),
        }
    }

    fn assert_close(rx: &mpsc::Receiver<CtrlOutbound>) {
        match rx.recv().unwrap() {
            CtrlOutbound::Close => {}
            CtrlOutbound::Event(json) => panic!("expected close, got {json}"),
        }
    }

    #[test]
    fn ctrl_takeover_disconnects_previous_driver_and_routes_updates_to_new_driver() {
        let mut clients = CtrlClients::default();
        let (old_id, old_rx) = add_client(&mut clients);
        let (new_id, new_rx) = add_client(&mut clients);

        clients.take_over(old_id, DisconnectReason::TakenOverByCtrlClient);
        clients.take_over(new_id, DisconnectReason::TakenOverByCtrlClient);

        match recv_event(&old_rx) {
            CtrlEvent::Disconnected { reason } => {
                assert_eq!(reason, DisconnectReason::TakenOverByCtrlClient);
            }
            _ => panic!("expected structured disconnect"),
        }
        assert_close(&old_rx);

        let registry = Arc::new(Mutex::new(clients));
        broadcast(
            &registry,
            &CtrlEvent::StatusOnly(PlayerStatus {
                volume: 77,
                ..PlayerStatus::default()
            }),
        );

        match recv_event(&new_rx) {
            CtrlEvent::StatusOnly(status) => assert_eq!(status.volume, 77),
            _ => panic!("expected status update"),
        }
        assert!(old_rx.try_recv().is_err());
    }

    #[test]
    fn pending_ctrl_clients_do_not_receive_broadcasts_before_takeover() {
        let mut clients = CtrlClients::default();
        let (_pending_id, pending_rx) = add_client(&mut clients);
        let registry = Arc::new(Mutex::new(clients));

        broadcast(
            &registry,
            &CtrlEvent::StatusOnly(PlayerStatus {
                volume: 12,
                ..PlayerStatus::default()
            }),
        );

        assert!(pending_rx.try_recv().is_err());
    }

    #[test]
    fn command_rejection_to_pending_client_does_not_take_over() {
        let mut clients = CtrlClients::default();
        let (driver_id, driver_rx) = add_client(&mut clients);
        let (_pending_id, pending_rx) = add_client(&mut clients);
        clients.take_over(driver_id, DisconnectReason::TakenOverByCtrlClient);

        send_to(
            &clients.clients[1].tx,
            &CtrlEvent::CommandRejected("rejected".to_string()),
        );

        match recv_event(&pending_rx) {
            CtrlEvent::CommandRejected(reason) => assert_eq!(reason, "rejected"),
            _ => panic!("expected rejection"),
        }

        let registry = Arc::new(Mutex::new(clients));
        broadcast(
            &registry,
            &CtrlEvent::StatusOnly(PlayerStatus {
                volume: 33,
                ..PlayerStatus::default()
            }),
        );

        match recv_event(&driver_rx) {
            CtrlEvent::StatusOnly(status) => assert_eq!(status.volume, 33),
            _ => panic!("expected driver status update"),
        }
        assert!(pending_rx.try_recv().is_err());
    }

    #[test]
    fn emby_remote_takeover_disconnects_current_ctrl_driver() {
        let mut clients = CtrlClients::default();
        let (driver_id, driver_rx) = add_client(&mut clients);
        clients.take_over(driver_id, DisconnectReason::TakenOverByCtrlClient);

        clients.disconnect_driver(DisconnectReason::TakenOverByEmbyRemote);

        match recv_event(&driver_rx) {
            CtrlEvent::Disconnected { reason } => {
                assert_eq!(reason, DisconnectReason::TakenOverByEmbyRemote);
            }
            _ => panic!("expected structured disconnect"),
        }
        assert_close(&driver_rx);
        assert!(!clients.has_driver());
    }

    #[test]
    fn cold_ctrl_player_command_does_not_take_over() {
        let player = cold_player();
        let client = Arc::new(Mutex::new(crate::api::EmbyClient::new(Config::default())));
        let registry = Arc::new(Mutex::new(CtrlClients::default()));
        let (sender_id, sender_rx) = {
            let mut clients = registry.lock().unwrap();
            add_client(&mut clients)
        };
        let (reply_tx, _reply_rx) = mpsc::channel();
        let mut items = Vec::new();
        let mut cursor = 0;
        let mut source = QueueSource::Unknown;

        handle_ctrl(
            CtrlCmd::PlayerCmd(WireCommand::from(PlayerCommand::TogglePause)),
            CtrlRequest {
                client_id: sender_id,
                reply_tx: &reply_tx,
            },
            &client,
            &player,
            false,
            &mut items,
            &mut cursor,
            &mut source,
            &shared_queue_state(),
            &registry,
        );

        assert!(!registry.lock().unwrap().has_driver());
        assert!(sender_rx.try_recv().is_err());
    }

    #[test]
    fn cold_websocket_noop_does_not_evict_ctrl_driver() {
        let player = cold_player();
        let client = Arc::new(Mutex::new(crate::api::EmbyClient::new(Config::default())));
        let registry = Arc::new(Mutex::new(CtrlClients::default()));
        let (driver_id, driver_rx) = {
            let mut clients = registry.lock().unwrap();
            let (driver_id, driver_rx) = add_client(&mut clients);
            clients.take_over(driver_id, DisconnectReason::TakenOverByCtrlClient);
            (driver_id, driver_rx)
        };
        let mut items = Vec::new();
        let mut cursor = 0;
        let mut source = QueueSource::Unknown;

        handle_ws(
            WsEvent::TogglePause,
            &client,
            &player,
            false,
            &mut items,
            &mut cursor,
            &mut source,
            &shared_queue_state(),
            &registry,
        );

        let clients = registry.lock().unwrap();
        assert_eq!(clients.driver, Some(driver_id));
        drop(clients);
        assert!(driver_rx.try_recv().is_err());
    }
}
