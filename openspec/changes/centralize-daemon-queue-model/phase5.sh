#!/usr/bin/env bash
set -euo pipefail

ROOT="/home/slatkin/Dev/mbv/.worktrees/centralize-daemon-queue-model"

echo "=== Phase 5: Daemon-owned persistence (tasks 5.1-5.5) ==="

# ── 5.1: config_types_paths.rs ── extend QueueState with optional slot-aware fields ──

echo "--- 5.1: Extending QueueState in config_types_paths.rs ---"

sed -i '/positions: std::collections::HashMap<String, i64>,/a\
    #[serde(default, skip_serializing_if = "Option::is_none")]\
    pub slot_ids: Option<Vec<u64>>,\
    #[serde(default, skip_serializing_if = "Option::is_none")]\
    pub revision: Option<u64>,\
    #[serde(default, skip_serializing_if = "Option::is_none")]\
    pub active_slot_id: Option<u64>,\
    #[serde(default, skip_serializing_if = "Option::is_none")]\
    pub next_slot_id: Option<u64>,' \
    "$ROOT/crates/mbv-core/src/config_types_paths.rs"

echo "5.1 done."

# ── 5.2: playback_queue.rs ── add from_persisted constructor ──

echo "--- 5.2: Adding from_persisted to PlaybackQueue ---"

python3 -c "
import re

path = '$ROOT/crates/mbv-core/src/playback_queue.rs'
with open(path, 'r') as f:
    content = f.read()

new_method = '''
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
                let q_slot_ids: Vec<QueueSlotId> = slot_ids.into_iter().map(QueueSlotId).collect();
                let active = active_slot_id.map(QueueSlotId);
                let slots: Vec<QueueSlot> = q_slot_ids
                    .into_iter()
                    .zip(items)
                    .map(|(slot_id, item)| QueueSlot::new(slot_id, item))
                    .collect();
                return Self::from_slot_snapshot(
                    slots,
                    active,
                    QueueRevision(revision.unwrap_or(0)),
                    next_slot_id.unwrap_or(1),
                );
            }
        }
        Self::from_items(items, None)
    }
'''

# Find from_slot_snapshot method end — right before 'pub fn revision'
pattern = r'(    pub fn from_slot_snapshot\(.*?\n    \}\n)(\n    pub fn revision)'
replacement = r'\1' + new_method + r'\n\2'
content = re.sub(pattern, replacement, content, flags=re.DOTALL)

with open(path, 'w') as f:
    f.write(content)
print('5.2 done.')
"

# ── 5.3: daemon_run.rs ── add persist_queue_state helper and call on mutation/shutdown ──

echo "--- 5.3: Adding persistence calls to daemon_run.rs ---"

python3 -c "
import re

path = '$ROOT/crates/mbv-core/src/daemon_run.rs'
with open(path, 'r') as f:
    content = f.read()

# 1. Add persist_queue_state helper right before the main event loop
persist_helper = '''
    /// Serialize the current queue state (including slot metadata) to the
    /// daemon-owned queue_state.json on disk. Called after every structural
    /// mutation and during graceful shutdown.
    fn persist_queue_state(shared_queue: &SharedQueueState) {
        let q = shared_queue.queue.lock().unwrap();
        let source = shared_queue.source.lock().unwrap().clone();
        let state = crate::config::QueueState {
            source,
            items: q.items_snapshot(),
            cursor: q.current_index().unwrap_or(0),
            last_played_item_id: q.active_slot().map(|s| s.item.id.clone()),
            last_played_completed: q.active_slot().map(|s| s.item.played).unwrap_or(false),
            positions: Default::default(),
            slot_ids: Some(q.slot_ids().into_iter().map(|sid| sid.raw()).collect()),
            revision: Some(q.revision().raw()),
            active_slot_id: q.active_slot_id().map(|sid| sid.raw()),
            next_slot_id: Some(q.next_slot_id()),
        };
        crate::config::save_queue_state(&state);
    }
'''

# Insert before 'loop {'
content = re.sub(r'(\n    loop \{)', persist_helper + r'\n\1', content, count=1)

# 2. After handle_ctrl calls, add persist_queue_state.
# Both handle_ctrl invocations share the pattern ending with '&merged_tx,\n                );'
# followed by the arm's closing brace.
# We insert right after the closing ');' and before the closing '}' of the arm.
# Use a pattern that matches the end of handle_ctrl(...) call and the closing brace of the match arm

content = re.sub(
    r'(&merged_tx,\s*\n\s*\);)\s*\n\s*\}(\s*\n\s*)(DaemonEvent::CtrlDisconnected|DaemonEvent::Shutdown)',
    r'\1\n                persist_queue_state(&shared_queue);\n            }\n\3',
    content
)

# 3. On Shutdown, persist before player stop
shutdown_pattern = r'(log::info!\(target: \"daemon\", \"graceful shutdown: stopping player\"\);)'
shutdown_replacement = r'\1\n                persist_queue_state(\&shared_queue);'
content = re.sub(shutdown_pattern, shutdown_replacement, content, count=1)

with open(path, 'w') as f:
    f.write(content)
print('5.3 done.')
"

# ── 5.4: queue_actions.rs ── guard save_queue_state / save_queue_state_no_clear ──

echo "--- 5.4: Guarding save_queue_state in queue_actions.rs ---"

# Guard save_queue_state(): add early return when daemon-connected
sed -i '/pub(super) fn save_queue_state(&self) {/a\
        // Daemon-owned persistence: when connected to a daemon (local or\
        // direct-remote), the daemon owns the queue snapshot on disk.\
        // The client must not race the daemon as an additional writer.\
        if self.is_local_daemon || self.has_direct_remote_queue() {\
            return;\
        }' \
    "$ROOT/src/app/queue_actions.rs"

# Guard save_queue_state_no_clear(): add same early return
sed -i '/pub(super) fn save_queue_state_no_clear(&self) {/a\
        if self.is_local_daemon || self.has_direct_remote_queue() {\
            return;\
        }' \
    "$ROOT/src/app/queue_actions.rs"

echo "5.4 done."

# ── 5.5: bootstrap.rs ── update bootstrap_local_daemon_queue for slot IDs ──

echo "--- 5.5: Updating bootstrap_local_daemon_queue for slot-aware persistence ---"

python3 -c "
path = '$ROOT/src/app/bootstrap.rs'
with open(path, 'r') as f:
    content = f.read()

# Add PlaybackQueue import
content = content.replace(
    'use super::types_player_tab::PlayerTab;\nuse mbv_core::api::MediaItem;',
    'use super::types_player_tab::PlayerTab;\nuse mbv_core::api::MediaItem;\nuse mbv_core::playback_queue::PlaybackQueue;'
)

# Replace the cursor computation block to use from_persisted when slot_ids present
old_block = '''    let cursor = super::actions::queue_restore_cursor(
        &state.items,
        state.cursor,
        state.last_played_item_id.as_deref(),
        state.last_played_completed,
    );
    LocalDaemonBootstrap {
        player_tab: PlayerTab::new(state.items.clone(), cursor),'''

new_block = '''    let (items, cursor) = if state.slot_ids.is_some() {
        // Slot-aware restore: build a temporary PlaybackQueue via
        // from_persisted to preserve slot ordering, then extract the
        // ordered item snapshot for PlayerTab.
        let pq = PlaybackQueue::from_persisted(
            state.items.clone(),
            state.slot_ids.clone(),
            state.active_slot_id,
            state.revision,
            state.next_slot_id,
        );
        let cursor = pq.current_index().unwrap_or(0);
        (pq.items_snapshot(), cursor)
    } else {
        // Legacy state without slot IDs: compute cursor as before
        let cursor = super::actions::queue_restore_cursor(
            &state.items,
            state.cursor,
            state.last_played_item_id.as_deref(),
            state.last_played_completed,
        );
        (state.items.clone(), cursor)
    };
    LocalDaemonBootstrap {
        player_tab: PlayerTab::new(items, cursor),'''

content = content.replace(old_block, new_block)

# Update adopt_queue to use slot-aware items
content = content.replace(
    'adopt_queue: Some((state.items, cursor, state.source)),',
    'adopt_queue: Some((items, cursor, state.source)),'
)

with open(path, 'w') as f:
    f.write(content)
print('5.5 done.')
"

echo "=== Phase 5 edits complete ==="
