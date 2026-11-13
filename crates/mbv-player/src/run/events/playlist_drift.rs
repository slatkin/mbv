use super::super::{PlaybackRun, divergent_entry};
use mbv_ctrl::player::PlayerEvent;

impl PlaybackRun {
    /// Whether a queue transition is in flight: the guard both
    /// `on_playlist_pos_changed` and `on_playlist_count_changed` apply while
    /// a queue load plan's rebuild lands (the transient window its layout
    /// observations arrive in).
    fn queue_transition_in_flight(&self) -> bool {
        self.pending_initial_playlist_layout
            || !self.load_is_ready()
            || !self.run_lifecycle.jump_arm().is_idle()
    }

    pub(crate) fn on_playlist_pos_changed(&mut self, pos: i64, mpv_pos_ticks: i64) {
        if self.run_lifecycle.is_active_file() {
            return;
        }
        if self.queue_transition_in_flight() {
            tracing::debug!(name: "player.playlist_position.ignored", target: "player", position = pos, "ignoring transient playlist position during queue transition");
            return;
        }
        let queue_len = self.queue_len();
        let Some(index) = divergent_entry(pos, self.current_idx, queue_len) else {
            if usize::try_from(pos).is_ok_and(|index| index >= queue_len) {
                tracing::warn!(name: "player.playlist_position.out_of_range", target: "player", position = pos, queue_length = queue_len, "ignoring out-of-range playlist position");
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
        tracing::warn!(name: "player.playlist_position.diverged", target: "player", position = index, previous_index = previous, "mpv moved off the active entry; reporting follows it");
        self.report_stopped_and_adopt_mpv_entry(previous_slot, previous_pos, index, mpv_pos_ticks);
    }

    pub(crate) fn on_playlist_count_changed(&mut self, count: usize) {
        if self.run_lifecycle.is_active_file() {
            return;
        }
        // A queue load plan's own rebuild moves the observed count while the
        // run's queue copy still holds the outgoing queue: the same transient
        // window `on_playlist_pos_changed` suppresses, and reading it as
        // external drift would delete the copy's slots the plan is about to
        // replace (the deleted items then keep playing — local-daemon.log
        // 2026-11-08T19:33:53, the GUI queue fix). The final layout is
        // `reassert_queue_layout`'s check, not this handler's.
        if self.queue_transition_in_flight() {
            self.settle_or_ignore_transient_count(count);
            return;
        }
        if count == self.queue_len() {
            return;
        }
        let old_n = self.queue_len();
        if count < old_n {
            self.on_playlist_count_decreased(old_n, count);
        } else {
            self.on_playlist_count_increased(old_n, count);
        }
    }

    /// The rebuild's settle: the first observed count equal to the copy's
    /// length says the load plan has landed, so the gate the arming command
    /// left up (its count observations arrive on later run loop iterations)
    /// disarms here. A count that does not match stays suppressed — the plan
    /// may still be mid-flight — and the start item's `PlaybackRestart` disarms
    /// the gate when playback resumes regardless.
    fn settle_or_ignore_transient_count(&mut self, count: usize) {
        if count == self.queue_len() {
            self.pending_initial_playlist_layout = false;
        }
        tracing::debug!(name: "player.playlist_count.ignored", target: "player", count, "ignoring transient playlist count during queue transition");
    }

    fn on_playlist_count_decreased(&mut self, old_n: usize, count: usize) {
        let removed = old_n - count;
        tracing::warn!(name: "player.playlist_count.decreased", target: "player", previous_count = old_n, count, removed, "mpv playlist entries removed externally");
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
        // permitted, design D2); the increase-branch clamp only covers a
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
    }

    fn on_playlist_count_increased(&mut self, old_n: usize, count: usize) {
        let added = count - old_n;
        tracing::warn!(name: "player.playlist_count.increased", target: "player", previous_count = old_n, count, added, "mpv playlist entries added externally");
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
