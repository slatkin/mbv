//! Canonical queue contents: slots, item kinds, progress, and lineage.
//!
//! `PlaybackQueue` holds the one ordered slot sequence addressed by stable
//! `QueueSlotId`; `QueueItem` snapshots media from every Service. The Player owner
//! holds the Bound queue and the playback lifecycle; admission stays on shell and
//! Player paths, never in components.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use mbv_emby_model::EmbyItem;

// FeedEntry and QueueItem — the two item kinds a playback queue slot can
// hold, plus QueueItem's custom (kind-tagged, legacy-fallback) Deserialize.
mod audiobookshelf;
#[doc(inline)]
pub use audiobookshelf::{AudiobookshelfEpisodeCatalog, EpisodeResume};
mod items;
#[doc(inline)]
pub use items::{
    AudiobookshelfBookQueueItem, AudiobookshelfItem, AudiobookshelfQueueItem, FeedEntry,
    MpvUrlSource, PlaybackTitlePart, PlaybackTitlePartRole, PlaybackTitleParts, QueueItem,
    QueueItemContentId, QueueItemKind,
};
mod kinds;
#[doc(inline)]
pub use kinds::{FeedKind, ServiceKind};
mod state;
#[doc(inline)]
pub use state::{
    LibraryPosition, LibraryPositionLevel, LibraryPositionState, QueueLineage, QueueSource,
    QueueState, TvContentMode,
};
mod execution_sequence;
#[doc(inline)]
pub use execution_sequence::{ExecSlot, ExecutionSequence};
mod progress;
pub(crate) use progress::apply_progress_to_queue_item;
#[doc(inline)]
pub use progress::{ProgressObservation, SlotProgress};

// serde derives so the owner-assigned slot identity can travel on
// `PlayerEvent` / `PlayerCommand` across the ctrl seam; a newtype over `u64`
// serializes as its inner value.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct QueueSlotId(u64);

impl QueueSlotId {
    #[must_use]
    pub fn raw(self) -> u64 {
        self.0
    }

    #[must_use]
    pub fn from_raw(raw: u64) -> Self {
        Self(raw)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct QueueRevision(u64);

/// An owner-local source of unique queue revisions, retained across queue replacements.
#[derive(Debug, Default)]
pub struct QueueRevisionMint(AtomicU64);

impl QueueRevisionMint {
    /// Mint a revision unique within this owner.
    #[must_use]
    pub fn mint(&self) -> QueueRevision {
        QueueRevision(self.0.fetch_add(1, Ordering::Relaxed))
    }
}

impl QueueRevision {
    #[must_use]
    pub fn raw(self) -> u64 {
        self.0
    }

    /// Mint a new owner-local revision.
    fn bump(&mut self, mint: &QueueRevisionMint) {
        *self = mint.mint();
    }
}

#[derive(Debug, Clone)]
pub struct QueueSlot {
    pub slot_id: QueueSlotId,
    pub item: QueueItem,
}

impl QueueSlot {
    fn new(slot_id: QueueSlotId, item: QueueItem) -> Self {
        Self { slot_id, item }
    }

    #[must_use]
    pub fn local_progress(&self) -> SlotProgress {
        SlotProgress::from_queue_item(&self.item)
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
}

#[derive(Debug, Clone)]
pub struct PlaybackQueue {
    slots: Vec<QueueSlot>,
    active_slot_id: Option<QueueSlotId>,
    revision: QueueRevision,
    mint: std::sync::Arc<QueueRevisionMint>,
    next_slot_id: u64,
}

impl PlaybackQueue {
    #[must_use]
    pub fn from_items(
        items: Vec<EmbyItem>,
        active_index: Option<usize>,
        mint: std::sync::Arc<QueueRevisionMint>,
    ) -> Self {
        let queue_items: Vec<QueueItem> = items
            .into_iter()
            .map(|item| QueueItem::Emby(Box::new(item)))
            .collect();
        Self::from_queue_items(queue_items, active_index, mint)
    }

    #[must_use]
    pub fn from_queue_items(
        items: Vec<QueueItem>,
        active_index: Option<usize>,
        mint: std::sync::Arc<QueueRevisionMint>,
    ) -> Self {
        let mut queue = Self {
            slots: Vec::with_capacity(items.len()),
            active_slot_id: None,
            revision: mint.mint(),
            mint,
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

    /// Reconstruct a queue snapshot while retaining the slot identities
    /// assigned by its owner, and mint a fresh revision from `mint`. Used at
    /// the unified ctrl boundary; local queue construction should use
    /// `from_queue_items` so it allocates identities.
    #[must_use]
    pub fn from_slot_items(
        slots: Vec<(QueueSlotId, QueueItem)>,
        active_slot_id: Option<QueueSlotId>,
        mint: std::sync::Arc<QueueRevisionMint>,
    ) -> Self {
        let next_slot_id = slots
            .iter()
            .map(|(slot_id, _)| slot_id.raw())
            .max()
            .unwrap_or(0)
            .saturating_add(1);
        let slots = slots
            .into_iter()
            .map(|(slot_id, item)| QueueSlot::new(slot_id, item))
            .collect::<Vec<_>>();
        let active_slot_id =
            active_slot_id.filter(|slot_id| slots.iter().any(|slot| slot.slot_id == *slot_id));
        Self {
            slots,
            active_slot_id,
            revision: mint.mint(),
            mint,
            next_slot_id,
        }
    }

    #[must_use]
    pub fn revision(&self) -> QueueRevision {
        self.revision
    }

    /// Continue this queue's revision sequence from another owner-local mint.
    /// Queue contents, slot identities, active slot, and slot allocator are unchanged.
    pub fn rebase_revision_mint(&mut self, mint: std::sync::Arc<QueueRevisionMint>) {
        self.revision = mint.mint();
        self.mint = mint;
    }

    #[must_use]
    pub fn revision_mint(&self) -> std::sync::Arc<QueueRevisionMint> {
        std::sync::Arc::clone(&self.mint)
    }

    #[must_use]
    pub fn slots(&self) -> &[QueueSlot] {
        &self.slots
    }

    /// Clone the canonical queue entries together with their stable slot identities.
    #[must_use]
    pub fn slot_pairs(&self) -> Vec<ExecSlot> {
        self.slots
            .iter()
            .map(|slot| ExecSlot {
                slot_id: slot.slot_id,
                item: slot.item.clone(),
            })
            .collect()
    }

    /// Consume the queue and return its slots. Used by tests and callers
    /// that need owned slot data.
    #[must_use]
    pub fn into_slots(self) -> Vec<QueueSlot> {
        self.slots
    }

    #[must_use]
    pub fn active_slot_id(&self) -> Option<QueueSlotId> {
        self.active_slot_id
    }

    #[must_use]
    pub fn active_index(&self) -> Option<usize> {
        self.active_slot_id.and_then(|id| self.slot_index(id))
    }

    #[must_use]
    pub fn active_slot(&self) -> Option<&QueueSlot> {
        self.active_slot_id.and_then(|slot_id| self.slot(slot_id))
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.slots.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    /// Returns `true` when the queue contains any Audiobookshelf slots.
    #[must_use]
    pub fn has_audiobookshelf_entries(&self) -> bool {
        self.slots.iter().any(|s| s.item.is_audiobookshelf())
    }

    /// Returns `true` when the queue contains any non-Emby slots (Feed or
    /// Audiobookshelf). Used at boundaries that strip server-owned state.
    #[must_use]
    pub fn has_non_emby_entries(&self) -> bool {
        self.slots
            .iter()
            .any(|s| !matches!(s.item, QueueItem::Emby(_)))
    }

    pub fn clear_active_slot(&mut self) {
        if self.active_slot_id.take().is_some() {
            self.revision.bump(&self.mint);
        }
    }

    #[must_use]
    pub fn slot(&self, slot_id: QueueSlotId) -> Option<&QueueSlot> {
        self.slots.iter().find(|slot| slot.slot_id == slot_id)
    }

    #[must_use]
    pub fn slot_index(&self, slot_id: QueueSlotId) -> Option<usize> {
        self.slots.iter().position(|slot| slot.slot_id == slot_id)
    }

    pub fn append(&mut self, item: QueueItem) -> QueueSlotId {
        self.insert(self.slots.len(), item)
    }

    /// Mint a fresh monotonic slot id without inserting a slot. Owner paths
    /// that assign identity before handing items to the Playback run use this
    /// so the run receives the id rather than allocating its own.
    pub fn mint_slot_id(&mut self) -> QueueSlotId {
        self.allocate_slot_id()
    }

    /// Append a slot that already carries an owner-assigned identity. Keeps
    /// `next_slot_id` ahead of any adopted id so later local allocations do
    /// not collide.
    pub fn append_with_id(&mut self, slot_id: QueueSlotId, item: QueueItem) {
        self.slots.push(QueueSlot::new(slot_id, item));
        self.next_slot_id = self.next_slot_id.max(slot_id.raw().saturating_add(1));
        self.revision.bump(&self.mint);
    }

    pub fn insert(&mut self, index: usize, item: QueueItem) -> QueueSlotId {
        let slot_id = self.allocate_slot_id();
        let index = index.min(self.slots.len());
        self.slots.insert(index, QueueSlot::new(slot_id, item));
        self.revision.bump(&self.mint);
        slot_id
    }

    /// Replace all slots with new items, clearing the active slot.
    /// Returns the active index (if any) from the *previous* queue so callers
    /// can carry forward presentation state if desired.
    pub fn replace(&mut self, items: Vec<QueueItem>) -> Option<usize> {
        let old_active = self.active_slot_id.and_then(|id| self.slot_index(id));
        self.slots.clear();
        self.active_slot_id = None;
        for item in items {
            let slot_id = self.allocate_slot_id();
            self.slots.push(QueueSlot::new(slot_id, item));
        }
        self.revision.bump(&self.mint);
        old_active
    }

    /// Remove all slots and clear the active slot.
    pub fn clear(&mut self) {
        if self.slots.is_empty() {
            return;
        }
        self.slots.clear();
        self.active_slot_id = None;
        self.revision.bump(&self.mint);
    }

    /// Truncate the slots to the given length. Used by tests to simulate
    /// a queue shrinking while a context menu is open.
    pub fn truncate_slots(&mut self, len: usize) {
        if len >= self.slots.len() {
            return;
        }
        self.slots.truncate(len);
        self.revision.bump(&self.mint);
        // Clear active slot if it's beyond the new length.
        if let Some(active_id) = self.active_slot_id
            && self.slot_index(active_id).is_none()
        {
            self.active_slot_id = None;
        }
    }

    pub fn set_active_slot(&mut self, slot_id: QueueSlotId) -> QueueMutationResult<()> {
        if self.slot_index(slot_id).is_none() {
            return QueueMutationResult::NotFound;
        }
        if self.active_slot_id != Some(slot_id) {
            self.active_slot_id = Some(slot_id);
            self.revision.bump(&self.mint);
        }
        QueueMutationResult::Applied(())
    }

    /// Set the local progress state on a slot by index. Intended for test
    /// helpers; production code should use player events to drive progress.
    /// Applies to whichever queue item kind occupies the indexed slot.
    pub fn set_slot_progress_by_index(&mut self, index: usize, position_ticks: i64) {
        if let Some((slot_id, played)) = self
            .slots
            .get(index)
            .map(|slot| (slot.slot_id, slot.local_progress().played))
        {
            let _ = self.apply_progress(slot_id, position_ticks, played);
        }
    }

    pub fn remove_slot(&mut self, slot_id: QueueSlotId) -> RemoveSlotResult {
        if self.active_slot_id == Some(slot_id) {
            return RemoveSlotResult::RequiresActiveConfirmation(slot_id);
        }
        self.remove_existing_slot(slot_id)
            .map(Box::new)
            .map_or(RemoveSlotResult::NotFound, RemoveSlotResult::Removed)
    }

    pub fn remove_active_slot_confirmed(&mut self, slot_id: QueueSlotId) -> RemoveSlotResult {
        let Some(index) = self.slot_index(slot_id) else {
            return RemoveSlotResult::NotFound;
        };
        let removed = self.slots.remove(index);
        self.revision.bump(&self.mint);

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
        self.revision.bump(&self.mint);
        QueueMutationResult::Applied(())
    }

    pub fn update_slot_item(
        &mut self,
        slot_id: QueueSlotId,
        item: QueueItem,
    ) -> QueueMutationResult<()> {
        let Some(slot) = self.slots.iter_mut().find(|slot| slot.slot_id == slot_id) else {
            return QueueMutationResult::NotFound;
        };
        let old_item = slot.item.clone();
        slot.item = item;
        if !queue_items_equal(&slot.item, &old_item) {
            self.revision.bump(&self.mint);
        }
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
        let old_item = slot.item.clone();
        apply_progress_to_queue_item(&mut slot.item, position_ticks, played);
        if !queue_items_equal(&slot.item, &old_item) {
            self.revision.bump(&self.mint);
        }
        QueueMutationResult::Applied(())
    }

    /// Applies a refresh to the specific queue slots captured before an
    /// asynchronous adoption fetch. Unlike [`Self::merge_refresh`], this is
    /// not a reconciliation: missing fetched items never prune the queue.
    pub fn merge_refresh_for_slots(
        &mut self,
        fetched_slots: Vec<(QueueSlotId, EmbyItem)>,
    ) -> RefreshMergeResult {
        let mut result = RefreshMergeResult::default();
        let mut changed = false;

        for (slot_id, fetched_item) in fetched_slots {
            let Some(slot) = self.slots.iter_mut().find(|slot| slot.slot_id == slot_id) else {
                continue;
            };
            // A queue replacement can leave an old slot id absent or reused
            // for another item. Never apply an old fetch to a different item.
            if slot.item.content_id() != QueueItemContentId::Emby(fetched_item.id.clone()) {
                continue;
            }
            let old_item = slot.item.clone();
            let updated_len = result.updated_slots.len();
            Self::merge_fetched_slot(slot, fetched_item, self.active_slot_id, &mut result);
            if queue_items_equal(&slot.item, &old_item) {
                result.updated_slots.truncate(updated_len);
            } else {
                changed = true;
            }
        }

        if changed {
            self.revision.bump(&self.mint);
        }
        result
    }

    pub fn merge_refresh(&mut self, fetched_items: Vec<EmbyItem>) -> RefreshMergeResult {
        let mut fetched_by_item_id = group_fetched_items_by_item_id(fetched_items);
        let old_slots = std::mem::take(&mut self.slots);
        let mut result = RefreshMergeResult::default();
        let mut merged_slots = Vec::with_capacity(old_slots.len());
        let active_slot_id = self.active_slot_id;
        let mut changed = false;

        for mut slot in old_slots {
            // Feed and Audiobookshelf slots (both episode and book shapes)
            // have no Emby server counterpart; keep them as-is
            // (group_fetched_items_by_item_id is Emby-only).
            if matches!(slot.item, QueueItem::Feed(_)) || slot.item.is_audiobookshelf() {
                if should_protect_missing_slot(&slot, active_slot_id) {
                    result.protected_slots.push(slot.slot_id);
                }
                merged_slots.push(slot);
                continue;
            }
            let fetched = fetched_by_item_id
                .get_mut(&slot.item.content_id())
                .map(FetchedItemMatches::next_match);
            match fetched {
                Some(fetched_item) => {
                    let old_item = slot.item.clone();
                    Self::merge_fetched_slot(&mut slot, fetched_item, active_slot_id, &mut result);
                    changed |= !queue_items_equal(&slot.item, &old_item);
                    merged_slots.push(slot);
                }
                None if should_protect_missing_slot(&slot, active_slot_id) => {
                    result.protected_slots.push(slot.slot_id);
                    merged_slots.push(slot);
                }
                None => {
                    result.pruned_slots.push(slot.slot_id);
                    changed = true;
                }
            }
        }

        self.slots = merged_slots;
        if let Some(active_slot_id) = self.active_slot_id
            && self.slot_index(active_slot_id).is_none()
        {
            self.active_slot_id = None;
        }
        if changed {
            self.revision.bump(&self.mint);
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
        self.revision.bump(&self.mint);

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
        slot: &mut QueueSlot,
        fetched_item: EmbyItem,
        active_slot_id: Option<QueueSlotId>,
        result: &mut RefreshMergeResult,
    ) {
        let is_active = active_slot_id == Some(slot.slot_id);
        if is_active {
            let local_progress = slot.local_progress();
            slot.item = QueueItem::Emby(Box::new(fetched_item));
            apply_progress_to_queue_item(
                &mut slot.item,
                local_progress.position_ticks,
                local_progress.played,
            );
            result.protected_slots.push(slot.slot_id);
            result.updated_slots.push(slot.slot_id);
            return;
        }

        slot.item = QueueItem::Emby(Box::new(fetched_item));
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

fn queue_items_equal(left: &QueueItem, right: &QueueItem) -> bool {
    serde_json::to_vec(left).ok() == serde_json::to_vec(right).ok()
}

fn group_fetched_items_by_item_id(
    items: Vec<EmbyItem>,
) -> HashMap<QueueItemContentId, FetchedItemMatches> {
    let mut grouped = HashMap::new();
    for item in items {
        grouped
            .entry(QueueItemContentId::Emby(item.id.clone()))
            .and_modify(|matches: &mut FetchedItemMatches| matches.push(item.clone()))
            .or_insert_with(|| FetchedItemMatches::new(item));
    }
    grouped
}

fn should_protect_missing_slot(slot: &QueueSlot, active_slot_id: Option<QueueSlotId>) -> bool {
    active_slot_id == Some(slot.slot_id)
}

#[cfg(test)]
mod tests;
