use mbv_emby_model::EmbyItem;
use mbv_queue::ExecSlot;
use mbv_queue::{PlaybackQueue, QueueItem, QueueRevisionMint, QueueSlot, QueueSlotId};
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct PlayerTab {
    pub queue_cursor: usize,
    pub queue: PlaybackQueue,
    /// The newest desired playback slot from the owner snapshot, if any.
    pub pending_playback_slot: Option<QueueSlotId>,
    revision_mint: Arc<QueueRevisionMint>,
}

impl Default for PlayerTab {
    fn default() -> Self {
        Self::new(Vec::new(), 0)
    }
}

impl PlayerTab {
    #[must_use]
    pub fn new(items: Vec<QueueItem>, queue_cursor: usize) -> Self {
        let queue_cursor = queue_cursor.min(items.len().saturating_sub(1));
        // The cursor is presentation state; construction is not a playback
        // event, so the active slot starts unset.  Playback events and
        // explicit `set_active_slot` calls own the active-slot lifecycle.
        let revision_mint = Arc::new(QueueRevisionMint::default());
        let queue = PlaybackQueue::from_queue_items(items, None, Arc::clone(&revision_mint));
        Self {
            queue_cursor,
            queue,
            pending_playback_slot: None,
            revision_mint,
        }
    }

    /// Creates a tab with a fresh revision mint. Use only when starting a
    /// projection chain with no remembered revision (startup/construction);
    /// replacement sites must use `adopt_revision_mint`.
    pub fn from_unified_state(state: &mbv_ctrl::UnifiedQueueStateData) -> Self {
        let revision_mint = Arc::new(QueueRevisionMint::default());
        Self::from_unified_state_with_mint(state, revision_mint)
    }

    /// Continue this tab's revision sequence from an existing projection chain.
    pub fn adopt_revision_mint(&mut self, mint: Arc<QueueRevisionMint>) {
        self.queue.rebase_revision_mint(Arc::clone(&mint));
        self.revision_mint = mint;
    }

    /// Revision mint for preserving this tab's projection sequence across replacement.
    #[must_use]
    pub fn revision_mint(&self) -> Arc<QueueRevisionMint> {
        Arc::clone(&self.revision_mint)
    }

    fn from_unified_state_with_mint(
        state: &mbv_ctrl::UnifiedQueueStateData,
        revision_mint: Arc<QueueRevisionMint>,
    ) -> Self {
        let active_index = state
            .active_slot
            .and_then(|slot_id| state.slots.iter().position(|slot| slot.slot_id == slot_id));
        let slots: Vec<(QueueSlotId, mbv_queue::QueueItem)> = state
            .slots
            .iter()
            .map(|slot| (QueueSlotId::from_raw(slot.slot_id), slot.item.clone()))
            .collect();
        // The owner's revision is not comparable in this client process; #836
        // gives each client its own revision sequence for projection identity.
        let queue = PlaybackQueue::from_slot_items(
            slots,
            state.active_slot.map(QueueSlotId::from_raw),
            Arc::clone(&revision_mint),
        );
        Self {
            queue_cursor: active_index.unwrap_or(0),
            queue,
            pending_playback_slot: state
                .queued_latest_transition
                .as_ref()
                .or(state.in_flight_transition.as_ref())
                .map(|transition| QueueSlotId::from_raw(transition.target_slot)),
            revision_mint,
        }
    }

    /// Creates a `PlayerTab` from a legacy `Vec<EmbyItem>`, wrapping each
    /// as `QueueItem::Emby`. Kept for callers that start from Emby-only
    /// sources (library browse, remote projection).
    #[must_use]
    pub fn from_emby_items(items: Vec<EmbyItem>, queue_cursor: usize) -> Self {
        let queue_items: Vec<QueueItem> = items
            .into_iter()
            .map(|i| QueueItem::Emby(Box::new(i)))
            .collect();
        Self::new(queue_items, queue_cursor)
    }

    pub fn set_items(&mut self, items: Vec<EmbyItem>, queue_cursor: usize) {
        let queue_items = items
            .into_iter()
            .map(|item| QueueItem::Emby(Box::new(item)))
            .collect();
        self.set_queue_items(queue_items, queue_cursor);
    }

    /// Replaces the canonical queue with arbitrary `QueueItem`s (Emby, Feed,
    /// or mixed) and resets the cursor. Use this when restoring or adopting a
    /// persisted queue that may contain Feed entries — `set_items` would
    /// silently drop them.
    pub fn set_queue_items(&mut self, items: Vec<QueueItem>, queue_cursor: usize) {
        // Replace in place so PlaybackQueue's allocator remains monotonic;
        // a new queue must never reuse an old occurrence's slot identity.
        self.queue.replace(items);
        self.queue_cursor = queue_cursor;
        self.pending_playback_slot = None;
        self.clamp_cursor();
    }

    pub fn set_unified_state(
        &mut self,
        state: &mbv_ctrl::UnifiedQueueStateData,
        queue_cursor: usize,
    ) {
        *self = Self::from_unified_state_with_mint(state, Arc::clone(&self.revision_mint));
        self.queue_cursor = queue_cursor;
        self.clamp_cursor();
    }

    /// Canonical queue length: the number of slots in the playback queue,
    /// regardless of item kind.
    #[must_use]
    pub fn total_queue_len(&self) -> usize {
        self.queue.slots().len()
    }

    pub fn clamp_cursor(&mut self) {
        let total = self.total_queue_len();
        if total == 0 {
            self.queue_cursor = 0;
        } else {
            self.queue_cursor = self.queue_cursor.min(total - 1);
        }
    }

    #[must_use]
    pub fn slot_id_at(&self, index: usize) -> Option<QueueSlotId> {
        self.queue.slots().get(index).map(|slot| slot.slot_id)
    }

    /// Extract a slice of all `QueueSlot`s from the canonical queue.
    #[must_use]
    pub fn slots(&self) -> &[QueueSlot] {
        self.queue.slots()
    }

    /// Canonical queue revision, bumped on every structural queue mutation.
    #[must_use]
    pub fn revision(&self) -> mbv_queue::QueueRevision {
        self.queue.revision()
    }

    /// Extract the `QueueItem` at the given slot index, if any.
    #[must_use]
    pub fn item_at(&self, index: usize) -> Option<&QueueItem> {
        self.queue.slots().get(index).map(|slot| &slot.item)
    }

    /// Extract the `EmbyItem` at the given slot index, if the slot holds an
    /// Emby variant.
    #[must_use]
    pub fn emby_item_at(&self, index: usize) -> Option<&EmbyItem> {
        self.item_at(index).and_then(|item| item.as_emby())
    }

    /// Collect all Emby items from the queue in slot order. Used by callers
    /// that need `Vec<EmbyItem>` for legacy APIs (session play, player
    /// submission, persistence).
    #[must_use]
    pub fn emby_items(&self) -> Vec<EmbyItem> {
        self.queue
            .slots()
            .iter()
            .filter_map(|slot| slot.item.as_emby().cloned())
            .collect()
    }

    /// Clone the `EmbyItem` at the given slot index, if present.
    #[must_use]
    pub fn clone_emby_item_at(&self, index: usize) -> Option<EmbyItem> {
        self.emby_item_at(index).cloned()
    }

    /// Collect all items from the canonical queue as `QueueItem`s in slot
    /// order.  Used when submitting the full queue to the player so that
    /// mixed Emby + Feed queues are preserved end-to-end.
    #[must_use]
    pub fn all_queue_items(&self) -> Vec<QueueItem> {
        self.queue
            .slots()
            .iter()
            .map(|slot| slot.item.clone())
            .collect()
    }

    #[must_use]
    pub fn all_queue_slots(&self) -> Vec<ExecSlot> {
        self.queue.slot_pairs()
    }

    /// Test helper: replace the item at a specific index. Used by tests
    /// that need to modify queue items after construction.
    #[cfg(test)]
    pub fn set_item_at(&mut self, index: usize, item: QueueItem) {
        if let Some(slot_id) = self.queue.slots().get(index).map(|slot| slot.slot_id) {
            let _ = self.queue.update_slot_item(slot_id, item);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::PlayerTab;
    use crate::app::tests::make_items;

    #[test]
    fn replaced_tab_chain_never_repeats_a_revision_issue_836() {
        let mut old_tab = PlayerTab::from_emby_items(make_items(1), 0);
        let first_revision = old_tab.revision();
        old_tab.set_items(make_items(2), 0);
        let latest_old_revision = old_tab.revision();

        let mut replacement_items = make_items(1);
        replacement_items[0].id = "replacement".into();
        let mut replacement_tab = PlayerTab::from_emby_items(replacement_items, 0);
        replacement_tab.adopt_revision_mint(old_tab.revision_mint());

        assert!(latest_old_revision > first_revision);
        assert!(replacement_tab.revision() > latest_old_revision);
    }
}
