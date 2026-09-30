use mbv_emby_model::EmbyItem;
use mbv_queue::{
    PlaybackQueue, QueueItem, QueueLineage, QueueRevision, QueueRevisionMint, QueueSlot,
    QueueSlotId, QueueSource,
};
use std::sync::Arc;

/// The Client's adopt-only view of one Player owner's queue.
#[derive(Clone, Debug)]
pub struct QueueView {
    cursor: usize,
    queue: PlaybackQueue,
    source: QueueSource,
    lineage: QueueLineage,
    pending_playback_slot: Option<QueueSlotId>,
    revision_mint: Arc<QueueRevisionMint>,
}

/// Why a Client is adopting an owner queue snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdoptCause {
    OwnAnswer,
    Background { held: bool },
    Replacement,
}

impl Default for QueueView {
    fn default() -> Self {
        let revision_mint = Arc::new(QueueRevisionMint::default());
        Self {
            cursor: 0,
            queue: PlaybackQueue::from_queue_items(Vec::new(), None, Arc::clone(&revision_mint)),
            source: QueueSource::Unknown,
            lineage: QueueLineage::default(),
            pending_playback_slot: None,
            revision_mint,
        }
    }
}

impl QueueView {
    /// Build a view from an initial owner snapshot.
    #[must_use]
    pub fn from_snapshot(state: &mbv_ctrl::UnifiedQueueStateData) -> Self {
        let mut view = Self::default();
        view.adopt(state, AdoptCause::Replacement);
        view
    }

    /// Adopt an owner snapshot and reconcile the selected slot.
    pub fn adopt(&mut self, state: &mbv_ctrl::UnifiedQueueStateData, cause: AdoptCause) {
        let previous_slot = self.slot_id_at(self.cursor);
        let old_cursor = self.cursor;
        let replacement = matches!(cause, AdoptCause::Replacement) || self.lineage != state.lineage;
        self.queue = queue_from_snapshot(state, Arc::clone(&self.revision_mint));
        self.cursor = if replacement {
            0
        } else if matches!(cause, AdoptCause::Background { held: false }) {
            state
                .active_slot
                .and_then(|slot_id| self.queue.slot_index(QueueSlotId::from_raw(slot_id)))
                .unwrap_or_else(|| clamp_cursor(old_cursor, self.queue.len()))
        } else {
            previous_slot
                .and_then(|slot_id| self.queue.slot_index(slot_id))
                .unwrap_or_else(|| clamp_cursor(old_cursor, self.queue.len()))
        };
        self.source = state.source.clone();
        self.lineage = state.lineage;
        self.pending_playback_slot = pending_playback_slot(state);
    }

    /// Set the selected queue position after user navigation.
    pub fn set_cursor(&mut self, cursor: usize) {
        self.cursor = clamp_cursor(cursor, self.queue.len());
    }

    #[must_use]
    pub fn slots(&self) -> &[QueueSlot] {
        self.queue.slots()
    }

    #[must_use]
    pub fn item_at(&self, index: usize) -> Option<&QueueItem> {
        self.slots().get(index).map(|slot| &slot.item)
    }

    #[must_use]
    pub fn emby_item_at(&self, index: usize) -> Option<&EmbyItem> {
        self.item_at(index).and_then(QueueItem::as_emby)
    }

    #[must_use]
    pub fn slot_id_at(&self, index: usize) -> Option<QueueSlotId> {
        self.slots().get(index).map(|slot| slot.slot_id)
    }

    #[must_use]
    pub fn slot_index(&self, slot_id: QueueSlotId) -> Option<usize> {
        self.queue.slot_index(slot_id)
    }

    #[must_use]
    pub fn revision(&self) -> QueueRevision {
        self.queue.revision()
    }

    #[must_use]
    pub fn source(&self) -> &QueueSource {
        &self.source
    }

    #[must_use]
    pub fn lineage(&self) -> QueueLineage {
        self.lineage
    }

    #[must_use]
    pub fn pending_playback_slot(&self) -> Option<QueueSlotId> {
        self.pending_playback_slot
    }

    #[must_use]
    pub fn total_queue_len(&self) -> usize {
        self.queue.len()
    }

    #[must_use]
    pub fn cursor(&self) -> usize {
        self.cursor
    }
}

fn queue_from_snapshot(
    state: &mbv_ctrl::UnifiedQueueStateData,
    revision_mint: Arc<QueueRevisionMint>,
) -> PlaybackQueue {
    let slots = state
        .slots
        .iter()
        .map(|slot| (QueueSlotId::from_raw(slot.slot_id), slot.item.clone()))
        .collect();
    PlaybackQueue::from_slot_items(
        slots,
        state.active_slot.map(QueueSlotId::from_raw),
        revision_mint,
    )
}

fn pending_playback_slot(state: &mbv_ctrl::UnifiedQueueStateData) -> Option<QueueSlotId> {
    state
        .queued_latest_transition
        .as_ref()
        .or(state.in_flight_transition.as_ref())
        .map(|transition| QueueSlotId::from_raw(transition.target_slot))
}

fn clamp_cursor(cursor: usize, queue_len: usize) -> usize {
    if queue_len == 0 {
        0
    } else {
        cursor.min(queue_len - 1)
    }
}

#[cfg(test)]
mod tests {
    use super::{AdoptCause, QueueView};
    use mbv_ctrl::{UnifiedQueueSlot, UnifiedQueueStateData};
    use mbv_queue::{QueueItem, QueueLineage, QueueSource};
    use rstest::rstest;

    fn snapshot(ids: &[u64], active: Option<u64>, lineage: u64) -> UnifiedQueueStateData {
        let items = crate::app::tests::make_items(ids.len());
        UnifiedQueueStateData {
            status: mbv_ctrl::player::PlayerStatus::default(),
            slots: ids
                .iter()
                .zip(items)
                .map(|(&slot_id, item)| UnifiedQueueSlot {
                    slot_id,
                    item: QueueItem::Emby(Box::new(item)),
                })
                .collect(),
            active_slot: active,
            revision: 1,
            source: QueueSource::Remote,
            lineage: QueueLineage(lineage),
            in_flight_transition: None,
            queued_latest_transition: None,
        }
    }

    #[rstest]
    #[case::move_keeps_selected_slot(&[1, 2, 3], 1, &[1, 3, 2], None, 7, AdoptCause::OwnAnswer, 2)]
    #[case::removal_selects_next(&[1, 2, 3], 1, &[1, 3], None, 7, AdoptCause::OwnAnswer, 1)]
    #[case::removing_last_selects_new_last(&[1, 2, 3], 2, &[1, 2], None, 7, AdoptCause::OwnAnswer, 1)]
    #[case::background_follows_active(&[1, 2, 3], 0, &[1, 2, 3], Some(3), 7, AdoptCause::Background { held: false }, 2)]
    #[case::background_held_keeps_selection(&[1, 2, 3], 1, &[3, 2, 1], Some(3), 7, AdoptCause::Background { held: true }, 1)]
    #[case::replacement_selects_start(&[1, 2, 3], 1, &[8, 9], Some(9), 7, AdoptCause::Replacement, 0)]
    fn adoption_reconciles_selection_by_slot(
        #[case] old_ids: &[u64],
        #[case] old_cursor: usize,
        #[case] new_ids: &[u64],
        #[case] active: Option<u64>,
        #[case] new_lineage: u64,
        #[case] cause: AdoptCause,
        #[case] expected_cursor: usize,
    ) {
        let mut view = QueueView::from_snapshot(&snapshot(old_ids, Some(old_ids[0]), 7));
        view.set_cursor(old_cursor);

        view.adopt(&snapshot(new_ids, active, new_lineage), cause);

        assert_eq!(view.cursor(), expected_cursor);
    }

    #[test]
    fn every_adoption_mints_a_new_revision() {
        let state = snapshot(&[1, 2], Some(1), 7);
        let mut view = QueueView::from_snapshot(&state);
        let first = view.revision();

        view.adopt(&state, AdoptCause::OwnAnswer);
        let second = view.revision();
        view.adopt(&state, AdoptCause::Background { held: true });

        assert!(second > first);
        assert!(view.revision() > second);
    }
}
