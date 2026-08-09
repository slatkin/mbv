use std::collections::HashMap;

use crate::api::{EmbyItem, TICKS_PER_SECOND};

const PROGRESS_CONFIRMATION_TOLERANCE_TICKS: i64 = TICKS_PER_SECOND * 3;

// ---------------------------------------------------------------------------
// FeedEntry — minimal identity + playback fields for RSS/podcast/YouTube
// items. No progress state (position_ticks / played) — deferred to #472.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FeedEntry {
    pub guid: String,
    pub title: String,
    pub enclosure_url: Option<String>,
    pub link: Option<String>,
    pub mime_type: Option<String>,
    pub duration_ticks: Option<u64>,
    /// Publish time in unix seconds UTC (RSS `pubDate` / Atom
    /// `published`/`updated`), for the "All" group's newest-first sort.
    /// Missing dates sort last. `#[serde(default)]` keeps old
    /// `queue_state.json` files (pre-#471) loading.
    #[serde(default)]
    pub pub_date_secs: Option<u64>,
    /// Subscription's `FeedKind` carried into the queued snapshot. Canonical
    /// media kind when enclosure MIME is absent or unrecognized; enclosure MIME
    /// refines it when recognized. `#[serde(default)]` preserves legacy
    /// serialized entries that lack this field.
    #[serde(default)]
    pub feed_kind: crate::config::FeedKind,
}

impl FeedEntry {
    /// The best playable URL: enclosure first, then link as fallback.
    pub fn primary_source(&self) -> Option<&str> {
        self.enclosure_url.as_deref().or(self.link.as_deref())
    }
}

// ---------------------------------------------------------------------------
// QueueItem — enum wrapping the two item kinds the playback queue can hold.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "kind")]
pub enum QueueItem {
    #[serde(rename = "Emby")]
    Emby(Box<EmbyItem>),
    #[serde(rename = "Feed")]
    Feed(FeedEntry),
}

/// Custom deserializer for `QueueItem` that accepts both the tagged form
/// (with `"kind": "Emby"` or `"kind": "Feed"`) and legacy bare `EmbyItem`
/// objects (no `kind` field). This preserves backward compatibility with
/// `queue_state.json` files written before the `QueueItem` enum existed.
impl<'de> serde::Deserialize<'de> for QueueItem {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de;

        let value = serde_json::Value::deserialize(deserializer)?;

        // Try tagged first: {"kind":"Emby",...} or {"kind":"Feed",...}
        if let Some(kind) = value.get("kind").and_then(|k| k.as_str()) {
            return match kind {
                "Emby" => {
                    let item = EmbyItem::deserialize(value).map_err(de::Error::custom)?;
                    Ok(QueueItem::Emby(Box::new(item)))
                }
                "Feed" => {
                    let entry = FeedEntry::deserialize(value).map_err(de::Error::custom)?;
                    Ok(QueueItem::Feed(entry))
                }
                other => Err(de::Error::unknown_variant(other, &["Emby", "Feed"])),
            };
        }

        // Legacy fallback: bare EmbyItem object (no `kind` field)
        let item = EmbyItem::deserialize(value).map_err(de::Error::custom)?;
        Ok(QueueItem::Emby(Box::new(item)))
    }
}

impl QueueItem {
    pub fn title(&self) -> &str {
        match self {
            QueueItem::Emby(item) => &item.name,
            QueueItem::Feed(entry) => &entry.title,
        }
    }

    pub fn duration(&self) -> Option<u64> {
        match self {
            QueueItem::Emby(item) => {
                if item.runtime_ticks > 0 {
                    Some(item.runtime_ticks as u64)
                } else {
                    None
                }
            }
            QueueItem::Feed(entry) => entry.duration_ticks,
        }
    }

    pub fn media_kind(&self) -> &str {
        match self {
            QueueItem::Emby(item) => &item.media_type,
            QueueItem::Feed(entry) => match entry.mime_type.as_deref() {
                Some(m) if m.starts_with("audio/") => "Audio",
                Some(m) if m.starts_with("video/") => "Video",
                _ => entry.feed_kind.as_str(),
            },
        }
    }

    pub fn is_audio(&self) -> bool {
        match self {
            QueueItem::Emby(item) => item.is_audio(),
            QueueItem::Feed(entry) => match entry.mime_type.as_deref() {
                Some(m) if m.starts_with("audio/") => true,
                Some(m) if m.starts_with("video/") => false,
                _ => entry.feed_kind == crate::config::FeedKind::Audio,
            },
        }
    }

    pub fn is_video(&self) -> bool {
        match self {
            QueueItem::Emby(item) => item.is_video(),
            QueueItem::Feed(entry) => match entry.mime_type.as_deref() {
                Some(m) if m.starts_with("video/") => true,
                Some(m) if m.starts_with("audio/") => false,
                _ => entry.feed_kind == crate::config::FeedKind::Video,
            },
        }
    }

    pub fn artwork_url(&self) -> Option<&str> {
        match self {
            QueueItem::Emby(_item) => None,
            QueueItem::Feed(_entry) => None,
        }
    }

    /// The Emby item ID for Emby items, or the feed GUID for feed entries.
    /// Used for server-refresh matching (only Emby items have server IDs,
    /// but this keeps the lookup uniform).
    pub fn id(&self) -> &str {
        match self {
            QueueItem::Emby(item) => &item.id,
            QueueItem::Feed(entry) => &entry.guid,
        }
    }

    pub fn display_name(&self) -> String {
        match self {
            QueueItem::Emby(item) => item.display_name(),
            QueueItem::Feed(entry) => entry.title.clone(),
        }
    }

    pub fn runtime_ticks(&self) -> i64 {
        match self {
            QueueItem::Emby(item) => item.runtime_ticks,
            QueueItem::Feed(entry) => entry.duration_ticks.unwrap_or(0) as i64,
        }
    }

    pub fn playback_position_ticks(&self) -> i64 {
        match self {
            QueueItem::Emby(item) => item.playback_position_ticks,
            QueueItem::Feed(_) => 0,
        }
    }

    pub fn played(&self) -> bool {
        match self {
            QueueItem::Emby(item) => item.played,
            QueueItem::Feed(_) => false,
        }
    }

    /// Returns the inner `EmbyItem` if this is an Emby variant.
    /// Used at boundaries that only operate on Emby items (send_ep_info,
    /// set_current_item_metadata, start_item, mark_played, etc.).
    pub fn as_emby(&self) -> Option<&EmbyItem> {
        match self {
            QueueItem::Emby(item) => Some(item),
            QueueItem::Feed(_) => None,
        }
    }

    pub fn playlist_item_id(&self) -> &str {
        match self {
            QueueItem::Emby(item) => &item.playlist_item_id,
            QueueItem::Feed(_) => "",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct QueueSlotId(u64);

impl QueueSlotId {
    pub fn raw(self) -> u64 {
        self.0
    }

    pub fn from_raw(raw: u64) -> Self {
        Self(raw)
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
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

    /// Progress is only meaningful for Emby items. Feed slots get a
    /// zeroed-out default.
    fn from_queue_item(item: &QueueItem) -> Self {
        match item {
            QueueItem::Emby(emby) => Self::from_item(emby),
            QueueItem::Feed(_) => Self::default(),
        }
    }

    fn matches_server_confirmation(&self, item: &EmbyItem) -> bool {
        (self.position_ticks - item.playback_position_ticks).abs()
            <= PROGRESS_CONFIRMATION_TOLERANCE_TICKS
            && self.played == item.played
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProgressState {
    pub local: SlotProgress,
    pub pending_sync: Option<SlotProgress>,
}

impl ProgressState {
    /// Progress is only meaningful for Emby items. Feed slots get a
    /// zeroed-out default (no-op sync).
    fn from_queue_item(item: &QueueItem) -> Self {
        match item {
            QueueItem::Emby(emby) => Self {
                local: SlotProgress::from_item(emby),
                pending_sync: None,
            },
            QueueItem::Feed(_) => Self::default(),
        }
    }

    /// Applies progress back to the item. Only touches EmbyItem fields;
    /// Feed slots are a no-op.
    fn apply_to_item(&self, item: &mut QueueItem) {
        if let QueueItem::Emby(emby) = item {
            emby.playback_position_ticks = self.local.position_ticks;
            emby.played = self.local.played;
        }
    }
}

#[derive(Debug, Clone)]
pub struct QueueSlot {
    pub slot_id: QueueSlotId,
    pub item: QueueItem,
    pub progress_state: ProgressState,
}

impl QueueSlot {
    fn new(slot_id: QueueSlotId, item: QueueItem) -> Self {
        let progress_state = ProgressState::from_queue_item(&item);
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
        let queue_items: Vec<QueueItem> = items
            .into_iter()
            .map(|item| QueueItem::Emby(Box::new(item)))
            .collect();
        Self::from_queue_items(queue_items, active_index)
    }

    pub fn from_queue_items(items: Vec<QueueItem>, active_index: Option<usize>) -> Self {
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

    /// Mutable access to slots. Intended for test helpers and internal
    /// mutation paths; callers should prefer the explicit mutation methods
    /// (`insert`, `remove_slot`, `move_slot`, etc.) for production code.
    pub fn slots_mut(&mut self) -> &mut [QueueSlot] {
        &mut self.slots
    }

    /// Consume the queue and return its slots. Used by tests and callers
    /// that need owned slot data.
    pub fn into_slots(self) -> Vec<QueueSlot> {
        self.slots
    }

    pub fn active_slot_id(&self) -> Option<QueueSlotId> {
        self.active_slot_id
    }

    pub fn active_index(&self) -> Option<usize> {
        self.active_slot_id.and_then(|id| self.slot_index(id))
    }

    pub fn active_slot(&self) -> Option<&QueueSlot> {
        self.active_slot_id.and_then(|slot_id| self.slot(slot_id))
    }

    pub fn len(&self) -> usize {
        self.slots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    /// Returns `true` when the queue contains any `Feed` slots that a
    /// legacy peer (without `unified-queue` capability) cannot represent.
    /// Used by the daemon to reject legacy queue replacements that would
    /// silently overwrite hidden canonical slots.
    pub fn has_feed_entries(&self) -> bool {
        self.slots
            .iter()
            .any(|s| matches!(s.item, QueueItem::Feed(_)))
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

    pub fn append(&mut self, item: QueueItem) -> QueueSlotId {
        self.insert(self.slots.len(), item)
    }

    pub fn insert(&mut self, index: usize, item: QueueItem) -> QueueSlotId {
        let slot_id = self.allocate_slot_id();
        let index = index.min(self.slots.len());
        self.slots.insert(index, QueueSlot::new(slot_id, item));
        self.revision.bump();
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
        self.revision.bump();
        old_active
    }

    /// Remove all slots and clear the active slot.
    pub fn clear(&mut self) {
        if self.slots.is_empty() {
            return;
        }
        self.slots.clear();
        self.active_slot_id = None;
        self.revision.bump();
    }

    /// Truncate the slots to the given length. Used by tests to simulate
    /// a queue shrinking while a context menu is open.
    pub fn truncate_slots(&mut self, len: usize) {
        if len >= self.slots.len() {
            return;
        }
        self.slots.truncate(len);
        self.revision.bump();
        // Clear active slot if it's beyond the new length.
        if let Some(active_id) = self.active_slot_id {
            if self.slot_index(active_id).is_none() {
                self.active_slot_id = None;
            }
        }
    }

    pub fn set_active_slot(&mut self, slot_id: QueueSlotId) -> QueueMutationResult<()> {
        if self.slot_index(slot_id).is_none() {
            return QueueMutationResult::NotFound;
        }
        self.active_slot_id = Some(slot_id);
        QueueMutationResult::Applied(())
    }

    /// Set the local progress state on a slot by index. Intended for test
    /// helpers; production code should use player events to drive progress.
    /// Only affects Emby slots; Feed slots are a no-op.
    pub fn set_slot_progress_by_index(&mut self, index: usize, position_ticks: i64) {
        if let Some(slot) = self.slots.get_mut(index) {
            slot.progress_state.local.position_ticks = position_ticks;
            slot.progress_state.apply_to_item(&mut slot.item);
        }
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
        item: QueueItem,
    ) -> QueueMutationResult<()> {
        let Some(slot) = self.slots.iter_mut().find(|slot| slot.slot_id == slot_id) else {
            return QueueMutationResult::NotFound;
        };
        slot.item = item;
        slot.progress_state.local = SlotProgress::from_queue_item(&slot.item);
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
        let pending = slot.progress_state.local;
        slot.progress_state.pending_sync = Some(pending);
        QueueMutationResult::Applied(pending)
    }

    pub fn merge_refresh(&mut self, fetched_items: Vec<EmbyItem>) -> RefreshMergeResult {
        let mut fetched_by_item_id = group_fetched_items_by_item_id(fetched_items);
        let old_slots = std::mem::take(&mut self.slots);
        let mut result = RefreshMergeResult::default();
        let mut merged_slots = Vec::with_capacity(old_slots.len());
        let active_slot_id = self.active_slot_id;

        for mut slot in old_slots {
            // Feed slots have no server-side counterpart; keep them as-is.
            if matches!(slot.item, QueueItem::Feed(_)) {
                if should_protect_missing_slot(&slot, active_slot_id) {
                    result.protected_slots.push(slot.slot_id);
                }
                merged_slots.push(slot);
                continue;
            }
            let fetched = fetched_by_item_id
                .get_mut(slot.item.id())
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
        if let Some(pending) = slot.progress_state.pending_sync {
            if pending.matches_server_confirmation(&fetched_item) {
                slot.progress_state.pending_sync = None;
                slot.item = QueueItem::Emby(Box::new(fetched_item));
                if is_active {
                    slot.progress_state.apply_to_item(&mut slot.item);
                    result.protected_slots.push(slot.slot_id);
                } else {
                    if let QueueItem::Emby(ref emby) = slot.item {
                        slot.progress_state.local = SlotProgress::from_item(emby);
                    }
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
            let local_progress = slot.progress_state.local;
            slot.item = QueueItem::Emby(Box::new(fetched_item));
            slot.progress_state.local = local_progress;
            slot.progress_state.apply_to_item(&mut slot.item);
            result.protected_slots.push(slot.slot_id);
            result.updated_slots.push(slot.slot_id);
            return;
        }

        slot.item = QueueItem::Emby(Box::new(fetched_item));
        if let QueueItem::Emby(ref emby) = slot.item {
            slot.progress_state.local = SlotProgress::from_item(emby);
        }
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
