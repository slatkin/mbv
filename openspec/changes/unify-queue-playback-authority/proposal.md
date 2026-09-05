## Why

`QueueSlotId` exists to make ordinal position non-authoritative, but slot
identity does not survive the Player owner boundary. `PlaybackRun` builds its
own queue with freshly allocated slot ids, so every command and event between
an owner and its Playback run is a `usize` index resolved against a queue the
sender no longer holds. When a mutation is in flight the two queues disagree,
and the code reconciles by clamping — `daemon_run.rs` literally comments *"the
player's internal queue may lag behind the canonical queue … clamp the reported
index"*. A clamp silently activates the wrong slot instead of rejecting a stale
command.

The same lifecycle is also written five times (`cmd_replace_queue`,
`cmd_load_new`, `cmd_submit_queue`, `replace_with_queue_items`,
`accept_stopped_replacement`), the 95%-watched rule three times with three
different meanings, and Consume exists only in the TUI — so a packaged `mbvd`
owner never consumes its canonical queue at all.

## What Changes

- **Slot identity crosses the owner/Playback-run boundary.** `PlaybackRun` is
  constructed from owner-assigned slot ids instead of allocating its own, and
  the queue-addressing `PlayerCommand`/`PlayerEvent` variants carry
  `QueueSlotId` instead of `usize`. A command naming a slot the owner no longer
  holds is rejected, not clamped onto a neighbour.
- **BREAKING (wire, unreachable):** the queue-addressing `WireCommand` variants
  (`JumpTo`, `QueueAppend`, `QueueRemove`, `QueueMove`, `ReplaceQueue`,
  `LoadNew`) are removed. Every current Client already routes these through
  `UnifiedQueue*` / `PlaybackIntent`, so nothing in this repository sends them;
  removing them is what frees `PlayerCommand` to carry slot identity without
  touching `CTRL_PROTOCOL_VERSION`. Transport commands (pause, seek, volume,
  tracks) keep their existing wire shape.
- **One queue-start lifecycle.** `cmd_replace_queue` is deleted (a strict subset
  of `cmd_submit_queue`, and the daemon already converts one into the other);
  the remaining start paths share a single `begin_queue` step so `stop_report`,
  `load_state`, `active_file` and the status projection cannot drift per call
  site.
- **One near-end rule.** The two inlined `last_valid_pos * 20 / runtime >= 19`
  copies are replaced by the existing `is_near_end`, which resolves the current
  disagreement over whether runtime comes from the completed item or from live
  status.
- **Consume moves to the queue owner**, so a daemon-owned queue consumes
  identically to an in-process one.
- Dead-condition cleanup: `natural_end` inside the `reason == Quit` branch of
  `on_end_file` is unconditionally `false` and its `||` arm is removed.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `unified-playback-queue`: "Queue occurrences have stable slot identity"
  currently permits *"its slot identity **or an index resolved against the same
  canonical queue**"*. That escape hatch is what today's index-clamping relies
  on; the requirement is tightened so an index is only ever a within-process
  presentation coordinate and never addresses a slot across an owner boundary.
  A new requirement makes stale slot addressing an explicit rejection rather
  than a clamp, and "Completion and consumption address the canonical slot"
  gains a scenario binding Consume to the queue owner rather than to a client.

## Impact

- `crates/mbv-core/src/`: `player_run_queue.rs`, `player_run_commands.rs`,
  `player_run_events.rs`, `player_types.rs`, `ctrl.rs`, `daemon_control.rs`,
  `daemon_run.rs`, `daemon_reconciliation.rs`, `player_proxy.rs`.
- `src/app/`: `player_event.rs` (Consume handoff), `queue_scope.rs`.
- No `CTRL_PROTOCOL_VERSION` bump and no new ctrl capability: the removed wire
  variants have no in-repo sender, and slot ids stay on the in-process channel.
- Persisted `QueueState` is untouched — slot ids are runtime-only, as today.

### Deliberately out of scope

- **`PlayerTab.queue_cursor`** and its five companion flags duplicate a position
  the canonical queue already knows. Worth removing, but it is client
  presentation state and its reconciliation rules change once slot identity
  lands; sequencing it after this change makes it a smaller diff.
- **The `include!` chain in `player.rs`** (ten files, one ~5,000-line module)
  routes around the repo line cap and is why `PlaybackRun`'s 35-field state
  machine sprawled unchallenged. A file-organization concern, not a queue
  behaviour one.
