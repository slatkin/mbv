use super::notify_actions::ToastSeverity;
use super::{App, SessionEvent};
use mbv_core::api::{EmbyClient, TICKS_PER_SECOND};
use mbv_core::remote_reconciliation::RemoteIntent;
#[cfg(test)]
use mbv_core::remote_reconciliation::{ReconciliationTracker, SequenceSource, SubmittedOccurrence};
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
        items: &[mbv_core::api::EmbyItem],
        start_idx: usize,
    ) {
        let generation = self.next_session_poll_generation();
        let id = conn_id.to_string();
        let item_ids: Vec<String> = items.iter().map(|item| item.id.clone()).collect();
        let start_ticks = items
            .get(start_idx)
            .map_or(0, |item| item.playback_position_ticks);
        self.dispatch_session_command(generation, move |client| {
            client.session_play_items(&id, &item_ids, start_idx, start_ticks)
        });
    }

    #[cfg(test)]
    pub(super) fn build_remote_tracker_with_source(
        conn_id: &str,
        items: &[mbv_core::api::EmbyItem],
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
        }
    }

    pub(super) fn retire_remote_tracking(&mut self, invalidate_lineage: bool) {
        // The reanchor modal is only ever raised off an active tracker, so
        // only emit its dismiss when there was tracking to retire. An
        // unconditional dismiss writes DismissRemoteReanchor into the
        // single-slot `pending_overlay` on every queue replace, clobbering
        // the slot the playlist-Enter paths check before dismissing the
        // sidebar and focusing Queue (sidebar stays open, focus stays put).
        let had_tracking = self.remote_tracker.is_some() || self.remote_queue_projection.is_some();
        self.remote_tracker = None;
        self.remote_queue_projection = None;
        if had_tracking {
            self.dismiss_remote_reanchor();
        }
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
            self.flash(
                "Choose a unique tracked occurrence to re-anchor".into(),
                ToastSeverity::Error,
            );
        } else if targets.len() == 1 {
            let target = targets[0].0;
            let effects = self
                .remote_tracker
                .as_mut()
                .map(|tracker| tracker.reanchor(target))
                .unwrap_or_default();
            self.update_remote_projection_epoch();
            if effects.is_empty() {
                self.flash(
                    "Choose a unique tracked occurrence to re-anchor".into(),
                    ToastSeverity::Error,
                );
            } else {
                self.flash("Queue tracking updated".into(), ToastSeverity::Success);
            }
        } else {
            self.pending_overlay = Some(super::types_overlay::OverlayRequest::RemoteReanchor(
                super::RemoteReanchorPopup { targets, cursor: 0 },
            ));
        }
    }

    pub(super) fn reanchor_remote_target(&mut self, target: usize) {
        let effects = self
            .remote_tracker
            .as_mut()
            .map(|tracker| tracker.reanchor(target))
            .unwrap_or_default();
        self.update_remote_projection_epoch();
        if effects.is_empty() {
            self.flash(
                "That occurrence is no longer available".into(),
                ToastSeverity::Error,
            );
        } else {
            self.flash("Queue tracking updated".into(), ToastSeverity::Success);
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
        let Some(client) = self.emby_snapshot() else {
            return;
        };
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
        // This path is untracked: the destination and payload come only from
        // `remote_jump_target` and `submit_attached_sequence`, with no tracking
        // on this path.
        let Some((target_idx, _)) = remote_jump_target(&self.player_tab, current_remote_id, delta)
        else {
            self.do_session_command(move |c| c.session_transport(&id, fallback_cmd));
            return;
        };
        let emby_items = self.player_tab.emby_items();
        // Remap the canonical queue index to the Emby-only projection index
        // used by the session API.
        let emby_start = self
            .player_tab
            .queue
            .slots()
            .iter()
            .take(target_idx)
            .filter(|s| s.item.as_emby().is_some())
            .count();
        self.submit_attached_sequence(&id, &emby_items, emby_start);
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
        self.next_up_item = None;
        self.status.clear();
    }

    pub(super) fn do_session_command(
        &mut self,
        f: impl FnOnce(&EmbyClient) -> Result<(), String> + Send + 'static,
    ) {
        let generation = self.next_session_poll_generation();
        self.dispatch_session_command(generation, f);
    }

    fn dispatch_session_command(
        &self,
        generation: u64,
        f: impl FnOnce(&EmbyClient) -> Result<(), String> + Send + 'static,
    ) {
        let Some(client) = self.emby_snapshot() else {
            return;
        };
        let tx = self.sessions_tx.clone();
        std::thread::spawn(move || {
            if let Err(e) = f(&client) {
                let _ = tx.send(SessionEvent::CommandError {
                    error: e,
                    reconciliation: None,
                });
                return;
            }
            // Refresh the directly observed Session state after a successful command.
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
    player_tab: &crate::app::PlayerTab,
    now_playing_item_id: Option<&str>,
    delta: i64,
) -> Option<(usize, i64)> {
    let current = now_playing_item_id.and_then(|rid| {
        player_tab
            .queue
            .slots()
            .iter()
            .position(|s| s.item.id() == rid)
    })?;
    let t = current as i64 + delta;
    if t < 0 || (t as usize) >= player_tab.total_queue_len() {
        return None;
    }
    let t = t as usize;
    Some((
        t,
        player_tab
            .emby_item_at(t)
            .map_or(0, |i| i.playback_position_ticks),
    ))
}
