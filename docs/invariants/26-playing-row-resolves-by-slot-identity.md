# Invariant 26 — The client resolves the playing row by slot identity, never by a status ordinal

**Scope:** `App::effective_playback_state` and `App::displayed_playback_state`
(`src/app/state/playback_target.rs`), the queue projection's
`projected_active_target` (`src/app/shell/queue.rs`), and every presentation
consumer of `PlaybackState::active_idx` (queue rows, cards, now-playing
derivation).

## The invariant

The now-playing highlight names the slot the owner's snapshot marks active.
The client resolves it in one direction only: slot identity → ordinal, against
the queue view the rows come from. The status' `current_idx` is an ordinal
snapshot taken at the owner at some earlier moment; it is a fallback for when
the queue view names no active slot, never the primary source.

## Why it matters

Queue edits (remove, undo, move, refresh prune) shift ordinals between status
refreshes. After deleting the row before the playing one, the stale ordinal
points at a different item, and an ordinal-resolved highlight paints that
wrong row as playing — observed 2026-10-29 playing to an Emby client: delete
row 1, the new row 2 highlighted; undo, the deleted row highlighted; the next
jump then landed against a queue whose visible state no longer matched the
user's mental model. Identity survives every one of those edits.

## How the code maintains it today

- `effective_playback_state` resolves `active_idx` through
  `resolved_active_index`: the playing queue view's `active_slot_id()` mapped
  by `slot_index`, falling back to the status ordinal only when the view has
  no active slot (no snapshot yet). The Emby-Session foreign-content arm
  stays slotless (design D12).
- The owner broadcast carries `active_slot` as identity, re-derived from the
  observed active slot on every snapshot (`unified_queue_state_for_peer`,
  invariant 25).
- Guard test: `deleting_before_playing_row_keeps_highlight_on_playing_slot`
  (`src/app/shell/queue.rs`) adopts a post-delete snapshot while the status
  still names the pre-delete ordinal, and asserts the highlight stays on the
  playing slot.

## Where it currently fails / how it could regress

- A new presentation consumer that reads `PlayerStatus::current_idx` directly
  instead of going through `effective_playback_state` reintroduces the stale
  ordinal. `play_queue_cursor_locally`'s `t == current_idx` restart-vs-jump
  comparison is a known residual: the cursor index and the owner's ordinal
  can disagree mid-edit until the next status refresh.
- An owner that changes the active slot without broadcasting a queue snapshot
  leaves the client's identity stale until the next broadcast; the ordinal
  fallback then wins for one refresh at most.
