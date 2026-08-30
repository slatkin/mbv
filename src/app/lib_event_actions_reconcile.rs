use super::types_selection_modal::SelectionModalSource;
use super::App;

impl App {
    /// Shared acknowledged-progress reconcile used by both the bare owner
    /// (`LibEvent::AudiobookshelfProgressAcknowledged`) and a Local-daemon
    /// client (`PlayerEvent::AudiobookshelfProgress`): matches queue slots by
    /// provider-qualified identity, applies position/completion, writes every
    /// browse state's progress map, and persists the queue. A no-match event
    /// still updates browse state; only persistence is gated on a queue match.
    /// Generation gating is the caller's concern: the bare owner gates on its
    /// own runtime generation, while the daemon drops stale updates before
    /// emitting, so the daemon client reconciles unconditionally.
    pub(super) fn reconcile_audiobookshelf_progress(
        &mut self,
        library_item_id: &str,
        episode_id: &str,
        position_ticks: i64,
        current_time_seconds: f64,
        is_finished: bool,
    ) {
        let matching_slot_ids: Vec<_> = self
            .player_tab
            .queue
            .slots()
            .iter()
            .filter_map(|slot| {
                slot.item.as_audiobookshelf().and_then(|episode| {
                    (episode.library_item_id == library_item_id && episode.episode_id == episode_id)
                        .then_some(slot.slot_id)
                })
            })
            .collect();
        for slot_id in matching_slot_ids.iter().cloned() {
            self.player_tab
                .queue
                .apply_progress(slot_id, position_ticks, is_finished);
        }
        for state in &mut self.audiobookshelf_browse {
            state.progress.insert(
                (library_item_id.to_string(), episode_id.to_string()),
                mbv_core::audiobookshelf::AudiobookshelfProgress {
                    library_item_id: library_item_id.to_string(),
                    episode_id: episode_id.to_string(),
                    current_time_seconds,
                    is_finished,
                },
            );
        }
        // Refresh the mounted podcast modal if this progress update belongs to
        // its show, rebuilding it at its own component-owned selected filter
        // (split-browse-state-interaction-fields task 3.2). The shell ignores
        // this request when the modal is closed or showing another show.
        if self.audiobookshelf_browse.iter().any(|state| {
            state
                .shows
                .iter()
                .any(|show| show.library_item_id == library_item_id)
        }) {
            self.pending_overlay = Some(
                super::types_overlay::OverlayRequest::RefreshSelectionModalAtSelectedFilter {
                    source: SelectionModalSource::Podcast {
                        library_item_id: library_item_id.to_owned(),
                    },
                },
            );
        }
        if !matching_slot_ids.is_empty() {
            self.save_queue_state();
        }
    }

    /// Book-shaped counterpart to `reconcile_audiobookshelf_progress`:
    /// matches queue slots by `library_item_id` only and applies
    /// position/completion. Every book browse state's progress map (keyed by
    /// `library_item_id` only) is updated the way the episode reconcile
    /// updates `audiobookshelf_browse`.
    pub(super) fn reconcile_audiobookshelf_book_progress(
        &mut self,
        library_item_id: &str,
        position_ticks: i64,
        is_finished: bool,
    ) {
        let matching_slot_ids: Vec<_> = self
            .player_tab
            .queue
            .slots()
            .iter()
            .filter_map(|slot| {
                slot.item.as_audiobookshelf_book().and_then(|book| {
                    (book.library_item_id == library_item_id).then_some(slot.slot_id)
                })
            })
            .collect();
        for slot_id in matching_slot_ids.iter().cloned() {
            self.player_tab
                .queue
                .apply_progress(slot_id, position_ticks, is_finished);
        }
        let current_time_seconds = position_ticks as f64 / mbv_core::api::TICKS_PER_SECOND as f64;
        for state in &mut self.audiobookshelf_book_browse {
            state.progress.insert(
                library_item_id.to_string(),
                mbv_core::audiobookshelf::AudiobookshelfBookProgress {
                    library_item_id: library_item_id.to_string(),
                    current_time_seconds,
                    is_finished,
                },
            );
        }
        if !matching_slot_ids.is_empty() {
            self.save_queue_state();
        }
    }
}
