## Context

See proposal.md - Why. The struct and reset sites are in
`crates/mbv-core/src/player_session_{types,commands,events,queue,run}.rs`;
current line numbers are recorded in the proposal's Impact section (subject
to drift — reverify before editing). `QueueSlotId` (`playback_queue.rs:7-8`)
is the one existing newtype in the codebase and sets the style precedent:
a tuple struct wrapping the primitive, no public field access.

## Goals / Non-Goals

**Goals:**
- Make the five flag groups' legal transitions the only reachable operations
  on `PlaybackSession`/`PlaybackRun` — illegal combinations stop compiling
  rather than being asserted in comments.
- Keep `cargo test -p mbv-core` passing unchanged; this is a representation
  change, not a behavior change.
- Land the enum extraction and the `PlaybackSession`→`PlaybackRun` rename as
  separate commits so a reviewer can verify semantics and mechanics
  independently.

**Non-Goals:**
- Touching `ctrl.rs`, `api_types.rs`, or any wire format.
- The four follow-up items listed in the proposal (ID newtypes, `is_queue_mode`
  dedup, shared lang-code table, `power` prefix removal, rustfmt hook) — those
  are separate issues.
- Adding new tests for the enum transitions beyond what's needed to replace
  `player_tests_session.rs:45`'s direct field write — the repo's testing
  policy (writing-tests skill) governs whether new tests are warranted, and
  is decided at implementation time, not here.

## Decisions

**Five small enums, not one combined state enum.** The flag groups are
largely independent — `LoadState` and `IntroState` don't constrain each
other — so one giant enum would reintroduce a combinatorial type (the same
problem as booleans, just with named cases) and would require every call
site to touch a field it doesn't care about. Five focused enums each answer
one question, matching how the reset sites already group them.

**`LoadState::drain` returns whether the count hit zero, rather than exposing
a getter.** The bug class being closed is "forgot to check `pending_load ==
0` and reset the stop-report pair." Returning a `Drained` value from the one
method that decrements forces the caller to look at it, the same way
`Vec::pop() -> Option<T>` forces a null check. A `fn is_zero(&self) -> bool`
getter would leave the same "forgot to call it" failure mode as today,
just moved one level up.

**`NonZeroU8` for `LoadState::Pending`.** `LoadState::Ready` already
represents zero; letting `Pending` hold zero would recreate the invalid
state the enum exists to remove. `NonZeroU8` makes `Pending(0)` a compile
error instead of a runtime possibility.

**Transition methods take `&mut self` and mutate in place, not `self ->
Self`.** `PlaybackSession`/`PlaybackRun` is a long-lived struct mutated
throughout an event loop (`player_session_run.rs`); matching the existing
ownership shape (see coding-practices: GC'd-language immutability doesn't
apply here — this is Rust with exclusive `&mut` access, so in-place mutation
via owned fields is idiomatic, not a hazard).

**`begin_item_lifecycle()` replaces the three duplicated reset sites.**
Today's correctness depends on `player_session_commands.rs:192-194`,
`253-255`, and `339-345` all resetting the same set; a fourth call site
added later could easily reset only `pending_load` and miss the
`stopped_event_sent`/`stopped_near_end` pair. Collapsing to one method
makes that a single edit point instead of three synchronized ones.
Alternative considered: leave the three sites as-is now that the fields are
enums (a `LoadState::begin_replace()` call plus a manual
`StopReport::NotSent` assignment at each site). Rejected — the enum alone
prevents *illegal* states, not the *omission* of a reset; the grouping
still needs enforcing at the call-site level.

**Rename lands as a separate commit after the enum work.** The rename is
purely mechanical (symbol and file renames) and touches every file the enum
work also touches; interleaving them would make the enum commit's diff
unreadable (impossible to tell "renamed" from "changed semantics" at a
glance). Doing enums first also means the rename commit is a pure
`s/PlaybackSession/PlaybackRun/` sweep, checkable independently of the
type-level reasoning above.

**`player_session_state.rs` is created under its pre-rename name, then
renamed to `player_run_state.rs` in the rename commit** — consistent with
every other `player_session_*.rs` file, rather than special-casing the new
file to skip the intermediate name.

## Risks / Trade-offs

- **Breakage surface is large by design.** The issue's own verification
  step expects `cargo check -p mbv-core` to fail at every raw flag
  assignment first, and that failure list *is* the audit of previously
  unenforced invariants. → Budget for a wide first pass; don't treat
  compiler errors past the first one as a sign of going off track.
- **`player_tests_session.rs:45` writes `pending_load = 1` directly.** →
  Replace with the equivalent `LoadState::begin_single()` constructor call;
  confirm no other test file does the same (grep for other direct field
  writes to the flags listed in the proposal before considering the enum
  work done).
- **No test covers the real Emby round-trip for `stopped_near_end` →
  played/consume** (`player_session_events.rs`, near the old line 494). →
  Manual verification step from the issue stays required: play a queue,
  skip mid-item, stop near the end, confirm Emby marks the item watched.
  This is unchanged by the refactor and not a new risk it introduces, but
  it's also not caught by `cargo test`.
- **File-line cap.** Renaming five files near the 800-line cap
  (`AGENTS.md`) could push one over if the enum extraction doesn't shrink
  them as much as expected. → Run `make check-code-file-lines` after the
  enum commit, before the rename commit; split further in the same PR if
  needed.
- **`handle_intro` and `quit_timeout_stop_flags` signature changes ripple
  to callers outside the primary file list.** → The proposal's Impact
  section names both call sites; grep for other callers of `handle_intro`
  and `quit_timeout_stop_flags` before considering the signature change
  complete, since the issue's file list is scoped to what was visible at
  analysis time, not guaranteed exhaustive.

## Migration Plan

1. Add `player_session_state.rs` with the five enums and their transition
   methods, no callers yet.
2. Migrate `player_session_types.rs`'s struct fields to the new enum types.
3. Fix every resulting compile error at call sites
   (`player_session_{commands,events,queue,run}.rs`, `player_proxy.rs`,
   `player_runtime.rs`) by replacing raw flag reads/writes with the
   corresponding enum transition or match.
4. Introduce `begin_item_lifecycle()` and replace the three duplicated
   reset blocks in `player_session_commands.rs` with calls to it.
5. Fix `player_tests_session.rs:45` and re-run `cargo test -p mbv-core`
   until green, unchanged from before the refactor.
6. Run `cargo clippy --workspace --all-targets` and
   `make check-code-file-lines`; split any file that crossed 800 lines.
7. Commit the enum work.
8. In a separate commit: rename `PlaybackSession`→`PlaybackRun`,
   `MpvSessionConfig`→`MpvRunConfig`, the five `player_session_*.rs`
   files (including the new state file) to `player_run_*.rs`, and add the
   **Playback run** glossary entry to `CONTEXT.md`. Re-run `cargo check`,
   `cargo test`, and `cargo clippy` to confirm the rename was purely
   mechanical.
9. Manual verification: play a queue, skip mid-item, stop near the end;
   confirm Emby marks the item watched.

Rollback is `git revert` of either commit independently — the rename commit
depends on the enum commit's file layout, so reverting only the enum
commit is not expected to be clean, but reverting only the rename commit
(restoring old names over the new enum-based code) is.
