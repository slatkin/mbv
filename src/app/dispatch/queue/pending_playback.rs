use super::{App, QueueOpEdit, ToastSeverity};
use mbv_ctrl::player::PlayerCommand;
use mbv_emby_model::EmbyItem;

impl App {
    pub(super) fn execute_pending_play_items(
        &mut self,
        items: Vec<EmbyItem>,
        start_idx: usize,
        source: mbv_queue::QueueSource,
    ) {
        if self.player.is_remote_disconnected() {
            self.flash(
                crate::app::dispatch::actions::CONNECTION_LOST_MESSAGE.into(),
                ToastSeverity::Warning,
            );
            return;
        }
        self.start_pending_queue_playback(items, start_idx, source);
    }

    fn start_pending_queue_playback(
        &mut self,
        items: Vec<EmbyItem>,
        start_idx: usize,
        source: mbv_queue::QueueSource,
    ) {
        // Row 5.3 (design D6): the replacement is an answered owner op —
        // the owner replaces its canonical queue, begins playback at
        // `start_idx`, and the Client adopts the resulting snapshot; the
        // Client holds no editable queue of its own.
        let link = self.playing_queue_link();
        let sent = self.replace_emby_queue_on_owner(link, items, start_idx, source);
        if sent == QueueOpEdit::NotApplied {
            return;
        }
        self.collapse_pinned_view_after_play();
        let _ = self
            .player
            .send_command(PlayerCommand::SetMute(self.mute_on));
    }

    pub(super) fn execute_pending_queue_clear(&mut self) {
        let link = self.viewed_queue_link();
        let had_items = self.queue_for_link(link).total_queue_len() > 0;
        if had_items {
            // Row 5.3 (design D6): the clear reaches the viewed queue's owner
            // as an answered op (UnifiedQueueClearOp for a capable owner); the
            // owner stops its player, clears the canonical queue, and the
            // Client adopts the empty queue only from the answer.
            let sent = self.queue_op(link, mbv_remote_player::QueueOp::Clear);
            if sent == QueueOpEdit::NotApplied {
                return;
            }
            if self.local_queue_metadata_applies(link) {
                self.clear_local_queue_metadata();
            } else {
                self.remote_queue_undo_stack.clear();
            }
            self.advance_queue_epoch();
        }
        self.flash("Queue cleared".into(), ToastSeverity::Success);
    }
}
