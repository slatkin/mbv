use std::io::{BufRead, BufReader, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::api::{mbv_direct_tcp_port_command, EmbyClient, EmbyItem};
use crate::ctrl::{
    AudiobookshelfProgressEvent, CtrlCmd, CtrlEvent, CtrlHello, DisconnectReason,
    PlaybackGeneration, PlaybackIntent, PlaybackIntentAction, PlaybackIntentEvent,
    PlaybackIntentOutcome, PlaybackRequestId,
};
use crate::playback_queue::{PlaybackQueue, QueueItem, QueueSlotId};
use crate::player::{Player, PlayerCommand, PlayerEvent};
use crate::ws::WsEvent;

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
    Ws {
        generation: crate::service_runtime::SetupGeneration,
        event: WsEvent,
    },
    /// Acknowledged Audiobookshelf progress from the daemon player's
    /// progress sender, routed here so the event loop can update the Bound
    /// queue and broadcast redacted progress to capable clients.
    AudiobookshelfProgress(crate::player::AudiobookshelfProgressUpdate),
    /// Carries the requesting client's own event sender alongside the
    /// command, so a rejection (see #90) can be replied to that one client
    /// instead of broadcast to every connected TUI.
    Ctrl(CtrlCmd, CtrlClientId, CtrlSender),
    PlaybackResolved {
        start_idx: usize,
        start_ticks: i64,
        source: crate::config::QueueSource,
        client_id: CtrlClientId,
        request_id: PlaybackRequestId,
        generation: PlaybackGeneration,
        fetched: Result<Vec<EmbyItem>, String>,
    },
    CtrlDisconnected(CtrlClientId),
    Shutdown,
}

type CtrlClientId = u64;
type CtrlSender = mpsc::Sender<CtrlOutbound>;

enum CtrlOutbound {
    Event(String),
    /// Writer-thread barrier used during daemon shutdown. The ack is sent
    /// only after all earlier events have been written and flushed to the
    /// socket, so process exit cannot discard a queued shutdown notice.
    Flush(mpsc::Sender<()>),
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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum AuthorityHolder {
    #[default]
    None,
    Ctrl,
    EmbyRemote,
}

#[derive(Default)]
struct CtrlClients {
    next_id: CtrlClientId,
    connection: Vec<CtrlClient>,
    authority: AuthorityHolder,
}

/// Transport identity for a ctrl client connection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CtrlTransport {
    /// Local Unix socket listener.
    Local,
    /// TCP listener.
    Tcp,
}

struct CtrlClient {
    id: CtrlClientId,
    tx: CtrlSender,
    transport: CtrlTransport,
    /// Whether this peer advertised `abs-queue` in its Hello. Gates whether
    /// it receives or may submit `QueueItem::Audiobookshelf` values.
    supports_abs_queue: bool,
    /// Whether this peer advertised `abs-progress` in its Hello. Gates
    /// whether it receives the redacted Audiobookshelf progress event.
    supports_abs_progress: bool,
}

type ClientRegistry = Arc<Mutex<CtrlClients>>;

struct CtrlRequest<'a> {
    reply_tx: &'a CtrlSender,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PlaybackIntentPhase {
    Accepted,
    Resolving,
    PlayerOpening,
    OutputBuffering,
    Applied,
}

#[derive(Clone, Debug, PartialEq)]
struct CurrentPlaybackIntent {
    connection_id: CtrlClientId,
    request_id: PlaybackRequestId,
    generation: PlaybackGeneration,
    action: PlaybackIntentAction,
    phase: PlaybackIntentPhase,
    /// The index this intent is expected to land on, set when the action is
    /// executed (e.g. the `next_idx()` computed at command time for `Next`).
    /// `None` until the command is actually dispatched; used by the
    /// `TrackChanged` handler to avoid crediting natural track transitions
    /// (end-of-file auto-advance) to a pending Next/Previous intent.
    target_idx: Option<usize>,
    accepted_at: Instant,
    pipe_output: bool,
    buffering_deadline: Option<Instant>,
}

/// Daemon-owned lifecycle state for the guarded direct-playback protocol.
/// Keeping this separate from the player status prevents queued commands and
/// early active flags from being mistaken for confirmed playback state.
#[derive(Default)]
struct PlaybackIntentState {
    current: Option<CurrentPlaybackIntent>,
}

impl PlaybackIntentState {
    fn is_current(
        &self,
        connection_id: CtrlClientId,
        request_id: PlaybackRequestId,
        generation: PlaybackGeneration,
    ) -> bool {
        self.current.as_ref().is_some_and(|current| {
            current.connection_id == connection_id
                && current.request_id == request_id
                && current.generation == generation
        })
    }

    fn accept(
        &mut self,
        connection_id: CtrlClientId,
        intent: PlaybackIntent,
        pipe_output: bool,
    ) -> Vec<PlaybackIntentEvent> {
        if let Some(current) = &self.current {
            let unresolved = current.phase != PlaybackIntentPhase::Applied;
            if unresolved && current.connection_id == connection_id {
                let equivalent = current.action == intent.action
                    || matches!(
                        (&current.action, &intent.action),
                        (PlaybackIntentAction::Next, PlaybackIntentAction::Next)
                            | (
                                PlaybackIntentAction::Previous,
                                PlaybackIntentAction::Previous
                            )
                    );
                if equivalent {
                    return vec![PlaybackIntentEvent {
                        request_id: intent.request_id,
                        generation: intent.generation,
                        outcome: PlaybackIntentOutcome::Coalesced {
                            canonical_request_id: current.request_id,
                        },
                    }];
                }
                if matches!(intent.action, PlaybackIntentAction::Stop)
                    || matches!(
                        (&current.action, &intent.action),
                        (
                            PlaybackIntentAction::Play { .. },
                            PlaybackIntentAction::Play { .. }
                        )
                    )
                {
                    let superseded = PlaybackIntentEvent {
                        request_id: current.request_id,
                        generation: current.generation,
                        outcome: PlaybackIntentOutcome::Superseded,
                    };
                    self.current = None;
                    let accepted = self.accept(connection_id, intent, pipe_output);
                    let mut events = vec![superseded];
                    events.extend(accepted);
                    return events;
                }
            }
        }
        let event = PlaybackIntentEvent {
            request_id: intent.request_id,
            generation: intent.generation,
            outcome: PlaybackIntentOutcome::Accepted,
        };
        self.current = Some(CurrentPlaybackIntent {
            connection_id,
            request_id: intent.request_id,
            generation: intent.generation,
            action: intent.action,
            phase: PlaybackIntentPhase::Accepted,
            target_idx: None,
            accepted_at: Instant::now(),
            pipe_output,
            buffering_deadline: None,
        });
        vec![event]
    }

    fn mark_resolving(&mut self, request_id: PlaybackRequestId) {
        if let Some(current) = &mut self.current {
            if current.request_id == request_id {
                current.phase = PlaybackIntentPhase::Resolving;
            }
        }
    }

    fn mark_starting(&mut self, request_id: PlaybackRequestId) {
        if let Some(current) = &mut self.current {
            if current.request_id == request_id {
                current.phase = PlaybackIntentPhase::PlayerOpening;
            }
        }
    }

    fn pipe_status(&self) -> Option<crate::ctrl::PipePlaybackStatus> {
        use crate::ctrl::PipePlaybackPhase;
        let current = self.current.as_ref()?;
        if !current.pipe_output {
            return None;
        }
        let (phase, estimated_remaining_ms) = match current.phase {
            PlaybackIntentPhase::Resolving => (PipePlaybackPhase::Resolving, None),
            PlaybackIntentPhase::PlayerOpening => (PipePlaybackPhase::PlayerOpening, None),
            PlaybackIntentPhase::OutputBuffering => (
                PipePlaybackPhase::OutputBuffering,
                current.buffering_deadline.map(|deadline| {
                    deadline
                        .saturating_duration_since(Instant::now())
                        .as_millis()
                        .try_into()
                        .unwrap_or(u64::MAX)
                }),
            ),
            PlaybackIntentPhase::Accepted | PlaybackIntentPhase::Applied => return None,
        };
        Some(crate::ctrl::PipePlaybackStatus {
            request_id: current.request_id,
            generation: current.generation,
            phase,
            estimated_remaining_ms,
        })
    }

    fn output_started_if_current(
        &mut self,
        delay: Option<Duration>,
    ) -> Option<(CtrlClientId, crate::ctrl::PipePlaybackStatus)> {
        let current = self.current.as_mut()?;
        if !current.pipe_output || !matches!(current.action, PlaybackIntentAction::Play { .. }) {
            return None;
        }
        let (phase, estimated_remaining_ms) = match delay {
            Some(delay) => {
                current.phase = PlaybackIntentPhase::OutputBuffering;
                current.buffering_deadline = Some(Instant::now() + delay);
                (
                    crate::ctrl::PipePlaybackPhase::OutputBuffering,
                    Some(delay.as_millis().try_into().unwrap_or(u64::MAX)),
                )
            }
            None => {
                current.phase = PlaybackIntentPhase::Applied;
                (crate::ctrl::PipePlaybackPhase::OutputStarted, None)
            }
        };
        Some((
            current.connection_id,
            crate::ctrl::PipePlaybackStatus {
                request_id: current.request_id,
                generation: current.generation,
                phase,
                estimated_remaining_ms,
            },
        ))
    }

    fn settle_buffering_if_due(&mut self) -> Option<(CtrlClientId, PlaybackIntentEvent)> {
        let current = self.current.as_mut()?;
        if current.phase != PlaybackIntentPhase::OutputBuffering
            || current
                .buffering_deadline
                .is_none_or(|deadline| deadline > Instant::now())
        {
            return None;
        }
        current.phase = PlaybackIntentPhase::Applied;
        current.buffering_deadline = None;
        Some((
            current.connection_id,
            PlaybackIntentEvent {
                request_id: current.request_id,
                generation: current.generation,
                outcome: PlaybackIntentOutcome::Applied,
            },
        ))
    }

    fn set_target_idx(&mut self, request_id: PlaybackRequestId, idx: usize) {
        if let Some(current) = &mut self.current {
            if current.request_id == request_id {
                current.target_idx = Some(idx);
            }
        }
    }

    fn applied_if_current(
        &mut self,
        connection_id: CtrlClientId,
        request_id: PlaybackRequestId,
        generation: PlaybackGeneration,
    ) -> Option<PlaybackIntentEvent> {
        let current = self.current.as_mut()?;
        if current.connection_id != connection_id
            || current.request_id != request_id
            || current.generation != generation
        {
            return None;
        }
        current.phase = PlaybackIntentPhase::Applied;
        Some(PlaybackIntentEvent {
            request_id,
            generation,
            outcome: PlaybackIntentOutcome::Applied,
        })
    }

    fn rejected_if_current(
        &mut self,
        connection_id: CtrlClientId,
        request_id: PlaybackRequestId,
        generation: PlaybackGeneration,
        reason: crate::ctrl::PlaybackIntentRejection,
    ) -> Option<PlaybackIntentEvent> {
        let current = self.current.as_ref()?;
        if current.connection_id != connection_id
            || current.request_id != request_id
            || current.generation != generation
        {
            return None;
        }
        self.current = None;
        Some(PlaybackIntentEvent {
            request_id,
            generation,
            outcome: PlaybackIntentOutcome::Rejected { reason },
        })
    }

    fn invalidate_connection(&mut self, connection_id: CtrlClientId) {
        if self
            .current
            .as_ref()
            .is_some_and(|current| current.connection_id == connection_id)
        {
            self.current = None;
        }
    }
}

/// Snapshot of the daemon's canonical queue used to seed newly-connecting
/// ctrl-socket clients.  The queue itself is the single source of truth;
/// `UnifiedQueueState` is derived from it at the broadcast boundary.
#[derive(Clone)]
struct SharedQueueState {
    queue: Arc<Mutex<PlaybackQueue>>,
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
    clients.lock().unwrap().broadcast_to_all(json);
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

/// Fans out redacted Audiobookshelf progress to peers that negotiated
/// `abs-progress`. Called from the daemon event loop after the acknowledged
/// update has been applied to the canonical Bound queue.
fn broadcast_audiobookshelf_progress(clients: &ClientRegistry, event: AudiobookshelfProgressEvent) {
    let Some(json) = serialize_ctrl_event(&CtrlEvent::AudiobookshelfProgress(event)) else {
        return;
    };
    clients.lock().unwrap().broadcast_progress_gated(json);
}

impl CtrlClients {
    /// Append `tx` as a new ctrl connection. Multiple clients may coexist.
    /// Does NOT override authority if it is currently `EmbyRemote` — the new
    /// client receives broadcasts but its commands are rejected until
    /// authority returns to `Ctrl`.
    fn connect(
        &mut self,
        tx: CtrlSender,
        transport: CtrlTransport,
        supports_abs_queue: bool,
        supports_abs_progress: bool,
    ) -> CtrlClientId {
        let id = self.next_id;
        self.next_id += 1;
        self.connection.push(CtrlClient {
            id,
            tx,
            transport,
            supports_abs_queue,
            supports_abs_progress,
        });
        if self.authority == AuthorityHolder::None {
            self.authority = AuthorityHolder::Ctrl;
        }
        id
    }

    fn remove(&mut self, id: CtrlClientId) {
        self.connection.retain(|c| c.id != id);
        if self.connection.is_empty() && self.authority == AuthorityHolder::Ctrl {
            self.authority = AuthorityHolder::None;
        }
    }

    fn has_client(&self, id: CtrlClientId) -> bool {
        self.connection.iter().any(|c| c.id == id)
    }

    fn is_local_client(&self, id: CtrlClientId) -> bool {
        self.connection
            .iter()
            .any(|c| c.id == id && c.transport == CtrlTransport::Local)
    }

    fn transport(&self, id: CtrlClientId) -> Option<CtrlTransport> {
        self.connection
            .iter()
            .find(|client| client.id == id)
            .map(|client| client.transport)
    }

    /// Whether the client `id` advertised `abs-queue` support at Hello.
    /// Used to gate Audiobookshelf `QueueItem` transport in both directions.
    fn supports_abs_queue(&self, id: CtrlClientId) -> bool {
        self.connection
            .iter()
            .find(|c| c.id == id)
            .is_some_and(|c| c.supports_abs_queue)
    }

    /// Whether the client `id` advertised `abs-progress` support at Hello.
    /// Used to gate delivery of the redacted Audiobookshelf progress event.
    #[allow(dead_code)]
    fn supports_abs_progress(&self, id: CtrlClientId) -> bool {
        self.connection
            .iter()
            .find(|c| c.id == id)
            .is_some_and(|c| c.supports_abs_progress)
    }

    fn send_to_client(&self, id: CtrlClientId, event: &CtrlEvent) {
        if let Some(client) = self.connection.iter().find(|client| client.id == id) {
            send_to(&client.tx, event);
        }
    }

    fn has_driver(&self) -> bool {
        !self.connection.is_empty()
    }

    /// Broadcast `json` to all connected ctrl clients. Removes any client
    /// whose channel has failed (broken pipe / disconnected).
    fn broadcast_to_all(&mut self, json: String) {
        self.connection
            .retain(|c| c.tx.send(CtrlOutbound::Event(json.clone())).is_ok());
    }

    /// Broadcasts a state event gated per client:
    /// - `unified_abs_json` → peers advertising `abs-queue`
    /// - `unified_json` → all others
    ///
    /// Mirrors `broadcast_to_all`'s drop-on-failed-send behavior.
    fn broadcast_state_gated(&mut self, unified_abs_json: String, unified_json: String) {
        self.connection.retain(|c| {
            let json = if c.supports_abs_queue {
                &unified_abs_json
            } else {
                &unified_json
            };
            c.tx.send(CtrlOutbound::Event(json.clone())).is_ok()
        });
    }

    /// Sends redacted Audiobookshelf progress `json` only to clients that
    /// advertised `abs-progress` at Hello. Peers lacking the capability are
    /// silently skipped (not dropped) — mirrors `broadcast_state_gated`'s
    /// drop-on-failed-send semantics for the peers that do receive it.
    fn broadcast_progress_gated(&mut self, json: String) {
        self.connection.retain(|c| {
            if !c.supports_abs_progress {
                return true;
            }
            c.tx.send(CtrlOutbound::Event(json.clone())).is_ok()
        });
    }

    /// Broadcast a `Disconnected` notification to all connected ctrl clients
    /// without closing their connections. Used for Emby remote authority
    /// transitions — clients observe the authority change but stay connected.
    fn notify_disconnected_all(&self, reason: DisconnectReason) {
        for client in &self.connection {
            send_to(&client.tx, &CtrlEvent::Disconnected { reason });
        }
    }

    /// Wait until every currently connected writer has drained the events
    /// queued before its barrier. A single deadline bounds the whole client
    /// set; a broken or non-reading socket must not hold daemon shutdown
    /// forever.
    fn flush_writers(&self, timeout: Duration) {
        let acks: Vec<_> = self
            .connection
            .iter()
            .filter_map(|client| {
                let (ack_tx, ack_rx) = mpsc::channel();
                client
                    .tx
                    .send(CtrlOutbound::Flush(ack_tx))
                    .ok()
                    .map(|_| ack_rx)
            })
            .collect();
        let deadline = Instant::now() + timeout;
        for ack_rx in acks {
            let remaining = deadline.saturating_duration_since(Instant::now());
            let _ = ack_rx.recv_timeout(remaining);
        }
    }

    fn take_authority_for_emby_remote(&mut self) {
        self.notify_disconnected_all(DisconnectReason::TakenOverByEmbyRemote);
        self.authority = AuthorityHolder::EmbyRemote;
    }
}

fn take_authority_for_emby_remote(ctrl_clients: &ClientRegistry) {
    ctrl_clients
        .lock()
        .unwrap()
        .take_authority_for_emby_remote();
}

/// A reason a ctrl-socket command is not acted on, computed server-side.
/// Currently the only case is audio-only mode rejecting a non-audio play
/// request; kept as a small pure function so it's testable without a live
/// `Player`/`EmbyClient`. Returns the bare reason (not a `CtrlEvent`) so the
/// same string can be reused for both the server-side log line and the wire
/// event the caller sends — one message, not two that can drift apart.
fn audio_only_rejection(audio_only: bool, fetched: &[QueueItem]) -> Option<String> {
    if audio_only && !all_audio(fetched) {
        Some("Daemon is running in audio-only mode; can't play video items".to_string())
    } else {
        None
    }
}

include!("daemon_core_ctrl_spawn.rs");
