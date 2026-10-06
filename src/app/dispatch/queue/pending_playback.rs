use super::{App, QueueOpEdit, ToastSeverity};
use mbv_ctrl::player::PlayerCommand;
use mbv_emby_model::EmbyItem;
use mbv_queue::QueueItem;

impl App {
    pub(super) fn execute_pending_play_items(
        &mut self,
        items: Vec<EmbyItem>,
        start_idx: usize,
        source: mbv_queue::QueueSource,
        autostart: bool,
    ) {
        if autostart && self.player.is_remote_disconnected() {
            self.flash(
                crate::app::dispatch::actions::CONNECTION_LOST_MESSAGE.into(),
                ToastSeverity::Warning,
            );
            return;
        }
        if !autostart {
            // Loading without starting playback keeps the owner's idle-load
            // request/result path (row 5.3, design D6): unblocked, and the
            // loaded playlist shows only through the owner's accepted state.
            self.load_idle_queue_on_owner(
                items
                    .into_iter()
                    .map(|item| QueueItem::Emby(Box::new(item)))
                    .collect(),
                start_idx,
                source,
            );
            return;
        }
        self.start_pending_queue_playback(items, start_idx, source);
    }

    /// Sends the idle queue load to the current Player owner. Returns whether
    /// the request was sent (and so accepted for correlation); a failed send
    /// flashes and reports `false`, so a session hand-off does not start
    /// playback of a queue the owner never accepted.
    pub(in crate::app) fn load_idle_queue_on_owner(
        &mut self,
        items: Vec<QueueItem>,
        start_idx: usize,
        source: mbv_queue::QueueSource,
    ) -> bool {
        let request_id = self.next_owner_queue_load_request;
        self.next_owner_queue_load_request = self.next_owner_queue_load_request.saturating_add(1);
        let slots = items
            .into_iter()
            .enumerate()
            .map(|(index, item)| mbv_ctrl::UnifiedQueueSlot {
                slot_id: (index + 1) as u64,
                item,
            })
            .collect();
        let result = self
            .player
            .remote()
            .load_queue_idle(request_id, slots, start_idx, source);
        match result {
            Ok(()) => {
                self.set_queue_scope(self.playing_queue_scope());
                true
            }
            Err(_) if self.player.is_remote_disconnected() => {
                self.flash(
                    crate::app::dispatch::actions::CONNECTION_LOST_MESSAGE.into(),
                    ToastSeverity::Warning,
                );
                false
            }
            Err(reason) => {
                self.flash(reason.to_string(), ToastSeverity::Error);
                false
            }
        }
    }

    fn start_pending_queue_playback(
        &mut self,
        items: Vec<EmbyItem>,
        start_idx: usize,
        source: mbv_queue::QueueSource,
    ) {
        // Attached Emby session: the queue is installed on the Player owner
        // through the idle-load path (ruling on row 5.3: the owner's Replace
        // op always starts playback, so it must not be used while another
        // target owns playback), then playback starts on the session.
        if let Some(ref conn_id) = self.connected_session_id.clone() {
            if !self.load_idle_queue_on_owner(
                items
                    .iter()
                    .map(|item| QueueItem::Emby(Box::new(item.clone())))
                    .collect(),
                start_idx,
                source,
            ) {
                return;
            }
            self.clear_playback_overlays();
            let label = items
                .get(start_idx)
                .map(mbv_emby_model::EmbyItem::playback_label)
                .unwrap_or_default();
            let id = conn_id.clone();
            self.flash(
                format!("Requesting playback: {label}"),
                ToastSeverity::Neutral,
            );
            self.submit_attached_sequence(&id, &items, start_idx);
            return;
        }
        // Row 5.3 (design D6): the replacement is an answered owner op —
        // the owner replaces its canonical queue, begins playback at
        // `start_idx`, and the Client adopts the resulting snapshot; the
        // Client holds no editable queue of its own.
        let scope = self.playing_queue_scope();
        let sent = self.replace_emby_queue_on_owner(scope, items, start_idx, source);
        if sent == QueueOpEdit::NotApplied {
            return;
        }
        self.set_queue_scope(scope);
        let _ = self
            .player
            .send_command(PlayerCommand::SetMute(self.mute_on));
    }

    pub(super) fn execute_pending_queue_clear(&mut self) {
        let scope = self.viewed_queue_scope();
        let had_items = self.queue_for_scope(scope).total_queue_len() > 0;
        if had_items {
            // Row 5.3 (design D6): the clear reaches the viewed queue's owner
            // as an answered op (UnifiedQueueClearOp for a capable owner); the
            // owner stops its player, clears the canonical queue, and the
            // Client adopts the empty queue only from the answer.
            let sent = self.queue_op(scope, mbv_remote_player::QueueOp::Clear);
            if sent == QueueOpEdit::NotApplied {
                return;
            }
            if self.local_queue_metadata_applies(scope) {
                self.clear_local_queue_metadata();
            } else {
                self.remote_queue_undo_stack.clear();
            }
            self.advance_queue_epoch();
        }
        self.flash("Queue cleared".into(), ToastSeverity::Success);
    }
}
