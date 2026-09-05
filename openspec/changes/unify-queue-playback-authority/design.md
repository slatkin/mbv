## Context

See `proposal.md` — Why. The structural facts that shape the approach:

- Three `PlaybackQueue` instances hold the same logical queue: the Client's
  `PlayerTab.queue`, the owner's canonical queue in `daemon_control`, and
  `PlaybackRun.queue`. The first two exchange slot identity over ctrl
  (`UnifiedQueue*`); the third is handed items through
  `PlaybackQueue::from_queue_items`, which allocates fresh ids from 1, so its
  identities are unrelated to the owner's.
- `PlaybackQueue::from_slot_items` already exists and preserves caller-assigned
  ids. It is used at the ctrl boundary but not when constructing a Playback run.
- The owner ↔ Playback run channel is an in-process `mpsc<PlayerCommand>` plus
  `mpsc<PlayerEvent>`. `PlayerCommand` is serialized *only* through
  `CtrlCmd::PlayerCmd(WireCommand)`.
- Every queue mutation a current Client issues to a remote owner already goes
  through `UnifiedQueue*` or `PlaybackIntent`
  (`remote_player.rs`, `player_proxy.rs`). The queue-addressing `WireCommand`
  variants have no in-repo sender.
- `PlaybackRun` carries `current_idx`, `PlayerStatus.current_idx` and
  `queue.active_slot_id()` for the same fact, reconciled by
  `refresh_current_idx_from_queue()` / `sync_status_position()`.

## Goals / Non-Goals

**Goals:**

- Slot identity is the only cross-component address for a queue occurrence.
- A stale address fails loudly at the receiver instead of being clamped.
- One queue-start lifecycle, one near-end rule, one Consume site.
- No `CTRL_PROTOCOL_VERSION` bump and no new negotiated capability.

**Non-Goals:**

- Collapsing `current_idx` / `PlayerStatus.current_idx` into the queue's active
  slot. Slot identity crossing the boundary is what makes that removal safe; it
  is a follow-up, not a precondition.
- Client-side cursor state (`queue_cursor` and companions) — see the proposal's
  out-of-scope note.
- Splitting `player.rs`'s `include!` chain. Real debt, unrelated axis.
- Any change to persisted `QueueState`. Slot ids stay runtime-only.

## Decisions

### D1: Delete the queue-addressing wire variants rather than widening them

`WireCommand` loses `JumpTo`, `QueueAppend`, `QueueRemove`, `QueueMove`,
`ReplaceQueue`, `LoadNew`. Transport variants (pause, seek, volume, tracks,
next-up, skip-intro) stay byte-identical.

*Why:* this is the enabling deletion. Once no `PlayerCommand` variant that names
a queue occurrence is serialized, `PlayerCommand` and `PlayerEvent` are
in-process types and can carry `QueueSlotId` freely — no wire representation for
slot ids, no capability negotiation, no version bump. `WireCommand`'s doc
comment already promises that adding a `PlayerCommand` variant is a compile
error until the conversions are updated; the exhaustive `From` impls make this
deletion compiler-checked rather than grep-checked.

*Alternative rejected:* add `slot_id` to the wire variants and gate on a new
`unified-slot-commands` capability. That keeps a second, index-shaped queue
protocol alive forever to serve a sender that does not exist, and contradicts
the existing requirement that compatibility handling "SHALL NOT create a second
internal queue model".

*Consequence:* a pre-unified peer loses remote queue mutation. That peer is
already refused by `abs_queue_transport_rejection` for anything but Emby, and
nothing in this repository is such a peer. The daemon arm for
`WireCommand::QueueAppend` is deleted with it — that arm currently answers an
append by re-submitting the entire queue as `SubmitQueue`, which restarts the
playing track (audit finding 3). It is deleted rather than fixed.

### D2: The Playback run adopts the owner's slot ids

`PlaybackRun` is constructed via `PlaybackQueue::from_slot_items`, and
`PlayerCommand::SubmitQueue` / `QueueAppend` carry `(QueueSlotId, QueueItem)`
pairs. `PlaybackRun` never calls `allocate_slot_id`.

*Why:* it is the smallest change that makes the two queues speak the same
language, and it reuses a constructor that already exists for exactly this
purpose.

*Consequence:* `on_playlist_pos_changed` still receives an mpv playlist index —
that is genuinely an adapter coordinate, and mpv is the authority for it. It
resolves that index against `PlaybackRun`'s own queue, which is legitimate
(same-component resolution, per the spec's narrowed wording), and emits a slot
id outward.

### D3: Reject stale addressing; delete the clamps

`daemon_run.rs`'s `idx.min(queue.len() - 1)` and
`src/app/player_event.rs`'s index resolution are replaced by slot lookup that
returns `QueueMutationResult::NotFound`. `PlaybackQueue` already returns
`NotFound` from every mutation; the callers currently discard it.

*Why:* the clamp is the bug. A report for a slot that is gone carries no
information about which slot should be active, so acting on it can only be
wrong. `CommandRejected` already exists as the user-visible path
(`src/app/player_event.rs` handles it), so rejection needs no new plumbing.

### D4: `cmd_replace_queue` is deleted, not refactored

`PlayerCommand::ReplaceQueue` is removed; the daemon already translates it into
`SubmitQueue` (`daemon_control.rs:177`). `cmd_submit_queue`,
`replace_with_queue_items` and `accept_stopped_replacement` share one private
`begin_queue(items, start_idx)` that owns the per-item reset:
`begin_item_lifecycle`, `stop_report`, `load_state`,
`pending_initial_playlist_layout`, the status projection, and the reporter
restart.

*Why:* five copies of one lifecycle is why `stop_report` and `load_state` are
set differently per call site. `cmd_load_new` (Standalone, single item, caller
supplies the URL) keeps its own path — it is a different lifecycle, not a fifth
copy of this one.

### D5: Consume moves to the owner, keyed off `TrackCompleted`

The consume decision (`should_consume_slot` policy + slot removal) is applied
where the canonical queue lives. `src/app/player_event.rs` keeps the UI reaction
(toasts, `on_audio_consumed` / `on_video_consumed`, feed lifecycle persistence)
but stops mutating the queue when the owner is out of process.

*Why:* today `TrackCompleted` has no handler in `crates/mbv-core/`, so a
daemon-owned queue never consumes, and the Client's local removal is overwritten
by the next `UnifiedQueueUpdated` broadcast. Same completion, two different
outcomes depending on who is attached.

*Consequence:* `pending_queue_removal`'s deferral (hold the removal until
`TrackChanged` so the completed index still resolves) becomes unnecessary —
under D2 the completed slot is named by identity, so removal can be immediate.

### D6: One near-end rule via the existing helper

The two inlined `last_valid_pos * 20 / runtime >= 19` copies
(`player_run_events.rs:311`, `:586`) call `is_near_end` instead, with runtime
taken from the completed occurrence.

*Why:* the three copies currently disagree — one gates on `!completed_is_audio`,
one on `has_session() && !is_audio`, and the two inlined ones read `runtime`
from live status, which at a track boundary already describes the *next*
occurrence. The helper is the intended rule; the copies are drift.

The dead `natural_end` in the `reason == Quit` branch (`:307`) is removed in the
same pass — it is `reason == Eof` inside a branch gated on `reason == Quit`.

## Risks / Trade-offs

- **A pre-unified ctrl peer loses queue mutation (D1).** → No such peer exists in
  this repository, and the affected variants have no in-repo sender. If one
  appears, it reaches queue mutation through `UnifiedQueue*`, which is the path
  every current Client already uses.
- **Rejection is louder than clamping: races that silently "worked" now surface
  as rejected commands.** → That is the intent, but a chatty rejection toast
  would be a regression in feel. Rejections caused by the owner's own reports
  (D3, second scenario) are discarded silently; only Client-initiated mutations
  surface. Watch the queue-mutation-under-playback tests for new rejections that
  indicate an over-eager reject rather than a real race.
- **Moving Consume (D5) changes which process removes slots, and consume policy
  reads client config (`consume_videos` / `consume_audio`).** → The policy input
  must reach the owner. It already does for the in-process owner; for a daemon
  owner the policy travels with the submission rather than being re-decided per
  completion. This is the one place where the change is not pure subtraction —
  sequence it last, after identity is in place, so a failure there does not
  block the rest.
- **`PlaybackRun` no longer allocating ids means a bug that duplicates an id
  upstream now corrupts the run's queue too.** → `from_slot_items` derives
  `next_slot_id` from the maximum incoming id, so locally appended slots cannot
  collide with adopted ones.

## Migration Plan

No data migration: slot ids are runtime-only and `QueueState` is untouched.
Deployment is a single build — the daemon and TUI cross-version only through
ctrl, and the removed variants have no sender. Rollback is a revert; nothing
persisted changes shape.

Sequencing is chosen so each step is independently revertible and the risky step
is last: deletions (D1, D6 and the dead condition) → identity threading (D2) →
rejection (D3) → lifecycle collapse (D4) → Consume relocation (D5).
