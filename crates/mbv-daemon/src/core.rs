use std::os::unix::net::UnixListener;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::control_queue::broadcast_queue_state;
use super::ws::all_audio;
use crate::ctrl::{ClientRegistry, CtrlClientId, CtrlSender, serialize_ctrl_event};
use mbv_ctrl::player::PlayerEvent;
use mbv_ctrl::{
    AudiobookshelfBookProgressEvent, AudiobookshelfProgressEvent, CtrlCmd, CtrlEvent,
    PlaybackGeneration, PlaybackIntent, PlaybackIntentAction, PlaybackIntentEvent,
    PlaybackIntentOutcome, PlaybackRequestId,
};
use mbv_emby::EmbyClient;
use mbv_emby_model::EmbyItem;
use mbv_player::Player;
use mbv_queue::{PlaybackQueue, QueueItem, QueueSlotId};
use mbv_ws::WsEvent;

pub(super) fn bind_ctrl_listener() -> Option<UnixListener> {
    let path = mbv_config::control_socket_path();
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
            tracing::error!(name: "daemon.ctrl_socket_bind.failed", target: "daemon", error = %e, "ctrl socket bind failed; remote TUI unavailable");
            None
        }
    }
}

pub(super) enum QueuePersistenceRequest {
    Save(mbv_config::StayAliveQueueState),
    Flush(mpsc::SyncSender<()>),
}

pub(super) enum DaemonEvent {
    Player(PlayerEvent),
    Transport(mbv_ctrl::TransportCommand),
    Ws {
        generation: mbv_core::service_runtime::SetupGeneration,
        event: WsEvent,
    },
    /// Acknowledged Audiobookshelf progress from the daemon player's
    /// progress sender, routed here so the event loop can update the Bound
    /// queue and broadcast redacted progress to capable clients.
    AudiobookshelfProgress(mbv_player::AudiobookshelfProgressUpdate),
    /// Book-shaped counterpart to `AudiobookshelfProgress`, keyed by
    /// `library_item_id` only.
    AudiobookshelfBookProgress(mbv_player::AudiobookshelfBookProgressUpdate),
    /// Carries freshly fetched Emby progress for a queue adopted from a
    /// persisted snapshot back to the daemon event loop.
    QueueEnriched(Vec<(QueueSlotId, EmbyItem)>),
    QueuePersistenceFailed(String),
    /// Carries the requesting client's own event sender alongside the
    /// command, so a rejection (see #90) can be replied to that one client
    /// instead of broadcast to every connected TUI.
    Ctrl(CtrlCmd, CtrlClientId, CtrlSender),
    PlaybackResolved {
        start_idx: usize,
        start_ticks: i64,
        source: mbv_queue::QueueSource,
        client_id: CtrlClientId,
        request_id: PlaybackRequestId,
        generation: PlaybackGeneration,
        fetched: Result<Vec<EmbyItem>, crate::DaemonLibError>,
    },
    CtrlDisconnected(CtrlClientId),
    LastClientGone,
    Shutdown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PlaybackIntentPhase {
    Accepted,
    Resolving,
    PlayerOpening,
    OutputBuffering,
    Applied,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct CurrentPlaybackIntent {
    pub(super) connection_id: CtrlClientId,
    pub(super) request_id: PlaybackRequestId,
    pub(super) generation: PlaybackGeneration,
    pub(super) action: PlaybackIntentAction,
    pub(super) phase: PlaybackIntentPhase,
    pub(super) accepted_at: Instant,
    pub(super) pipe_output: bool,
    pub(super) buffering_deadline: Option<Instant>,
}

/// Daemon-owned lifecycle state for the guarded direct-playback protocol.
/// Keeping this separate from the player status prevents queued commands and
/// early active flags from being mistaken for confirmed playback state.
#[derive(Default)]
pub(super) struct PlaybackIntentState {
    pub(super) current: Option<CurrentPlaybackIntent>,
}

impl PlaybackIntentState {
    pub(super) fn is_current(
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

    pub(super) fn accept(
        &mut self,
        connection_id: CtrlClientId,
        intent: PlaybackIntent,
        pipe_output: bool,
    ) -> Vec<PlaybackIntentEvent> {
        // A relative step (Next/Previous) has no resolve/open/buffer
        // lifecycle, so it must never latch as `current` — the only code
        // that settles `current` to Applied handles Play/SetPaused/Stop, so
        // a latched Next would sit Accepted forever and swallow every later
        // press from the connection. Dedup/ordering for relative steps is
        // owned by the owner's transition state (in-flight/queued).
        if matches!(
            intent.action,
            PlaybackIntentAction::Next | PlaybackIntentAction::Previous
        ) {
            return vec![PlaybackIntentEvent {
                request_id: intent.request_id,
                generation: intent.generation,
                outcome: PlaybackIntentOutcome::Accepted,
            }];
        }
        if let Some(current) = &self.current {
            let unresolved = current.phase != PlaybackIntentPhase::Applied;
            if unresolved && current.connection_id == connection_id {
                let equivalent = current.action == intent.action;
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

    pub(super) fn mark_resolving(&mut self, request_id: PlaybackRequestId) {
        if let Some(current) = &mut self.current
            && current.request_id == request_id
        {
            current.phase = PlaybackIntentPhase::Resolving;
        }
    }

    pub(super) fn mark_starting(&mut self, request_id: PlaybackRequestId) {
        if let Some(current) = &mut self.current
            && current.request_id == request_id
        {
            current.phase = PlaybackIntentPhase::PlayerOpening;
        }
    }

    pub(super) fn pipe_status(&self) -> Option<mbv_ctrl::PipePlaybackStatus> {
        use mbv_ctrl::PipePlaybackPhase;
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
        Some(mbv_ctrl::PipePlaybackStatus {
            request_id: current.request_id,
            generation: current.generation,
            phase,
            estimated_remaining_ms,
        })
    }

    pub(super) fn output_started_if_current(
        &mut self,
        delay: Option<Duration>,
    ) -> Option<(CtrlClientId, mbv_ctrl::PipePlaybackStatus)> {
        let current = self.current.as_mut()?;
        if !current.pipe_output || !matches!(current.action, PlaybackIntentAction::Play { .. }) {
            return None;
        }
        let (phase, estimated_remaining_ms) = if let Some(delay) = delay {
            current.phase = PlaybackIntentPhase::OutputBuffering;
            current.buffering_deadline = Some(Instant::now() + delay);
            (
                mbv_ctrl::PipePlaybackPhase::OutputBuffering,
                Some(delay.as_millis().try_into().unwrap_or(u64::MAX)),
            )
        } else {
            current.phase = PlaybackIntentPhase::Applied;
            (mbv_ctrl::PipePlaybackPhase::OutputStarted, None)
        };
        Some((
            current.connection_id,
            mbv_ctrl::PipePlaybackStatus {
                request_id: current.request_id,
                generation: current.generation,
                phase,
                estimated_remaining_ms,
            },
        ))
    }

    pub(super) fn settle_buffering_if_due(
        &mut self,
    ) -> Option<(CtrlClientId, PlaybackIntentEvent)> {
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

    pub(super) fn applied_if_current(
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

    pub(super) fn rejected_if_current(
        &mut self,
        connection_id: CtrlClientId,
        request_id: PlaybackRequestId,
        generation: PlaybackGeneration,
        reason: mbv_ctrl::PlaybackIntentRejection,
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

    pub(super) fn invalidate_connection(&mut self, connection_id: CtrlClientId) {
        if self
            .current
            .as_ref()
            .is_some_and(|current| current.connection_id == connection_id)
        {
            self.current = None;
        }
    }
}

use mbv_player::PlayerOwnerState;

/// The daemon's Player owner: the reusable [`PlayerOwnerState`] core plus the
/// daemon-only guarded direct-playback lifecycle coordinator. The daemon event
/// loop owns exactly one of these.
#[derive(Default)]
pub(super) struct DaemonPlayerOwner {
    pub(super) core: PlayerOwnerState,
    pub(super) pending_idle_load: Option<PendingIdleQueueLoad>,
    /// Guarded direct-playback lifecycle coordinator. Retained functionally
    /// as-is (task 3.2 folds its single `current` into the core `transitions`);
    /// kept here so the event loop owns one struct. Meaningless in Bare mode,
    /// so it stays daemon-side.
    pub(super) intents: PlaybackIntentState,
    /// Origin client of the transition currently in `core.transitions
    /// .queued_latest` (there is only ever one). Routes the `Superseded` event
    /// when it is displaced. task 3.5 folds transition/intent identity tracking
    /// together.
    pub(super) queued_transition_origin: Option<(PlaybackRequestId, CtrlClientId)>,
}

pub(crate) struct PendingIdleQueueLoad {
    pub(super) client_id: CtrlClientId,
    pub(super) request_id: mbv_ctrl::QueueLoadRequestId,
    pub(super) slots: Vec<(QueueSlotId, QueueItem)>,
    pub(super) cursor: usize,
    pub(super) source: mbv_queue::QueueSource,
    pub(super) reply_tx: CtrlSender,
    pub(super) stopped_run: mbv_ctrl::PlaybackGeneration,
    pub(super) started_at: Instant,
}

/// The daemon's owner-side playback authority: everything the paths that
/// read or mutate the Bound queue, playback transitions, or the ctrl client
/// registry (`dispatch_slot_jump`, `play_resolved_items`, the packaged
/// service reconciles) take whole instead of repeating the same fields as a
/// positional argument list. Many of the fields share types (`&mut` into
/// owner state), so two of them could silently swap positionally. Built per
/// call: handlers and the event loop hold disjoint borrows into `owner`, so
/// a longer-lived context cannot coexist with them.
pub(super) struct DaemonOwnerContext<'a> {
    pub(super) player: &'a Player,
    pub(super) client: &'a Arc<Mutex<EmbyClient>>,
    pub(super) owner: &'a mut DaemonPlayerOwner,
    pub(super) shared_queue: &'a SharedQueueState,
    pub(super) ctrl_clients: &'a ClientRegistry,
}

/// Route one slot-jump transition through the owner's one-in-flight dispatch
/// gate (design D4): dispatch it now, or hold it behind the in-flight one and
/// report `Superseded` for whatever queued transition it displaced.
#[derive(Clone, Copy)]
pub(super) enum JumpOrigin {
    Ctrl(CtrlClientId),
    Transport,
}

pub(super) fn dispatch_slot_jump(
    ctx: &mut DaemonOwnerContext<'_>,
    origin: JumpOrigin,
    transition: mbv_player::transition::Transition,
    except: Option<CtrlClientId>,
) {
    let DaemonPlayerOwner {
        core:
            PlayerOwnerState {
                queue,
                source,
                transitions,
                ..
            },
        queued_transition_origin: queued_origin,
        ..
    } = &mut *ctx.owner;
    let transition_target = transition.target;
    let transition_request_id = transition.request_id;
    let transition_generation = transition.generation;
    match transitions.accept(transition) {
        mbv_player::transition::DispatchDecision::DispatchNow(t) => {
            tracing::info!(name: "daemon.transition_dispatch.started", target: "transition", target_slot = ?transition_target, request = %transition_request_id, generation = %transition_generation, "slot jump dispatched");
            let resume_ticks = mbv_player::resume_ticks_for_slot(queue, transition_target);
            ctx.player.send_command(t.into_jump(resume_ticks));
        }
        mbv_player::transition::DispatchDecision::Queued { superseded } => {
            tracing::info!(name: "daemon.transition_dispatch.queued", target: "transition", target_slot = ?transition_target, request = %transition_request_id, generation = %transition_generation, "slot jump queued");
            if let Some(s) = superseded
                && let Some((origin_request_id, origin_client)) = *queued_origin
                && origin_request_id == s.request_id
            {
                ctx.ctrl_clients.lock().unwrap().send_to_client(
                    origin_client,
                    &CtrlEvent::PlaybackIntent(PlaybackIntentEvent {
                        request_id: s.request_id,
                        generation: s.generation,
                        outcome: PlaybackIntentOutcome::Superseded,
                    }),
                );
            }
            // Transport senders (MPRIS/tray/Emby-ws) have no ctrl wire form and
            // so no `Superseded` event to receive; only `Ctrl` origins are worth
            // tracking here.
            *queued_origin = match origin {
                JumpOrigin::Ctrl(client_id) => Some((transition.request_id, client_id)),
                JumpOrigin::Transport => None,
            };
        }
    }
    // Accepting a transition mutates desired playback state (in_flight /
    // queued_latest); publish the coherent snapshot so Clients can render the
    // pending slot before it settles (task 4.1, design D5).
    broadcast_queue_state(
        ctx.ctrl_clients,
        ctx.player,
        ctx.shared_queue,
        queue,
        source,
        transitions,
        except,
    );
}

/// Drop any in-flight/queued transition: the caller is issuing a
/// queue-replacing playback command, which deliberately interrupts them.
pub(super) fn reset_slot_jumps(
    transitions: &mut mbv_player::transition::OwnerTransitionState,
    queued_origin: &mut Option<(PlaybackRequestId, CtrlClientId)>,
) {
    transitions.reset();
    *queued_origin = None;
}

/// Settle the in-flight transition against a Playback-run observation and, if
/// a newer transition was queued behind it, dispatch that one now (task 3.3).
pub(super) fn settle_and_redispatch(
    owner: &mut DaemonPlayerOwner,
    player: &Player,
    observed_request_id: PlaybackRequestId,
    observed_slot: QueueSlotId,
) {
    let mbv_player::transition::SettleOutcome::Settled { dispatch_next } = owner
        .core
        .transitions
        .settle(observed_request_id, observed_slot)
    else {
        tracing::info!(name: "daemon.transition_settle.completed", target: "transition", request = %observed_request_id, slot = ?observed_slot, settled = false, dispatch_next = false, "transition settled");
        return;
    };
    tracing::info!(name: "daemon.transition_settle.completed", target: "transition", request = %observed_request_id, slot = ?observed_slot, settled = true, dispatch_next = dispatch_next.is_some(), "transition settled");
    owner.queued_transition_origin = None;
    if let Some(next) = dispatch_next {
        let resume_ticks = mbv_player::resume_ticks_for_slot(&owner.core.queue, next.target);
        player.send_command(next.into_jump(resume_ticks));
    }
}

/// Bounded in-flight failure handling (task 3.4): if the Playback run has not
/// confirmed the in-flight transition before its deadline, abandon it, report a
/// timeout to its origin ctrl client, and dispatch whatever was queued behind
/// it — rebuilding the execution projection from `owner.core.queue` exactly as
/// [`settle_and_redispatch`] does.
pub(super) fn expire_and_redispatch(
    owner: &mut DaemonPlayerOwner,
    player: &Player,
    ctrl_clients: &ClientRegistry,
    shared_queue: &SharedQueueState,
) {
    let mbv_player::transition::ExpireOutcome::Expired {
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
        .map(|current| {
            (
                current.connection_id,
                current.request_id,
                current.generation,
            )
        })
        && let Some(event) = owner.intents.rejected_if_current(
            connection_id,
            request_id,
            generation,
            mbv_ctrl::PlaybackIntentRejection::Unavailable,
        )
    {
        ctrl_clients
            .lock()
            .unwrap()
            .send_to_client(connection_id, &CtrlEvent::PlaybackIntent(event));
    }
    if let Some(next) = dispatch_next {
        let resume_ticks = mbv_player::resume_ticks_for_slot(&owner.core.queue, next.target);
        player.send_command(next.into_jump(resume_ticks));
    }
    // Abandoning / promoting a transition changed desired state; republish.
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

/// Snapshot of the daemon's canonical queue used to seed newly-connecting
/// ctrl-socket clients.  The queue itself is the single source of truth;
/// `UnifiedQueueState` is derived from it at the broadcast boundary.
#[derive(Clone, Debug)]
pub(crate) struct SharedQueueState {
    pub(super) queue: Arc<Mutex<PlaybackQueue>>,
    pub(super) source: Arc<Mutex<mbv_queue::QueueSource>>,
    pub(super) lineage: Arc<Mutex<mbv_queue::QueueLineage>>,
    pub(super) observed_active_slot: Arc<Mutex<Option<QueueSlotId>>>,
}

impl SharedQueueState {
    pub(crate) fn publish_observed(&self, owner: &mbv_player::PlayerOwnerState) {
        *self.observed_active_slot.lock().unwrap() = owner.observed_active_slot();
    }
}

#[derive(Debug)]
pub struct DaemonPlayerHandle {
    pub status: Arc<Mutex<mbv_ctrl::player::PlayerStatus>>,
    pub transport_tx: mpsc::Sender<mbv_ctrl::TransportCommand>,
}

type OnPlayerReady = Box<dyn FnOnce(DaemonPlayerHandle)>;
type OnTrayReady = Box<dyn FnOnce(mpsc::SyncSender<()>) -> Option<Box<dyn Send>>>;

pub struct DaemonRuntimeHooks {
    pub on_player_ready: OnPlayerReady,
    pub on_tray_ready: OnTrayReady,
}

impl std::fmt::Debug for DaemonRuntimeHooks {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // One-shot callbacks have no Debug form; presence is structural.
        f.debug_struct("DaemonRuntimeHooks")
            .field("on_player_ready", &"<callback>")
            .field("on_tray_ready", &"<callback>")
            .finish()
    }
}

#[must_use]
pub fn pid_file() -> std::path::PathBuf {
    let dir = mbv_config::data_dir_system_or_local();
    let _ = std::fs::create_dir_all(&dir);
    dir.join("mbv.pid")
}

pub(super) fn broadcast(clients: &ClientRegistry, event: &CtrlEvent) {
    let Some(json) = serialize_ctrl_event(event) else {
        return;
    };
    clients.lock().unwrap().broadcast_to_all(&json);
}

/// Fans out redacted Audiobookshelf progress to peers that negotiated
/// `abs-progress`. Called from the daemon event loop after the acknowledged
/// update has been applied to the canonical Bound queue.
pub(super) fn broadcast_audiobookshelf_progress(
    clients: &ClientRegistry,
    event: AudiobookshelfProgressEvent,
) {
    let Some(json) = serialize_ctrl_event(&CtrlEvent::AudiobookshelfProgress(event)) else {
        return;
    };
    clients.lock().unwrap().broadcast_progress_gated(&json);
}

/// Fans out redacted Audiobookshelf book progress to peers that negotiated
/// `abs-book-progress`.
pub(super) fn broadcast_audiobookshelf_book_progress(
    clients: &ClientRegistry,
    event: AudiobookshelfBookProgressEvent,
) {
    let Some(json) = serialize_ctrl_event(&CtrlEvent::AudiobookshelfBookProgress(event)) else {
        return;
    };
    clients.lock().unwrap().broadcast_book_progress_gated(&json);
}

/// A reason a ctrl-socket command is not acted on, computed server-side.
/// Currently the only case is audio-only mode rejecting a non-audio play
/// request; kept as a small pure function so it's testable without a live
/// `Player`/`EmbyClient`. Returns the bare reason (not a `CtrlEvent`) so the
/// same string can be reused for both the server-side log line and the wire
/// event the caller sends — one message, not two that can drift apart.
pub(super) fn audio_only_rejection<'a>(
    audio_only: bool,
    fetched: impl IntoIterator<Item = &'a QueueItem>,
) -> Option<String> {
    (audio_only && !all_audio(fetched))
        .then(|| "Daemon is running in audio-only mode; can't play video items".to_string())
}
