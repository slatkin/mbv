use std::collections::HashMap;
use std::os::unix::net::UnixListener;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::ws::all_audio;
use crate::AudiobookshelfOwnerContext;
use crate::ctrl::{ClientRegistry, CtrlClientId, CtrlSender, serialize_ctrl_event};
use mbv_audiobookshelf::{AudiobookshelfBookProgress, AudiobookshelfProgress};
use mbv_ctrl::player::PlayerEvent;
use mbv_ctrl::{
    AudiobookshelfBookProgressEvent, AudiobookshelfProgressEvent, CtrlCmd, CtrlEvent,
    PlaybackGeneration, PlaybackIntent, PlaybackIntentAction, PlaybackIntentEvent,
    PlaybackIntentOutcome, PlaybackRequestId,
};
use mbv_emby::EmbyClient;
use mbv_emby_model::EmbyItem;
use mbv_player::Player;
use mbv_queue::{ExecSlot, PlaybackQueue, QueueItem, QueueSlotId};
use mbv_ws::WsEvent;

mod transition_dispatch;

pub(super) use transition_dispatch::{
    dispatch_slot_jump, expire_and_redispatch, reset_slot_jumps, send_jump_or_cold_start,
    settle_and_redispatch, take_promoted_origin,
};

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
    /// Carries one bulk Audiobookshelf progress refresh for a queue's episode
    /// and book slots back to the daemon event loop. Both maps hold only the
    /// entries for items the queue holds.
    AudiobookshelfProgressRefreshed {
        generation: mbv_core::service_runtime::SetupGeneration,
        progress: HashMap<(String, String), AudiobookshelfProgress>,
        book_progress: HashMap<String, AudiobookshelfBookProgress>,
    },
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
    /// A Pin-swap waiter thread reports the replacement process's exit
    /// status, keyed by the one-shot swap token (tray-pin-swap design D3).
    PinSwapChildExited {
        token: String,
        success: bool,
    },
    /// The ctrl admission path consumed the pending swap token: the
    /// replacement Client attached (tray-pin-swap design D4). The loop tells
    /// the machine, which sends `SwapQuit` to the replaced target if it is
    /// still attached and returns to `Idle`.
    PinSwapAdmitted {
        token: String,
    },
    LastClientGone,
    /// One Cast-output event from the Cast connector or worker (cast-owner-
    /// output design D2/D8): connect and reconnect outcomes, and status-poll
    /// results from the Cast timers on the daemon tick.
    Cast(crate::cast::CastOwnerEvent),
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
    /// The Owner's single active playback output (cast-owner-output design
    /// D1): local playback, or one Cast receiver. Starts and resets to Local.
    pub(super) output: crate::cast::PlaybackOutput,
    /// The Cast selection waiting for its worker to connect (task 3.2).
    pub(super) pending_cast_connect: Option<crate::cast::PendingCastConnect>,
    /// The Cast source resolve in flight for the output's next load (task
    /// 4.2). Its outcome re-enters through `CastOwnerEvent::Resolved` and is
    /// matched against it by slot and transition generation.
    pub(super) pending_cast_resolve: Option<crate::cast::PendingCastResolve>,
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
    /// The loop's merged-event sender, so a Cast dispatch can spawn its
    /// source resolve whose outcome re-enters as `DaemonEvent::Cast`
    /// (cast-owner-output design D4).
    pub(super) merged_tx: &'a mpsc::Sender<DaemonEvent>,
    /// The Audiobookshelf owner runtime, when connected: a Cast resolve for
    /// an episode needs its setup to open the playback session (design D6).
    pub(super) audiobookshelf: Option<&'a AudiobookshelfOwnerContext>,
}

/// Route one slot-jump transition through the owner's one-in-flight dispatch
/// gate (design D4): dispatch it now, or hold it behind the in-flight one and
/// report `Superseded` for whatever queued transition it displaced.
#[derive(Clone, Copy)]
pub(super) enum JumpOrigin {
    Ctrl(CtrlClientId),
    Transport,
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
    /// Mirror of the owner core's order mode, read by snapshots built off the
    /// shared state (cold ctrl-client seeds and queue broadcasts). The owner
    /// core stays the authority; `publish_observed` and the order-change sites
    /// keep this in sync (queue-shuffle-play-order design D5).
    pub(super) play_order: Arc<Mutex<mbv_queue::PlayOrderMode>>,
    /// Mirror of the owner's active playback output, read by the ctrl accept
    /// path to seed newly-connecting Clients that negotiated
    /// `playback-output` (cast-owner-output design D3). The owner stays the
    /// authority; the Cast handlers keep this in sync on every change.
    pub(super) playback_output: Arc<Mutex<mbv_ctrl::OutputState>>,
}

impl SharedQueueState {
    pub(crate) fn publish_observed(&self, owner: &mbv_player::PlayerOwnerState) {
        *self.observed_active_slot.lock().unwrap() = owner.observed_active_slot();
        *self.play_order.lock().unwrap() = owner.play_order().mode();
    }
}

/// Which direction a Pin swap runs in (tray-pin-swap design D5). The
/// daemon's swap state machine derives it from the registry, not from the
/// Tray label, so a stale menu cannot act on the wrong client.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SwapDirection {
    Pin,
    Unpin,
}

/// Resolves the command that starts the replacement Client (design D5).
/// `Err` carries the user-facing reason: the daemon notifies with it and the
/// swap stays idle.
pub(crate) type SwapCommandHook =
    Box<dyn Fn(SwapDirection) -> Result<std::process::Command, String> + Send>;
/// Desktop-notification hook for swap failures (design D5).
pub(crate) type NotifyHook = Box<dyn Fn(&str) + Send>;

#[derive(Clone, Debug)]
pub struct DaemonPlayerHandle {
    pub status: Arc<Mutex<mbv_ctrl::player::PlayerStatus>>,
    pub transport_tx: mpsc::Sender<mbv_ctrl::TransportCommand>,
    /// Which local Clients are attached — shared with the ctrl registry,
    /// which refreshes it on connect and disconnect (design D1). The Tray
    /// reads it to label its Pin swap item.
    pub attached_clients: Arc<mbv_ctrl::AttachedClientsCell>,
}

type OnPlayerReady = Box<dyn FnOnce(DaemonPlayerHandle)>;
pub(crate) type OnTrayReady = Box<dyn FnMut(mpsc::SyncSender<()>) -> Option<Box<dyn Send>>>;

pub struct DaemonRuntimeHooks {
    pub on_player_ready: OnPlayerReady,
    pub on_tray_ready: OnTrayReady,
    pub swap_command: SwapCommandHook,
    pub notify: NotifyHook,
    /// Cast connector for the runtime (cast-owner-output design D2): the
    /// Local daemon supplies one, `mbvd` passes `None`. `None` keeps the
    /// Owner on local playback only and hides the `playback-output`
    /// capability from the Hello.
    pub cast_connector: Option<crate::cast::CastConnector>,
}

impl std::fmt::Debug for DaemonRuntimeHooks {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // One-shot callbacks have no Debug form; presence is structural.
        f.debug_struct("DaemonRuntimeHooks")
            .field("on_player_ready", &"<callback>")
            .field("on_tray_ready", &"<callback>")
            .field("swap_command", &"<callback>")
            .field("notify", &"<callback>")
            .field("cast_connector", &self.cast_connector.is_some())
            .finish()
    }
}

/// Submit a whole canonical queue to the Player, cold-starting a playback run
/// when none is alive (`submit_queue_slots` spawns one). The one shared form of
/// the submission `handle_queue_replace`, owner reconciliation, and the
/// cold-start slot jump all use. `100` is the initial volume a freshly spawned
/// run starts at; playback paths apply mute parity separately via `SetMute`.
pub(crate) fn submit_queue_slots_cold_start(
    player: &Player,
    slots: Vec<ExecSlot>,
    start_idx: usize,
    client: &Arc<Mutex<EmbyClient>>,
) -> bool {
    let all_audio = all_audio(slots.iter().map(|slot| &slot.item));
    let client = Arc::new(client.lock().unwrap().clone());
    let headless = player.headless_for(&client, all_audio);
    player.submit_queue_slots(slots, start_idx, Some(client), headless, 100)
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
