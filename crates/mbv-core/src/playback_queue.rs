use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::api::{MediaItem, TICKS_PER_SECOND};

const PROGRESS_CONFIRMATION_TOLERANCE_TICKS: i64 = TICKS_PER_SECOND * 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct QueueSlotId(u64);

impl QueueSlotId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    pub fn raw(self) -> u64 {
        self.0
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default, Serialize, Deserialize,
)]
pub struct QueueRevision(u64);

impl QueueRevision {
    pub fn new(revision: u64) -> Self {
        Self(revision)
    }

    pub fn raw(self) -> u64 {
        self.0
    }

    /// Returns true if this is the zero / default revision. Used by
    /// `serde(skip_serializing_if = "QueueRevision::is_default")` so
    /// v7 peers never see the field.
    pub fn is_default(&self) -> bool {
        self.0 == 0
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
    pub fn from_item(item: &MediaItem) -> Self {
        Self {
            position_ticks: item.playback_position_ticks,
            played: item.played,
        }
    }

    fn matches_server_confirmation(&self, item: &MediaItem) -> bool {
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
    fn from_item(item: &MediaItem) -> Self {
        Self {
            local: SlotProgress::from_item(item),
            pending_sync: None,
        }
    }

    fn apply_to_item(&self, item: &mut MediaItem) {
        item.playback_position_ticks = self.local.position_ticks;
        item.played = self.local.played;
    }
}

#[derive(Debug, Clone)]
pub struct QueueSlot {
    pub slot_id: QueueSlotId,
    pub item: MediaItem,
    pub progress_state: ProgressState,
}

impl QueueSlot {
    pub fn new(slot_id: QueueSlotId, item: MediaItem) -> Self {
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
    pub fn from_items(items: Vec<MediaItem>, active_index: Option<usize>) -> Self {
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

    pub fn from_slot_snapshot(
        slots: Vec<QueueSlot>,
        active_slot_id: Option<QueueSlotId>,
        revision: QueueRevision,
        next_slot_id: u64,
    ) -> Self {
        debug_assert!(
            {
                let mut seen: Vec<QueueSlotId> = slots.iter().map(|s| s.slot_id).collect();
                let len = seen.len();
                seen.sort();
                seen.dedup();
                seen.len() == len
            },
            "from_slot_snapshot: duplicate slot IDs detected"
        );
        debug_assert!(
            slots.iter().all(|s| s.slot_id.raw() < next_slot_id),
            "from_slot_snapshot: all slot IDs must be < next_slot_id"
        );
        debug_assert!(
            active_slot_id.is_none()
                || active_slot_id
                    .map(|id| slots.iter().any(|s| s.slot_id == id))
                    .unwrap_or(false),
            "from_slot_snapshot: active_slot_id not found in slots"
        );

        Self {
            slots,
            active_slot_id,
            revision,
            next_slot_id,
        }
    }

    /// Construct from persisted QueueState fields. When slot_ids are present and
    /// match the items count, restores exact slot identities. Otherwise falls
    /// back to legacy from_items (allocating fresh slot IDs).
    pub fn from_persisted(
        items: Vec<MediaItem>,
        slot_ids: Option<Vec<u64>>,
        active_slot_id: Option<u64>,
        revision: Option<u64>,
        next_slot_id: Option<u64>,
    ) -> Self {
        if let Some(slot_ids) = slot_ids {
            if slot_ids.len() == items.len() && !items.is_empty() {
                let q_slot_ids: Vec<QueueSlotId> =
                    slot_ids.into_iter().map(QueueSlotId::new).collect();
                let active = active_slot_id.map(QueueSlotId::new);
                let slots: Vec<QueueSlot> = q_slot_ids
                    .into_iter()
                    .zip(items)
                    .map(|(slot_id, item)| QueueSlot::new(slot_id, item))
                    .collect();
                return Self::from_slot_snapshot(
                    slots,
                    active,
                    QueueRevision::new(revision.unwrap_or(0)),
                    next_slot_id.unwrap_or(1),
                );
            }
        }
        Self::from_items(items, None)
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

    // ── Positional-index adapters needed by the daemon control layer for
    //     the legacy v7 wire protocol, which carries positional indices
    //     rather than slot IDs.

    pub fn len(&self) -> usize {
        self.slots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    /// Ordered snapshot of every item in the queue, for v7 `CtrlState`.
    pub fn items_snapshot(&self) -> Vec<MediaItem> {
        self.slots.iter().map(|s| s.item.clone()).collect()
    }

    /// Positional index of the active slot, or `None` when nothing is
    /// active. Used to derive the v7 `cursor` field in `CtrlState`.
    pub fn current_index(&self) -> Option<usize> {
        self.active_slot_id
            .and_then(|slot_id| self.slot_index(slot_id))
    }

    /// The `QueueSlotId` at a positional index.
    pub fn slot_id_at(&self, index: usize) -> Option<QueueSlotId> {
        self.slots.get(index).map(|s| s.slot_id)
    }

    /// Borrow the `MediaItem` at a positional index.
    pub fn item_at(&self, index: usize) -> Option<&MediaItem> {
        self.slots.get(index).map(|s| &s.item)
    }

    /// Atomically replace the entire queue's items; slot identities for the
    /// new items are freshly allocated, but `next_slot_id` keeps counting up
    /// from wherever it was (never resets), and `revision` is bumped once
    /// rather than reset to zero, so clients that only translate mutations
    /// once they've seen a non-zero revision keep working after a replace.
    pub fn replace_all(&mut self, items: Vec<MediaItem>, active_index: Option<usize>) {
        self.slots = Vec::with_capacity(items.len());
        for item in items {
            let slot_id = self.allocate_slot_id();
            self.slots.push(QueueSlot::new(slot_id, item));
        }
        self.active_slot_id =
            active_index.and_then(|index| self.slots.get(index).map(|s| s.slot_id));
        self.revision.bump();
    }

    /// Append multiple items at the tail, bumping revision once per item.
    pub fn append_items(&mut self, new_items: Vec<MediaItem>) {
        if new_items.is_empty() {
            return;
        }
        for item in new_items {
            let slot_id = self.allocate_slot_id();
            self.slots.push(QueueSlot::new(slot_id, item));
        }
        self.revision.bump();
    }

    /// Remove the slot at a positional index. Bypasses the active-
    /// confirmation guard: this is the daemon's index-based adapter
    /// (v7 wire), where the client has already decided to remove.
    pub fn remove_at(&mut self, index: usize) -> Option<QueueSlot> {
        if index >= self.slots.len() {
            return None;
        }
        let slot = self.slots.remove(index);
        let slot_id = slot.slot_id;
        self.revision.bump();
        if self.active_slot_id == Some(slot_id) {
            self.active_slot_id = self
                .slots
                .get(index)
                .or_else(|| self.slots.last())
                .map(|s| s.slot_id);
        }
        Some(slot)
    }

    /// Move the slot at `from` to positional index `to`.
    pub fn move_item(&mut self, from: usize, to: usize) -> QueueMutationResult<()> {
        if from >= self.slots.len() || to >= self.slots.len() {
            return QueueMutationResult::NotFound;
        }
        if from == to {
            return QueueMutationResult::Applied(());
        }
        let slot_id = self.slots[from].slot_id;
        self.move_slot(slot_id, to)
    }

    /// Set the active slot by positional index.
    pub fn set_active_at(&mut self, index: usize) -> QueueMutationResult<()> {
        match self.slot_id_at(index) {
            Some(id) => self.set_active_slot(id),
            None => QueueMutationResult::NotFound,
        }
    }

    pub fn next_slot_id(&self) -> u64 {
        self.next_slot_id
    }

    pub fn slot_ids(&self) -> Vec<QueueSlotId> {
        self.slots.iter().map(|s| s.slot_id).collect()
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

    pub fn append(&mut self, item: MediaItem) -> QueueSlotId {
        self.insert(self.slots.len(), item)
    }

    pub fn insert(&mut self, index: usize, item: MediaItem) -> QueueSlotId {
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
        item: MediaItem,
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

    pub fn merge_refresh(&mut self, fetched_items: Vec<MediaItem>) -> RefreshMergeResult {
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
        let slot_id = QueueSlotId::new(self.next_slot_id);
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
        fetched_item: MediaItem,
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
    items: Vec<MediaItem>,
    next_index: usize,
}

impl FetchedItemMatches {
    fn new(item: MediaItem) -> Self {
        Self {
            items: vec![item],
            next_index: 0,
        }
    }

    fn push(&mut self, item: MediaItem) {
        self.items.push(item);
    }

    fn next_match(&mut self) -> MediaItem {
        let index = self.next_index.min(self.items.len() - 1);
        self.next_index = self.next_index.saturating_add(1);
        self.items[index].clone()
    }
}

fn group_fetched_items_by_item_id(items: Vec<MediaItem>) -> HashMap<String, FetchedItemMatches> {
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
