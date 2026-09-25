//! Control-client-originated daemon events: control commands, resolved play
//! intents, client disconnects, and graceful shutdown.

use super::super::*;
use super::{DaemonLoop, EventOutcome};
use crate::api::EmbyItem;
use crate::ctrl::{CtrlCmd, CtrlEvent, DisconnectReason, PlaybackGeneration, PlaybackRequestId};
use crate::daemon::ctrl::send_to;
use crate::playback_queue::QueueItem;

impl DaemonLoop {
    /// `DaemonEvent::Ctrl`: apply a service-setup reconcile inline, otherwise
    /// route the command through the role-gated handler.
    pub(super) fn handle_ctrl_event(
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
                    crate::config::ServiceKind::Audiobookshelf => {
                        reconcile_packaged_audiobookshelf(
                            revision,
                            &mut self.audiobookshelf_runtime,
                            &mut DaemonOwnerContext {
                                player: &self.player,
                                client: &self.client,
                                owner: &mut self.owner,
                                shared_queue: &self.shared_queue,
                                ctrl_clients: &self.ctrl_clients,
                            },
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
    pub(super) fn handle_playback_resolved(
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
}
