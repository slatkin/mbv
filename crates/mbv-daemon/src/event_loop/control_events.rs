//! Control-client-originated daemon events: control commands, resolved play
//! intents, client disconnects, and graceful shutdown.

use super::super::{
    CtrlClientId, CtrlContext, CtrlSender, DaemonLoop, DaemonOwnerContext, DaemonRole,
    audio_only_rejection, handle_ctrl_for_role, install_daemon_audiobookshelf_context,
    owner_admin_transport_allowed, play_resolved_items, reconcile_packaged_audiobookshelf,
    reconcile_packaged_emby, reset_slot_jumps, send_to,
};
use super::EventOutcome;
use crate::control::intent_span;
use mbv_ctrl::{CtrlCmd, CtrlEvent, DisconnectReason, PlaybackGeneration, PlaybackRequestId};
use mbv_emby_model::EmbyItem;
use mbv_queue::QueueItem;
use std::sync::Arc;

impl DaemonLoop {
    /// `DaemonEvent::Ctrl`: apply a service-setup reconcile inline, otherwise
    /// route the command through the role-gated handler.
    pub(super) fn handle_ctrl_event(
        &mut self,
        cmd: CtrlCmd,
        client_id: CtrlClientId,
        reply_tx: &CtrlSender,
    ) -> EventOutcome {
        if !self.ctrl_clients.lock().unwrap().has_client(client_id) {
            return EventOutcome::CONTINUE;
        }
        if let CtrlCmd::ApplyServiceSetup { kind, revision } = cmd {
            let transport = self.ctrl_clients.lock().unwrap().transport(client_id);
            let allowed = owner_admin_transport_allowed(self.role, kind, transport);
            let result = if allowed {
                match kind {
                    mbv_queue::ServiceKind::Emby => reconcile_packaged_emby(
                        revision,
                        &mut self.emby_runtime,
                        &mut self.ws_send_tx,
                        &self.merged_tx,
                        &self.direct_commands,
                        self.audio_only,
                        &mut DaemonOwnerContext {
                            player: &self.player,
                            client: &self.client,
                            owner: &mut self.owner,
                            shared_queue: &self.shared_queue,
                            ctrl_clients: &self.ctrl_clients,
                        },
                    ),
                    mbv_queue::ServiceKind::Audiobookshelf => reconcile_packaged_audiobookshelf(
                        revision,
                        &mut self.audiobookshelf_runtime,
                        &mut DaemonOwnerContext {
                            player: &self.player,
                            client: &self.client,
                            owner: &mut self.owner,
                            shared_queue: &self.shared_queue,
                            ctrl_clients: &self.ctrl_clients,
                        },
                    ),
                }
            } else {
                Err(mbv_ctrl::ServiceSetupRejection::TransitionRejected)
            };
            match result {
                Ok(()) => send_to(reply_tx, &CtrlEvent::ServiceSetupApplied { kind, revision }),
                Err(reason) => send_to(
                    reply_tx,
                    &CtrlEvent::ServiceSetupRejected {
                        kind,
                        revision,
                        reason,
                    },
                ),
            }
            if result.is_ok() && kind == mbv_queue::ServiceKind::Audiobookshelf {
                install_daemon_audiobookshelf_context(
                    &self.player,
                    self.audiobookshelf_runtime.as_ref(),
                    &self.merged_tx,
                );
            }
            return EventOutcome::CONTINUE;
        }
        let persist_after_command = cmd.mutates_owner_queue();
        handle_ctrl_for_role(
            cmd,
            CtrlContext {
                reply_tx,
                client_id,
                client: &self.client,
                player: &self.player,
                audio_only: self.audio_only,
                owner: &mut self.owner,
                shared_queue: &self.shared_queue,
                ctrl_clients: &self.ctrl_clients,
                audiobookshelf: self.audiobookshelf_runtime.as_ref(),
                merged_tx: &self.merged_tx,
                owner_settings: Arc::clone(&self.owner_settings),
                role: self.role,
                op: std::cell::Cell::new(None),
            },
        );
        if persist_after_command && self.owner.pending_idle_load.is_none() {
            return EventOutcome::DIRTY;
        }
        EventOutcome::CONTINUE
    }

    /// `DaemonEvent::PlaybackResolved`: validate a resolved play intent, then
    /// replace the queue and start playback.
    pub(super) fn handle_playback_resolved(
        &mut self,
        start_idx: usize,
        start_ticks: i64,
        new_source: mbv_queue::QueueSource,
        client_id: CtrlClientId,
        request_id: PlaybackRequestId,
        generation: PlaybackGeneration,
        fetched: Result<Vec<EmbyItem>, crate::DaemonLibError>,
    ) -> EventOutcome {
        // Rejoin rule (design D5): rebuild the intent span from the ids the
        // resolved event already carries; no `Span` is stored anywhere.
        let _intent_span = intent_span(client_id, request_id, generation).entered();
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
                mbv_ctrl::PlaybackIntentRejection::ResolutionFailed,
            ) {
                self.ctrl_clients
                    .lock()
                    .unwrap()
                    .send_to_client(client_id, &CtrlEvent::PlaybackIntent(event));
            }
            tracing::warn!(
                name: "ctrl.intent.failed",
                target: "daemon",
                error = %error,
                "ctrl play resolution failed"
            );
            return EventOutcome::CONTINUE;
        }
        if let Ok(items_for_intent) = &fetched {
            let rejection = if items_for_intent.is_empty() {
                Some(mbv_ctrl::PlaybackIntentRejection::EmptyTarget)
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
                Some(mbv_ctrl::PlaybackIntentRejection::AudioOnly)
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
            tracing::info!(name: "daemon.pipe_latency.status", target: "pipe_latency", request = %status.request_id, generation = status.generation, phase = ?status.phase, elapsed_ms = self.owner.intents.current.as_ref().map(|current| current.accepted_at.elapsed().as_millis()).unwrap_or_default(), "pipe playback status");
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
                &mut DaemonOwnerContext {
                    player: &self.player,
                    client: &self.client,
                    owner: &mut self.owner,
                    shared_queue: &self.shared_queue,
                    ctrl_clients: &self.ctrl_clients,
                },
                fetched_items,
                start_idx,
                start_ticks,
                new_source,
            );
            return EventOutcome::DIRTY;
        }
        EventOutcome::CONTINUE
    }

    /// `DaemonEvent::CtrlDisconnected`: drop the client and invalidate any
    /// playback intent it owned.
    pub(super) fn handle_ctrl_disconnected(&mut self, client_id: CtrlClientId) -> EventOutcome {
        self.ctrl_clients.lock().unwrap().remove(client_id);
        self.owner.intents.invalidate_connection(client_id);
        EventOutcome::CONTINUE
    }

    /// `DaemonEvent::Shutdown`: persist the Stay-alive queue, announce the
    /// deliberate shutdown, and stop the player.
    pub(super) fn handle_shutdown(&mut self) -> EventOutcome {
        self.ctrl_clients.lock().unwrap().begin_shutdown();
        tracing::info!(name: "daemon.shutdown.started", target: "daemon", "graceful shutdown: stopping player");
        if let Some(tx) = &self.queue_persist_tx {
            let (ack_tx, ack_rx) = std::sync::mpsc::sync_channel(0);
            if tx
                .send(super::super::QueuePersistenceRequest::Flush(ack_tx))
                .is_ok()
            {
                let _ = ack_rx.recv();
            }
        }
        if self.role == DaemonRole::Local
            && let Err(error) = self.persist_owner_queue()
        {
            tracing::error!(name: "daemon.queue_state_persist.failed", target: "queue", error = %error, "failed to persist Stay-alive queue on shutdown");
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
}
