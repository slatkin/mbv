use super::core::DaemonEvent;
use super::*;
use crate::api::{EmbyClient, EmbyItem};
use crate::ctrl::{
    CtrlCmd, CtrlEvent, DisconnectReason, PlaybackGeneration, PlaybackIntentAction,
    PlaybackIntentEvent, PlaybackIntentOutcome, PlaybackRequestId,
};
use crate::daemon::ctrl::{send_to, ClientRegistry};
use crate::id_types::ItemId;
use crate::playback_queue::{QueueItem, QueueSlotId};
use crate::player::{
    AudiobookshelfBookProgressUpdate, AudiobookshelfProgressUpdate, Player, PlayerCommand,
    PlayerEvent,
};
use crate::service_runtime::SetupGeneration;
use crate::ws::WsEvent;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LoopFlow {
    Continue,
    Shutdown,
}

/// What one per-event handler reports back to `handle_event`: whether the
/// loop keeps running, and whether the handler changed the canonical owner
/// queue (persisted once by the dispatcher, `DaemonRole::Local` only).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct EventOutcome {
    flow: LoopFlow,
    owner_queue_dirty: bool,
}

impl EventOutcome {
    /// The loop keeps running and the owner queue is unchanged.
    const CONTINUE: Self = Self {
        flow: LoopFlow::Continue,
        owner_queue_dirty: false,
    };
    /// The loop keeps running and the canonical owner queue changed.
    const DIRTY: Self = Self {
        flow: LoopFlow::Continue,
        owner_queue_dirty: true,
    };
    /// The daemon is shutting down.
    const SHUTDOWN: Self = Self {
        flow: LoopFlow::Shutdown,
        owner_queue_dirty: false,
    };
}

/// Injected owner-queue persistence hook.
pub(crate) type OwnerQueueStore =
    Box<dyn FnMut(&crate::config::StayAliveQueueState) -> Result<(), String>>;

/// Owns every local the daemon event loop reads, so one event can be handled
/// without exiting the process (`Shutdown` is returned to the caller).
pub(crate) struct DaemonLoop {
    pub(super) owner: DaemonPlayerOwner,
    pub(super) player: Player,
    pub(super) shared_queue: SharedQueueState,
    pub(super) ctrl_clients: ClientRegistry,
    pub(super) client: Arc<Mutex<EmbyClient>>,
    pub(super) emby_runtime: Option<EmbyOwnerContext>,
    pub(super) audiobookshelf_runtime: Option<AudiobookshelfOwnerContext>,
    pub(super) merged_tx: mpsc::Sender<DaemonEvent>,
    pub(super) ws_send_tx: Option<crate::ws::WsSender>,
    pub(super) direct_commands: Vec<String>,
    pub(super) stay_alive: bool,
    pub(super) role: DaemonRole,
    pub(super) audio_only: bool,
    pub(super) last_keepalive: Instant,
    pub(super) last_capabilities: Instant,
    /// Injected owner-queue persistence, so tests can record snapshots instead
    /// of writing real state files.
    pub(super) store: OwnerQueueStore,
}

impl DaemonLoop {
    /// Keepalive/capability timers, run once per loop pass.
    pub(super) fn tick(&mut self, now: Instant) {
        if self.emby_runtime.is_some() && self.last_keepalive.elapsed() >= Duration::from_secs(30) {
            if let Some(ws_send_tx) = &self.ws_send_tx {
                let _ = ws_send_tx.send_text("{\"MessageType\":\"KeepAlive\"}".to_string());
            }
            self.last_keepalive = now;
        }
        if self.emby_runtime.is_some()
            && self.last_capabilities.elapsed() >= Duration::from_secs(600)
        {
            let client = self.client.lock().unwrap().clone();
            let direct_commands = self.direct_commands.clone();
            let audio_only = self.audio_only;
            std::thread::spawn(move || {
                client.register_capabilities_with_options(&direct_commands, audio_only)
            });
            self.last_capabilities = now;
        }
    }

    /// Idle work performed when no event arrived within the poll timeout.
    pub(super) fn on_recv_timeout(&mut self, now: Instant) {
        cancel_pending_idle_queue_load_if_run_changed(&mut self.owner, &self.player);
        expire_pending_idle_queue_load(&mut self.owner, now);
        if let Some((connection_id, event)) = self.owner.intents.settle_buffering_if_due() {
            log::info!(target: "pipe_latency", "request={} generation={} outcome=settled", event.request_id, event.generation);
            let clients = self.ctrl_clients.lock().unwrap();
            if clients.has_client(connection_id) {
                clients.send_to_client(connection_id, &CtrlEvent::PlaybackIntent(event));
            } else {
                drop(clients);
                self.owner.intents.invalidate_connection(connection_id);
            }
        }
        expire_and_redispatch(
            &mut self.owner,
            &self.player,
            &self.ctrl_clients,
            &self.shared_queue,
        );
    }

    /// Processes exactly one event. Never exits the process: `Shutdown` is
    /// returned so `run_with_options` can remove the pid file and exit.
    ///
    /// The match stays exhaustive; every arm delegates to a named handler.
    /// The queue-dirty flag is collected here and the owner queue persisted
    /// once after the handler returns, instead of inline per mutation site.
    pub(super) fn handle_event(&mut self, ev: DaemonEvent) -> LoopFlow {
        let outcome = match ev {
            DaemonEvent::Player(PlayerEvent::TrackChanged {
                slot_id,
                transition,
            }) => self.handle_track_changed(slot_id, transition),
            DaemonEvent::Player(PlayerEvent::NextUpThreshold {
                series_id,
                season,
                episode,
            }) => self.handle_next_up_threshold(series_id, season, episode),
            DaemonEvent::Player(PlayerEvent::QueueNextUp { next_idx }) => {
                self.handle_queue_next_up(next_idx)
            }
            DaemonEvent::Player(PlayerEvent::OutputStarted) => self.handle_output_started(),
            DaemonEvent::Player(pe @ PlayerEvent::TrackCompleted { .. }) => {
                self.handle_track_completed(pe)
            }
            DaemonEvent::Player(pe) => self.handle_player_event(pe),
            DaemonEvent::Ws { generation, event } => self.handle_ws_event(generation, event),
            DaemonEvent::QueueEnriched(items) => self.handle_queue_enriched(items),
            DaemonEvent::AudiobookshelfProgress(update) => {
                self.handle_audiobookshelf_progress(update)
            }
            DaemonEvent::AudiobookshelfBookProgress(update) => {
                self.handle_audiobookshelf_book_progress(update)
            }
            DaemonEvent::Ctrl(cmd, client_id, reply_tx) => {
                self.handle_ctrl_event(cmd, client_id, reply_tx)
            }
            DaemonEvent::PlaybackResolved {
                start_idx,
                start_ticks,
                source,
                client_id,
                request_id,
                generation,
                fetched,
            } => self.handle_playback_resolved(
                start_idx,
                start_ticks,
                source,
                client_id,
                request_id,
                generation,
                fetched,
            ),
            DaemonEvent::CtrlDisconnected(client_id) => self.handle_ctrl_disconnected(client_id),
            DaemonEvent::Shutdown => self.handle_shutdown(),
        };

        if self.role == DaemonRole::Local && outcome.owner_queue_dirty {
            if let Err(error) = self.persist_owner_queue() {
                log::error!(target: "queue", "failed to persist Stay-alive queue: {error}");
            }
        }

        outcome.flow
    }

    /// `PlayerEvent::TrackChanged`: resolve the reported slot against the
    /// canonical queue, settle any transition it closes, and publish.
    fn handle_track_changed(
        &mut self,
        slot_id: QueueSlotId,
        transition: Option<(PlaybackRequestId, PlaybackGeneration)>,
    ) -> EventOutcome {
        // Resolve the reported slot against the canonical queue. A
        // report naming a slot the daemon no longer holds carries no
        // evidence about which surviving slot was intended, so it is
        // discarded and logged without touching canonical queue or
        // observed slot (design D6).
        let Some((observed_idx, resolved_slot_id)) = self.owner.core.observe_track_change(slot_id)
        else {
            log::warn!(
                target: "queue",
                "discarding TrackChanged for unknown slot {slot_id:?}"
            );
            return EventOutcome::CONTINUE;
        };
        broadcast(
            &self.ctrl_clients,
            &CtrlEvent::Player(PlayerEvent::TrackChanged {
                slot_id: resolved_slot_id,
                transition,
            }),
        );
        // Settle the desired transition before publishing so the
        // snapshot contains every owner change from this turn.
        if let Some((observed_request_id, _)) = transition {
            settle_and_redispatch(
                &mut self.owner,
                &self.player,
                observed_request_id,
                resolved_slot_id,
            );
        }
        *self.shared_queue.observed_active_slot.lock().unwrap() =
            self.owner.core.observed_active_slot();
        self.broadcast_owner_queue_state();
        // Settle playback intent if the reported slot matches.
        if let Some((connection_id, request_id, generation)) = self
            .owner
            .intents
            .current
            .as_ref()
            .filter(|current| match &current.action {
                PlaybackIntentAction::Play { item_ids, .. } => self
                    .owner
                    .core
                    .queue
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
                self.owner
                    .intents
                    .applied_if_current(connection_id, request_id, generation)
            {
                self.ctrl_clients
                    .lock()
                    .unwrap()
                    .send_to_client(connection_id, &CtrlEvent::PlaybackIntent(event));
            }
        }
        EventOutcome::CONTINUE
    }

    /// `PlayerEvent::NextUpThreshold`: show the Next-Up card for the item
    /// after the active slot, then relay the event.
    fn handle_next_up_threshold(
        &mut self,
        series_id: ItemId,
        season: i64,
        episode: i64,
    ) -> EventOutcome {
        let active_idx = self.owner.core.queue.active_index().unwrap_or(0);
        if let Some(slot) = self.owner.core.queue.slots().get(active_idx + 1) {
            if let Some(emby) = slot.item.as_emby() {
                self.player.send_command(PlayerCommand::NextUpShow {
                    item_id: emby.id.clone(),
                    show_title: emby.series_name.clone(),
                    ep_title: emby.name.clone(),
                    artist: emby.artist.clone(),
                });
            }
        }
        broadcast(
            &self.ctrl_clients,
            &CtrlEvent::Player(PlayerEvent::NextUpThreshold {
                series_id,
                season,
                episode,
            }),
        );
        EventOutcome::CONTINUE
    }

    /// `PlayerEvent::QueueNextUp`: show the Next-Up card for the peeked
    /// ordinal, then relay the event.
    fn handle_queue_next_up(&mut self, next_idx: usize) -> EventOutcome {
        if let Some(slot) = self.owner.core.queue.slots().get(next_idx) {
            if let Some(emby) = slot.item.as_emby() {
                self.player.send_command(PlayerCommand::NextUpShow {
                    item_id: emby.id.clone(),
                    show_title: emby.series_name.clone(),
                    ep_title: emby.name.clone(),
                    artist: emby.artist.clone(),
                });
            }
        }
        broadcast(
            &self.ctrl_clients,
            &CtrlEvent::Player(PlayerEvent::QueueNextUp { next_idx }),
        );
        EventOutcome::CONTINUE
    }

    /// `PlayerEvent::OutputStarted`: settle any current playback intent that
    /// was waiting on the pipe, relay the event, and publish a snapshot.
    fn handle_output_started(&mut self) -> EventOutcome {
        let delay = self
            .client
            .lock()
            .unwrap()
            .config
            .audio_pipe_playout_delay_ms
            .map(Duration::from_millis);
        if let Some((connection_id, status)) = self.owner.intents.output_started_if_current(delay) {
            log::info!(target: "pipe_latency", "request={} generation={} phase={:?} elapsed_ms={}", status.request_id, status.generation, status.phase, self.owner.intents.current.as_ref().map(|current| current.accepted_at.elapsed().as_millis()).unwrap_or_default());
            self.ctrl_clients
                .lock()
                .unwrap()
                .send_to_client(connection_id, &CtrlEvent::PipePlaybackStatus(status));
            if delay.is_none() {
                if let Some(current) = self.owner.intents.current.as_ref() {
                    self.ctrl_clients.lock().unwrap().send_to_client(
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
            &self.ctrl_clients,
            &CtrlEvent::Player(PlayerEvent::OutputStarted),
        );
        // A cold-started queue plays its first track with no
        // track-to-track transition, so clients never get a snapshot
        // reflecting `status.active` and the started slot. Push one
        // here so their now-playing highlight lands on the right row.
        self.broadcast_owner_queue_state();
        EventOutcome::CONTINUE
    }

    /// `PlayerEvent::TrackCompleted`: apply the completion observation, then
    /// publish the updated queue and relay the raw event.
    fn handle_track_completed(&mut self, pe: PlayerEvent) -> EventOutcome {
        let PlayerEvent::TrackCompleted {
            slot_id,
            run_identity,
            position_ticks,
            played,
            consume,
            ..
        } = pe
        else {
            return EventOutcome::CONTINUE;
        };
        let (consume_videos, consume_audio) = {
            let cfg = self.client.lock().unwrap();
            (cfg.config.consume_videos, cfg.config.consume_audio)
        };
        if !apply_track_completed_observation(
            &mut self.owner,
            &self.player,
            &self.shared_queue,
            run_identity.into(),
            slot_id,
            position_ticks,
            played,
            consume,
            consume_videos,
            consume_audio,
        ) {
            return EventOutcome::CONTINUE;
        }
        self.broadcast_owner_queue_state();
        broadcast(&self.ctrl_clients, &CtrlEvent::Player(pe));
        EventOutcome::DIRTY
    }

    /// Any other `PlayerEvent`: settle `Stopped`/`PausedChanged` intents and
    /// observations, then relay the event unless a committed replacement
    /// already published the new queue.
    fn handle_player_event(&mut self, pe: PlayerEvent) -> EventOutcome {
        let stopped = if let PlayerEvent::Stopped {
            slot_id,
            run_identity,
            position_ticks,
            played,
            error,
            ..
        } = &pe
        {
            Some((
                *slot_id,
                *run_identity,
                *position_ticks,
                *played,
                error.clone(),
            ))
        } else {
            None
        };
        let stopped_run = stopped.as_ref().map(|(_, run_identity, ..)| *run_identity);
        if stopped_run.is_some_and(|run_identity| {
            self.owner
                .pending_idle_load
                .as_ref()
                .is_some_and(|pending| pending.stopped_run != run_identity)
        }) {
            cancel_pending_idle_queue_load(
                &mut self.owner,
                "playback stopped for a different run during queue load",
            );
        }
        let pending_idle_load_matches = stopped_run.is_some_and(|run_identity| {
            self.owner
                .pending_idle_load
                .as_ref()
                .is_some_and(|pending| pending.stopped_run == run_identity)
        });
        let stopped_queue_updated =
            if let Some((slot_id, run_identity, position_ticks, played, _)) = stopped.as_ref() {
                let Some(updated) = apply_stopped_observation(
                    &mut self.owner,
                    &self.player,
                    (*run_identity).into(),
                    *slot_id,
                    *position_ticks,
                    *played,
                ) else {
                    return EventOutcome::CONTINUE;
                };
                updated
            } else {
                false
            };
        let replacement_committed = if pending_idle_load_matches {
            let (_, run_identity, _, _, error) = stopped.as_ref().unwrap();
            complete_pending_idle_queue_load(
                *run_identity,
                error.clone(),
                &mut self.owner,
                &self.player,
                &self.shared_queue,
                &self.ctrl_clients,
            ) && error.is_none()
        } else {
            false
        };
        if stopped_queue_updated && !replacement_committed {
            // Unlike TrackCompleted (which broadcasts unconditionally
            // below via the raw player event too), a full Stopped has
            // no other broadcast carrying the corrected queue. The
            // successful pending-load commit publishes the new stopped
            // queue once instead of first publishing this old queue.
            self.broadcast_owner_queue_state();
        }
        if let PlayerEvent::PausedChanged(paused) = &pe {
            if let Some((connection_id, request_id, generation)) = self
                .owner
                .intents
                .current
                .as_ref()
                .and_then(|current| match &current.action {
                    PlaybackIntentAction::SetPaused { paused: desired } if desired == paused => {
                        Some((
                            current.connection_id,
                            current.request_id,
                            current.generation,
                        ))
                    }
                    _ => None,
                })
            {
                if let Some(event) =
                    self.owner
                        .intents
                        .applied_if_current(connection_id, request_id, generation)
                {
                    self.ctrl_clients
                        .lock()
                        .unwrap()
                        .send_to_client(connection_id, &CtrlEvent::PlaybackIntent(event));
                }
            }
        }
        if matches!(pe, PlayerEvent::Stopped { .. }) {
            if let Some((connection_id, request_id, generation)) = self
                .owner
                .intents
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
                if let Some(event) =
                    self.owner
                        .intents
                        .applied_if_current(connection_id, request_id, generation)
                {
                    self.ctrl_clients
                        .lock()
                        .unwrap()
                        .send_to_client(connection_id, &CtrlEvent::PlaybackIntent(event));
                }
            }
        }
        broadcast_player_event_if_not_replaced(&self.ctrl_clients, pe, replacement_committed);
        if stopped_queue_updated || replacement_committed {
            return EventOutcome::DIRTY;
        }
        EventOutcome::CONTINUE
    }

    /// `DaemonEvent::Ws`: apply the service socket event when it belongs to
    /// the generation this daemon is running.
    fn handle_ws_event(&mut self, generation: SetupGeneration, event: WsEvent) -> EventOutcome {
        if self
            .emby_runtime
            .as_ref()
            .is_some_and(|runtime| runtime.generation == generation)
        {
            handle_ws(
                event,
                Some(&self.client),
                &self.player,
                self.audio_only,
                &mut self.owner.core.queue,
                &mut self.owner.core.source,
                &mut self.owner.core.transitions,
                &self.shared_queue,
                &self.ctrl_clients,
            );
            return EventOutcome::DIRTY;
        }
        EventOutcome::CONTINUE
    }

    /// `DaemonEvent::QueueEnriched`: fold freshly fetched Emby items into an
    /// adopted queue.
    fn handle_queue_enriched(&mut self, items: Vec<(QueueSlotId, EmbyItem)>) -> EventOutcome {
        apply_queue_enriched(
            items,
            &mut self.owner,
            &self.player,
            &self.shared_queue,
            &self.ctrl_clients,
        );
        EventOutcome::CONTINUE
    }

    /// `DaemonEvent::AudiobookshelfProgress`: apply acknowledged audiobook
    /// progress for the running service generation.
    fn handle_audiobookshelf_progress(
        &mut self,
        update: AudiobookshelfProgressUpdate,
    ) -> EventOutcome {
        apply_audiobookshelf_progress(
            update,
            self.audiobookshelf_runtime
                .as_ref()
                .map(|runtime| runtime.generation),
            &mut self.owner.core.queue,
            &self.ctrl_clients,
        );
        EventOutcome::CONTINUE
    }

    /// `DaemonEvent::AudiobookshelfBookProgress`: book-shaped counterpart to
    /// `AudiobookshelfProgress`, keyed by `library_item_id`.
    fn handle_audiobookshelf_book_progress(
        &mut self,
        update: AudiobookshelfBookProgressUpdate,
    ) -> EventOutcome {
        apply_audiobookshelf_book_progress(
            update,
            self.audiobookshelf_runtime
                .as_ref()
                .map(|runtime| runtime.generation),
            &mut self.owner.core.queue,
            &self.ctrl_clients,
        );
        EventOutcome::CONTINUE
    }

    /// `DaemonEvent::Ctrl`: apply a service-setup reconcile inline, otherwise
    /// route the command through the role-gated handler.
    fn handle_ctrl_event(
        &mut self,
        cmd: CtrlCmd,
        client_id: CtrlClientId,
        reply_tx: CtrlSender,
    ) -> EventOutcome {
        if !self.ctrl_clients.lock().unwrap().has_client(client_id) {
            return EventOutcome::CONTINUE;
        }
        if let CtrlCmd::ApplyServiceSetup { kind, revision } = cmd {
            let transport = self.ctrl_clients.lock().unwrap().transport(client_id);
            let allowed = owner_admin_transport_allowed(self.role, kind, transport);
            let result = if !allowed {
                Err(crate::ctrl::ServiceSetupRejection::TransitionRejected)
            } else {
                match kind {
                    crate::config::ServiceKind::Emby => reconcile_packaged_emby(
                        revision,
                        &mut self.emby_runtime,
                        &mut self.ws_send_tx,
                        &self.client,
                        &self.player,
                        &mut self.owner.core.queue,
                        &mut self.owner.core.source,
                        &mut self.owner.core.transitions,
                        &self.shared_queue,
                        &self.ctrl_clients,
                        &self.merged_tx,
                        &self.direct_commands,
                        self.audio_only,
                    ),
                    crate::config::ServiceKind::Audiobookshelf => {
                        reconcile_packaged_audiobookshelf(
                            revision,
                            &mut self.audiobookshelf_runtime,
                            &self.player,
                            &mut self.owner.core.queue,
                            &mut self.owner.core.source,
                            &mut self.owner.core.transitions,
                            &self.shared_queue,
                            &self.ctrl_clients,
                            &self.client,
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
                    &self.player,
                    &self.audiobookshelf_runtime,
                    &self.merged_tx,
                );
            }
            return EventOutcome::CONTINUE;
        }
        let persist_after_command = cmd.mutates_owner_queue();
        handle_ctrl_for_role(
            cmd,
            CtrlContext {
                reply_tx: &reply_tx,
                client_id,
                client: &self.client,
                player: &self.player,
                audio_only: self.audio_only,
                owner: &mut self.owner,
                shared_queue: &self.shared_queue,
                ctrl_clients: &self.ctrl_clients,
                has_audiobookshelf: self.audiobookshelf_runtime.is_some(),
                merged_tx: &self.merged_tx,
                stay_alive: self.stay_alive,
                role: self.role,
            },
        );
        if persist_after_command && self.owner.pending_idle_load.is_none() {
            return EventOutcome::DIRTY;
        }
        EventOutcome::CONTINUE
    }

    /// `DaemonEvent::PlaybackResolved`: validate a resolved play intent, then
    /// replace the queue and start playback.
    fn handle_playback_resolved(
        &mut self,
        start_idx: usize,
        start_ticks: i64,
        new_source: crate::config::QueueSource,
        client_id: CtrlClientId,
        request_id: PlaybackRequestId,
        generation: PlaybackGeneration,
        fetched: Result<Vec<EmbyItem>, String>,
    ) -> EventOutcome {
        if !self.ctrl_clients.lock().unwrap().has_client(client_id) {
            self.owner.intents.invalidate_connection(client_id);
            return EventOutcome::CONTINUE;
        }
        if !self
            .owner
            .intents
            .is_current(client_id, request_id, generation)
        {
            return EventOutcome::CONTINUE;
        }
        if let Err(error) = &fetched {
            if let Some(event) = self.owner.intents.rejected_if_current(
                client_id,
                request_id,
                generation,
                crate::ctrl::PlaybackIntentRejection::ResolutionFailed,
            ) {
                self.ctrl_clients
                    .lock()
                    .unwrap()
                    .send_to_client(client_id, &CtrlEvent::PlaybackIntent(event));
            }
            log::warn!(target: "daemon", "ctrl play resolution failed: {error}");
            return EventOutcome::CONTINUE;
        }
        if let Ok(items_for_intent) = &fetched {
            let rejection = if items_for_intent.is_empty() {
                Some(crate::ctrl::PlaybackIntentRejection::EmptyTarget)
            } else if audio_only_rejection(
                self.audio_only,
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
                if let Some(event) = self
                    .owner
                    .intents
                    .rejected_if_current(client_id, request_id, generation, reason)
                {
                    self.ctrl_clients
                        .lock()
                        .unwrap()
                        .send_to_client(client_id, &CtrlEvent::PlaybackIntent(event));
                }
                return EventOutcome::CONTINUE;
            }
        }
        self.owner.intents.mark_starting(request_id);
        if let Some(status) = self.owner.intents.pipe_status() {
            log::info!(target: "pipe_latency", "request={} generation={} phase={:?} elapsed_ms={}", status.request_id, status.generation, status.phase, self.owner.intents.current.as_ref().map(|current| current.accepted_at.elapsed().as_millis()).unwrap_or_default());
            self.ctrl_clients
                .lock()
                .unwrap()
                .send_to_client(client_id, &CtrlEvent::PipePlaybackStatus(status));
        }
        if let Ok(fetched_items) = fetched {
            // A resolved Play replaces the queue: it deliberately
            // interrupts any in-flight or queued slot jump.
            reset_slot_jumps(
                &mut self.owner.core.transitions,
                &mut self.owner.queued_transition_origin,
            );
            play_resolved_items(
                fetched_items,
                start_idx,
                start_ticks,
                new_source,
                &self.client,
                &self.player,
                &mut self.owner.core.queue,
                &mut self.owner.core.source,
                &self.shared_queue,
                &self.ctrl_clients,
                &self.owner.core.transitions,
            );
            return EventOutcome::DIRTY;
        }
        EventOutcome::CONTINUE
    }

    /// `DaemonEvent::CtrlDisconnected`: drop the client and invalidate any
    /// playback intent it owned.
    fn handle_ctrl_disconnected(&mut self, client_id: CtrlClientId) -> EventOutcome {
        self.ctrl_clients.lock().unwrap().remove(client_id);
        self.owner.intents.invalidate_connection(client_id);
        EventOutcome::CONTINUE
    }

    /// `DaemonEvent::Shutdown`: persist the Stay-alive queue, announce the
    /// deliberate shutdown, and stop the player.
    fn handle_shutdown(&mut self) -> EventOutcome {
        log::info!(target: "daemon", "graceful shutdown: stopping player");
        if self.role == DaemonRole::Local {
            if let Err(error) = self.persist_owner_queue() {
                log::error!(target: "queue", "failed to persist Stay-alive queue on shutdown: {error}");
            }
        }
        // Announce the deliberate shutdown to every connected client
        // before closing their connections, so they exit cleanly
        // instead of treating this as an unannounced crash.
        self.ctrl_clients
            .lock()
            .unwrap()
            .notify_disconnected_all(DisconnectReason::DaemonShutdown);
        self.ctrl_clients
            .lock()
            .unwrap()
            .flush_writers(std::time::Duration::from_secs(1));
        self.player.stop();
        self.player
            .join_or_timeout(std::time::Duration::from_secs(5));
        EventOutcome::SHUTDOWN
    }

    /// Broadcasts the canonical owner queue to every ctrl peer.
    fn broadcast_owner_queue_state(&self) {
        broadcast_queue_state(
            &self.ctrl_clients,
            &self.player,
            &self.shared_queue,
            &self.owner.core.queue,
            &self.owner.core.source,
            &self.owner.core.transitions,
        );
    }

    /// Persists the owner queue through the injected store, returning the
    /// store's error so the caller can log it in context.
    fn persist_owner_queue(&mut self) -> Result<(), String> {
        persist_stay_alive_owner_queue(
            &self.owner,
            &self.player,
            &self.shared_queue,
            &mut *self.store,
        )
    }
}
