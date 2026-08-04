use super::types_playback::RemoteQueueProjection;
use super::{App, ReconciliationCommand, SessionEvent};
use mbv_core::api::{EmbyClient, TICKS_PER_SECOND};
use mbv_core::remote_reconciliation::{
    ReconciliationTracker, RemoteIntent, SequenceSource, SubmittedOccurrence,
};
use std::time::SystemTime;

impl App {
    fn next_session_poll_generation(&mut self) -> u64 {
        self.session_poll_generation = self.session_poll_generation.saturating_add(1);
        self.session_poll_generation
    }

    pub(super) fn now_ms() -> u64 {
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }

    pub(super) fn submit_attached_sequence(
        &mut self,
        conn_id: &str,
        items: &[mbv_core::api::MediaItem],
        start_idx: usize,
    ) {
        let generation = self.next_session_poll_generation();
        self.retire_remote_tracking(true);
        if items.len() < 2 {
            self.remote_tracker = None;
            self.remote_queue_projection = None;
        } else if let Some(tracker) = Self::build_remote_tracker_with_source(
            conn_id,
            items,
            start_idx,
            generation,
            self.queue_playlist_id().map(str::to_string),
        ) {
            self.remote_tracker = Some(tracker);
            let queue = self.displayed_queue();
            let exact_queue = queue.items.len() == items.len()
                && queue.items.iter().zip(items).all(|(visible, submitted)| {
                    visible.id == submitted.id
                        && visible.playlist_item_id == submitted.playlist_item_id
                });
            if exact_queue {
                let occurrence_slots: std::collections::HashMap<_, _> = queue
                    .queue
                    .slots()
                    .iter()
                    .enumerate()
                    .map(|(index, slot)| (index as u64 + 1, slot.slot_id))
                    .collect();
                let slot_occurrences = occurrence_slots
                    .iter()
                    .map(|(occurrence_id, slot_id)| (*slot_id, *occurrence_id))
                    .collect();
                self.remote_queue_projection = Some(RemoteQueueProjection {
                    session_id: conn_id.to_string(),
                    epoch: self.remote_tracker.as_ref().map_or(0, |t| t.epoch()),
                    queue_lineage: self.remote_queue_lineage,
                    occurrence_slots,
                    slot_occurrences,
                });
            } else {
                self.remote_queue_projection = None;
            }
        }
        let id = conn_id.to_string();
        let item_ids: Vec<String> = items.iter().map(|item| item.id.clone()).collect();
        let start_ticks = items
            .get(start_idx)
            .map_or(0, |item| item.playback_position_ticks);
        let reconciliation = self.reconciliation_command(conn_id, generation);
        self.dispatch_session_command(generation, reconciliation, move |client| {
            client.session_play_items(&id, &item_ids, start_idx, start_ticks)
        });
    }

    pub(super) fn build_remote_tracker_with_source(
        conn_id: &str,
        items: &[mbv_core::api::MediaItem],
        start_idx: usize,
        generation: u64,
        playlist_id: Option<String>,
    ) -> Option<ReconciliationTracker> {
        let occurrences = items
            .iter()
            .enumerate()
            .map(|(index, item)| {
                let mut occurrence = SubmittedOccurrence::new(index as u64 + 1, item.id.clone())
                    .runtime_ticks(item.runtime_ticks);
                if let Some(id) = playlist_id.as_ref() {
                    occurrence = occurrence.source(SequenceSource::Playlist { id: id.clone() });
                }
                if !item.playlist_item_id.is_empty() {
                    occurrence = occurrence.playlist_entry(item.playlist_item_id.clone());
                }
                occurrence
            })
            .collect();
        ReconciliationTracker::new(conn_id, occurrences, start_idx, Self::now_ms()).map(
            |mut tracker| {
                tracker.accept_observations_from(generation);
                tracker
            },
        )
    }

    pub(super) fn issue_remote_intent(&mut self, intent: RemoteIntent) {
        if let Some(tracker) = self.remote_tracker.as_mut() {
            tracker.issue_intent(intent, Self::now_ms());
        }
    }

    pub(super) fn tracked_occurrence_at_queue_index(&mut self, index: usize) -> Option<u64> {
        self.player_tab.sync_queue_model_from_items_if_needed();
        let slot_id = self.player_tab.resolve_slot_at(index)?;
        let projection = self.remote_queue_projection.as_ref()?;
        (projection.queue_lineage == self.remote_queue_lineage)
            .then(|| projection.slot_occurrences.get(&slot_id).copied())
            .flatten()
    }

    pub(super) fn stop_remote_tracking(&mut self) {
        if self.remote_tracker.is_some() {
            if let Some(tracker) = self.remote_tracker.as_mut() {
                tracker.stop_tracking();
            }
            self.retire_remote_tracking(false);
            self.flash_status("Remote tracking stopped".into());
        }
    }

    pub(super) fn retire_remote_tracking(&mut self, invalidate_lineage: bool) {
        self.remote_tracker = None;
        self.remote_queue_projection = None;
        self.remote_reanchor_popup = None;
        self.remote_unresolved_outcomes = 0;
        if invalidate_lineage {
            self.remote_queue_lineage = self.remote_queue_lineage.saturating_add(1);
        }
    }

    pub(super) fn remote_tracking_source_is(&self, playlist_id: &str) -> bool {
        self.remote_tracker.as_ref().is_some_and(|tracker| {
            tracker
                .submitted()
                .first()
                .and_then(|occurrence| occurrence.playlist_id())
                == Some(playlist_id)
        })
    }

    pub(super) fn reanchor_remote_tracking(&mut self) {
        let Some(tracker) = self.remote_tracker.as_ref() else {
            return;
        };
        let targets = tracker.reanchor_targets();
        if targets.is_empty() {
            self.flash_status_high("Choose a unique tracked occurrence to re-anchor".into());
        } else if targets.len() == 1 {
            let target = targets[0].0;
            let effects = self
                .remote_tracker
                .as_mut()
                .map(|tracker| tracker.reanchor(target))
                .unwrap_or_default();
            self.update_remote_projection_epoch();
            if effects.is_empty() {
                self.flash_status_high("Choose a unique tracked occurrence to re-anchor".into());
            } else {
                self.flash_status("Remote tracking re-anchored".into());
            }
        } else {
            self.remote_reanchor_popup = Some(super::RemoteReanchorPopup { targets, cursor: 0 });
        }
    }

    pub(super) fn select_remote_reanchor_target(&mut self) {
        let Some(popup) = self.remote_reanchor_popup.take() else {
            return;
        };
        let Some((target, _)) = popup.targets.get(popup.cursor) else {
            return;
        };
        let effects = self
            .remote_tracker
            .as_mut()
            .map(|tracker| tracker.reanchor(*target))
            .unwrap_or_default();
        self.update_remote_projection_epoch();
        if effects.is_empty() {
            self.flash_status_high("That occurrence is no longer available".into());
        } else {
            self.flash_status("Remote tracking re-anchored".into());
        }
    }

    fn update_remote_projection_epoch(&mut self) {
        if let (Some(projection), Some(tracker)) = (
            self.remote_queue_projection.as_mut(),
            self.remote_tracker.as_ref(),
        ) {
            projection.epoch = tracker.epoch();
        }
    }

    pub(super) fn spawn_sessions_load(&mut self) {
        self.sessions_loading = true;
        let client = self.client.lock().unwrap().clone();
        let tx = self.sessions_tx.clone();
        let generation = self.next_session_poll_generation();
        std::thread::spawn(move || match client.get_sessions() {
            Ok(sessions) => {
                let _ = tx.send(SessionEvent::Loaded {
                    sessions,
                    generation,
                });
            }
            Err(e) => {
                let _ = tx.send(SessionEvent::Error(e));
            }
        });
    }

    pub(super) fn session_jump_track(
        &mut self,
        conn_id: &str,
        delta: i64,
        fallback_cmd: &'static str,
    ) {
        self.clear_playback_overlays();
        let id = conn_id.to_string();
        let current_remote_id = self
            .connected_session_state
            .as_ref()
            .and_then(|s| s.now_playing_item_id.as_deref());
        // Choose the destination and payload through the established untracked
        // path first: resolve the connected session's now-playing item against
        // the visible queue and apply the delta. Tracking only observes the
        // already-selected slot afterward and must never choose a different
        // target_idx or payload.
        let Some((target_idx, start_ticks)) =
            remote_jump_target(&self.player_tab.items, current_remote_id, delta)
        else {
            self.do_session_command(move |c| c.session_transport(&id, fallback_cmd));
            return;
        };
        let items = self.player_tab.items.clone();
        if self.remote_tracker.is_some() {
            if let Some(target) = self.tracked_occurrence_at_queue_index(target_idx) {
                let intent = if delta > 0 {
                    RemoteIntent::Next { target }
                } else {
                    RemoteIntent::Previous { target }
                };
                self.issue_remote_intent(intent);
            } else {
                log::debug!(
                    target: "sessions",
                    "tracked jump slot {} has no Submitted occurrence; issuing untracked command",
                    target_idx
                );
            }
            let item_ids: Vec<String> = items.iter().map(|item| item.id.clone()).collect();
            self.do_reconciliation_session_command(&id.clone(), move |client| {
                client.session_play_items(&id, &item_ids, target_idx, start_ticks)
            });
        } else {
            self.submit_attached_sequence(&id, &items, target_idx);
        }
    }

    /// Compute the absolute tick position for a remote-session seek, given
    /// the current position in seconds and a relative delta in seconds.
    ///
    /// This reconstructs the asymmetric math the old inline remote-session
    /// `<`/`>` handlers in `input.rs` had: rewinding (`delta < 0`) clamps at
    /// zero, fast-forwarding does not (matching the prior
    /// `(pos_s - 5).max(0)` vs. `(pos_s + 5)`). Used by `action::dispatch`'s
    /// `Action::SeekRelative` arm; kept here alongside its sibling
    /// session-math helpers (`session_jump_track`, `do_session_command`)
    /// rather than in `action.rs`, since it's pure session-position math with
    /// no dependency on the `Action` seam itself.
    pub(super) fn remote_seek_ticks(pos_s: i64, delta: f64) -> i64 {
        let moved = pos_s + delta as i64;
        let target = if delta < 0.0 { moved.max(0) } else { moved };
        target * TICKS_PER_SECOND
    }

    pub(super) fn clear_playback_overlays(&mut self) {
        self.skip_intro_end_ticks = None;
        self.next_up_item = None;
        self.status.clear();
    }

    pub(super) fn do_session_command(
        &mut self,
        f: impl FnOnce(&EmbyClient) -> Result<(), String> + Send + 'static,
    ) {
        let generation = self.next_session_poll_generation();
        self.dispatch_session_command(generation, None, f);
    }

    pub(super) fn do_reconciliation_session_command(
        &mut self,
        session_id: &str,
        f: impl FnOnce(&EmbyClient) -> Result<(), String> + Send + 'static,
    ) {
        let generation = self.next_session_poll_generation();
        let reconciliation = self.reconciliation_command(session_id, generation);
        self.dispatch_session_command(generation, reconciliation, f);
    }

    fn reconciliation_command(
        &mut self,
        session_id: &str,
        generation: u64,
    ) -> Option<ReconciliationCommand> {
        let tracker = self.remote_tracker.as_mut()?;
        if tracker.session_id() != session_id {
            return None;
        }
        tracker.track_command_generation(generation);
        Some(ReconciliationCommand {
            session_id: session_id.to_string(),
            tracking_id: tracker.tracking_id(),
            tracker_epoch: tracker.epoch(),
            generation,
        })
    }

    fn dispatch_session_command(
        &self,
        generation: u64,
        reconciliation: Option<ReconciliationCommand>,
        f: impl FnOnce(&EmbyClient) -> Result<(), String> + Send + 'static,
    ) {
        let client = self.client.lock().unwrap().clone();
        let tx = self.sessions_tx.clone();
        std::thread::spawn(move || {
            if let Err(e) = f(&client) {
                let _ = tx.send(SessionEvent::CommandError {
                    error: e,
                    reconciliation,
                });
                return;
            }
            // A successful remote command is acknowledged independently of the
            // post-command session poll, so command acknowledgment never
            // depends on the follow-up poll succeeding. Tracking may then
            // reconcile a later ordinary poll issued after the acknowledgment
            // boundary.
            if let Some(command) = reconciliation {
                let _ = tx.send(SessionEvent::CommandAcknowledged(command));
            }
            match client.get_sessions() {
                Ok(sessions) => {
                    let _ = tx.send(SessionEvent::Loaded {
                        sessions,
                        generation,
                    });
                }
                Err(e) => {
                    let _ = tx.send(SessionEvent::Error(e));
                }
            }
        });
    }
}

/// The pre-tracking destination and payload for a remote Next/Previous jump:
/// resolve the connected session's now-playing item against the visible queue
/// and apply the delta. Tracking must never change this choice, so command
/// construction routes through this established untracked path first.
pub(super) fn remote_jump_target(
    items: &[mbv_core::api::MediaItem],
    now_playing_item_id: Option<&str>,
    delta: i64,
) -> Option<(usize, i64)> {
    let current =
        now_playing_item_id.and_then(|rid| items.iter().position(|item| item.id == rid))?;
    let t = current as i64 + delta;
    if t < 0 || (t as usize) >= items.len() {
        return None;
    }
    let t = t as usize;
    Some((t, items[t].playback_position_ticks))
}
