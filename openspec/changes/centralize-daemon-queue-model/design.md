## Context

Today the daemon's queue lives as three independent `Arc<Mutex<>>` values in `daemon_run.rs`:
`items: Vec<MediaItem>`, `cursor: usize`, and `source: QueueSource`. A `SharedQueueState`
struct in `daemon_core.rs` wraps the same three for cross-thread seeding of new client
connections. Every ctrl mutation — `QueueRemove(usize)`, `QueueMove(usize, usize)`,
`JumpTo(usize)` — addresses items by positional index. The daemon applies mutations to its
bare `Vec`, broadcasts the full `CtrlState` to every connected client, and forwards the
same index-based `PlayerCommand` to the player session.

Meanwhile, `PlaybackQueue` in the same crate (`playback_queue.rs`) already provides
everything the daemon needs: monotonically-allocated `QueueSlotId(u64)`, a `QueueRevision`
bumped on every structural mutation, slot-based move/remove/set_active operations, and an
active-slot removal guard (`RequiresActiveConfirmation`). The player session
(`player_session_commands.rs`) already translates incoming indices to slot IDs at the
boundary — the translation is correct but it happens *after* the daemon has already applied
the mutation by index, so the index is the real authority.

The client maintains a parallel `PlaybackQueue` shadow in `PlayerTab` (`types_player_tab.rs`),
kept in sync with `items: Vec<MediaItem>` through manual `sync_queue_model_from_items_if_needed`
/ `sync_items_from_queue_model` calls. Every `QueueUpdated` event from the daemon replaces the
entire `PlayerTab` via `set_items()`, discarding all local slot IDs and rebuilding from
scratch. The client's `queue_cursor: usize` is already separate from the player's
`current_idx`, but full-state replacement and index-based reconciliation cannot preserve the
selected item's identity reliably when the queue mutates.

Active-item deletion is a four-step client-side dance: show confirmation modal, call
`player.stop()`, wait for `PlayerEvent::Stopped`, then call `remove_active_slot_confirmed()`
locally and send `QueueRemove(idx)` to the daemon. Under multi-client, the window between
stop and removal is a consistency gap: other clients see a stopped player with the item still
in the queue.

Constraints:

- `mbv` and `mbvd` ship from one workspace and are versioned together.
- The player session already uses `PlaybackQueue` internally; the change is about moving that
  authority upstream to the daemon control layer.
- Bare mode (no daemon) must continue to work unchanged.
- The ctrl protocol is JSON-over-Unix-socket/TCP with serde externally-tagged enums.
- The `retire-pty-relay-for-local-daemon-stay-alive` change makes multi-client attach routine,
  so queue consistency under concurrent mutation is no longer a corner case.

## Goals / Non-Goals

**Goals:**

- Make the daemon the single canonical queue authority: one `PlaybackQueue`, one revision
  counter, one slot-ID allocator. Clients are projections, not peers.
- Address every queue mutation on the wire by `QueueSlotId` with a `QueueRevision` for
  conflict detection, replacing positional indices.
- Preserve each client's UI selection locally by slot identity, independently from the
  daemon's active playback slot.
- Make active-item deletion a single daemon transaction: atomic removal plus active-marker
  advancement, with progress finalization completing asynchronously.
- Reduce the client's shadow synchronization burden: the client trusts daemon-assigned slot
  IDs and revision, and maintains only its local selection cursor.
- Continue broadcasting full state snapshots, enriched with slot identity and revision for v8
  peers; defer deltas until measurement demonstrates a need.
- Provide phased migration: a v8 daemon accepts v7 peers through an explicitly version-gated
  legacy adapter for one release window.

**Non-Goals:**

- **Rewriting the player session.** The player session already uses `PlaybackQueue` internally.
  This change moves authority upstream to the daemon control layer; the player session's
  internal `PlaybackQueue` becomes a downstream projection of the daemon's.
- **Changing bare mode.** A bare-mode `mbv` owns its own `PlaybackQueue` in-process, exactly
  as today. No daemon, no wire protocol, no slot-ID negotiation.
- **Session continuity.** Undo stacks, cursor position, and scroll state remain client-local
  and are not preserved across reconnect. This is consistent with the
  `retire-pty-relay-for-local-daemon-stay-alive` decision to reject session continuity.
- **Queue persistence format change.** `queue_state.json` continues to serialize full
  `MediaItem` objects. Slot IDs are optionally persisted for warm-daemon restart but are not
  required for cold-daemon restore.
- **Changing the Emby progress/playback reporting model.** Progress sync with Emby
  (`ProgressState`, `pending_sync`) remains orthogonal and unchanged.
- **UI redesign.** The TUI queue view, key bindings, and rendering are unchanged.

## Decisions

### 1. The daemon owns `PlaybackQueue` directly

The daemon's main loop in `daemon_run.rs` replaces `items: Vec<MediaItem>`, `cursor: usize`,
and `source: QueueSource` with a single `queue: PlaybackQueue` plus `source: QueueSource`.
`SharedQueueState` wraps `Arc<Mutex<PlaybackQueue>>` and `Arc<Mutex<QueueSource>>` instead of
three separate mutexes.

The player session's internal `PlaybackQueue` (`player_session_queue.rs`) becomes a downstream
projection: the daemon sends it slot-aware commands, and the player session reconciles its own
`PlaybackQueue` from those. The player session already translates indices to slot IDs at its
boundary (`player_session_commands.rs`); this change moves the translation upstream so the
daemon sends slot IDs directly.

*Alternative rejected — keep `Vec<MediaItem>` in the daemon and add a slot-ID mapping layer.*
This would create two sources of truth (the Vec and the mapping) that must be kept in sync,
which is exactly the problem this change eliminates. `PlaybackQueue` already exists and is
well-tested.

### 2. Slot IDs and revision on the wire

`QueueSlotId` and `QueueRevision` gain `Serialize`/`Deserialize` (they are newtypes over
`u64`, so this is trivial). The wire protocol changes:

- `WireCommand::QueueRemove(usize)` → `WireCommand::QueueRemoveBySlot { slot_id: QueueSlotId, revision: QueueRevision }`
- `WireCommand::QueueMove(usize, usize)` → `WireCommand::QueueMoveBySlot { slot_id: QueueSlotId, to_position: usize, revision: QueueRevision }`
- `WireCommand::JumpTo(usize)` → `WireCommand::JumpToSlot { slot_id: QueueSlotId }`
- New: `WireCommand::QueueInsertAt { item: MediaItem, position: usize, revision: QueueRevision }`
  restores a removed item for undo; the daemon assigns a new slot ID.
- `CtrlState` gains `slot_ids: Vec<QueueSlotId>` (parallel to `items`),
  `revision: QueueRevision`, and `active_slot_id: Option<QueueSlotId>`. Client selection is not
  wire state. The old `cursor: usize` is retained only for v7 compatibility and retired for
  v8 peers.
- New: `WireCommand::QueueRemoveActive { revision: QueueRevision }` for transactional active-item deletion.
- Append, adopt, and replace communicate daemon-assigned identities through the full state
  snapshot emitted after the accepted mutation; no second identity-assignment response format
  is introduced.

`to_position` in `QueueMoveBySlot` remains a positional index because "insert at position N"
is the natural semantic — the item being moved is identified by slot, but its destination is
a position in the ordering. This avoids the ambiguity of "move before slot X or after slot X."

*Alternative rejected — use slot IDs for move destinations too (move-before-slot).* More
ambiguous (before or after?), harder to express "move to end," and no practical benefit since
the destination is a client's intent about ordering, not about a specific item.

### 3. Revision-based conflict detection

Every mutation command carries the client's last-known `QueueRevision`. The daemon compares
it against its current revision:

- **Match**: apply the mutation, bump the revision, broadcast.
- **Mismatch**: reject with `CtrlEvent::CommandRejected { reason: "stale revision" }`
  followed by a full `CtrlEvent::State` reconciliation snapshot. The client replaces its
  local state from the snapshot and retries if the user's intent still applies.

This is a lightweight optimistic concurrency control. It does not require vector clocks or
per-client sequence numbers because the daemon processes all commands serially on one thread
(the main run loop in `daemon_run.rs`). The revision is a simple monotonic counter.

A client that has just connected and received its initial `CtrlState` has a valid revision.
Each client permits at most one structural queue mutation in flight. Further local mutation
intents are held until the resulting full snapshot or rejection arrives, then resolved against
the new slot order and revision. Rejected commands are not retried blindly; the client retries
only if the original slot-based intent still applies after reconciliation.

*Alternative rejected — per-client sequence numbers.* More complex, requires the daemon to
track per-client state, and provides no benefit over a global revision since the daemon is
single-threaded for command processing.

*Alternative rejected — CRDT/operational transform.* Massively over-engineered for a
single-writer (daemon) multi-reader (clients) model where the writer is single-threaded.

### 4. Selection remains client-local and identity-preserving

`active_slot_id: Option<QueueSlotId>` is authoritative daemon playback state. UI selection is
an `Option<QueueSlotId>` owned solely by each client; it is not sent to or stored by the
daemon. Arrow-key navigation therefore remains immediate local interaction and does not add
network traffic.

When a new full queue snapshot arrives, the client reconciles selection as follows:

1. If the selected slot still exists, preserve it regardless of its new index.
2. If the selected slot was deleted, select the slot now at its former visual position.
3. If there is no successor at that position, select the preceding slot.
4. If the queue is empty, clear selection.

On initial connect or reconnect, where no prior connected selection is retained, selection
defaults to the active slot when present, otherwise the first slot. This intentionally does
not provide session continuity.

The player session's `current_idx` continues to exist internally because mpv needs a
positional index, but it is derived from `active_slot_id` via
`PlaybackQueue::slot_index()`.

*Alternative rejected — store selection per connection in the daemon.* Selection is neither
shared queue state nor persistent session state. Sending every navigation key to the daemon
would add protocol surface and latency without improving queue correctness.

### 5. Active-item deletion as a daemon transaction

A new `WireCommand::QueueRemoveActive { revision }` command:

1. The daemon checks the revision.
2. Atomically: records the successor at the active slot's former position, removes the active
   slot via `PlaybackQueue::remove_active_slot_confirmed()`, then sets that successor active
   (or leaves active state clear if the queue is empty).
3. Broadcasts the new state (with the new `active_slot_id` and bumped revision).
4. Sends a `PlayerCommand` to the player session to stop and advance. The player session
   handles the mpv-level stop-and-next.
5. Progress finalization for the removed item (Emby scrobble, `pending_sync` flush) runs
   asynchronously on a background thread, using the removed slot's `ProgressState` captured
   before removal.

The client's four-step dance (confirm → stop → wait → remove) collapses to: confirm → close
the modal, update the local projection optimistically, and send `QueueRemoveActive`. The item
is absent in the next rendered frame without waiting for mpv shutdown. The daemon's full
snapshot confirms the transition; if the command is rejected as stale, that snapshot restores
the authoritative state. There is no intermediate stopped-but-still-visible queue state.

Undo records the removed item and its former logical position. Undo re-inserts the item at
that position through a revision-checked daemon mutation; the daemon assigns a new slot ID.
Undo does not make the restored item active and does not resume playback.

For non-active items, `QueueRemoveBySlot` is immediate and requires no confirmation — the
existing `PlaybackQueue::remove_slot()` handles it.

*Alternative rejected — keep the client-side dance but make it slot-based.* This preserves
the consistency gap under multi-client (other clients see a stopped player with the item
still in the queue during the stop-to-remove window) and adds complexity for no benefit.

### 6. Full slot-aware state broadcasts

The daemon continues to broadcast a complete `CtrlState` after structural mutations. For v8
peers the snapshot includes ordered items, parallel slot IDs, revision, active slot ID, and
source. For v7 peers it contains the legacy items and positional cursor fields.

Full snapshots keep projection replacement and stale-revision reconciliation on one path and
avoid adding delta application, gap detection, and explicit resynchronization machinery to
this refactor. Typical queue sizes do not justify that complexity without measurement.

*Alternative deferred — incremental slot-level deltas.* Stable identities and revisions make
deltas possible later. They should be introduced only if profiling shows full-state fan-out is
a meaningful bottleneck.

### 7. Phased migration by peer protocol version

The protocol version bumps from 7 to 8. For one release window the v8 daemon accepts both
peer versions:

- **v8 client + v8 daemon**: use slot-based commands and slot-aware full snapshots.
- **v7 client + v8 daemon**: the daemon records the connection as legacy, accepts old
  index-based commands, translates them to slot-based operations internally, and emits the
  v7 full-state format.
- **v8 client + v7 daemon**: rejected at the hello handshake (protocol version mismatch).

The translation in the v8 daemon for v7 clients: look up the slot at the given index, apply
the slot-based operation, and broadcast in the old format. This is straightforward because
the daemon now owns `PlaybackQueue` and can resolve any index to a slot ID.

The legacy handlers are explicitly gated by the connection's negotiated peer version and are
not reachable for v8 peers. The migration window lasts one release cycle. After that, v7
acceptance and old index-based wire variants are retired and the protocol version bumps again.

*Alternative rejected — big-bang protocol switch.* Simpler but forces lockstep upgrade of
all clients and the daemon simultaneously. Under stay-alive, a user might have a running
daemon and open a new terminal with an updated `mbv` — the phased migration handles this
gracefully.

### 8. Persistence ownership

The daemon is the sole writer of `queue_state.json`. Clients never write it. Today, clients
write `queue_state.json` on every queue mutation (`save_queue_state()` in
`queue_actions.rs`), which under multi-client means multiple writers racing on the same
file.

Under this design:

- The daemon writes `queue_state.json` on structural mutations and on shutdown, exactly as
  clients do today — but there is exactly one writer.
- The serialized format gains an optional `slot_ids: Vec<QueueSlotId>` field. On cold-daemon
  restore (no daemon running, client reads the file), slot IDs are absent and a fresh
  `PlaybackQueue` is built from items (as today). On warm-daemon restart, slot IDs survive
  and the daemon's `PlaybackQueue` is reconstructed with the same identities.
- Clients in bare mode continue to write `queue_state.json` directly (no daemon involved).

*Alternative rejected — let clients continue writing.* Multiple writers under multi-client
is a race condition. The daemon is the natural single writer.

### 9. Reconnect and bootstrap

When a client connects to a daemon:

1. The daemon sends `CtrlEvent::State` with the full queue including slot IDs, revision,
   active slot, and source.
2. The client builds its `PlayerTab` from this snapshot. `PlaybackQueue::from_items` is
   replaced by a new `PlaybackQueue::from_slot_snapshot` that uses the daemon-assigned slot
   IDs directly, preserving identity.
3. The client's undo stack starts empty. Undo is bounded by the connection lifetime. Local
   selection defaults to the active slot, otherwise the first slot.
4. For a cold daemon (empty queue), the client sends `AdoptQueue` and learns the
   daemon-assigned slot IDs from the resulting full snapshot.

The existing `bootstrap_local_daemon_queue` logic (warm vs. cold daemon) is preserved; only
the data format changes.

### 10. Undo boundaries

Undo remains client-local. The boundaries are:

- **Connection lifetime**: the undo stack is cleared on disconnect. A reconnecting client
  gets a fresh stack from the bootstrap snapshot.
- **Revision scope**: an undo entry records the revision at which the mutation was applied.
  If the daemon's revision has advanced beyond the entry's revision (because another client
  mutated the queue), the undo is rejected locally — the client does not send a stale
  mutation.
- **No cross-client undo**: client A cannot undo client B's mutations. This is consistent
  with the existing model where each client has its own undo stack.

The `UndoEntry` type gains a `revision: QueueRevision` field and uses `QueueSlotId` instead
of positional index for `Remove` entries:

```
enum UndoEntry {
    Remove { removed_slot_id: QueueSlotId, position: usize, item: MediaItem, revision: QueueRevision },
    Move { slot_id: QueueSlotId, from: usize, to: usize, revision: QueueRevision },
}
```

Undoing a remove uses `QueueInsertAt` with the recorded position and current revision. The restored
slot receives a new daemon-assigned ID. If the removed item was active, undo restores only
queue membership and ordering; it does not resume or redirect playback.

### 11. Bare mode is unchanged

A bare-mode `mbv` (no daemon) owns its own `PlaybackQueue` in-process. The player session
already uses `PlaybackQueue` internally, and the app layer already uses `PlayerTab` with its
shadow `PlaybackQueue`. No wire protocol is involved. The only change is that
`PlayerTab`'s shadow sync can be simplified since the local `PlaybackQueue` is now the
single source of truth (no daemon broadcast to reconcile against).

## Risks / Trade-offs

- **Protocol complexity increase.** The wire format gains slot IDs and revisions. → Mitigated
  by retaining full-state snapshots and by limiting compatibility to one explicit v7 adapter
  window. The new fields extend types that already exist (`QueueSlotId`, `QueueRevision`).

- **Player session reconciliation.** The player session already has its own `PlaybackQueue`.
  Making it a downstream projection of the daemon's queue requires care: the player session
  must reconcile its internal state when the daemon sends slot-aware commands. → Mitigated
  by the fact that the player session already translates indices to slot IDs at its
  boundary. The change is to accept slot IDs directly rather than translating.

- **Optimistic active-deletion reconciliation.** A stale-revision rejection can briefly restore
  an item the client optimistically hid. → Mitigated by immediate full-state reconciliation;
  this is rare in single-client use and preserves correctness under concurrent clients.

- **Warm-daemon slot-ID persistence.** If the daemon crashes and restarts, slot IDs from
  `queue_state.json` must be correctly reconstructed. → Mitigated by introducing one validated
  constructor for pre-assigned slots, revision, active slot, and next-ID allocation state.

- **Two queue models during migration.** During the phased migration, the daemon must
  support both index-based and slot-based commands. → Mitigated by the fact that the daemon
  owns `PlaybackQueue` and can resolve any index to a slot ID. The translation is a thin
  adapter layer that is deleted after the migration window.

- **Undo staleness under multi-client.** A user who removes an item, then another client
  mutates the queue, cannot undo the removal because the revision has advanced. → This is
  the correct behavior: the queue has changed and reinserting at the old position may not
  make sense. The client surfaces this as "undo unavailable: queue changed" rather than
  silently applying a stale mutation.

## Migration Plan

1. **Phase 1 — Daemon queue model** (no protocol change): Replace `Vec<MediaItem>` +
   `cursor` with `PlaybackQueue` in the daemon. The daemon translates its slot-based
   internal operations to the existing index-based wire protocol. Clients are unchanged.
   This phase is invisible to the wire and can be validated independently.

2. **Phase 2 — Slot-based wire protocol** (protocol version bump to 8): Add slot-based
   command variants, revision-based conflict detection,
   `QueueRemoveActive`, and slot-aware full snapshots. Peer protocol version gates the format;
   v7 clients continue to work through the daemon's explicitly gated translation layer.

3. **Phase 3 — Client migration**: Update clients to use the new slot-based commands.
   Simplify `PlayerTab` shadow sync. Collapse the active-item deletion dance. Update undo
   to use slot IDs and revision.

4. **Phase 4 — Retire legacy format** (protocol version bump to 9): Delete v7 acceptance,
   index-based wire command variants, and the daemon translation layer. Bare-mode internal
   index operations remain where required by the in-process player API.

Each phase is independently shippable and testable. Phase 1 can land without any client
changes. Phase 2 can land with the old clients still working. Phase 3 can land client-by-
client. Phase 4 is a cleanup that happens after the migration window.
