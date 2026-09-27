use crate::app::App;
use mbv_core::ctrl::{AudiobookshelfBookProgressEvent, AudiobookshelfProgressEvent};

impl App {
    pub(super) fn handle_audiobookshelf_progress(&mut self, ev: &AudiobookshelfProgressEvent) {
        // No client-side generation gate: the daemon drops stale updates before emitting, and its
        // generation counter is unrelated to this client's runtime generation.
        let current_time_seconds = mbv_core::api::ticks_to_seconds(ev.position_ticks);
        self.reconcile_audiobookshelf_progress(
            &ev.library_item_id,
            &ev.episode_id,
            ev.position_ticks,
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
            ev.position_ticks,
            ev.is_finished,
        );
    }
}
