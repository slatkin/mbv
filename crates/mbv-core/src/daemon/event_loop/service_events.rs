//! Service- and socket-originated daemon events: Emby websocket messages,
//! enriched queue items, and acknowledged Audiobookshelf progress.

use super::super::{
    apply_audiobookshelf_book_progress, apply_audiobookshelf_progress, apply_queue_enriched,
    handle_ws, DaemonLoop,
};
use super::EventOutcome;
use crate::api::EmbyItem;
use crate::playback_queue::QueueSlotId;
use crate::player::{AudiobookshelfBookProgressUpdate, AudiobookshelfProgressUpdate};
use crate::service_runtime::SetupGeneration;
use crate::ws::WsEvent;

impl DaemonLoop {
    /// `DaemonEvent::Ws`: apply the service socket event when it belongs to
    /// the generation this daemon is running.
    pub(super) fn handle_ws_event(
        &mut self,
        generation: SetupGeneration,
        event: WsEvent,
    ) -> EventOutcome {
        if self
            .emby_runtime
            .as_ref()
            .is_some_and(|runtime| runtime.generation == generation)
        {
            handle_ws(
                event,
                Some(&self.client),
                &self.player,
                self.audio_only,
                &mut self.owner.core.queue,
                &mut self.owner.core.source,
                &mut self.owner.core.transitions,
                &self.shared_queue,
                &self.ctrl_clients,
            );
            return EventOutcome::DIRTY;
        }
        EventOutcome::CONTINUE
    }

    /// `DaemonEvent::QueueEnriched`: fold freshly fetched Emby items into an
    /// adopted queue.
    pub(super) fn handle_queue_enriched(
        &mut self,
        items: Vec<(QueueSlotId, EmbyItem)>,
    ) -> EventOutcome {
        apply_queue_enriched(
            items,
            &mut self.owner,
            &self.player,
            &self.shared_queue,
            &self.ctrl_clients,
        );
        EventOutcome::CONTINUE
    }

    /// `DaemonEvent::AudiobookshelfProgress`: apply acknowledged audiobook
    /// progress for the running service generation.
    pub(super) fn handle_audiobookshelf_progress(
        &mut self,
        update: &AudiobookshelfProgressUpdate,
    ) -> EventOutcome {
        apply_audiobookshelf_progress(
            update,
            self.audiobookshelf_runtime
                .as_ref()
                .map(|runtime| runtime.generation),
            &mut self.owner.core.queue,
            &self.ctrl_clients,
        );
        EventOutcome::CONTINUE
    }

    /// `DaemonEvent::AudiobookshelfBookProgress`: book-shaped counterpart to
    /// `AudiobookshelfProgress`, keyed by `library_item_id`.
    pub(super) fn handle_audiobookshelf_book_progress(
        &mut self,
        update: &AudiobookshelfBookProgressUpdate,
    ) -> EventOutcome {
        apply_audiobookshelf_book_progress(
            update,
            self.audiobookshelf_runtime
                .as_ref()
                .map(|runtime| runtime.generation),
            &mut self.owner.core.queue,
            &self.ctrl_clients,
        );
        EventOutcome::CONTINUE
    }
}
