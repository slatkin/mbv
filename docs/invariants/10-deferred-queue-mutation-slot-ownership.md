# Invariant 10 — A deferred queue mutation executes only through its owning boundary, from a slot exactly one writer and one reader own

**Scope:** `App::pending_queue_action` (`src/app/state/app_struct.rs:345`) and its
save/discard writer and `SessionEvent::PlaylistMutationComplete` reader
(`src/app/dispatch/run_loop/session.rs:191-194`), versus the D6 gate's
`App::pending_queue_replacement`
(`src/app/state/app_struct.rs:354`, typed
`Option<(PendingQueueAction, ReplacementExecutor)>`), written only by
`App::request_queue_replacement` (`src/app/dispatch/queue.rs:391`) and
read/taken only by the `ConfirmAction::ReplacePopulatedQueue` arm
(`src/app/input/confirm_keys.rs:178` take-on-confirm, `:183`
clear-on-cancel-or-dismiss, both handing the `(action, executor)` payload to
`run_replacement`, which dispatches to the executor the entry point chose).

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

No type fully enforces this. The save-deferral slot holds
`Option<PendingQueueAction>`; the gate slot now holds
`Option<(PendingQueueAction, ReplacementExecutor)>`, whose payload also
carries which executor runs on confirm — so a cross-boundary read is no
longer type-identical, and code that swaps one payload for the other no
longer compiles by accident. That distinction is an additional barrier, not
a replacement for the ownership discipline: the invariant still rests on the
same one-writer, one-reader rule it always has, held by comments and
convention rather than the compiler. The defect it guards against is real:
commit 519583ea fixed exactly it, where a populated-queue replacement the
user had *never confirmed* sat in the shared `pending_queue_action` slot and
was fired by the `PlaylistMutationComplete` boundary when an unrelated
playlist save completed — playback the user never asked for, started by a
boundary that had no business reading the gate's payload.

The failure has a second face. A deferred payload that is *dropped* — its
slot cleared by a reader that cannot execute it, or overwritten by a
different flow's payload — is silently lost: the user pressed Play, the
save finished, and nothing happens. Silence is worse than the wrong
playback, because there is no signal at all.

## How the code maintains it today

- **Two slots, two lifecycles.** `pending_queue_replacement` is written
  only in `request_queue_replacement` (`dispatch/queue.rs:391`) when the
  gate decides confirmation is needed, as an `(action, executor)` tuple
  whose executor is the entry point's `ReplacementExecutor`. The
  `ReplacePopulatedQueue` arm in `input/confirm_keys.rs` takes it on
  confirm (`y`/`Y`/`Enter`) and hands it to `run_replacement`, which
  dispatches `Routed(prep)` through `run_routed_replacement` (that entry
  point's pre-play prep, then `play_items_routed`) and `Pending` through
  `execute_queue_replacement`; every other key (cancel or dismiss) clears
  the slot, so no executable payload survives a closed modal.
  `pending_queue_action` is written by the save/discard flow and consumed
  only at the `PlaylistMutationComplete` boundary
  (`run_loop_events_session.rs:191-194`), which executes it only after
  lineage and playlist-identity checks.
- **Comment-anchored intent.** Both sites state the exclusivity in place:
  `state/types/confirm.rs:32` documents that the gate uses its own
  `pending_queue_replacement` slot so the save-deferral slot stays with its
  own callers, and `dispatch/queue.rs` documents why the shared deferral
  slot must not carry a gated replacement. These comments are the only
  barrier against a future "simplification" back into one slot.
- **Tests.** `input/confirm_keys/tests.rs` pins both slots' independence
  (confirm/cancel paths assert `pending_queue_replacement` alone moves) and
  (`src/app/tests/tick_integration/music_mouse.rs:947-998` drives the gate
  through real `tick()` composition, asserting the slot is taken exactly
  once.

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

**Known strengthening (partially done):** the gate slot's payload is now
distinguishable at the type level — its
`Option<(PendingQueueAction, ReplacementExecutor)>` cannot be confused with
the save-deferral slot's `Option<PendingQueueAction>`, and it names the
executor that runs. The general strengthening remains open: typing each
save/discard deferral payload apart (or a single enum whose variants name
their executing boundary) so a cross-boundary read fails to compile instead
of relying on comments and convention.
