## Why

The daemon's queue model is a bare `Vec<MediaItem>` plus a `cursor: usize`, while every queue
mutation on the ctrl wire — `QueueRemove(usize)`, `QueueMove(usize, usize)`, `JumpTo(usize)` —
addresses items by positional index. Meanwhile, `PlaybackQueue` in the same crate already
allocates stable `QueueSlotId`s, tracks a `QueueRevision` on every structural mutation, and
refuses unsafe active-slot removal — but none of that reaches the wire or the daemon's main
loop. The result is a queue authority that is positional, a protocol that cannot express "the
item the user meant" under concurrent mutation, client selection that is reconciled by index
instead of stable identity, and an active-item deletion that requires a four-step client-side dance
(stop, wait for Stopped, remove locally, send QueueRemove) because the daemon has no concept of
a transactional removal. With `retire-pty-relay-for-local-daemon-stay-alive` making multi-client
attach the routine path, these gaps stop being theoretical.

## What Changes

- The daemon (both local daemon and `mbvd`) SHALL own a `PlaybackQueue` as its canonical queue
  model, replacing the bare `Vec<MediaItem>` + `cursor: usize` in `daemon_run.rs`. Slot IDs and
  revision numbers become first-class daemon state.
- **BREAKING**: Queue mutation commands on the ctrl wire (`QueueRemove`, `QueueMove`, `JumpTo`)
  SHALL address items by `QueueSlotId` instead of positional index, and SHALL carry the client's
  last-known `QueueRevision` so the daemon can reject stale mutations with a reconciliation
  snapshot instead of silently applying them to the wrong item.
- **BREAKING**: `CtrlState` SHALL carry `QueueSlotId` per item and a `QueueRevision`, alongside
  the existing `items` and `source`. Playback position SHALL be represented by
  `active_slot_id`; each client SHALL keep its UI selection locally by slot identity. The
  legacy wire `cursor: usize` is retired for v8 peers.
- Active-item deletion SHALL become an immediate daemon transaction: after client-side
  confirmation, a single
  `QueueRemoveActive` command that atomically removes the active slot, advances the active
  marker, and returns the new state replaces the stop/wait/remove round-trip. The client
  closes the modal and reflects the intended removal in the next rendered frame without
  waiting for mpv shutdown; authoritative reconciliation and progress finalization complete
  asynchronously.
- `AdoptQueue`, `ReplaceQueue`, and append operations SHALL assign canonical slot IDs and
  communicate them through the resulting full state snapshot.
- Queue state broadcasts SHALL remain full snapshots, but v8 snapshots SHALL include slot IDs,
  revision, and active-slot identity. Incremental deltas are deferred until there is evidence
  that full snapshots are a meaningful bottleneck.
- The client's `PlayerTab` dual-representation shadow (`items` + `PlaybackQueue`) SHALL be
  reduced: the client trusts the daemon's slot identities and revision, and maintains its
  display projection plus an identity-preserving local selection. If the selected slot survives
  a mutation it remains selected; if deleted, selection falls to the successor at the former
  visual position, otherwise the predecessor.
- Undo boundaries SHALL be defined: undo is a client-local operation bounded by the client's
  connection lifetime and the daemon's revision. A reconnecting client gets a fresh undo stack
  from the bootstrap snapshot. Undoing active-item deletion restores the item at its prior
  logical position through a revision-checked `QueueInsertAt` command, with a new
  daemon-assigned slot ID, and does not resume playback.
- Protocol compatibility: the wire changes require a protocol version bump. During one release
  window, a v8 daemon SHALL accept v7 clients and gate the legacy index adapter by the peer's
  negotiated protocol version; v8 clients remain incompatible with v7 daemons.

## Capabilities

### New Capabilities
- `daemon-canonical-queue`: the daemon owns `PlaybackQueue` as its authoritative queue model;
  slot identity and revision are first-class state; the wire protocol addresses mutations by
  slot ID with revision-based conflict detection; selection is separated from playback cursor;
  active-item deletion is an immediate daemon transaction; full slot-aware state broadcasts;
  persistence ownership; reconnect/bootstrap semantics; undo boundaries.

### Modified Capabilities
- `ctrl-protocol`: wire format gains slot IDs, revision numbers, active-slot identity, and a
  `QueueRemoveActive` command; protocol version bump; phased migration keyed by peer protocol
  version.
- `daemon-multi-connection`: concurrent queue mutation ordering under revision-based conflict
  detection; reconciliation on stale revision; broadcast fan-out sends the full format
  appropriate to each peer version.

## Impact

**Core types**: `crates/mbv-core/src/ctrl.rs` (WireCommand, CtrlCmd, CtrlEvent, CtrlState),
`crates/mbv-core/src/playback_queue.rs` (already has the model; may need serialization support
for `QueueSlotId` and `QueueRevision` on the wire), `crates/mbv-core/src/player_types.rs`
(PlayerCommand variants gain slot-ID overloads or are replaced).

**Daemon**: `crates/mbv-core/src/daemon_run.rs` (replace `Vec<MediaItem>` + `cursor` with
`PlaybackQueue`), `crates/mbv-core/src/daemon_control.rs` (rewrite mutation handlers to use
slot IDs and revision checks), `crates/mbv-core/src/daemon_core.rs` (SharedQueueState gains
slot/revision state; broadcast format changes).

**Player session**: `crates/mbv-core/src/player_session_commands.rs` (already translates indices
to slot IDs internally; the translation moves upstream to the daemon),
`crates/mbv-core/src/player_session_queue.rs` (no structural change; already uses
`PlaybackQueue`).

**Client**: `src/app/queue_actions.rs` (mutations send slot IDs instead of indices; active-item
deletion collapses to one command), `src/app/types_player_tab.rs` (shadow sync simplified),
`src/app/player_event.rs` (QueueUpdated handler consumes slot-aware state),
`src/app/bootstrap.rs` (bootstrap receives slot IDs from daemon),
`crates/mbv-core/src/remote_player.rs` (command translation updated),
`crates/mbv-core/src/remote_player_connect.rs` (state application handles new format).

**Persistence**: `crates/mbv-core/src/config_types_paths.rs` (`QueueState` gains optional slot
IDs for warm-daemon persistence; cold-daemon restore still works without them).

**Verification**: existing focused checks in `crates/mbv-core/src/daemon_tests.rs`,
`crates/mbv-core/src/remote_player_tests.rs`, and
`crates/mbv-core/src/playback_queue_tests.rs` are updated where the wire format changes;
multi-client conflict, active deletion, selection stability, and compatibility are exercised
through the implementation verification matrix.

**Protocol version**: bumped from 7 to 8. For one release window, a v8 daemon accepts a v7
hello, records that connection as legacy, translates its index-based commands daemon-side,
and emits v7 full-state snapshots. A v7 daemon rejects a v8 client.

**Dependencies**: no new external dependencies. `QueueSlotId` and `QueueRevision` need serde
`Serialize`/`Deserialize` (they are simple newtypes over `u64`).
