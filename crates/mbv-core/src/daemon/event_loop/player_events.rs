//! Player-originated daemon events: track changes, Next-Up cards, output start,
//! completions, and all other player observations relayed to control clients.

use super::super::*;
use super::{DaemonLoop, EventOutcome};
use crate::ctrl::{
    CtrlEvent, PlaybackGeneration, PlaybackIntentAction, PlaybackIntentEvent,
    PlaybackIntentOutcome, PlaybackRequestId,
};
use crate::id_types::ItemId;
use crate::playback_queue::QueueSlotId;
use crate::player::{PlayerCommand, PlayerEvent};
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
    pub(super) fn handle_next_up_threshold(
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
    pub(super) fn handle_queue_next_up(&mut self, next_idx: usize) -> EventOutcome {
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
    pub(super) fn handle_output_started(&mut self) -> EventOutcome {
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
        let (consume_videos, consume_audio) = {
            let cfg = self.client.lock().unwrap();
            (cfg.config.consume_videos, cfg.config.consume_audio)
        };
        if !apply_track_completed_observation(
            &mut self.owner,
            &self.player,
            &self.shared_queue,
            run_identity,
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
    pub(super) fn handle_player_event(&mut self, pe: PlayerEvent) -> EventOutcome {
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
                    *run_identity,
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
}
