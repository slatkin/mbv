use super::core::DaemonEvent;
use super::*;
use crate::api::EmbyClient;
use crate::ctrl::{
    CtrlCmd, CtrlEvent, DisconnectReason, PlaybackIntentAction, PlaybackIntentEvent,
    PlaybackIntentOutcome,
};
use crate::daemon::ctrl::{send_to, ClientRegistry};
use crate::playback_queue::QueueItem;
use crate::player::{Player, PlayerCommand, PlayerEvent};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LoopFlow {
    Continue,
    Shutdown,
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
    pub(super) fn handle_event(&mut self, ev: DaemonEvent) -> LoopFlow {
        // Set by any arm below that mutated the owner's canonical queue;
        // persisted once after the match instead of inline per mutation site.
        let mut owner_queue_dirty = false;

        match ev {
            DaemonEvent::Player(PlayerEvent::TrackChanged {
                slot_id,
                transition,
            }) => {
                // Resolve the reported slot against the canonical queue. A
                // report naming a slot the daemon no longer holds carries no
                // evidence about which surviving slot was intended, so it is
                // discarded and logged without touching canonical queue or
                // observed slot (design D6).
                let Some((observed_idx, resolved_slot_id)) =
                    self.owner.core.observe_track_change(slot_id)
                else {
                    log::warn!(
                        target: "queue",
                        "discarding TrackChanged for unknown slot {slot_id:?}"
                    );
                    return LoopFlow::Continue;
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
                if let Some((connection_id, request_id, generation)) =
                    self.owner
                        .intents
                        .current
                        .as_ref()
                        .filter(|current| match &current.action {
                            PlaybackIntentAction::Play { item_ids, .. } => {
                                self.owner.core.queue.slots().get(observed_idx).is_some_and(
                                    |slot| item_ids.iter().any(|id| id == slot.item.id()),
                                )
                            }
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
            }
            DaemonEvent::Player(PlayerEvent::NextUpThreshold {
                series_id,
                season,
                episode,
            }) => {
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
            }
            DaemonEvent::Player(PlayerEvent::QueueNextUp { next_idx }) => {
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
            }
            DaemonEvent::Player(PlayerEvent::OutputStarted) => {
                let delay = self
                    .client
                    .lock()
                    .unwrap()
                    .config
                    .audio_pipe_playout_delay_ms
                    .map(Duration::from_millis);
                if let Some((connection_id, status)) =
                    self.owner.intents.output_started_if_current(delay)
                {
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
            }
            DaemonEvent::Player(
                pe @ PlayerEvent::TrackCompleted {
                    slot_id,
                    run_identity,
                    position_ticks,
                    played,
                    consume,
                    ..
                },
            ) => {
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
                    return LoopFlow::Continue;
                }
                self.broadcast_owner_queue_state();
                broadcast(&self.ctrl_clients, &CtrlEvent::Player(pe));
                owner_queue_dirty = true;
            }
            DaemonEvent::Player(pe) => {
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
                    if let Some((slot_id, run_identity, position_ticks, played, _)) =
                        stopped.as_ref()
                    {
                        let Some(updated) = apply_stopped_observation(
                            &mut self.owner,
                            &self.player,
                            (*run_identity).into(),
                            *slot_id,
                            *position_ticks,
                            *played,
                        ) else {
                            return LoopFlow::Continue;
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
                    if let Some((connection_id, request_id, generation)) =
                        self.owner.intents.current.as_ref().and_then(|current| {
                            match &current.action {
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
                            }
                        })
                    {
                        if let Some(event) = self.owner.intents.applied_if_current(
                            connection_id,
                            request_id,
                            generation,
                        ) {
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
                        if let Some(event) = self.owner.intents.applied_if_current(
                            connection_id,
                            request_id,
                            generation,
                        ) {
                            self.ctrl_clients
                                .lock()
                                .unwrap()
                                .send_to_client(connection_id, &CtrlEvent::PlaybackIntent(event));
                        }
                    }
                }
                broadcast_player_event_if_not_replaced(
                    &self.ctrl_clients,
                    pe,
                    replacement_committed,
                );
                if stopped_queue_updated || replacement_committed {
                    owner_queue_dirty = true;
                }
            }
            DaemonEvent::Ws { generation, event } => {
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
                    owner_queue_dirty = true;
                }
            }
            DaemonEvent::QueueEnriched(items) => {
                apply_queue_enriched(
                    items,
                    &mut self.owner,
                    &self.player,
                    &self.shared_queue,
                    &self.ctrl_clients,
                );
            }
            DaemonEvent::AudiobookshelfProgress(update) => {
                apply_audiobookshelf_progress(
                    update,
                    self.audiobookshelf_runtime
                        .as_ref()
                        .map(|runtime| runtime.generation),
                    &mut self.owner.core.queue,
                    &self.ctrl_clients,
                );
            }
            DaemonEvent::AudiobookshelfBookProgress(update) => {
                apply_audiobookshelf_book_progress(
                    update,
                    self.audiobookshelf_runtime
                        .as_ref()
                        .map(|runtime| runtime.generation),
                    &mut self.owner.core.queue,
                    &self.ctrl_clients,
                );
            }
            DaemonEvent::Ctrl(cmd, client_id, reply_tx) => {
                if !self.ctrl_clients.lock().unwrap().has_client(client_id) {
                    return LoopFlow::Continue;
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
                    return LoopFlow::Continue;
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
                if !self.ctrl_clients.lock().unwrap().has_client(client_id) {
                    self.owner.intents.invalidate_connection(client_id);
                    return LoopFlow::Continue;
                }
                if !self
                    .owner
                    .intents
                    .is_current(client_id, request_id, generation)
                {
                    return LoopFlow::Continue;
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
                    return LoopFlow::Continue;
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
                        return LoopFlow::Continue;
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
                    owner_queue_dirty = true;
                }
            }
            DaemonEvent::CtrlDisconnected(client_id) => {
                self.ctrl_clients.lock().unwrap().remove(client_id);
                self.owner.intents.invalidate_connection(client_id);
            }
            DaemonEvent::Shutdown => {
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
                return LoopFlow::Shutdown;
            }
        }

        if self.role == DaemonRole::Local && owner_queue_dirty {
            if let Err(error) = self.persist_owner_queue() {
                log::error!(target: "queue", "failed to persist Stay-alive queue: {error}");
            }
        }

        LoopFlow::Continue
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
