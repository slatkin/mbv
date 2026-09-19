//! The Playback run's execution sequence.
//!
//! This is the smallest slot-bearing projection mpv needs: the ordered
//! `(QueueSlotId, QueueItem)` sequence with the observed active slot. It is
//! deliberately *not* a [`crate::playback_queue::PlaybackQueue`] — it carries no
//! queue revision, allocates no slot ids, and exposes only the lookups and
//! mutations the run performs while projecting the owner's Bound queue into mpv
//! (design D1/D2). Owner-assigned identity is supplied by callers; the sequence
//! never mints.

use crate::playback_queue::{QueueItem, QueueSlotId};

/// One entry of the execution sequence: an owner-assigned slot id and its item.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExecSlot {
    pub slot_id: QueueSlotId,
    pub item: QueueItem,
}

#[derive(Debug, Clone, Default)]
pub struct ExecutionSequence {
    slots: Vec<ExecSlot>,
    active_slot_id: Option<QueueSlotId>,
}

impl ExecutionSequence {
    pub fn empty() -> Self {
        Self::default()
    }

    /// Build from an owner-assigned `(slot id, item)` sequence. The active slot
    /// is retained only when it names one of the supplied slots.
    pub fn from_slot_items(
        slots: Vec<(QueueSlotId, QueueItem)>,
        active_slot_id: Option<QueueSlotId>,
    ) -> Self {
        assert_unique_slot_ids(&slots, &[]);
        let slots: Vec<ExecSlot> = slots
            .into_iter()
            .map(|(slot_id, item)| ExecSlot { slot_id, item })
            .collect();
        let active_slot_id =
            active_slot_id.filter(|id| slots.iter().any(|slot| slot.slot_id == *id));
        Self {
            slots,
            active_slot_id,
        }
    }

    pub fn slots(&self) -> &[ExecSlot] {
        &self.slots
    }

    pub fn len(&self) -> usize {
        self.slots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    pub fn slot(&self, slot_id: QueueSlotId) -> Option<&ExecSlot> {
        self.slots.iter().find(|slot| slot.slot_id == slot_id)
    }

    pub fn slot_index(&self, slot_id: QueueSlotId) -> Option<usize> {
        self.slots.iter().position(|slot| slot.slot_id == slot_id)
    }

    pub fn active_slot_id(&self) -> Option<QueueSlotId> {
        self.active_slot_id
    }

    pub fn active_slot(&self) -> Option<&ExecSlot> {
        self.active_slot_id.and_then(|id| self.slot(id))
    }

    /// True when the sequence contains any Audiobookshelf entries — the
    /// predicate that selects the active-file projection branch.
    pub fn has_audiobookshelf_entries(&self) -> bool {
        self.slots
            .iter()
            .any(|slot| slot.item.is_audiobookshelf_any())
    }

    /// Record a completed/stopped occurrence's resolved position on this
    /// slot, so a later local resume lookup (relative `Next`/`Previous`
    /// navigation back to it) sees what was actually watched this session
    /// instead of this sequence's submission-time snapshot. No-op if the slot
    /// is gone.
    pub fn apply_progress(&mut self, slot_id: QueueSlotId, position_ticks: i64, played: bool) {
        if let Some(slot) = self.slots.iter_mut().find(|slot| slot.slot_id == slot_id) {
            crate::playback::queue::apply_progress_to_queue_item(
                &mut slot.item,
                position_ticks,
                played,
            );
        }
    }

    /// Append a slot carrying an owner-assigned identity.
    pub fn append_with_id(&mut self, slot_id: QueueSlotId, item: QueueItem) {
        assert_unique_slot_ids(
            &[(slot_id, item.clone())],
            self.slots
                .iter()
                .map(|slot| slot.slot_id)
                .collect::<Vec<_>>()
                .as_slice(),
        );
        self.slots.push(ExecSlot { slot_id, item });
    }

    /// Point the active marker at an existing slot. Returns `false` and does
    /// nothing when the slot is absent.
    pub fn set_active_slot(&mut self, slot_id: QueueSlotId) -> bool {
        if self.slot_index(slot_id).is_none() {
            return false;
        }
        self.active_slot_id = Some(slot_id);
        true
    }

    /// Move a slot to `to_index`. Returns `false` when the slot is absent.
    pub fn move_slot(&mut self, slot_id: QueueSlotId, to_index: usize) -> bool {
        let Some(from_index) = self.slot_index(slot_id) else {
            return false;
        };
        let slot = self.slots.remove(from_index);
        let to_index = to_index.min(self.slots.len());
        self.slots.insert(to_index, slot);
        true
    }

    /// Remove a non-active slot. The active slot is left untouched; callers
    /// route active removal through [`Self::remove_active_slot_confirmed`].
    pub fn remove_slot(&mut self, slot_id: QueueSlotId) {
        if self.active_slot_id == Some(slot_id) {
            return;
        }
        if let Some(index) = self.slot_index(slot_id) {
            self.slots.remove(index);
        }
    }

    /// Remove a slot even when it is the active one, clearing the active
    /// marker in that case.
    pub fn remove_active_slot_confirmed(&mut self, slot_id: QueueSlotId) {
        let Some(index) = self.slot_index(slot_id) else {
            return;
        };
        self.slots.remove(index);
        if self.active_slot_id == Some(slot_id) {
            self.active_slot_id = None;
        }
    }

    pub fn clear(&mut self) {
        self.slots.clear();
        self.active_slot_id = None;
    }
}

// A real (not debug-only) assert: CI runs `cargo test --release`, where
// `debug_assert!` compiles out, and a slot-id collision here means the run
// silently desyncs from the owner's canonical queue.
fn assert_unique_slot_ids(incoming: &[(QueueSlotId, QueueItem)], existing: &[QueueSlotId]) {
    assert!(
        incoming.iter().enumerate().all(|(index, (slot_id, _))| {
            !incoming[..index].iter().any(|(other, _)| other == slot_id)
                && !existing.iter().any(|other| other == slot_id)
        }),
        "execution sequence slot identities must be unique"
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::playback_queue::FeedEntry;

    fn item() -> QueueItem {
        QueueItem::Feed(FeedEntry {
            guid: "guid".into(),
            title: "title".into(),
            enclosure_url: Some("https://example.test/audio".into()),
            link: None,
            mime_type: Some("audio/mpeg".into()),
            duration_ticks: Some(100),
            pub_date_secs: None,
            feed_kind: None,
            feed_id: None,
            position_ticks: 0,
            played: false,
        })
    }

    #[test]
    #[should_panic(expected = "execution sequence slot identities must be unique")]
    fn rejects_duplicate_slot_ids_on_submission() {
        let id = QueueSlotId::from_raw(7);
        ExecutionSequence::from_slot_items(vec![(id, item()), (id, item())], Some(id));
    }

    #[test]
    #[should_panic(expected = "execution sequence slot identities must be unique")]
    fn rejects_appended_slot_id_collision() {
        let id = QueueSlotId::from_raw(7);
        let mut sequence = ExecutionSequence::from_slot_items(vec![(id, item())], Some(id));
        sequence.append_with_id(id, item());
    }

    // Regression: this sequence used to be write-once (only ever replaced in
    // bulk), so relative Next/Previous navigation back to an already-played
    // slot resolved its resume position from the submission-time snapshot,
    // not what was actually watched this session.
    #[test]
    fn apply_progress_updates_the_slots_own_item() {
        let id = QueueSlotId::from_raw(1);
        let mut sequence = ExecutionSequence::from_slot_items(vec![(id, item())], Some(id));

        sequence.apply_progress(id, 42, false);

        assert_eq!(
            sequence.slot(id).unwrap().item.playback_position_ticks(),
            42
        );
        assert!(!sequence.slot(id).unwrap().item.played());
    }

    #[test]
    fn apply_progress_on_missing_slot_is_a_no_op() {
        let id = QueueSlotId::from_raw(1);
        let mut sequence = ExecutionSequence::from_slot_items(vec![(id, item())], Some(id));

        sequence.apply_progress(QueueSlotId::from_raw(99), 42, true);

        assert_eq!(sequence.slot(id).unwrap().item.playback_position_ticks(), 0);
    }
}
