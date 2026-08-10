use mbv_core::api::EmbyItem;
use mbv_core::playback_queue::{
    PlaybackQueue, QueueItem, QueueMutationResult, QueueSlot, QueueSlotId, RefreshMergeResult,
    RemoveSlotResult,
};

#[derive(Clone, Default)]
pub(super) struct PlayerTab {
    pub(super) queue_cursor: usize,
    pub(super) queue: PlaybackQueue,
}

impl PlayerTab {
    pub(super) fn new(items: Vec<QueueItem>, queue_cursor: usize) -> Self {
        let queue_cursor = queue_cursor.min(items.len().saturating_sub(1));
        // The cursor is presentation state; construction is not a playback
        // event, so the active slot starts unset.  Playback events and
        // explicit `set_active_slot` calls own the active-slot lifecycle.
        let queue = PlaybackQueue::from_queue_items(items, None);
        Self {
            queue_cursor,
            queue,
        }
    }

    pub(super) fn from_unified_state(state: &mbv_core::ctrl::UnifiedQueueStateData) -> Self {
        let active_index = state
            .active_slot
            .and_then(|slot_id| state.slots.iter().position(|slot| slot.slot_id == slot_id));
        let slots = state
            .slots
            .iter()
            .map(|slot| (QueueSlotId::from_raw(slot.slot_id), slot.item.clone()))
            .collect();
        let queue = PlaybackQueue::from_slot_items(
            slots,
            state.active_slot.map(QueueSlotId::from_raw),
            mbv_core::playback_queue::QueueRevision::from_raw(state.revision),
        );
        Self {
            queue_cursor: active_index.unwrap_or(0),
            queue,
        }
    }

    /// Creates a `PlayerTab` from a legacy `Vec<EmbyItem>`, wrapping each
    /// as `QueueItem::Emby`. Kept for callers that start from Emby-only
    /// sources (library browse, remote projection).
    pub(super) fn from_emby_items(items: Vec<EmbyItem>, queue_cursor: usize) -> Self {
        let queue_items: Vec<QueueItem> = items
            .into_iter()
            .map(|i| QueueItem::Emby(Box::new(i)))
            .collect();
        Self::new(queue_items, queue_cursor)
    }

    pub(super) fn set_items(&mut self, items: Vec<EmbyItem>, queue_cursor: usize) {
        *self = Self::from_emby_items(items, queue_cursor);
    }

    /// Replaces the canonical queue with arbitrary `QueueItem`s (Emby, Feed,
    /// or mixed) and resets the cursor. Use this when restoring or adopting a
    /// persisted queue that may contain Feed entries — `set_items` would
    /// silently drop them.
    pub(super) fn set_queue_items(&mut self, items: Vec<QueueItem>, queue_cursor: usize) {
        *self = Self::new(items, queue_cursor);
    }

    pub(super) fn set_unified_state(
        &mut self,
        state: &mbv_core::ctrl::UnifiedQueueStateData,
        queue_cursor: usize,
    ) {
        *self = Self::from_unified_state(state);
        self.queue_cursor = queue_cursor;
        self.clamp_cursor();
    }

    pub(super) fn sync_active_slot(&mut self, active_index: Option<usize>) {
        let active_slot_id = active_index.and_then(|index| self.resolve_slot_at(index));
        if let Some(slot_id) = active_slot_id {
            let _ = self.queue.set_active_slot(slot_id);
        } else {
            self.queue.clear_active_slot();
        }
    }

    pub(super) fn merge_refresh(&mut self, fetched_items: Vec<EmbyItem>) -> RefreshMergeResult {
        let result = self.queue.merge_refresh(fetched_items);
        self.clamp_cursor();
        result
    }

    /// Canonical queue length: the number of slots in the playback queue,
    /// regardless of item kind.
    pub(super) fn total_queue_len(&self) -> usize {
        self.queue.slots().len()
    }

    pub(super) fn clamp_cursor(&mut self) {
        let total = self.total_queue_len();
        if total == 0 {
            self.queue_cursor = 0;
        } else {
            self.queue_cursor = self.queue_cursor.min(total - 1);
        }
    }

    pub(super) fn slot_id_at(&self, index: usize) -> Option<QueueSlotId> {
        self.queue.slots().get(index).map(|slot| slot.slot_id)
    }

    /// Read-only resolution of a display index to the slot currently at that
    /// position.
    pub(super) fn resolve_slot_at(&self, index: usize) -> Option<QueueSlotId> {
        self.queue.slots().get(index).map(|slot| slot.slot_id)
    }

    pub(super) fn slot_id_matches_at(&self, index: usize, slot_id: QueueSlotId) -> bool {
        self.queue
            .slots()
            .get(index)
            .is_some_and(|slot| slot.slot_id == slot_id)
    }

    pub(super) fn remove_slot_at(&mut self, index: usize) -> Option<QueueItem> {
        let slot_id = self.slot_id_at(index)?;
        let removed = match self.queue.remove_slot(slot_id) {
            RemoveSlotResult::Removed(slot) => slot.item,
            RemoveSlotResult::RequiresActiveConfirmation(_) | RemoveSlotResult::NotFound => {
                return None;
            }
        };
        self.clamp_cursor();
        Some(removed)
    }

    pub(super) fn insert_item_at(&mut self, index: usize, item: QueueItem) {
        self.queue.insert(index, item);
        // Cursor clamp uses the canonical queue length, not an Emby-only shadow.
        self.queue_cursor = index.min(self.total_queue_len().saturating_sub(1));
    }

    pub(super) fn append_item(&mut self, item: EmbyItem) {
        self.queue.append(QueueItem::Emby(Box::new(item)));
    }

    pub(super) fn append_items(&mut self, items: Vec<EmbyItem>) {
        for item in items {
            self.queue.append(QueueItem::Emby(Box::new(item)));
        }
    }

    pub(super) fn move_slot(&mut self, slot_id: QueueSlotId, to: usize) -> bool {
        if !matches!(
            self.queue.move_slot(slot_id, to),
            QueueMutationResult::Applied(())
        ) {
            return false;
        }
        // Cursor clamp uses the canonical queue length, not an Emby-only shadow.
        self.queue_cursor = to.min(self.total_queue_len().saturating_sub(1));
        true
    }

    pub(super) fn clear(&mut self) {
        self.set_items(Vec::new(), 0);
    }

    /// Extract a slice of all `QueueSlot`s from the canonical queue.
    pub(super) fn slots(&self) -> &[QueueSlot] {
        self.queue.slots()
    }

    /// Extract the `QueueItem` at the given slot index, if any.
    pub(super) fn item_at(&self, index: usize) -> Option<&QueueItem> {
        self.queue.slots().get(index).map(|slot| &slot.item)
    }

    /// Extract the `EmbyItem` at the given slot index, if the slot holds an
    /// Emby variant.
    pub(super) fn emby_item_at(&self, index: usize) -> Option<&EmbyItem> {
        self.item_at(index).and_then(|item| item.as_emby())
    }

    /// Collect all Emby items from the queue in slot order. Used by callers
    /// that need `Vec<EmbyItem>` for legacy APIs (session play, player
    /// submission, persistence).
    pub(super) fn emby_items(&self) -> Vec<EmbyItem> {
        self.queue
            .slots()
            .iter()
            .filter_map(|slot| slot.item.as_emby().cloned())
            .collect()
    }

    /// Clone the `EmbyItem` at the given slot index, if present.
    pub(super) fn clone_emby_item_at(&self, index: usize) -> Option<EmbyItem> {
        self.emby_item_at(index).cloned()
    }

    /// Collect all items from the canonical queue as `QueueItem`s in slot
    /// order.  Used when submitting the full queue to the player so that
    /// mixed Emby + Feed queues are preserved end-to-end.
    pub(super) fn all_queue_items(&self) -> Vec<QueueItem> {
        self.queue
            .slots()
            .iter()
            .map(|slot| slot.item.clone())
            .collect()
    }

    /// Test helper: set the local progress state on a slot by index.
    /// This simulates playback progress without going through the full
    /// player event path. Only affects Emby slots; Feed slots are a no-op.
    #[cfg(test)]
    pub(super) fn set_slot_progress_at(&mut self, index: usize, position_ticks: i64) {
        self.queue.set_slot_progress_by_index(index, position_ticks);
    }

    /// Test helper: replace the item at a specific index. Used by tests
    /// that need to modify queue items after construction.
    #[cfg(test)]
    pub(super) fn set_item_at(&mut self, index: usize, item: QueueItem) {
        if let Some(slot) = self.queue.slots_mut().get_mut(index) {
            slot.item = item;
        }
    }
}
