# 0016 - Centralized Daemon Queue Model

## Status
Accepted

## Context
The queue was co-owned by clients and daemon through independent `Vec<MediaItem>` copies with positional indexing, leading to unstable selection and a complex active-item deletion dance. Each mutation required careful coordination between daemon and player session with no stable identity across reorder/move operations.

## Decision
The daemon owns the canonical `PlaybackQueue`. All mutations address items by stable `QueueSlotId` with `QueueRevision`-based conflict detection. Client UI selection is local and identity-preserving. Active deletion is a single daemon transaction with immediate UI removal and asynchronous mpv cleanup.

### Key design elements

1. **Daemon queue authority** — The daemon's `SharedQueueState` holds the one authoritative `PlaybackQueue`. Every structural mutation (append, remove, move, insert, adopt) is applied here first, the revision is advanced, and a full slot-aware snapshot is broadcast to all clients.

2. **Slot identity** — Each queue entry has a `QueueSlotId` that is assigned once and never reused within the lifetime of a queue instance. Slots remain stable across reorder and move operations. `next_slot_id` is a monotonically increasing allocation counter persisted alongside the queue to prevent ID reuse across restarts.

3. **Revision-based conflict detection** — Every structural mutation command carries the client's last-known `QueueRevision`. If the daemon's current revision does not match, the command is rejected with `CommandRejected` and a full authoritative snapshot is sent for the client to reconcile. This prevents stale mutations from silently corrupting the queue.

4. **Client-local identity-preserving selection** — Each client tracks its visual selection as `Option<QueueSlotId>`, deriving a positional index only for rendering. When a snapshot arrives, the selected slot survives if still present; when the selected slot is deleted, the successor at the same visual position is chosen (or the predecessor if at tail). Selection is never sent over the wire or stored in the daemon.

5. **Active-item deletion transaction** — `QueueRemoveActive` captures the active slot identity and its positional index in one lock acquisition, removes the slot from the queue, chooses the successor, broadcasts the committed state, then drives the player to advance or stop asynchronously. On the client side, the removed row is deleted from the local projection in the same input handling cycle, so the next rendered frame never shows it. The mpv cleanup (stop/advance) runs asynchronously without blocking UI updates.

6. **Bounded v7 compatibility** — A v8 daemon accepts v7 peers for one release window, records their peer version, and gates legacy index-based wire handlers explicitly on the peer version. Legacy wire commands from v8 peers are rejected. A v7 daemon rejects a v8 client. Bare-mode in-process index operations remain separate from wire compatibility handlers.

7. **Daemon-owned persistence** — When clients are attached to any daemon, the daemon is the sole writer of `queue_state.json` (persists after structural mutations and during orderly shutdown). Bare-mode persistence ownership is preserved, and daemon-connected clients are prevented from racing the daemon as additional writers.

## Consequences

- Slots are stable across reorder/move; IDs are never reused
- Stale mutations are rejected with authoritative snapshot reconciliation
- Active-item deletion removes the row in the next frame — no intermediate "stopped but still queued" state
- Multiple clients see consistent slot identities and can independently maintain their own selection
- Undo is bounded by revision: undo history is cleared on reconnect, and a stale undo (after another client mutates the queue) is rejected locally
- v7 clients are supported for one release window through explicitly gated legacy handlers
- Bare-mode operation continues unchanged, using index-based player commands that are internally slot-aware
- The player session remains a downstream playback projection; daemon and session state cannot independently become authoritative
