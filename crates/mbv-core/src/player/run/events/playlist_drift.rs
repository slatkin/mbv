use super::super::{divergent_entry, PlaybackRun};
use crate::player::PlayerEvent;

impl PlaybackRun {
    pub(in crate::player) fn on_playlist_pos_changed(&mut self, pos: i64, mpv_pos_ticks: i64) {
        if self.active_file {
            return;
        }
        if self.pending_initial_playlist_layout
            || !self.load_state.is_ready()
            || self.forced_slot_id.is_some()
        {
            log::debug!(
                target: "player",
                "ignoring transient playlist-pos={pos} while queue transition is pending"
            );
            return;
        }
        let queue_len = self.queue_len();
        let Some(index) = divergent_entry(pos, self.current_idx, queue_len) else {
            if usize::try_from(pos).is_ok_and(|index| index >= queue_len) {
                log::warn!(
                    target: "player",
                    "ignoring out-of-range playlist-pos={pos} for queue len {queue_len}"
                );
            }
            return;
        };
        // Nothing in flight, so mpv navigated itself: it is authoritative for
        // what is playing now. Report the outgoing item stopped, adopt mpv's
        // entry, re-point reporting at it, and announce the observation (no
        // request identity) — otherwise the run keeps a stale ordinal while
        // another item streams and Emby keeps counting the old one as playing.
        let previous = self.current_idx;
        let previous_slot = self.slot_id_at(previous);
        let previous_pos = self.last_valid_pos;
        log::warn!(
            target: "player",
            "playlist-pos={index}: mpv moved off the active entry (was {previous}); reporting now follows it"
        );
        self.report_stopped_and_adopt_mpv_entry(previous_slot, previous_pos, index, mpv_pos_ticks);
    }

    pub(in crate::player) fn on_playlist_count_changed(&mut self, count: usize) {
        if self.active_file {
            return;
        }
        if count == self.queue_len() {
            return;
        }
        let old_n = self.queue_len();
        if count < old_n {
            let removed = old_n - count;
            log::warn!(target: "player", "playlist-count dropped from {old_n} to {count}: {removed} item(s) removed externally");
            let removed_slot_ids: Vec<_> = self
                .queue
                .slots()
                .iter()
                .skip(count)
                .map(|slot| slot.slot_id)
                .collect();
            for slot_id in removed_slot_ids {
                if self.active_slot_id() == Some(slot_id) {
                    self.queue.remove_active_slot_confirmed(slot_id);
                } else {
                    self.queue.remove_slot(slot_id);
                }
            }
            // Re-derive this run's mpv-local ordinal from the still-active
            // slot's identity after an external shrink (identity -> ordinal is
            // permitted, design D2); the else-branch clamp below only covers a
            // shrink past the end, not an earlier removal shifting the active
            // slot down.
            self.current_idx = self
                .active_slot_id()
                .and_then(|s| self.queue.slot_index(s))
                .unwrap_or(self.current_idx);
            self.sync_status_position();
            let _ = self.event_tx.send(PlayerEvent::QueueDesynced(format!(
                "Queue desynced: {removed} item(s) removed externally"
            )));
        } else {
            let added = count - old_n;
            log::warn!(target: "player", "playlist-count increased from {old_n} to {count}: {added} item(s) added externally");
            // We cannot reconstruct the added EmbyItems from mpv's playlist,
            // so we keep the queue as-is. Clamp current_idx to the last
            // known item in case the external tool also changed position.
            if self.current_idx >= self.queue_len() && self.queue_len() > 0 {
                self.current_idx = self.queue_len() - 1;
            }
            self.sync_status_position();
            let _ = self.event_tx.send(PlayerEvent::QueueDesynced(format!(
                "Queue desynced: {added} item(s) added externally"
            )));
        }
    }
}
