# Implementation Plan: Intermittent Slow Exit Leaves Foreground-Instance Lock Held

Issue: #202. Related (weak coupling, proceeding independently): #192.
Spec: `docs/superpowers/specs/2026-07-16-slow-exit-lock-held.md`

This plan was revised after an independent grilling review found five
load-bearing problems in the original version. See "Revision History" at
the bottom.

## Overview

`App::run()`'s two teardown branches diverge in a way that lets one of them
hold the single-instance flock indefinitely: the signal-triggered path
(SIGHUP/SIGTERM) already bounds its player-thread join to 5s, but the
normal in-app quit path joins unbounded. Fix this by (1) unifying both
branches into one teardown sequence, bounded by a new configurable
`quit_timeout_secs` (default 5), and (2) making the *actual* quit-triggered
`report_stopped` call (`on_shutdown`, not the rare `quit_at`-elapsed
watchdog) shutdown-aware — a tighter internal budget with its retry
skipped — rather than letting an external timeout blindly guillotine a
call that doesn't know it's being raced. A second, pre-existing unbounded
join sits directly upstream of that call (`ProgressGuard::stop_and_join()`)
and must be bounded too, or the outer timeout composition doesn't actually
work. Add logging around all join/report sites so a future recurrence is
diagnosable without a live repro, and log the lock PID in the
`Resolution::Refuse` message for the same reason.

Mid-playback `report_stopped` call sites (track transitions, queue
replacement) and the mpv-died-independently case (`quit_at.is_none()`
inside `on_shutdown`) are explicitly out of scope and must keep their
current full-budget, full-retry behavior — only the real quit sequence
changes.

## Architecture Decisions

- One config value, `quit_timeout_secs: u64` (default `5`), added to
  `Config` in `crates/mbv-core/src/config.rs` following the existing
  `progress_interval_secs` pattern (manual TOML parse from `[general]`,
  fallback default).
- **The real quit-triggered `report_stopped` call site is `on_shutdown`**
  (`crates/mbv-core/src/player.rs`, currently around line 2170), reached
  via `Event::Shutdown` after `Player::stop()` → `mpv.command("quit")`.
  This is the common, healthy-mpv-exit path — the overwhelming majority of
  real quits. The `quit_at`-elapsed watchdog (currently around line
  2244-2251) is a secondary/fallback path that only fires when mpv fails
  to emit `Event::Shutdown` within 2s; it should get the same treatment,
  but it is not the primary target.
- **`on_shutdown` is not cleanly "the quit path."** It also fires when mpv
  exits on its own (crashes, or the user closes the mpv window directly),
  distinguished by `self.quit_at.is_none()` at the point `report_stopped`
  is called. In that case the app's event loop does *not* break —
  `PlayerEvent::MpvQuit`'s handler (`App::handle_player_event`,
  `src/app/mod.rs:3532-3537`) just clears some UI state and returns
  `false`; despite the misleading inline comment ("tell the app to quit"),
  it does not quit. `on_shutdown` must branch on `self.quit_at.is_some()`:
  the real-quit case uses the shutdown-aware report call, the
  independent-death case keeps the ordinary full-budget/full-retry
  `report_stopped` — there is no time pressure in that case.
- **`ProgressGuard::stop_and_join()` (`player.rs:567-572`) is a second,
  pre-existing unbounded join in the same path**, called immediately
  before `report_stopped` inside `on_shutdown`. It does a plain `h.join()`
  on the progress-reporter thread, which only checks its stop signal
  between iterations of `report_progress`/`report_ping` HTTP calls — so a
  slow progress call can block `stop_and_join()` for up to the shared
  agent's own budget (~30s), independent of anything this plan does to
  `report_stopped`. This must be bounded the same way (reuse the
  `join_or_timeout`-style watcher-thread/`recv_timeout` mechanism) or no
  amount of margin on the outer join bound is sufficient.
- Add a shutdown-aware report function (e.g. `report_stopped_for_shutdown`)
  that takes an explicit `Duration` budget, uses it as both connect and
  total timeout for its `ureq` request, and skips the existing
  500ms-sleep-then-retry entirely. New function, not a parameter/branch on
  the existing `report_stopped` — mid-playback callers must be
  structurally untouched, not just "expected to be unaffected."
- Standardize the unified teardown on `last_valid_pos` (not
  `position_ticks`) for the final position save — the existing
  justification for `last_valid_pos` applies equally to both former
  branches.
- **Timeout composition is not "pass the same `Duration` to everything."**
  The outer `join_or_timeout` bound must have headroom over the sum of
  what happens inside it: `progress.stop_and_join()`'s own (now-bounded)
  budget + `report_stopped_for_shutdown`'s budget + a small fixed cushion
  for remaining bookkeeping (mark_played retry is already fire-and-forget
  in a detached thread; the `PlayerEvent::Stopped` send is a cheap channel
  op). Concretely: give `report_stopped_for_shutdown` and the bounded
  `stop_and_join` each a budget derived from `quit_timeout_secs` (e.g. each
  gets a fraction of it, or each gets `quit_timeout_secs` and the outer
  `join_or_timeout` bound is `quit_timeout_secs * 2 + 1`-ish — exact split
  is an implementation decision, see Task 3b/5 below), not three identical
  values racing each other.
- **`App::run()`'s teardown sequence must be extracted into its own
  method** (e.g. `fn teardown(&mut self, ...)`), callable independently of
  `run()`'s terminal/event-loop setup. `run()` calls `enable_raw_mode()`
  unconditionally and has never been unit-tested (confirmed: zero test
  call sites for `.run()` in the existing suite) — the bounded-teardown
  behavior this plan needs to prove is only testable if the sequence is a
  standalone method callable against a stubbed `App`.
- Thread `quit_timeout_secs` from `Config` to where it's needed. `Player`
  does not store a `Config` reference — it takes ~10 individual scalar
  constructor params (`show_audio_window`, `use_mpv_config`, etc.), and
  `SessionReporter::new` (the natural landing spot, since the shutdown
  report call is a `SessionReporter` method) is called from inside
  `player.rs` itself using `Player`'s stored fields. This means
  `Player::new`'s signature changes, which means updating all 5 existing
  call sites (`daemon.rs::run_with_options`, `App::new` in `src/app/mod.rs`,
  and 3 call sites inside `player.rs` itself, including 2 test helpers).
- Log the lock's PID (`single_instance::read_pid`) in the
  `Resolution::Refuse` message in `src/main.rs`.
- Do not change `single_instance.rs`'s detection semantics (ADR 0006) and
  do not touch `EmbyClient`'s shared general `ureq::Agent` config (#192's
  concern) — this fix is fully decoupled from that.

## Dependency Graph

```text
Pre-edit GitNexus impact checks
    |
    v
Add quit_timeout_secs to Config (crates/mbv-core/src/config.rs)
    |
    v
Confirm on_shutdown as primary call site + quit_at branch requirement
  (documentation/comment fix for the stale "tell the app to quit" comment)
    |
    v
Bound ProgressGuard::stop_and_join() (new, was missing from original plan)
    |
    v
Add report_stopped_for_shutdown (tighter budget, no retry) in api.rs/player.rs
    |
    v
Thread quit_timeout_secs through Player::new -> SessionReporter
  (touches daemon.rs, app/mod.rs, player.rs incl. test call sites)
    |
    v
Branch on_shutdown on self.quit_at.is_some() to call the shutdown-aware
  path only for the real quit case
    |
    v
Extract App::run()'s teardown into a standalone, independently-testable
  method; unify the two former branches onto it (single bounded join,
  last_valid_pos everywhere, timeout composition with real headroom)
    |
    v
Add diagnostic logging around all join/report sites in the sequence
    |
    v
Log lock PID in Resolution::Refuse (src/main.rs)
    |
    v
Tests: bounded stop_and_join, shutdown-aware report (stalled-listener +
  retry-skip), unified teardown method (both trigger reasons, via stub,
  no tty needed), mid-playback and mpv-died-independently paths unaffected
    |
    v
Full verification: build, test, clippy, fmt, detect_changes
```

## Blast Radius

- `Player::join` / `Player::join_or_timeout` (`crates/mbv-core/src/player.rs`):
  run `impact()` before editing.
- `ProgressGuard::stop_and_join` (`crates/mbv-core/src/player.rs`): new
  edit target identified in this revision — run `impact()` before editing;
  expect LOW (private, single call site inside `on_shutdown`).
- The extracted `App` teardown method (`src/app/mod.rs`): the two former
  branches being unified are the entire scope; confirm via `impact()` that
  no other code depends on `QUIT_REQUESTED` gating this block beyond
  what's already read in this file.
- `Player::new` (`crates/mbv-core/src/player.rs`): MEDIUM — signature
  change with 5 known call sites across 3 files (`daemon.rs`, `app/mod.rs`,
  `player.rs` incl. 2 test helpers). Run `impact()` to confirm no
  additional call sites exist beyond what was found via
  `find_referencing_symbols` in the grilling review.
- `EmbyClient::report_stopped` / `SessionReporter::report_stopped`
  (`crates/mbv-core/src/api.rs`, `crates/mbv-core/src/player.rs`): HIGH
  fan-out — the new `report_stopped_for_shutdown` function must be
  genuinely additive, and `on_shutdown`'s existing call to ordinary
  `report_stopped` for the `quit_at.is_none()` case must remain unchanged.
- `Config` struct (`crates/mbv-core/src/config.rs`): LOW risk, additive
  field with a default.
- `single_instance::read_pid` (`src/single_instance.rs`): LOW risk,
  already-existing read-only helper, one new call site in `src/main.rs`.

## Task List

### Phase 0: Pre-Edit Checks

- [ ] Task 0: Run mandatory GitNexus impact checks for `Player::join`,
  `Player::join_or_timeout`, `ProgressGuard::stop_and_join`, `Player::new`,
  `EmbyClient::report_stopped`, `SessionReporter::report_stopped`, and
  `App::run`. Record risk levels and update this plan / warn the user
  before implementation if any is HIGH or CRITICAL.

### Checkpoint: Impact

- [ ] Blast radius recorded for all Task 0 symbols; no unexpected
  HIGH/CRITICAL risk, or explicitly surfaced to the user if there is.

### Phase 1: Configurable Quit Timeout

- [ ] Task 1: Add `quit_timeout_secs: u64` to `Config`
  (`crates/mbv-core/src/config.rs`), default `5`, following the
  `progress_interval_secs` parse/default pattern exactly.

### Checkpoint: Config

- [ ] Config test target passes with the new field present and defaulted.
- [ ] A `config.toml` with an explicit `quit_timeout_secs` override is
  correctly parsed.

### Phase 2: Bound the Pre-Existing Progress-Reporter Join

- [ ] Task 2: Change `ProgressGuard::stop_and_join()` from a plain `h.join()`
  to a bounded join (reuse or mirror `Player::join_or_timeout`'s
  watcher-thread + `recv_timeout` mechanism), with its own budget derived
  from `quit_timeout_secs`.

### Checkpoint: Bounded Progress Stop

- [ ] New `mbv-core` test: `ProgressGuard::stop_and_join` returns promptly
  even when the progress-reporter thread is deliberately blocked past its
  budget (e.g. stalled inside a simulated slow `report_progress` call).

### Phase 3: Shutdown-Aware `report_stopped`

- [ ] Task 3: Add `report_stopped_for_shutdown(&self, timeout: Duration, ...)
  -> bool` (exact signature TBD at implementation) that uses `timeout` as
  both connect and total budget for its `ureq` request and skips the
  existing 500ms-sleep-then-retry entirely. New function, not a branch on
  the existing `report_stopped`.
- [ ] Task 4: Thread `quit_timeout_secs` from `Config` to `Player::new`
  (new constructor parameter, stored field) and update all 5 known call
  sites (`daemon.rs::run_with_options`, `App::new` in `src/app/mod.rs`,
  and the 3 call sites inside `player.rs` — 1 production, 2 test helpers).
  Land the value on `SessionReporter` (constructed from `Player`'s stored
  fields at session-spawn time) rather than `MpvSessionConfig`.
- [ ] Task 5: In `on_shutdown`, branch on `self.quit_at.is_some()`: call
  `report_stopped_for_shutdown` for the real-quit case, keep the ordinary
  `report_stopped` call unchanged for the `quit_at.is_none()`
  (mpv-died-independently) case. Fix the stale inline comment above the
  `PlayerEvent::MpvQuit` send ("tell the app to quit" — it doesn't; the
  handler just clears UI state and returns `false`).

### Checkpoint: Shutdown Report

- [ ] New `mbv-core` test: `report_stopped_for_shutdown` against a
  deliberately-stalling local `TcpListener` returns within its configured
  timeout, not the ordinary ~65s worst case.
- [ ] New `mbv-core` test: the same stalled-listener setup proves no retry
  is attempted via the shutdown variant, vs. a parallel test proving
  ordinary `report_stopped` still retries once (regression guard).
- [ ] New/adapted test proving `on_shutdown`'s branch is correct: real-quit
  (`quit_at.is_some()`) uses the shutdown-aware path; mpv-died-independently
  (`quit_at.is_none()`) still uses ordinary `report_stopped`.
- [ ] `cargo test -p mbv-core player`

### Phase 4: Extract and Unify Teardown

- [ ] Task 6: Extract `App::run()`'s teardown tail into a standalone method
  (e.g. `fn teardown(&mut self, ...)`) that does not depend on `run()`'s
  terminal/event-loop setup, so it's callable directly against a stubbed
  `App` in tests without a real tty.
- [ ] Task 7: Within that extracted method, replace the two separate
  branches (`QUIT_REQUESTED`-gated signal path, normal fallthrough path)
  with one shared sequence: read `last_valid_pos`, save queue state, stop +
  bounded join (with real headroom over the now-bounded
  `stop_and_join`/`report_stopped_for_shutdown` budgets inside it — not an
  identical `Duration` racing the same clock), restore terminal. Only the
  log line explaining *why* we're quitting should still differ.
- [ ] Task 8: Add timestamped logging (start, duration, outcome) around
  every join/report call in the sequence, so a future recurrence is
  diagnosable from `~/.local/state/mbv/mbv.log` without a live repro.

### Checkpoint: Unified Teardown

- [ ] New test calling the extracted teardown method directly (via
  `make_app_stub` or equivalent), with the player thread simulated as
  slow/hung: both trigger reasons (former signal path, former normal path)
  return within a bounded window.
- [ ] Existing signal-path (terminal-close/SIGHUP) behavior is unchanged
  apart from the `last_valid_pos` standardization.
- [ ] `run()` itself remains untested end-to-end (unchanged status quo, not
  a regression) — verify it still compiles and its tail is now a thin call
  into the extracted method.

### Phase 5: Diagnosability — Lock PID on Refusal

- [ ] Task 9: In `src/main.rs`'s `Resolution::Refuse` arm, call
  `single_instance::read_pid(&lock_path)` and include the PID (if present)
  in the printed refusal message.

### Checkpoint: Refusal Diagnosability

- [ ] Manual check: with a lock file holding a real PID, the refusal
  message includes it.
- [ ] With no PID readable, the message degrades gracefully.

### Phase 6: Full Verification

- [ ] Task 10: `cargo build --workspace` clean.
- [ ] Task 11: `cargo test --workspace` — full suite green.
- [ ] Task 12: `cargo clippy --all-targets -- -D warnings` clean.
- [ ] Task 13: `cargo fmt --all -- --check` clean.
- [ ] Task 14: `detect_changes({scope: "compare", base_ref: "main"})` —
  confirm the affected-symbol/flow set matches this plan's Blast Radius
  section; investigate and explain any surprise.

### Checkpoint: Ship

- [ ] All Phase 6 checks pass.
- [ ] PR description explains: the root asymmetry, the two upstream
  unbounded-join findings (`report_stopped`'s retry design,
  `ProgressGuard::stop_and_join`), the `on_shutdown` quit_at branch, the
  teardown extraction, the new config field/default, and links #202 and
  the spec/plan docs.
- [ ] PR is opened, not merged — merging requires explicit human go-ahead.

## Open Items Carried Into Implementation (lower-stakes)

- Confirm exact `EndFile` reason variant(s), if any beyond `on_shutdown`,
  correspond to a real quit vs. other stop causes.
- Verify the terminal-close/SIGHUP path has no other independent gaps
  (tray/watchdog thread interaction) while doing Phase 4.
- Exact numeric split of `quit_timeout_secs` across `stop_and_join`'s bound,
  `report_stopped_for_shutdown`'s budget, and the outer join's total —
  left to implementation judgment as long as real headroom exists; not
  expected to need further human input unless testing reveals the default
  5s is too tight once split three ways.

## Revision History

- **Original version**: unified the two teardown branches and added
  `report_stopped_for_shutdown`, but named the wrong primary call site
  (the rare watchdog instead of `on_shutdown`), estimated the config
  plumbing as "1 file" when it's 3-4, specified an untestable checkpoint
  (`run()` can't be unit-tested as written), didn't account for
  `on_shutdown` also covering the non-quit mpv-died-independently case,
  and passed the same `Duration` to every layer of the timeout composition
  without headroom.
- **After grilling review**: all five findings verified against the real
  source and folded in — correct primary call site (`on_shutdown`),
  honest plumbing scope (`Player::new` + 5 call sites), teardown extracted
  into its own testable method, `quit_at.is_some()` branch added to keep
  the non-quit case unaffected, and a second pre-existing unbounded join
  (`ProgressGuard::stop_and_join`) identified and added to scope, since no
  timeout-composition fix works until it's bounded too.
