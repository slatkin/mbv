use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::os::unix::net::UnixListener;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::api::{mbv_direct_tcp_port_command, EmbyClient, EmbyItem};
use crate::daemon_ctrl::{
    serialize_ctrl_event, send_to, take_authority_for_emby_remote, AuthorityHolder,
    ClientRegistry, CtrlClientId, CtrlClients, CtrlOutbound, CtrlRequest, CtrlSender,
};
pub use crate::daemon_ctrl::CtrlTransport;
use crate::ctrl::{
    AudiobookshelfBookProgressEvent, AudiobookshelfProgressEvent, CtrlCmd, CtrlEvent, CtrlHello,
    DisconnectReason, PlaybackGeneration, PlaybackIntent, PlaybackIntentAction,
    PlaybackIntentEvent, PlaybackIntentOutcome, PlaybackRequestId,
};
use crate::playback_queue::{PlaybackQueue, QueueItem, QueueSlotId};
use crate::player::{Player, PlayerCommand, PlayerEvent};
use crate::stream::SocketStream;
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
    /// Book-shaped counterpart to `AudiobookshelfProgress`, keyed by
    /// `library_item_id` only.
    AudiobookshelfBookProgress(crate::player::AudiobookshelfBookProgressUpdate),
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

use crate::player_owner_state::PlayerOwnerState;

/// The daemon's Player owner: the reusable [`PlayerOwnerState`] core plus the
/// daemon-only guarded direct-playback lifecycle coordinator. The daemon event
/// loop owns exactly one of these.
#[derive(Default)]
struct DaemonPlayerOwner {
    core: PlayerOwnerState,
    /// Guarded direct-playback lifecycle coordinator. Retained functionally
    /// as-is (task 3.2 folds its single `current` into the core `transitions`);
    /// kept here so the event loop owns one struct. Meaningless in Bare mode,
    /// so it stays daemon-side.
    intents: PlaybackIntentState,
    /// Origin client of the transition currently in `core.transitions
    /// .queued_latest` (there is only ever one). Routes the `Superseded` event
    /// when it is displaced. task 3.5 folds transition/intent identity tracking
    /// together.
    queued_transition_origin: Option<(PlaybackRequestId, CtrlClientId)>,
}

/// Route one slot-jump transition through the owner's one-in-flight dispatch
/// gate (design D4): dispatch it now, or hold it behind the in-flight one and
/// report `Superseded` for whatever queued transition it displaced.
#[allow(clippy::too_many_arguments)]
fn dispatch_slot_jump(
    transitions: &mut crate::playback_transition::OwnerTransitionState,
    queued_origin: &mut Option<(PlaybackRequestId, CtrlClientId)>,
    ctrl_clients: &ClientRegistry,
    player: &Player,
    shared_queue: &SharedQueueState,
    queue: &PlaybackQueue,
    source: &crate::config::QueueSource,
    client_id: CtrlClientId,
    transition: crate::playback_transition::Transition,
) {
    match transitions.accept(transition) {
        crate::playback_transition::DispatchDecision::DispatchNow(t) => {
            player.send_command(t.into_jump());
        }
        crate::playback_transition::DispatchDecision::Queued { superseded } => {
            if let (Some(s), Some((origin_request_id, origin_client))) = (superseded, *queued_origin)
            {
                if origin_request_id == s.request_id {
                    ctrl_clients.lock().unwrap().send_to_client(
                        origin_client,
                        &CtrlEvent::PlaybackIntent(PlaybackIntentEvent {
                            request_id: s.request_id,
                            generation: s.generation,
                            outcome: PlaybackIntentOutcome::Superseded,
                        }),
                    );
                }
            }
            *queued_origin = Some((transition.request_id, client_id));
        }
    }
    // Accepting a transition mutates desired playback state (in_flight /
    // queued_latest); publish the coherent snapshot so Clients can render the
    // pending slot before it settles (task 4.1, design D5).
    broadcast_queue_state(ctrl_clients, player, shared_queue, queue, source, transitions);
}

/// Drop any in-flight/queued transition: the caller is issuing a
/// queue-replacing playback command, which deliberately interrupts them.
fn reset_slot_jumps(
    transitions: &mut crate::playback_transition::OwnerTransitionState,
    queued_origin: &mut Option<(PlaybackRequestId, CtrlClientId)>,
) {
    transitions.reset();
    *queued_origin = None;
}

/// Settle the in-flight transition against a Playback-run observation and, if
/// a newer transition was queued behind it, dispatch that one now (task 3.3).
fn settle_and_redispatch(
    owner: &mut DaemonPlayerOwner,
    player: &Player,
    observed_request_id: PlaybackRequestId,
    observed_slot: QueueSlotId,
) {
    let crate::playback_transition::SettleOutcome::Settled { dispatch_next } = owner
        .core
        .transitions
        .settle(observed_request_id, observed_slot)
    else {
        return;
    };
    owner.queued_transition_origin = None;
    if let Some(next) = dispatch_next {
        player.send_command(next.into_jump());
    }
}

/// Bounded in-flight failure handling (task 3.4): if the Playback run has not
/// confirmed the in-flight transition before its deadline, abandon it, report a
/// timeout to its origin ctrl client, and dispatch whatever was queued behind
/// it — rebuilding the execution projection from `owner.core.queue` exactly as
/// [`settle_and_redispatch`] does.
fn expire_and_redispatch(
    owner: &mut DaemonPlayerOwner,
    player: &Player,
    ctrl_clients: &ClientRegistry,
    shared_queue: &SharedQueueState,
) {
    let crate::playback_transition::ExpireOutcome::Expired {
        expired,
        dispatch_next,
    } = owner.core.transitions.expire(Instant::now())
    else {
        return;
    };
    owner.queued_transition_origin = None;
    // Emit a timeout to the abandoned request's origin when it is the current
    // guarded intent (Next/Previous). Owner-minted jumps carry no ctrl origin
    // and need no event.
    if let Some((connection_id, request_id, generation)) = owner
        .intents
        .current
        .as_ref()
        .filter(|current| current.request_id == expired.request_id)
        .map(|current| (current.connection_id, current.request_id, current.generation))
    {
        if let Some(event) = owner.intents.rejected_if_current(
            connection_id,
            request_id,
            generation,
            crate::ctrl::PlaybackIntentRejection::Unavailable,
        ) {
            ctrl_clients
                .lock()
                .unwrap()
                .send_to_client(connection_id, &CtrlEvent::PlaybackIntent(event));
        }
    }
    if let Some(next) = dispatch_next {
        player.send_command(next.into_jump());
    }
    // Abandoning / promoting a transition changed desired state; republish.
    broadcast_queue_state(
        ctrl_clients,
        player,
        shared_queue,
        &owner.core.queue,
        &owner.core.source,
        &owner.core.transitions,
    );
}

/// Snapshot of the daemon's canonical queue used to seed newly-connecting
/// ctrl-socket clients.  The queue itself is the single source of truth;
/// `UnifiedQueueState` is derived from it at the broadcast boundary.
#[derive(Clone)]
struct SharedQueueState {
    queue: Arc<Mutex<PlaybackQueue>>,
    source: Arc<Mutex<crate::config::QueueSource>>,
    observed_active_slot: Arc<Mutex<Option<QueueSlotId>>>,
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


/// Fans out redacted Audiobookshelf progress to peers that negotiated
/// `abs-progress`. Called from the daemon event loop after the acknowledged
/// update has been applied to the canonical Bound queue.
fn broadcast_audiobookshelf_progress(clients: &ClientRegistry, event: AudiobookshelfProgressEvent) {
    let Some(json) = serialize_ctrl_event(&CtrlEvent::AudiobookshelfProgress(event)) else {
        return;
    };
    clients.lock().unwrap().broadcast_progress_gated(json);
}

/// Fans out redacted Audiobookshelf book progress to peers that negotiated
/// `abs-book-progress`.
fn broadcast_audiobookshelf_book_progress(
    clients: &ClientRegistry,
    event: AudiobookshelfBookProgressEvent,
) {
    let Some(json) = serialize_ctrl_event(&CtrlEvent::AudiobookshelfBookProgress(event)) else {
        return;
    };
    clients.lock().unwrap().broadcast_book_progress_gated(json);
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
