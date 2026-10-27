//! Player-originated daemon events: track changes, Next-Up cards, output start,
//! completions, and all other player observations relayed to control clients.

use super::super::{
    DaemonLoop, DaemonOwnerContext, apply_stopped_observation, apply_track_completed_observation,
    broadcast, next_up_slot, settle_and_redispatch,
};
use super::EventOutcome;
use mbv_ctrl::player::{PlayerCommand, PlayerEvent};
use mbv_ctrl::{
    CtrlEvent, PlaybackGeneration, PlaybackIntentAction, PlaybackIntentEvent,
    PlaybackIntentOutcome, PlaybackRequestId,
};
use mbv_ids::ItemId;
use mbv_queue::QueueSlotId;
use std::time::Duration;

impl DaemonLoop {
    /// `PlayerEvent::TrackChanged`: resolve the reported slot against the
    /// canonical queue, settle any transition it closes, and publish.
    pub(super) fn handle_track_changed(
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
            tracing::warn!(name: "daemon.queue_track_change.discarded", target: "queue", slot = ?slot_id, "discarding track change for unknown slot");
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
                &mut DaemonOwnerContext {
                    player: &self.player,
                    client: &self.client,
                    owner: &mut self.owner,
                    shared_queue: &self.shared_queue,
                    ctrl_clients: &self.ctrl_clients,
                    merged_tx: &self.merged_tx,
                    audiobookshelf: self.audiobookshelf_runtime.as_ref(),
                },
                observed_request_id,
                resolved_slot_id,
            );
        }
        self.shared_queue.publish_observed(&self.owner.core);
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
            && let Some(event) =
                self.owner
                    .intents
                    .applied_if_current(connection_id, request_id, generation)
        {
            self.ctrl_clients
                .lock()
                .unwrap()
                .send_to_client(connection_id, &CtrlEvent::PlaybackIntent(event));
        }
        EventOutcome::CONTINUE
    }

    /// `PlayerEvent::NextUpThreshold`: show the Next-Up card for the slot
    /// the core will really play next, then relay the event.
    pub(super) fn handle_next_up_threshold(
        &mut self,
        series_id: ItemId,
        season: i64,
        episode: i64,
    ) -> EventOutcome {
        // The card names the queue neighbor after the active slot; at the
        // end of the queue there is no card.
        if let Some(slot_id) = next_up_slot(&self.owner.core.queue)
            && let Some(slot) = self.owner.core.queue.slot(slot_id)
            && let Some(emby) = slot.item.as_emby()
        {
            self.player.send_command(PlayerCommand::NextUpShow {
                item_id: emby.id.clone(),
                show_title: emby.series_name.clone(),
                ep_title: emby.name.clone(),
                artist: emby.artist.clone(),
            });
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
    pub(super) fn handle_queue_next_up(&mut self, next_idx: usize) -> EventOutcome {
        if let Some(slot) = self.owner.core.queue.slots().get(next_idx)
            && let Some(emby) = slot.item.as_emby()
        {
            self.player.send_command(PlayerCommand::NextUpShow {
                item_id: emby.id.clone(),
                show_title: emby.series_name.clone(),
                ep_title: emby.name.clone(),
                artist: emby.artist.clone(),
            });
        }
        broadcast(
            &self.ctrl_clients,
            &CtrlEvent::Player(PlayerEvent::QueueNextUp { next_idx }),
        );
        EventOutcome::CONTINUE
    }

    /// `PlayerEvent::OutputStarted`: settle any current playback intent that
    /// was waiting on the pipe, relay the event, and publish a snapshot.
    pub(super) fn handle_output_started(&mut self) -> EventOutcome {
        let delay = self
            .client
            .lock()
            .unwrap()
            .config
            .audio_pipe_playout_delay_ms
            .map(Duration::from_millis);
        if let Some((connection_id, status)) = self.owner.intents.output_started_if_current(delay) {
            tracing::info!(name: "daemon.pipe_latency.status", target: "pipe_latency", request = %status.request_id, generation = status.generation, phase = ?status.phase, elapsed_ms = self.owner.intents.current.as_ref().map(|current| current.accepted_at.elapsed().as_millis()).unwrap_or_default(), "pipe playback status");
            self.ctrl_clients
                .lock()
                .unwrap()
                .send_to_client(connection_id, &CtrlEvent::PipePlaybackStatus(status));
            if delay.is_none()
                && let Some(current) = self.owner.intents.current.as_ref()
            {
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
    pub(super) fn handle_track_completed(&mut self, pe: PlayerEvent) -> EventOutcome {
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
        let settings = (self.owner_settings)();
        let consume_policy = settings.consume;
        if !apply_track_completed_observation(
            &mut self.owner,
            &self.player,
            &self.shared_queue,
            run_identity,
            slot_id,
            position_ticks,
            played,
            consume,
            consume_policy,
        ) {
            return EventOutcome::CONTINUE;
        }
        self.broadcast_owner_queue_state();
        broadcast(&self.ctrl_clients, &CtrlEvent::Player(pe));
        EventOutcome::DIRTY
    }

    fn handle_stopped_event(&mut self, pe: &PlayerEvent) -> Option<bool> {
        let PlayerEvent::Stopped {
            slot_id,
            run_identity,
            position_ticks,
            played,
            ..
        } = pe
        else {
            return Some(false);
        };

        apply_stopped_observation(
            &mut self.owner,
            &self.player,
            *run_identity,
            *slot_id,
            *position_ticks,
            *played,
        )
    }

    fn settle_player_event_intent(&mut self, pe: &PlayerEvent) {
        if let PlayerEvent::PausedChanged(paused) = pe
            && let Some((connection_id, request_id, generation)) = self
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
            && let Some(event) =
                self.owner
                    .intents
                    .applied_if_current(connection_id, request_id, generation)
        {
            self.ctrl_clients
                .lock()
                .unwrap()
                .send_to_client(connection_id, &CtrlEvent::PlaybackIntent(event));
        }
        if matches!(pe, PlayerEvent::Stopped { .. })
            && let Some((connection_id, request_id, generation)) = self
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
            && let Some(event) =
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

    /// One player observation, routed to the handler that owns it. Every
    /// event that carries queue or Next-Up evidence keeps its explicit arm;
    /// any other player event goes to `handle_player_event`.
    pub(super) fn handle_player_observation(&mut self, pe: PlayerEvent) -> EventOutcome {
        match pe {
            PlayerEvent::TrackChanged {
                slot_id,
                transition,
            } => self.handle_track_changed(slot_id, transition),
            PlayerEvent::NextUpThreshold {
                series_id,
                season,
                episode,
            } => self.handle_next_up_threshold(series_id, season, episode),
            PlayerEvent::QueueNextUp { next_idx } => self.handle_queue_next_up(next_idx),
            PlayerEvent::OutputStarted => self.handle_output_started(),
            pe @ PlayerEvent::TrackCompleted { .. } => self.handle_track_completed(pe),
            pe => self.handle_player_event(pe),
        }
    }

    /// Any other `PlayerEvent`: settle `Stopped`/`PausedChanged` intents and
    /// observations, then relay the event.
    pub(super) fn handle_player_event(&mut self, pe: PlayerEvent) -> EventOutcome {
        let Some(stopped_queue_updated) = self.handle_stopped_event(&pe) else {
            return EventOutcome::CONTINUE;
        };
        if stopped_queue_updated {
            // Unlike TrackCompleted (which broadcasts unconditionally
            // below via the raw player event too), a full Stopped has
            // no other broadcast carrying the corrected queue.
            self.broadcast_owner_queue_state();
        }
        self.settle_player_event_intent(&pe);
        broadcast(&self.ctrl_clients, &CtrlEvent::Player(pe));
        if stopped_queue_updated {
            return EventOutcome::DIRTY;
        }
        EventOutcome::CONTINUE
    }
}
