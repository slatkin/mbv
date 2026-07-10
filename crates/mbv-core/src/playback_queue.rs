use std::collections::HashMap;

use crate::api::{MediaItem, TICKS_PER_SECOND};

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
    fn new(slot_id: QueueSlotId, item: MediaItem) -> Self {
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
        let fetched_by_item_id = group_fetched_items_by_item_id(fetched_items);
        let old_slots = std::mem::take(&mut self.slots);
        let mut result = RefreshMergeResult::default();
        let mut merged_slots = Vec::with_capacity(old_slots.len());
        let active_slot_id = self.active_slot_id;

        for mut slot in old_slots {
            let fetched = fetched_by_item_id.get(&slot.item.id).cloned();
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

fn group_fetched_items_by_item_id(items: Vec<MediaItem>) -> HashMap<String, MediaItem> {
    let mut grouped = HashMap::new();
    for item in items {
        grouped.insert(item.id.clone(), item);
    }
    grouped
}

fn should_protect_missing_slot(slot: &QueueSlot, active_slot_id: Option<QueueSlotId>) -> bool {
    active_slot_id == Some(slot.slot_id) || slot.progress_state.pending_sync.is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str) -> MediaItem {
        MediaItem {
            id: id.to_string(),
            name: format!("Item {id}"),
            item_type: "Episode".to_string(),
            is_folder: false,
            media_type: "Video".to_string(),
            collection_type: String::new(),
            runtime_ticks: 30 * TICKS_PER_SECOND,
            played: false,
            playback_position_ticks: 0,
            series_id: String::new(),
            series_name: String::new(),
            album_id: String::new(),
            album: String::new(),
            index_number: 0,
            parent_index_number: 0,
            unplayed_item_count: 0,
            path: String::new(),
            artist: String::new(),
            sort_name: String::new(),
            production_year: 0,
            end_year: 0,
            overview: String::new(),
            premiere_date: String::new(),
            date_added: String::new(),
            total_count: 0,
            container: String::new(),
            director: String::new(),
            video_info: String::new(),
            audio_info: String::new(),
            genre: String::new(),
            playlist_item_id: String::new(),
        }
    }

    fn item_with_progress(id: &str, position_seconds: i64, played: bool) -> MediaItem {
        let mut item = item(id);
        item.playback_position_ticks = position_seconds * TICKS_PER_SECOND;
        item.played = played;
        item
    }

    fn slot_ids(queue: &PlaybackQueue) -> Vec<QueueSlotId> {
        queue.slots().iter().map(|slot| slot.slot_id).collect()
    }

    #[test]
    fn duplicate_item_ids_receive_distinct_queue_slot_ids() {
        let queue = PlaybackQueue::from_items(vec![item("same"), item("same")], Some(0));

        assert_ne!(queue.slots()[0].slot_id, queue.slots()[1].slot_id);
        assert_eq!(queue.slots()[0].item.id, queue.slots()[1].item.id);
    }

    #[test]
    fn removing_before_active_slot_preserves_active_identity() {
        let mut queue = PlaybackQueue::from_items(vec![item("a"), item("b"), item("c")], Some(2));
        let active = queue.active_slot_id().unwrap();
        let before_active = queue.slots()[0].slot_id;

        assert!(matches!(
            queue.remove_slot(before_active),
            RemoveSlotResult::Removed(_)
        ));

        assert_eq!(queue.active_slot_id(), Some(active));
        assert_eq!(queue.slot_index(active), Some(1));
    }

    #[test]
    fn moving_slots_around_active_slot_preserves_active_identity() {
        let mut queue = PlaybackQueue::from_items(vec![item("a"), item("b"), item("c")], Some(1));
        let ids = slot_ids(&queue);
        let active = queue.active_slot_id().unwrap();

        assert!(matches!(
            queue.move_slot(ids[0], 2),
            QueueMutationResult::Applied(())
        ));
        assert_eq!(queue.active_slot_id(), Some(active));
        assert_eq!(queue.slot_index(active), Some(0));

        assert!(matches!(
            queue.move_slot(ids[2], 0),
            QueueMutationResult::Applied(())
        ));
        assert_eq!(queue.active_slot_id(), Some(active));
        assert_eq!(queue.slot_index(active), Some(1));
    }

    #[test]
    fn moving_active_slot_keeps_active_identity_on_that_slot() {
        let mut queue = PlaybackQueue::from_items(vec![item("a"), item("b"), item("c")], Some(1));
        let active = queue.active_slot_id().unwrap();

        assert!(matches!(
            queue.move_slot(active, 0),
            QueueMutationResult::Applied(())
        ));

        assert_eq!(queue.active_slot_id(), Some(active));
        assert_eq!(queue.slot_index(active), Some(0));
    }

    #[test]
    fn set_active_slot_targets_slot_after_reorder() {
        let mut queue = PlaybackQueue::from_items(vec![item("a"), item("b"), item("c")], Some(0));
        let target = queue.slots()[2].slot_id;

        assert!(matches!(
            queue.move_slot(target, 0),
            QueueMutationResult::Applied(())
        ));
        assert!(matches!(
            queue.set_active_slot(target),
            QueueMutationResult::Applied(())
        ));

        assert_eq!(queue.active_slot_id(), Some(target));
        assert_eq!(queue.slot_index(target), Some(0));
    }

    #[test]
    fn consume_removes_intended_slot_occurrence() {
        let mut queue =
            PlaybackQueue::from_items(vec![item("same"), item("same"), item("c")], Some(2));
        let consumed = queue.slots()[1].slot_id;

        let QueueMutationResult::Applied(slot) = queue.consume_slot(consumed) else {
            panic!("expected consume to remove the slot");
        };

        assert_eq!(slot.slot_id, consumed);
        assert!(queue.slot(consumed).is_none());
        assert_eq!(queue.slots().len(), 2);
        assert_eq!(queue.slots()[0].item.id, "same");
    }

    #[test]
    fn progress_applies_to_intended_slot_after_index_shifts() {
        let mut queue = PlaybackQueue::from_items(vec![item("a"), item("b"), item("c")], Some(2));
        let target = queue.slots()[2].slot_id;
        let removed = queue.slots()[0].slot_id;

        assert!(matches!(
            queue.remove_slot(removed),
            RemoveSlotResult::Removed(_)
        ));
        assert!(matches!(
            queue.apply_progress(target, 12 * TICKS_PER_SECOND, false),
            QueueMutationResult::Applied(())
        ));

        assert_eq!(
            queue.slot(target).unwrap().item.playback_position_ticks,
            12 * TICKS_PER_SECOND
        );
    }

    #[test]
    fn progress_for_removed_slot_is_rejected() {
        let mut queue = PlaybackQueue::from_items(vec![item("a"), item("b")], Some(1));
        let removed = queue.slots()[0].slot_id;
        assert!(matches!(
            queue.remove_slot(removed),
            RemoveSlotResult::Removed(_)
        ));

        assert!(matches!(
            queue.apply_progress(removed, 12 * TICKS_PER_SECOND, false),
            QueueMutationResult::NotFound
        ));
    }

    #[test]
    fn active_slot_progress_is_protected_from_server_refresh() {
        let mut queue = PlaybackQueue::from_items(vec![item("a"), item("b")], Some(0));
        let active = queue.active_slot_id().unwrap();
        assert!(matches!(
            queue.apply_progress(active, 20 * TICKS_PER_SECOND, false),
            QueueMutationResult::Applied(())
        ));

        let result = queue.merge_refresh(vec![
            item_with_progress("a", 3, false),
            item_with_progress("b", 4, false),
        ]);

        assert!(result.protected_slots.contains(&active));
        assert_eq!(
            queue.slot(active).unwrap().item.playback_position_ticks,
            20 * TICKS_PER_SECOND
        );
    }

    #[test]
    fn refresh_applies_one_fetched_item_to_duplicate_queue_slots() {
        let mut queue = PlaybackQueue::from_items(vec![item("same"), item("same")], Some(0));
        let duplicate = queue.slots()[1].slot_id;

        let result = queue.merge_refresh(vec![item_with_progress("same", 5, false)]);

        assert!(result.pruned_slots.is_empty());
        assert!(queue.slot(duplicate).is_some());
        assert_eq!(
            queue.slot(duplicate).unwrap().item.playback_position_ticks,
            5 * TICKS_PER_SECOND
        );
    }

    #[test]
    fn pending_progress_sync_blocks_stale_server_userdata() {
        let mut queue = PlaybackQueue::from_items(vec![item("a")], Some(0));
        let slot = queue.active_slot_id().unwrap();
        assert!(matches!(
            queue.apply_progress(slot, 20 * TICKS_PER_SECOND, false),
            QueueMutationResult::Applied(())
        ));
        assert!(matches!(
            queue.mark_progress_sync_pending(slot),
            QueueMutationResult::Applied(_)
        ));

        let result = queue.merge_refresh(vec![item_with_progress("a", 2, false)]);

        assert!(result.stale_pending_slots.contains(&slot));
        assert_eq!(
            queue.slot(slot).unwrap().item.playback_position_ticks,
            20 * TICKS_PER_SECOND
        );
        assert!(queue
            .slot(slot)
            .unwrap()
            .progress_state
            .pending_sync
            .is_some());
    }

    #[test]
    fn active_pending_progress_confirmation_clears_pending_but_keeps_local_progress() {
        let mut queue = PlaybackQueue::from_items(vec![item("a")], Some(0));
        let active = queue.active_slot_id().unwrap();
        assert!(matches!(
            queue.apply_progress(active, 20 * TICKS_PER_SECOND, false),
            QueueMutationResult::Applied(())
        ));
        assert!(matches!(
            queue.mark_progress_sync_pending(active),
            QueueMutationResult::Applied(_)
        ));

        let result = queue.merge_refresh(vec![item_with_progress("a", 22, false)]);

        assert!(result.pending_confirmed_slots.contains(&active));
        assert!(result.protected_slots.contains(&active));
        assert!(queue
            .slot(active)
            .unwrap()
            .progress_state
            .pending_sync
            .is_none());
        assert_eq!(
            queue.slot(active).unwrap().item.playback_position_ticks,
            20 * TICKS_PER_SECOND
        );
    }

    #[test]
    fn pending_progress_sync_clears_when_server_position_matches_within_tolerance() {
        let mut queue = PlaybackQueue::from_items(vec![item("a")], None);
        let slot = queue.slots()[0].slot_id;
        assert!(matches!(
            queue.apply_progress(slot, 20 * TICKS_PER_SECOND, false),
            QueueMutationResult::Applied(())
        ));
        assert!(matches!(
            queue.mark_progress_sync_pending(slot),
            QueueMutationResult::Applied(_)
        ));

        let result = queue.merge_refresh(vec![item_with_progress("a", 22, false)]);

        assert!(result.pending_confirmed_slots.contains(&slot));
        assert!(queue
            .slot(slot)
            .unwrap()
            .progress_state
            .pending_sync
            .is_none());
        assert_eq!(
            queue.slot(slot).unwrap().item.playback_position_ticks,
            22 * TICKS_PER_SECOND
        );
    }

    #[test]
    fn watched_state_confirmation_requires_exact_match() {
        let mut queue = PlaybackQueue::from_items(vec![item("a")], Some(0));
        let slot = queue.active_slot_id().unwrap();
        assert!(matches!(
            queue.apply_progress(slot, 20 * TICKS_PER_SECOND, true),
            QueueMutationResult::Applied(())
        ));
        assert!(matches!(
            queue.mark_progress_sync_pending(slot),
            QueueMutationResult::Applied(_)
        ));

        let result = queue.merge_refresh(vec![item_with_progress("a", 20, false)]);

        assert!(result.stale_pending_slots.contains(&slot));
        assert!(queue
            .slot(slot)
            .unwrap()
            .progress_state
            .pending_sync
            .is_some());
        assert!(queue.slot(slot).unwrap().item.played);
    }

    #[test]
    fn refresh_prunes_inactive_non_pending_missing_slots() {
        let mut queue = PlaybackQueue::from_items(vec![item("a"), item("b"), item("c")], Some(0));
        let pruned = queue.slots()[1].slot_id;

        let result = queue.merge_refresh(vec![item("a"), item("c")]);

        assert_eq!(result.pruned_slots, vec![pruned]);
        assert!(queue.slot(pruned).is_none());
        assert_eq!(queue.slots().len(), 2);
    }

    #[test]
    fn refresh_cannot_prune_active_or_pending_sync_slots() {
        let mut queue = PlaybackQueue::from_items(vec![item("a"), item("b"), item("c")], Some(0));
        let active = queue.slots()[0].slot_id;
        let pending = queue.slots()[1].slot_id;
        assert!(matches!(
            queue.apply_progress(pending, 9 * TICKS_PER_SECOND, false),
            QueueMutationResult::Applied(())
        ));
        assert!(matches!(
            queue.mark_progress_sync_pending(pending),
            QueueMutationResult::Applied(_)
        ));

        let result = queue.merge_refresh(vec![item("c")]);

        assert!(result.protected_slots.contains(&active));
        assert!(result.protected_slots.contains(&pending));
        assert!(queue.slot(active).is_some());
        assert!(queue.slot(pending).is_some());
    }

    #[test]
    fn active_slot_removal_requires_confirmation_decision() {
        let mut queue = PlaybackQueue::from_items(vec![item("a"), item("b")], Some(0));
        let active = queue.active_slot_id().unwrap();

        assert!(matches!(
            queue.remove_slot(active),
            RemoveSlotResult::RequiresActiveConfirmation(slot_id) if slot_id == active
        ));
        assert!(queue.slot(active).is_some());
    }

    #[test]
    fn confirmed_active_slot_removal_clears_active_identity() {
        let mut queue = PlaybackQueue::from_items(vec![item("a"), item("b")], Some(0));
        let active = queue.active_slot_id().unwrap();

        assert!(matches!(
            queue.remove_active_slot_confirmed(active),
            RemoveSlotResult::Removed(_)
        ));

        assert!(queue.slot(active).is_none());
        assert_eq!(queue.active_slot_id(), None);
    }

    #[test]
    fn structural_mutations_bump_revision() {
        let mut queue = PlaybackQueue::from_items(vec![item("a"), item("b")], Some(0));
        let initial = queue.revision();

        let inserted = queue.append(item("c"));
        assert!(queue.revision() > initial);
        let after_insert = queue.revision();

        assert!(matches!(
            queue.move_slot(inserted, 0),
            QueueMutationResult::Applied(())
        ));
        assert!(queue.revision() > after_insert);
        let after_move = queue.revision();

        assert!(matches!(
            queue.consume_slot(inserted),
            QueueMutationResult::Applied(_)
        ));
        assert!(queue.revision() > after_move);
    }
}
