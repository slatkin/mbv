use super::{App, SessionEvent};
use mbv_core::api::{EmbyClient, TICKS_PER_SECOND};
use mbv_core::remote_reconciliation::{ReconciliationTracker, RemoteIntent, SubmittedOccurrence};
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
        if items.len() < 2 {
            self.remote_tracker = None;
        } else if let Some(tracker) = Self::build_remote_tracker(conn_id, items, start_idx) {
            self.remote_tracker = Some(tracker);
        }
        self.tracking_edit_warning_shown = false;
        let id = conn_id.to_string();
        let item_ids: Vec<String> = items.iter().map(|item| item.id.clone()).collect();
        let start_ticks = items
            .get(start_idx)
            .map_or(0, |item| item.playback_position_ticks);
        self.do_session_command(move |client| {
            client.session_play_items(&id, &item_ids, start_idx, start_ticks)
        });
    }

    fn build_remote_tracker(
        conn_id: &str,
        items: &[mbv_core::api::MediaItem],
        start_idx: usize,
    ) -> Option<ReconciliationTracker> {
        let occurrences = items
            .iter()
            .enumerate()
            .map(|(index, item)| {
                let mut occurrence = SubmittedOccurrence::new(index as u64 + 1, item.id.clone())
                    .runtime_ticks(item.runtime_ticks);
                if !item.playlist_item_id.is_empty() {
                    occurrence = occurrence.playlist_entry(item.playlist_item_id.clone());
                }
                occurrence
            })
            .collect();
        ReconciliationTracker::new(conn_id, occurrences, start_idx, Self::now_ms())
    }

    pub(super) fn issue_remote_intent(&mut self, intent: RemoteIntent) {
        if let Some(tracker) = self.remote_tracker.as_mut() {
            tracker.issue_intent(intent, Self::now_ms());
        }
    }

    pub(super) fn stop_remote_tracking(&mut self) {
        if let Some(tracker) = self.remote_tracker.as_mut() {
            tracker.stop_tracking();
            self.remote_tracker = None;
            self.flash_status("Remote tracking stopped".into());
        }
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
        if effects.is_empty() {
            self.flash_status_high("That occurrence is no longer available".into());
        } else {
            self.flash_status("Remote tracking re-anchored".into());
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
            .and_then(|s| s.now_playing_item_id.as_deref())
            .map(str::to_string);
        let target = current_remote_id
            .and_then(|rid| self.player_tab.items.iter().position(|i| i.id == rid))
            .and_then(|idx| {
                let t = idx as i64 + delta;
                if t >= 0 && (t as usize) < self.player_tab.items.len() {
                    Some(t as usize)
                } else {
                    None
                }
            })
            .map(|t| (t, self.player_tab.items[t].playback_position_ticks));
        if let Some((target_idx, _start_ticks)) = target {
            let intent = if delta > 0 {
                RemoteIntent::Next { target: target_idx }
            } else {
                RemoteIntent::Previous { target: target_idx }
            };
            let items = self.player_tab.items.clone();
            if self.remote_tracker.is_some() {
                self.issue_remote_intent(intent);
                let item_ids: Vec<String> = items.iter().map(|item| item.id.clone()).collect();
                let start_ticks = items
                    .get(target_idx)
                    .map_or(0, |item| item.playback_position_ticks);
                self.do_session_command(move |client| {
                    client.session_play_items(&id, &item_ids, target_idx, start_ticks)
                });
            } else {
                self.submit_attached_sequence(&id, &items, target_idx);
                self.issue_remote_intent(intent);
            }
        } else {
            self.do_session_command(move |c| c.session_transport(&id, fallback_cmd));
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
        let client = self.client.lock().unwrap().clone();
        let tx = self.sessions_tx.clone();
        let generation = self.next_session_poll_generation();
        std::thread::spawn(move || {
            if let Err(e) = f(&client) {
                let _ = tx.send(SessionEvent::CommandError(e));
                return;
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
