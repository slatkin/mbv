use std::collections::HashMap;

use crate::api::{EmbyItem, TICKS_PER_SECOND};

const PROGRESS_CONFIRMATION_TOLERANCE_TICKS: i64 = TICKS_PER_SECOND * 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct QueueSlotId(u64);

impl QueueSlotId {
    pub fn raw(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct QueueRevision(u64);

impl QueueRevision {
    pub fn raw(self) -> u64 {
        self.0
    }

    fn bump(&mut self) {
        self.0 = self.0.saturating_add(1);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlotProgress {
    pub position_ticks: i64,
    pub played: bool,
}

impl SlotProgress {
    pub fn from_item(item: &EmbyItem) -> Self {
        Self {
            position_ticks: item.playback_position_ticks,
            played: item.played,
        }
    }

    fn matches_server_confirmation(&self, item: &EmbyItem) -> bool {
        (self.position_ticks - item.playback_position_ticks).abs()
            <= PROGRESS_CONFIRMATION_TOLERANCE_TICKS
            && self.played == item.played
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgressState {
    pub local: SlotProgress,
    pub pending_sync: Option<SlotProgress>,
}

impl ProgressState {
    fn from_item(item: &EmbyItem) -> Self {
        Self {
            local: SlotProgress::from_item(item),
            pending_sync: None,
        }
    }

    fn apply_to_item(&self, item: &mut EmbyItem) {
        item.playback_position_ticks = self.local.position_ticks;
        item.played = self.local.played;
    }
}

#[derive(Debug, Clone)]
pub struct QueueSlot {
    pub slot_id: QueueSlotId,
    pub item: EmbyItem,
    pub progress_state: ProgressState,
}

impl QueueSlot {
    fn new(slot_id: QueueSlotId, item: EmbyItem) -> Self {
        let progress_state = ProgressState::from_item(&item);
        Self {
            slot_id,
            item,
            progress_state,
        }
    }
}

#[derive(Debug, Clone)]
pub enum RemoveSlotResult {
    Removed(Box<QueueSlot>),
    RequiresActiveConfirmation(QueueSlotId),
    NotFound,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueueMutationResult<T> {
    Applied(T),
    NotFound,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RefreshMergeResult {
    pub updated_slots: Vec<QueueSlotId>,
    pub pruned_slots: Vec<QueueSlotId>,
    pub protected_slots: Vec<QueueSlotId>,
    pub pending_confirmed_slots: Vec<QueueSlotId>,
    pub stale_pending_slots: Vec<QueueSlotId>,
}

#[derive(Debug, Clone)]
pub struct PlaybackQueue {
    slots: Vec<QueueSlot>,
    active_slot_id: Option<QueueSlotId>,
    revision: QueueRevision,
    next_slot_id: u64,
}

impl Default for PlaybackQueue {
    fn default() -> Self {
        Self::from_items(Vec::new(), None)
    }
}

impl PlaybackQueue {
    pub fn from_items(items: Vec<EmbyItem>, active_index: Option<usize>) -> Self {
        let mut queue = Self {
            slots: Vec::with_capacity(items.len()),
            active_slot_id: None,
            revision: QueueRevision::default(),
            next_slot_id: 1,
        };

        for item in items {
            let slot_id = queue.allocate_slot_id();
            queue.slots.push(QueueSlot::new(slot_id, item));
        }

        queue.active_slot_id =
            active_index.and_then(|index| queue.slots.get(index).map(|s| s.slot_id));
        queue
    }

    pub fn revision(&self) -> QueueRevision {
        self.revision
    }

    pub fn slots(&self) -> &[QueueSlot] {
        &self.slots
    }

    pub fn active_slot_id(&self) -> Option<QueueSlotId> {
        self.active_slot_id
    }

    pub fn active_slot(&self) -> Option<&QueueSlot> {
        self.active_slot_id.and_then(|slot_id| self.slot(slot_id))
    }

    pub fn clear_active_slot(&mut self) {
        self.active_slot_id = None;
    }

    pub fn slot(&self, slot_id: QueueSlotId) -> Option<&QueueSlot> {
        self.slots.iter().find(|slot| slot.slot_id == slot_id)
    }

    pub fn slot_index(&self, slot_id: QueueSlotId) -> Option<usize> {
        self.slots.iter().position(|slot| slot.slot_id == slot_id)
    }

    pub fn append(&mut self, item: EmbyItem) -> QueueSlotId {
        self.insert(self.slots.len(), item)
    }

    pub fn insert(&mut self, index: usize, item: EmbyItem) -> QueueSlotId {
        let slot_id = self.allocate_slot_id();
        let index = index.min(self.slots.len());
        self.slots.insert(index, QueueSlot::new(slot_id, item));
        self.revision.bump();
        slot_id
    }

    pub fn set_active_slot(&mut self, slot_id: QueueSlotId) -> QueueMutationResult<()> {
        if self.slot_index(slot_id).is_none() {
            return QueueMutationResult::NotFound;
        }
        self.active_slot_id = Some(slot_id);
        QueueMutationResult::Applied(())
    }

    pub fn remove_slot(&mut self, slot_id: QueueSlotId) -> RemoveSlotResult {
        if self.active_slot_id == Some(slot_id) {
            return RemoveSlotResult::RequiresActiveConfirmation(slot_id);
        }
        self.remove_existing_slot(slot_id)
            .map(Box::new)
            .map(RemoveSlotResult::Removed)
            .unwrap_or(RemoveSlotResult::NotFound)
    }

    pub fn remove_active_slot_confirmed(&mut self, slot_id: QueueSlotId) -> RemoveSlotResult {
        let Some(index) = self.slot_index(slot_id) else {
            return RemoveSlotResult::NotFound;
        };
        let removed = self.slots.remove(index);
        self.revision.bump();

        if self.active_slot_id == Some(slot_id) {
            self.active_slot_id = None;
        }

        RemoveSlotResult::Removed(Box::new(removed))
    }

    pub fn consume_slot(&mut self, slot_id: QueueSlotId) -> QueueMutationResult<QueueSlot> {
        match self.remove_existing_slot(slot_id) {
            Some(slot) => QueueMutationResult::Applied(slot),
            None => QueueMutationResult::NotFound,
        }
    }

    pub fn move_slot(&mut self, slot_id: QueueSlotId, to_index: usize) -> QueueMutationResult<()> {
        let Some(from_index) = self.slot_index(slot_id) else {
            return QueueMutationResult::NotFound;
        };
        let slot = self.slots.remove(from_index);
        let to_index = to_index.min(self.slots.len());
        self.slots.insert(to_index, slot);
        self.revision.bump();
        QueueMutationResult::Applied(())
    }

    pub fn update_slot_item(
        &mut self,
        slot_id: QueueSlotId,
        item: EmbyItem,
    ) -> QueueMutationResult<()> {
        let Some(slot) = self.slots.iter_mut().find(|slot| slot.slot_id == slot_id) else {
            return QueueMutationResult::NotFound;
        };
        slot.item = item;
        slot.progress_state.local = SlotProgress::from_item(&slot.item);
        QueueMutationResult::Applied(())
    }

    pub fn apply_progress(
        &mut self,
        slot_id: QueueSlotId,
        position_ticks: i64,
        played: bool,
    ) -> QueueMutationResult<()> {
        let Some(slot) = self.slots.iter_mut().find(|slot| slot.slot_id == slot_id) else {
            return QueueMutationResult::NotFound;
        };
        slot.progress_state.local = SlotProgress {
            position_ticks,
            played,
        };
        slot.progress_state.apply_to_item(&mut slot.item);
        QueueMutationResult::Applied(())
    }

    pub fn mark_progress_sync_pending(
        &mut self,
        slot_id: QueueSlotId,
    ) -> QueueMutationResult<SlotProgress> {
        let Some(slot) = self.slots.iter_mut().find(|slot| slot.slot_id == slot_id) else {
            return QueueMutationResult::NotFound;
        };
        let pending = slot.progress_state.local.clone();
        slot.progress_state.pending_sync = Some(pending.clone());
        QueueMutationResult::Applied(pending)
    }

    pub fn merge_refresh(&mut self, fetched_items: Vec<EmbyItem>) -> RefreshMergeResult {
        let mut fetched_by_item_id = group_fetched_items_by_item_id(fetched_items);
        let old_slots = std::mem::take(&mut self.slots);
        let mut result = RefreshMergeResult::default();
        let mut merged_slots = Vec::with_capacity(old_slots.len());
        let active_slot_id = self.active_slot_id;

        for mut slot in old_slots {
            let fetched = fetched_by_item_id
                .get_mut(&slot.item.id)
                .map(FetchedItemMatches::next_match);
            match fetched {
                Some(fetched_item) => {
                    self.merge_fetched_slot(&mut slot, fetched_item, active_slot_id, &mut result);
                    merged_slots.push(slot);
                }
                None if should_protect_missing_slot(&slot, active_slot_id) => {
                    result.protected_slots.push(slot.slot_id);
                    merged_slots.push(slot);
                }
                None => {
                    result.pruned_slots.push(slot.slot_id);
                    self.revision.bump();
                }
            }
        }

        self.slots = merged_slots;
        if let Some(active_slot_id) = self.active_slot_id {
            if self.slot_index(active_slot_id).is_none() {
                self.active_slot_id = None;
            }
        }
        result
    }

    fn allocate_slot_id(&mut self) -> QueueSlotId {
        let slot_id = QueueSlotId(self.next_slot_id);
        self.next_slot_id = self.next_slot_id.saturating_add(1);
        slot_id
    }

    fn remove_existing_slot(&mut self, slot_id: QueueSlotId) -> Option<QueueSlot> {
        let index = self.slot_index(slot_id)?;
        let removed = self.slots.remove(index);
        self.revision.bump();

        if self.active_slot_id == Some(slot_id) {
            self.active_slot_id = self
                .slots
                .get(index)
                .or_else(|| self.slots.last())
                .map(|s| s.slot_id);
        }

        Some(removed)
    }

    fn merge_fetched_slot(
        &mut self,
        slot: &mut QueueSlot,
        fetched_item: EmbyItem,
        active_slot_id: Option<QueueSlotId>,
        result: &mut RefreshMergeResult,
    ) {
        let is_active = active_slot_id == Some(slot.slot_id);
        if let Some(pending) = slot.progress_state.pending_sync.clone() {
            if pending.matches_server_confirmation(&fetched_item) {
                slot.progress_state.pending_sync = None;
                slot.item = fetched_item;
                if is_active {
                    slot.progress_state.apply_to_item(&mut slot.item);
                    result.protected_slots.push(slot.slot_id);
                } else {
                    slot.progress_state.local = SlotProgress::from_item(&slot.item);
                }
                result.pending_confirmed_slots.push(slot.slot_id);
                result.updated_slots.push(slot.slot_id);
            } else {
                result.stale_pending_slots.push(slot.slot_id);
                result.protected_slots.push(slot.slot_id);
            }
            return;
        }

        if is_active {
            let local_progress = slot.progress_state.local.clone();
            slot.item = fetched_item;
            slot.progress_state.local = local_progress;
            slot.progress_state.apply_to_item(&mut slot.item);
            result.protected_slots.push(slot.slot_id);
            result.updated_slots.push(slot.slot_id);
            return;
        }

        slot.item = fetched_item;
        slot.progress_state.local = SlotProgress::from_item(&slot.item);
        result.updated_slots.push(slot.slot_id);
    }
}

#[derive(Debug)]
struct FetchedItemMatches {
    items: Vec<EmbyItem>,
    next_index: usize,
}

impl FetchedItemMatches {
    fn new(item: EmbyItem) -> Self {
        Self {
            items: vec![item],
            next_index: 0,
        }
    }

    fn push(&mut self, item: EmbyItem) {
        self.items.push(item);
    }

    fn next_match(&mut self) -> EmbyItem {
        let index = self.next_index.min(self.items.len() - 1);
        self.next_index = self.next_index.saturating_add(1);
        self.items[index].clone()
    }
}

fn group_fetched_items_by_item_id(items: Vec<EmbyItem>) -> HashMap<String, FetchedItemMatches> {
    let mut grouped = HashMap::new();
    for item in items {
        grouped
            .entry(item.id.clone())
            .and_modify(|matches: &mut FetchedItemMatches| matches.push(item.clone()))
            .or_insert_with(|| FetchedItemMatches::new(item));
    }
    grouped
}

fn should_protect_missing_slot(slot: &QueueSlot, active_slot_id: Option<QueueSlotId>) -> bool {
    active_slot_id == Some(slot.slot_id) || slot.progress_state.pending_sync.is_some()
}

#[cfg(test)]
#[path = "playback_queue_tests.rs"]
mod tests;
