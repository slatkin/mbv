use mbv_core::api::EmbyItem;
use mbv_core::playback_queue::{
    PlaybackQueue, QueueItem, QueueMutationResult, QueueSlotId, RefreshMergeResult,
    RemoveSlotResult,
};

#[derive(Clone, Default)]
pub(super) struct PlayerTab {
    pub(super) items: Vec<EmbyItem>,
    pub(super) queue_cursor: usize,
    pub(super) queue: PlaybackQueue,
}

impl PlayerTab {
    pub(super) fn new(items: Vec<EmbyItem>, queue_cursor: usize) -> Self {
        let queue_cursor = queue_cursor.min(items.len().saturating_sub(1));
        let queue = PlaybackQueue::from_items(items.clone(), None);
        Self {
            items,
            queue_cursor,
            queue,
        }
    }

    pub(super) fn set_items(&mut self, items: Vec<EmbyItem>, queue_cursor: usize) {
        *self = Self::new(items, queue_cursor);
    }

    pub(super) fn queue_model_matches_items(&self) -> bool {
        self.queue.slots().len() == self.items.len()
            && self
                .queue
                .slots()
                .iter()
                .zip(&self.items)
                .all(|(slot, item)| same_queue_occurrence(&slot.item, item))
    }

    pub(super) fn sync_queue_model_from_items_if_needed(&mut self) {
        if self.queue_model_matches_items() {
            let updates: Vec<_> = self
                .queue
                .slots()
                .iter()
                .zip(&self.items)
                .map(|(slot, item)| (slot.slot_id, item.clone()))
                .collect();
            for (slot_id, item) in updates {
                let _ = self
                    .queue
                    .update_slot_item(slot_id, QueueItem::Emby(Box::new(item)));
            }
        } else {
            self.queue = PlaybackQueue::from_items(self.items.clone(), None);
        }
    }

    pub(super) fn sync_items_from_queue_model(&mut self) {
        self.items = self
            .queue
            .slots()
            .iter()
            .filter_map(|slot| match &slot.item {
                QueueItem::Emby(e) => Some((**e).clone()),
                QueueItem::Feed(_) => None,
            })
            .collect();
        self.clamp_cursor();
    }

    pub(super) fn sync_active_slot(&mut self, active_index: Option<usize>) {
        self.sync_queue_model_from_items_if_needed();
        let active_slot_id = active_index.and_then(|index| self.resolve_slot_at(index));
        if let Some(slot_id) = active_slot_id {
            let _ = self.queue.set_active_slot(slot_id);
        } else {
            self.queue.clear_active_slot();
        }
    }

    pub(super) fn merge_refresh(&mut self, fetched_items: Vec<EmbyItem>) -> RefreshMergeResult {
        self.sync_queue_model_from_items_if_needed();
        let result = self.queue.merge_refresh(fetched_items);
        self.sync_items_from_queue_model();
        result
    }

    pub(super) fn clamp_cursor(&mut self) {
        if self.items.is_empty() {
            self.queue_cursor = 0;
        } else {
            self.queue_cursor = self.queue_cursor.min(self.items.len() - 1);
        }
    }

    pub(super) fn slot_id_at(&mut self, index: usize) -> Option<QueueSlotId> {
        self.sync_queue_model_from_items_if_needed();
        self.queue.slots().get(index).map(|slot| slot.slot_id)
    }

    /// Read-only resolution of a display index to the slot currently at that
    /// position. Unlike `slot_id_at`, this does not rebuild the shadow; callers
    /// in the event path want the queue exactly as it stands now.
    pub(super) fn resolve_slot_at(&self, index: usize) -> Option<QueueSlotId> {
        self.queue.slots().get(index).map(|slot| slot.slot_id)
    }

    pub(super) fn slot_id_matches_at(&self, index: usize, slot_id: QueueSlotId) -> bool {
        self.queue_model_matches_items()
            && self
                .queue
                .slots()
                .get(index)
                .is_some_and(|slot| slot.slot_id == slot_id)
    }

    pub(super) fn remove_slot_at(&mut self, index: usize) -> Option<EmbyItem> {
        let slot_id = self.slot_id_at(index)?;
        let removed = match self.queue.remove_slot(slot_id) {
            RemoveSlotResult::Removed(slot) => slot.item,
            RemoveSlotResult::RequiresActiveConfirmation(_) | RemoveSlotResult::NotFound => {
                return None;
            }
        };
        self.sync_items_from_queue_model();
        match removed {
            QueueItem::Emby(e) => Some(*e),
            QueueItem::Feed(_) => None,
        }
    }

    pub(super) fn insert_item_at(&mut self, index: usize, item: EmbyItem) {
        self.sync_queue_model_from_items_if_needed();
        self.queue.insert(index, QueueItem::Emby(Box::new(item)));
        self.sync_items_from_queue_model();
        self.queue_cursor = index.min(self.items.len().saturating_sub(1));
    }

    pub(super) fn append_item(&mut self, item: EmbyItem) {
        self.sync_queue_model_from_items_if_needed();
        self.queue.append(QueueItem::Emby(Box::new(item)));
        self.sync_items_from_queue_model();
    }

    pub(super) fn append_items(&mut self, items: Vec<EmbyItem>) {
        self.sync_queue_model_from_items_if_needed();
        for item in items {
            self.queue.append(QueueItem::Emby(Box::new(item)));
        }
        self.sync_items_from_queue_model();
    }

    pub(super) fn move_slot(&mut self, slot_id: QueueSlotId, to: usize) -> bool {
        self.sync_queue_model_from_items_if_needed();
        if !matches!(
            self.queue.move_slot(slot_id, to),
            QueueMutationResult::Applied(())
        ) {
            return false;
        }
        self.sync_items_from_queue_model();
        self.queue_cursor = to.min(self.items.len().saturating_sub(1));
        true
    }

    pub(super) fn clear(&mut self) {
        self.set_items(Vec::new(), 0);
    }
}

pub(super) fn same_queue_occurrence(left: &QueueItem, right: &EmbyItem) -> bool {
    match left {
        QueueItem::Emby(e) => e.id == right.id && e.playlist_item_id == right.playlist_item_id,
        QueueItem::Feed(_) => false,
    }
}
