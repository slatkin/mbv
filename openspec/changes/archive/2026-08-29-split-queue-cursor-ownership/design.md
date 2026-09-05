## Context

See `proposal.md` — Why. `PlayerTab::queue_cursor` (`app_struct.rs` via
`types_player_tab.rs`) is the canonical queue's position field, one per
`QueueScope`. `App::queue_scroll` (`app_struct.rs:184`) is a single
shell-owned scroll offset. `sync_queue` (`shell_queue.rs:32`) pushes both
into `QueueComponent::set_content` every frame; `select_queue_slot`
(`shell_queue.rs:137`) writes the component's resolved index back into
`queue_cursor`. That write-back is the mirror #617 asks to remove, but
`queue_cursor` also has writers this change must not disturb:
playback advance (`player_event.rs:272,319`), remote-session reconciliation
(`run_loop_events.rs:130`, `run_loop_events_session.rs:133`), queue-edit
follow (`queue_actions.rs:95`, `types_player_tab.rs:141,162`), and
restore/select-on-enqueue (`library_position_state.rs:117-119`,
`actions.rs:376`). `App::pending_queue_edit_cursor` /
`pending_remote_move_cursor` (`app_struct.rs:164-169`) exist to hand a
target cursor across an async round trip to the daemon and back
(`player_event.rs:337-352`).

`QueueComponent` (`components/queue.rs`) already carries its own `cursor`
and `scroll` fields. `set_content` currently *reconciles* rather than
overwrites: it re-finds the previously-tracked slot by identity in the new
slot list, and only falls back to the pushed `cursor` when that slot is
gone; `scroll` is clamped as `self.scroll.max(pushed_scroll).min(cursor)`.
This reconciliation is what has been silently absorbing most of the
divergence between "what the user selected" and "what the shell pushed" —
which is exactly why the round trip through `select_queue_slot` has been
tolerable so far, and exactly the two-way pin D17 and #611 want gone.

D17 (`migrate-tui-to-tuirealm/design.md`) sets the teardown shape this
change follows: separate mount/content projection from interaction-state
pins, replace projection with targeted pushes at validated writer
choke points, and do not repair or expand legacy raw-key/mouse paths
while doing it (D16: mouse is accepted-broken for the alpha).

## Goals / Non-Goals

**Goals:**
- Answer whether `queue_cursor` is one thing or two, and give each resulting
  role exactly one owner.
- Remove the `App`→`QueueComponent` cursor/scroll back-projection
  (`select_queue_slot`'s write, and `set_content`'s scroll-merge clamp).
- Preserve every non-component writer's current effect on canonical queue
  state, remote reconciliation, and playback.

**Non-Goals:**
- Deleting or "fixing" `input_queue_keys.rs`'s raw-key cursor navigation,
  `mouse_gestures.rs`, `context_menu_actions.rs`, or the legacy
  `render/screens/queue.rs` renderer. These still read/write `queue_cursor`
  directly, but they are dead-in-practice once the router resolves Queue
  focus to `QueueComponent` first; deleting them is `remove-legacy-
  keyboard-endpoint`'s job, not this change's. This change adds no new
  callers of them.
- Repairing mouse routing (D16, accepted-broken for the alpha).
- Renaming `PlayerTab::queue_cursor` or changing its wire/persistence shape.
  It stays the field many legitimate shell readers already use
  (`playback_target_local.rs`, session reporting, persistence); only its
  *role* narrows.

## Decisions

### D1 — `queue_cursor` is two things; the split holds

Investigation confirms the issue's candidate framing, with one addition the
issue didn't name. `queue_cursor` today serves three roles, not two:

1. **Shell-owned follow position.** Where playback advance, remote-session
   reconciliation, queue-edit follow, and restore/select-on-enqueue say the
   canonical queue should be pointing. All six non-component writers named
   in the issue are this role, and so is `App`'s own read of its previous
   `queue_cursor` inside `UnifiedQueueUpdated` reconciliation
   (`player_event.rs:347`) — an owner reading its own field, not a mirror.
2. **Component-owned user cursor.** Where the user has arrowed to.
   `QueueComponent` already tracks this locally (`cursor`/`scroll` fields);
   it never needs `App` to tell it the index it just resolved from its own
   key handling.
3. **Implicit argument channel for shell-owned edit effects** (not named in
   the issue, found by reading `select_queue_slot`'s callers). When the
   component emits `QueueRequest::Play` / `Remove` / `Move`,
   `select_queue_slot` resolves `slot_id` to an index and *writes it into
   `queue_cursor`* (`shell_queue.rs:217`), and the arm then reads that same
   value straight back out as its operand (`shell_queue.rs:91-111`):
   `Remove` does `let cursor = queue_for_scope(scope).queue_cursor;
   remove_from_queue(cursor)`, `Move` calls `move_queue_item_up/down()`
   which read `queue.queue_cursor` as `from` (`queue_actions.rs:124`), and
   `Play` dispatches `Command::QueuePlayCursor`, which reads the cursor
   inside `action.rs:377`. The write is load-bearing today — not only
   presentation — because it is the only way the resolved index reaches
   those effect functions.

Role 1 legitimately keeps the name `queue_cursor` on `PlayerTab` and stays
shell-owned. Role 2 belongs entirely to `QueueComponent` and must never be
read back by `App`. Role 3 is not a real third owner — it is role 1's field
being reused as a parameter-passing shortcut, and it is exactly what breaks
if role 2's write-back is simply deleted without replacement: `remove_from_
queue` and `move_queue_item_by` would silently act on a stale follow
position instead of the slot the user actually selected.

**Alternative considered:** treat `queue_cursor` as a single shell-owned
value and have the component always defer to it (no local user cursor).
Rejected — this is the status quo's actual bug. The component's local
`cursor` field already exists because per-frame overwrite by a stale
`App`-side value was the accepted-broken interim state the mirror was
built to paper over; keeping it means every reorder/remove leaves a
one-frame lag or snap while `App` catches up, which the current
reconciliation logic in `set_content` already works around unnecessarily.

### D2 — `select_queue_slot` stops writing `queue_cursor`; effects take an explicit index

`select_queue_slot` still resolves `slot_id` → index (needed to validate
the request and to drive scope/focus/hold-window side effects), but no
longer assigns it to `queue.queue_cursor`. The resolved index is instead
passed explicitly to the effect it is really an argument for:

- `QueueRequest::Remove` → `remove_from_queue(index)` (already takes an
  explicit `pos`; no signature change).
- `QueueRequest::Move` → `move_queue_item_by` gains an explicit `from:
  usize` parameter instead of reading `queue.queue_cursor` internally.
- `QueueRequest::Play` → the `QueuePlayCursor` command's target index is
  passed the same way rather than dispatched through a synthetic key event
  that depends on `queue_cursor` already being set.
- `QueueRequest::Cursor` (plain navigation, no effect) → after this change
  it does not touch `App` state at all beyond scope/focus/hold-window
  (`set_queue_scope`, `set_panel_focus`, `mark_queue_cursor_user_active`).
  The component already knows its own cursor; there is nothing left to
  write.

The existing effect entry points are kept (D17: "keep shell-owned effects
at existing boundaries"). Only the operand each one reads changes, from an
ambient `queue_cursor` read to an explicit parameter:
`remove_from_queue(pos)` already takes one, so its arm simply stops
round-tripping through the field; `move_queue_item_up/down` gain an
explicit `from`; and `Command::QueuePlayCursor` needs the resolved index
supplied rather than read from `App` — the one entry point of the three
whose signature change reaches beyond `shell_queue.rs`, since
`context_menu_actions.rs:37` and `mouse_gestures.rs:176` also dispatch it.

### D3 — `queue_scroll` moves wholly into `QueueComponent`

`App::queue_scroll` is deleted. `QueueComponent` already owns a `scroll`
field; `set_content` stops accepting a pushed scroll value and stops
clamping against it (`self.scroll.max(scroll).min(self.cursor)` goes away —
the component clamps its own scroll against its own cursor and slot count
only). `set_queue_scope`'s `self.queue_scroll = 0` reset
(`queue_scope.rs:295`) is replaced by the component detecting its own
`scope` input changed inside `set_content` and resetting `self.scroll`
there — scope is already part of the pushed content, so no new shell→
component channel is needed.

### D4 — The follow-position push stays, reshaped from mirror to targeted push

`sync_queue` still pushes the follow position into `QueueComponent`, but
`set_content`'s job changes from "reconcile two candidate cursors" to
"accept a shell-requested position when the component doesn't already have
a better one." Concretely: the component keeps tracking its cursor by slot
identity across content refreshes (unchanged), and adopts the pushed follow
position only when that identity-tracked slot no longer exists (queue
replaced, remote reconciliation landed on a different item, restore) —
which is what today's `unwrap_or_else` fallback already expresses. The
difference after this change is that the *only* thing writing the value
`set_content` receives is role-1 writers (playback/remote/edit-follow/
restore); `select_queue_slot` no longer contributes to it, so the push is a
one-way projection of shell-decided state, not a closed loop.

### Terminology

No new domain term is introduced. "Follow position" and "user cursor" are
design-internal names for the two roles `queue_cursor` already plays;
`CONTEXT.md`'s existing **Queue slot** entry is unaffected since neither
role changes what a slot is.

## Risks / Trade-offs

- [Risk] `move_queue_item_by`'s new explicit `from` parameter could drift
  from `queue.queue_cursor` at other call sites that aren't going through
  `select_queue_slot` → Mitigation: `move_queue_item_up/down` have no other
  production callers on this branch, so convert them outright rather than
  keeping a `queue_cursor`-reading overload. `Command::QueuePlayCursor` is
  the real fan-out (`context_menu_actions.rs:37`, `mouse_gestures.rs:176`);
  resolve each of those to an explicit index at its own call site.
- [Risk] Removing the scroll merge/clamp in `set_content` changes clamp
  timing relative to today's per-frame `.max(scroll).min(cursor)` → could
  under- or over-scroll on the first frame after a large queue edit →
  Mitigation: component clamps `scroll` immediately whenever `cursor` or
  slot count changes inside `set_content`, not only at render time;
  `tests_queue_reorder.rs`/`components/queue_component_tests.rs` pin the
  visible behaviour.
- [Risk] `pending_queue_edit_cursor`/`pending_remote_move_cursor` round-trip
  timing is unchanged by this design, but is easy to regress accidentally
  while touching `queue_actions.rs` → Mitigation: no changes to their
  set/take sites beyond D2's parameter plumbing; `tests_remote_
  reconciliation*.rs` covers the round trip.

## Migration Plan

Single change, no data migration (`queue_cursor` keeps its name, type, and
persisted shape). Land in dependency order: D3 (scroll) is independent and
can land first or in parallel; D2 (explicit-index effects) must land before
D4 can remove the write in `select_queue_slot`, since D4 depends on nothing
still relying on that write. Rollback is a plain revert — no persisted
state format changes.

## Open Questions

None — the ownership question the issue required answering before
implementation is resolved in D1.
