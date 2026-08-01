Implementation is phased so the daemon model can land before the wire and client migrations.
Retiring v7 compatibility remains a separate future change.

## 1. PlaybackQueue foundation

- [x] 1.1 Make `QueueSlotId` and `QueueRevision` transparently serializable.
- [x] 1.2 Add a validated constructor that restores pre-assigned slots, active slot, revision,
      and next-slot allocation state without reusing IDs.
- [x] 1.3 Expose the ordered slot snapshot needed by daemon persistence and ctrl snapshots.
- [x] 1.4 Run the existing focused `PlaybackQueue` checks and verify slot/revision round-tripping.

## 2. Phase 1: canonical daemon queue without wire changes

- [x] 2.1 Replace `daemon_run.rs`'s authoritative `items: Vec<MediaItem>` and positional cursor
      with one `PlaybackQueue`; keep queue source alongside it.
- [x] 2.2 Update `SharedQueueState` and all readers/writers to share that canonical model rather
      than independent item and cursor mutexes.
- [x] 2.3 Adapt existing v7 index commands at the daemon boundary by resolving the addressed
      slot and applying the operation to `PlaybackQueue`.
- [x] 2.4 Derive the existing v7 `CtrlState` items and active index from `PlaybackQueue` so this
      phase makes no externally visible protocol change.
- [x] 2.5 Keep the player session as a downstream playback projection and make command ordering
      explicit so daemon and session state cannot independently become authoritative.
- [x] 2.6 Verify existing daemon, remote-player, and playback-session behavior plus a manual
      append/remove/move/play sequence through a connected client.

## 3. Phase 2: versioned slot-aware wire model

- [x] 3.1 Add v8 queue commands addressed by identity:
      `QueueRemoveBySlot { slot_id, revision }`,
      `QueueMoveBySlot { slot_id, to_position, revision }`,
      `JumpToSlot { slot_id }`, `QueueRemoveActive { revision }`, and
      `QueueInsertAt { item, position, revision }` for undo restoration.
- [x] 3.2 Extend v8 `CtrlState` with ordered slot IDs, queue revision, and active slot ID. Retain
      the positional cursor only in the v7 serialization path.
- [x] 3.3 Bump the protocol to 8 and update compatibility negotiation so a v8 daemon accepts
      v7 peers for one release window, records their peer version, and rejects unsupported
      combinations. A v7 daemon continues to reject a v8 client.
- [x] 3.4 Gate legacy index-based wire handlers explicitly on a v7 peer connection. Reject
      legacy wire commands from v8 peers. Keep bare-mode in-process index operations separate
      and clearly marked rather than treating them as wire compatibility handlers.
- [x] 3.5 Update wire serialization fixtures and existing handshake checks for the negotiated
      v7-to-v8 compatibility path.

## 4. Phase 2: daemon mutation and broadcast behavior

- [x] 4.1 Implement revision checking for structural v8 commands. On mismatch, reject the
      command and send the requesting client a full authoritative snapshot.
- [x] 4.2 Apply accepted remove, move, insert, append, replace, and adopt operations to the
      canonical daemon queue and advance revision exactly once per structural transaction.
- [x] 4.3 Implement `QueueRemoveActive` as one daemon transaction: capture final reporting
      context, remove the active slot, choose the successor, broadcast committed state, then
      drive player stop/advance and progress finalization asynchronously.
- [x] 4.4 Broadcast a complete queue snapshot after each accepted structural mutation. Emit
      slot-aware v8 state to v8 peers and legacy positional state to v7 peers.
- [x] 4.5 Forward slot-aware effects to the player session without allowing its projection to
      overwrite newer daemon state.
- [x] 4.6 Verify stale-revision reconciliation, mixed v7/v8 fan-out, active deletion, and
      legacy-handler gating with existing focused checks and a two-client manual scenario.

## 5. Daemon-owned persistence

- [x] 5.1 Extend `QueueState` compatibly with optional slot IDs, revision, active slot, and
      next-slot allocation state needed for identity-safe restoration.
- [x] 5.2 Make the daemon the sole writer of queue state while clients are attached to any
      daemon; persist after structural mutation and during orderly shutdown.
- [x] 5.3 Restore the daemon queue from slot-aware state when available and allocate fresh
      identities for legacy state without slot metadata.
- [x] 5.4 Preserve existing bare-mode persistence ownership and prevent daemon-connected
      clients from racing the daemon as additional writers.
- [x] 5.5 Verify orderly restart preserves slot identities and legacy queue files remain
      readable. Verify unannounced-loss recovery uses the latest daemon-owned snapshot.

## 6. Phase 3: client projection and slot commands

- [x] 6.1 Update remote command translation and state application for v8 slot commands and
      full slot-aware snapshots; retain the bounded v7 path only when negotiated.
- [x] 6.2 Build `PlayerTab` from daemon-assigned slots and derive displayed items from that
      projection instead of rebuilding local slot identities from each item snapshot.
- [x] 6.3 Send remove, move, jump, and restore intentions by slot ID and last-known revision.
- [x] 6.4 Define one in-flight/reconciliation policy for rapid local mutations so successive
      keypresses cannot unknowingly send the same stale revision.
- [x] 6.5 Update `QueueUpdated` handling to replace the projection from each full authoritative
      snapshot while preserving client-local selection by identity.
- [x] 6.6 Verify remote queue mutations reconcile to the daemon snapshot without maintaining a
      second authoritative client queue.

## 7. Active-item deletion UX

- [x] 7.1 Retain the client-side confirmation modal. On confirmation, close it, optimistically
      remove the slot from the local projection, choose the local successor, and send
      `QueueRemoveActive { revision }` in the same input handling cycle.
- [x] 7.2 Remove the client `stop → pending_delete_idx → wait for Stopped → remove` path and
      its app state. The next rendered frame must not show the deleted row while mpv shuts down.
- [x] 7.3 Reconcile the optimistic projection from the daemon's next full snapshot. If the
      mutation is rejected, restore authoritative state rather than retaining the optimistic
      deletion.
- [x] 7.4 Verify two attached clients see the committed removal before playback teardown
      completes and never observe a stopped-but-still-queued intermediate state.

## 8. Client-local identity-preserving selection

- [x] 8.1 Represent queue selection in `PlayerTab` as `Option<QueueSlotId>` while deriving a
      positional index only for rendering and navigation.
- [x] 8.2 Preserve the selected slot when it survives a snapshot. When deleted, select the
      successor at its former visual position, otherwise the predecessor; clear on empty.
- [x] 8.3 On initial connect or reconnect, default selection to the active slot, otherwise the
      first slot. Do not send selection changes over ctrl or store them in the daemon.
- [x] 8.4 Apply the same selection semantics in bare mode so deleting the top row cannot snap
      focus to the playing item merely because playback state changed.
- [x] 8.5 Verify selection remains independent between two clients and independent of playback
      advancement.

## 9. Undo boundaries and active-delete restoration

- [x] 9.1 Record undo entries by slot identity, item, prior logical position, applied revision,
      and whether the removed slot had been active.
- [x] 9.2 Reject undo locally when another structural mutation has advanced the daemon revision;
      clear undo history on reconnect.
- [x] 9.3 Undo removal with `QueueInsertAt` at the recorded position. Accept the daemon's newly
      assigned slot ID for the restored item.
- [x] 9.4 When undoing active-item deletion, restore queue membership only: do not make the item
      active, resume it, or redirect current playback.
- [x] 9.5 Verify ordinary remove/move undo and active-delete undo within one connection, plus
      stale undo after another client mutates the queue.

## 10. Bootstrap and player-session migration

- [x] 10.1 Update warm-daemon bootstrap and reconnect to consume full slot-aware state. Update
      cold adoption to replace temporary identities with the daemon snapshot.
- [x] 10.2 Update `App::new_remote` and restart paths to construct the client projection from
      daemon-assigned slots and clear connection-bounded undo state.
- [x] 10.3 Update the existing `src/app/tests_daemon_bootstrap.rs` checks for warm snapshots,
      cold adoption, reconnect selection defaults, and undo clearing.
- [x] 10.4 Add slot-aware player-session command handlers. Keep index handlers only for bare
      in-process operation and the explicitly peer-gated v7 daemon adapter; do not leave an
      ambiguous ungated coexistence path.
- [x] 10.5 Verify playback-session behavior for daemon slot commands and unchanged bare mode.

## 11. Documentation and final verification

- [x] 11.1 Record daemon queue authority, slot identity, revision conflicts, client-local
      selection, active-delete transaction semantics, and bounded v7 compatibility in the ADR.
- [x] 11.2 Update queue vocabulary in the project domain documentation without duplicating the
      OpenSpec requirements.
- [x] 11.3 Run formatting, clippy, and the existing focused modules touched by implementation.
- [x] 11.4 Exercise the compatibility matrix: v8 client/v8 daemon, v7 client/v8 daemon, rejected
      v8 client/v7 daemon, and two concurrent v8 clients with stale-revision reconciliation.
- [x] 11.5 Exercise active deletion, immediate next-frame visibility, asynchronous cleanup,
      identity-preserving selection, reconnect defaults, undo without playback resumption,
      and daemon-owned persistence.
