use crate::app::App;
use mbv_core::ctrl::{AudiobookshelfBookProgressEvent, AudiobookshelfProgressEvent};

impl App {
    pub(super) fn handle_audiobookshelf_progress(&mut self, ev: &AudiobookshelfProgressEvent) {
        // No client-side generation gate: the daemon drops stale updates before emitting, and its
        // generation counter is unrelated to this client's runtime generation.
        #[expect(
            clippy::cast_precision_loss,
            reason = "seconds↔ticks conversion through f64; no lossless integer-path conversion exists (approved, issue #804)"
        )]
        let current_time_seconds =
            ev.position_ticks as f64 / mbv_core::api::TICKS_PER_SECOND as f64;
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
