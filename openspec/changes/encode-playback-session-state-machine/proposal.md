## Why

`PlaybackSession` (`crates/mbv-core/src/player_session_types.rs`) carries ~14
boolean/u8 flags that together encode one implicit state machine (load
pending-count, stop-report progress, next-up armed/fired, intro
shown/dismissed, startup-pause holding). The type permits every combination,
so the legal transitions are asserted only in comments at the reset sites
(e.g. "use `=` not `+=` so a stale `pending_load` never stacks"). Comments
rot; nothing enforces them. Separately, the struct and its module family are
named `PlaybackSession`/`player_session_*`, but `CONTEXT.md` reserves
"Session" for the Emby-tracked server record — the local mpv playback loop
using the same word is a standing naming collision.

Source: [issue #449](https://github.com/slatkin/mbv/issues/449).

## What Changes

- Add `crates/mbv-core/src/player_session_state.rs` with state enums
  replacing each flag group: load-pending count, stop-report progress,
  next-up armed/fired (both single-item and queue variants), intro
  shown/dismissed, startup-pause holding. Each enum exposes transition
  methods (e.g. a `drain` that returns whether the count reached zero)
  instead of public fields, so the reset invariants become the only
  reachable operations.
- Update the reset sites in `player_session_commands.rs`,
  `player_session_events.rs`, and `player_session_queue.rs` to call these
  transition methods instead of assigning raw fields.
- **BREAKING (internal API only)**: rename `PlaybackSession` →
  `PlaybackRun`, `MpvSessionConfig` → `MpvRunConfig`, and
  `player_session_{types,commands,queue,run,events}.rs` →
  `player_run_*.rs`. `SessionReporter` keeps its name (it reports to the
  Emby Session). Callers in `player_proxy.rs` and `player_runtime.rs`
  follow the signature/name changes.
- Add a "Playback run" entry to the `CONTEXT.md` glossary (local mpv
  playback loop, one per mpv invocation, distinct from Session), with
  `_Avoid_: session, playback session`.
- No behavior changes. `cargo test -p mbv-core` must pass unchanged; the
  enum work is a mechanical re-expression of the existing flag semantics.

Lands as two commits: the enum/state-machine extraction first, the
`PlaybackSession` → `PlaybackRun` rename second — so the semantic change
isn't buried inside a file-rename diff.

## Capabilities

No spec-level behavior changes. This is an internal type/naming refactor
with identical observable behavior (verified by the existing test suite
passing unchanged); `skip_specs: true` is set in `.openspec.yaml`.

### New Capabilities
(none)

### Modified Capabilities
(none)

## Impact

- **Primary**: `crates/mbv-core/src/player_session_types.rs`,
  `player_session_commands.rs`, `player_session_events.rs`,
  `player_session_queue.rs`, `player_session_run.rs` (renamed to
  `player_run_*.rs`); new `player_session_state.rs`.
- **Call sites**: `player_proxy.rs:408`, `player_runtime.rs:518`
  (`handle_intro`) take these flags as parameters and follow the signature
  change.
- **Tests**: `player_tests_session.rs`, `player_tests_status.rs` are the
  regression net. `player_tests_session.rs:45` sets `pending_load = 1`
  directly and needs a constructor call instead.
- **Docs**: `CONTEXT.md` glossary gains "Playback run".
- Out of scope (tracked as issue #449 follow-ups, not this change): ID
  newtypes, `origin`/`is_queue_mode` duplication, shared lang-code table,
  dropping the `power` prefix, a rustfmt post-tool-use hook.
