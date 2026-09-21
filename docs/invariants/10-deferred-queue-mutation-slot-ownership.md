# Invariant 10 — A deferred queue mutation executes only through its owning boundary, from a slot exactly one writer and one reader own

**Scope:** `App::pending_queue_action` (`src/app/app_struct.rs:331`) and its
save/discard writer and `SessionEvent::PlaylistMutationComplete` reader
(`src/app/run_loop_events_session.rs:183`), versus the D6 gate's
`App::pending_queue_replacement` (`src/app/app_struct.rs:332`), written only
by `App::request_queue_replacement` (`src/app/queue_actions.rs:288`) and
read/taken only by the `ConfirmAction::ReplacePopulatedQueue` arm
(`src/app/input_confirm_keys.rs:178` take-on-confirm, `:183`
clear-on-cancel-or-dismiss, both handing the payload to
`execute_queue_replacement`).

## The invariant

1. A queue mutation whose execution is deferred past the moment the user
   asked for it (a save/discard deferral, a gated populated-queue
   replacement) is held as an executable payload in a dedicated `App` slot.
2. Each slot has exactly one writer — the flow that created the deferral —
   and exactly one reader — the boundary that legitimately executes it.
   `pending_queue_action` is written by the save/discard flow and consumed
   only by the `SessionEvent::PlaylistMutationComplete` boundary;
   `pending_queue_replacement` is written only by
   `request_queue_replacement` and consumed only by the
   `ReplacePopulatedQueue` confirmation arm.
3. No reader may consume a payload from a slot it does not own, and no
   writer may deposit a payload into another kind's slot. The two slots
   must never merge back into one.

## Why it matters

No type enforces this: both slots hold the same `Option<PendingQueueAction>`
type, so nothing in the compiler distinguishes a save-deferral payload from
a gated replacement payload. If the two share a slot, the boundaries can
execute each other's payloads: commit 519583ea fixed exactly this defect,
where a populated-queue replacement the user had *never confirmed* sat in
the shared `pending_queue_action` slot and was fired by the
`PlaylistMutationComplete` boundary when an unrelated playlist save
completed — playback the user never asked for, started by a boundary that
had no business reading the gate's payload.

The failure has a second face. A deferred payload that is *dropped* — its
slot cleared by a reader that cannot execute it, or overwritten by a
different flow's payload — is silently lost: the user pressed Play, the
save finished, and nothing happens. Silence is worse than the wrong
playback, because there is no signal at all.

## How the code maintains it today

- **Two slots, two lifecycles.** `pending_queue_replacement` is written
  only in `request_queue_replacement` (`queue_actions.rs:288`) when the
  gate decides confirmation is needed; the `ReplacePopulatedQueue` arm in
  `input_confirm_keys.rs` takes it on confirm (`y`/`Y`/`Enter`) and hands
  it straight to `execute_queue_replacement`, and clears it on every other
  key (cancel or dismiss), so no executable payload survives a closed
  modal. `pending_queue_action` is written by the save/discard flow and
  consumed only at the `PlaylistMutationComplete` boundary
  (`run_loop_events_session.rs:183-187`), which executes it only after
  lineage and playlist-identity checks.
- **Comment-anchored intent.** Both sites state the exclusivity in place:
  `types_confirm.rs:28` documents that the gate uses its own
  `pending_queue_replacement` slot so the save-deferral slot stays with its
  own callers, and `queue_actions.rs` documents why the shared deferral
  slot must not carry a gated replacement. These comments are the only
  barrier against a future "simplification" back into one slot.
- **Tests.** `input_confirm_keys_tests.rs` pins both slots' independence
  (confirm/cancel paths assert `pending_queue_replacement` alone moves) and
  `tests_tick_integration_music_mouse.rs:879-986` drives the gate through
  real `tick()` composition, asserting the slot is taken exactly once.

## Where it still fails / what to watch

- **Any new writer to either slot** reintroduces the defect shape: a
  payload deposited by a flow its reader does not know about is executed at
  the wrong time (if the reader coincidentally fires) or silently dropped
  (if the reader's guards reject it).
- **Any boundary that starts reading `pending_queue_replacement`** — e.g. a
  session or worker event handler "helpfully" draining it — can fire a
  replacement the user never confirmed, which is precisely the bug 519583ea
  fixed. The confirmation arm is the only legitimate reader, by design.
- **Adding a second deferred-mutation kind** requires its own slot plus an
  ownership proof (who writes, which boundary reads, what happens when the
  user dismisses), or a redesign that types the payload so the compiler
  separates the consumers. Do not reuse an existing slot "because the type
  fits" — `Option<PendingQueueAction>` fitting is exactly what caused the
  original defect.

**Known strengthening (not done here):** type the slots apart (distinct
payload types per deferral kind, or a single enum whose variants name their
executing boundary) so that a cross-boundary read fails to compile instead
of relying on comments and convention.
