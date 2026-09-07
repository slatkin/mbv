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
#[derive(Debug, Clone)]
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

    /// Append a slot carrying an owner-assigned identity.
    pub fn append_with_id(&mut self, slot_id: QueueSlotId, item: QueueItem) {
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
