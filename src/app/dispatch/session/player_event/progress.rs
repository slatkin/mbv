use crate::app::App;
use mbv_ctrl::{AudiobookshelfBookProgressEvent, AudiobookshelfProgressEvent};

impl App {
    pub(super) fn handle_audiobookshelf_progress(&mut self, ev: &AudiobookshelfProgressEvent) {
        // No client-side generation gate: the daemon drops stale updates before emitting, and its
        // generation counter is unrelated to this client's runtime generation.
        let current_time_seconds = mbv_emby_model::ticks_to_seconds(ev.position_ticks);
        self.reconcile_audiobookshelf_progress(
            &ev.library_item_id,
            &ev.episode_id,
            current_time_seconds,
            ev.is_finished,
        );
    }

    pub(super) fn handle_audiobookshelf_book_progress(
        &mut self,
        ev: &AudiobookshelfBookProgressEvent,
    ) {
        self.reconcile_audiobookshelf_book_progress(
            &ev.library_item_id,
            mbv_emby_model::ticks_to_seconds(ev.position_ticks),
            ev.is_finished,
        );
    }
}
