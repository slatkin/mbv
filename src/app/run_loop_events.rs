use super::types_playback::{PlaylistMutation, RemoteConsumeOperation};
use super::App;
use mbv_core::remote_reconciliation::{ReconciliationEffect, RemoteObservation};

#[path = "run_loop_events_session.rs"]
mod run_loop_events_session;
#[path = "run_loop_events_teardown.rs"]
mod run_loop_events_teardown;

pub(super) fn validate_remote_playlist_entry(
    items: &[mbv_core::api::MediaItem],
    entry_id: &str,
    expected_media_id: &str,
) -> Result<bool, String> {
    match items.iter().find(|item| item.playlist_item_id == entry_id) {
        None => Ok(false),
        Some(item) if item.id == expected_media_id => Ok(true),
        Some(item) => Err(format!(
            "playlist entry {entry_id} now identifies media {} instead of {expected_media_id}",
            item.id
        )),
    }
}

impl App {
    fn apply_remote_observation(&mut self, session: &mbv_core::api::SessionInfo, generation: u64) {
        let Some(tracker) = self.remote_tracker.as_mut() else {
            return;
        };
        let observation = if let Some(media_id) = session.now_playing_item_id.clone() {
            RemoteObservation::playing(
                generation,
                session.id.clone(),
                media_id,
                session.position_ticks,
                session.runtime_ticks,
                Self::now_ms(),
            )
        } else {
            RemoteObservation::stopped(generation, session.id.clone(), Self::now_ms())
        };
        let effects = tracker.observe(observation);
        for effect in effects {
            match effect {
                ReconciliationEffect::Completion(item) => {
                    log::info!(
                        target: "remote_reconciliation",
                        "completed remote occurrence={} media={}",
                        item.occurrence_id,
                        item.media_id
                    );
                    self.begin_remote_consume(item);
                }
                ReconciliationEffect::StateChanged { state, reason } => {
                    log::debug!(
                        target: "remote_reconciliation",
                        "tracking state={state:?} reason={reason:?}"
                    );
                }
                _ => {}
            }
        }
    }

    fn unresolved_consume(&mut self, error: String) {
        self.remote_unresolved_outcomes = self.remote_unresolved_outcomes.saturating_add(1);
        log::warn!(target: "remote_reconciliation", "unresolved playlist consume: {error}");
    }

    fn apply_remote_consumed_occurrence(&mut self, operation: &RemoteConsumeOperation) {
        let Some(slot_id) = operation.queue_slot_id else {
            return;
        };
        if operation.queue_lineage != self.remote_queue_lineage {
            return;
        }

        self.player_tab.sync_queue_model_from_items_if_needed();
        let selected_slot = self
            .player_tab
            .queue
            .slots()
            .get(self.player_tab.queue_cursor)
            .map(|slot| slot.slot_id);
        if !matches!(
            self.player_tab.queue.consume_slot(slot_id),
            mbv_core::playback_queue::QueueMutationResult::Applied(_)
        ) {
            return;
        }
        self.player_tab.sync_items_from_queue_model();
        if let Some(index) =
            selected_slot.and_then(|selected| self.player_tab.queue.slot_index(selected))
        {
            self.player_tab.queue_cursor = index;
        }
        // An attached session must still persist an intentionally empty queue;
        // the ordinary empty-save guard protects unrelated remote-control UI.
        self.save_queue_state_after_remote_projection();
    }

    fn begin_remote_consume(
        &mut self,
        occurrence: mbv_core::remote_reconciliation::SubmittedOccurrence,
    ) {
        let Some(playlist_id) = occurrence.playlist_id().map(str::to_string) else {
            return;
        };
        let is_audio = self
            .player_tab
            .items
            .iter()
            .find(|item| item.id == occurrence.media_id)
            .is_some_and(|item| item.is_audio());
        let consume_enabled = {
            let config = &self.client.lock().unwrap().config;
            if is_audio {
                config.consume_audio
            } else {
                config.consume_videos
            }
        };
        if !consume_enabled {
            return;
        }
        let Some(entry_id) = occurrence.playlist_item_id.clone() else {
            return;
        };
        let Some(tracker) = self.remote_tracker.as_mut() else {
            return;
        };
        if self.remote_consume_operations.len() >= 128 {
            log::warn!(target: "remote_reconciliation", "consume operation limit reached; deferring new consume");
            return;
        }
        let session_id = tracker.session_id().to_string();
        let tracking_id = tracker.tracking_id();
        let epoch = tracker.epoch();
        if !tracker.mark_consumed(occurrence.occurrence_id) {
            return;
        }
        let media_id = occurrence.media_id.clone();
        let occurrence_id = occurrence.occurrence_id;
        let operation_id = self.next_remote_consume_operation;
        self.next_remote_consume_operation = self.next_remote_consume_operation.saturating_add(1);
        let mutation_id = self.next_playlist_mutation;
        self.next_playlist_mutation = self.next_playlist_mutation.saturating_add(1);
        let queue_slot_id = self
            .remote_queue_projection
            .as_ref()
            .and_then(|projection| {
                (projection.session_id == session_id
                    && projection.epoch == epoch
                    && projection.queue_lineage == self.remote_queue_lineage)
                    .then(|| projection.occurrence_slots.get(&occurrence_id).copied())
                    .flatten()
            });
        self.remote_consume_operations.push(RemoteConsumeOperation {
            operation_id,
            mutation_id,
            session_id: session_id.clone(),
            tracking_id,
            epoch,
            occurrence_id,
            playlist_id: playlist_id.clone(),
            entry_id: entry_id.clone(),
            media_id: media_id.clone(),
            queue_slot_id,
            queue_lineage: self.remote_queue_lineage,
        });
        self.enqueue_playlist_mutation(
            playlist_id,
            PlaylistMutation::ConsumeValidate {
                mutation_id,
                operation_id,
                session_id,
                tracking_id,
                epoch,
                occurrence_id,
                entry_id,
                media_id,
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::tests::make_item;

    #[test]
    fn playlist_entry_validation_distinguishes_absent_match_and_mismatch() {
        let mut item = make_item("Track", "Audio");
        item.id = "media-1".into();
        item.playlist_item_id = "entry-1".into();
        let items = vec![item];

        assert_eq!(
            validate_remote_playlist_entry(&items, "missing", "media-1"),
            Ok(false)
        );
        assert_eq!(
            validate_remote_playlist_entry(&items, "entry-1", "media-1"),
            Ok(true)
        );
        assert!(validate_remote_playlist_entry(&items, "entry-1", "media-2").is_err());
    }
}
