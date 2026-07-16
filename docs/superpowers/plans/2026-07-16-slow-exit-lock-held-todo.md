# Task Checklist: Intermittent Slow Exit Leaves Foreground-Instance Lock Held

Issue: #202. Revised after grilling review — see plan's Revision History.

## Task 0: Run Pre-Edit Impact Checks

**Description:** Run mandatory GitNexus impact analysis for every symbol
this plan touches. Update the plan and warn the user before implementation
if any result is HIGH or CRITICAL risk.

**Acceptance criteria:**
- [ ] Impact recorded for `Player::join`, `Player::join_or_timeout`,
  `ProgressGuard::stop_and_join`, `Player::new`,
  `EmbyClient::report_stopped`, `SessionReporter::report_stopped`,
  `App::run`.

**Verification:**
- [ ] `impact()` calls for each symbol above, direction upstream.

**Dependencies:** None
**Files likely touched:** None
**Estimated scope:** XS: read-only

## Task 1: Add `quit_timeout_secs` Config Field

**Description:** Add `quit_timeout_secs: u64` to `Config`
(`crates/mbv-core/src/config.rs`), default `5`, following the exact
`progress_interval_secs` parse/default pattern.

**Acceptance criteria:**
- [ ] Field exists, defaults to `5`.
- [ ] Explicit `config.toml` override parses correctly.
- [ ] Missing field still defaults to `5`.

**Verification:** `cargo test -p mbv-core config`
**Dependencies:** Task 0
**Files likely touched:** `crates/mbv-core/src/config.rs`
**Estimated scope:** Small: 1 file

## Task 2: Bound `ProgressGuard::stop_and_join()`

**Description:** Replace the plain `h.join()` in
`ProgressGuard::stop_and_join()` (`player.rs:567-572`) with a bounded join
(watcher-thread + `recv_timeout`, same mechanism as
`Player::join_or_timeout`), budget derived from `quit_timeout_secs`. This
was missing from the original plan — found during grilling review: the
progress-reporter thread only checks its stop signal between HTTP calls,
so a slow `report_progress`/`report_ping` can block this join for up to
the shared agent's own timeout, independent of anything else in this fix.

**Acceptance criteria:**
- [ ] `stop_and_join` returns within its bound even if the progress-reporter
  thread is mid-HTTP-call past the bound.
- [ ] Normal (fast) case is unaffected — no added latency when the thread
  isn't stuck.

**Verification:**
- [ ] `cargo test -p mbv-core player`
- [ ] New test: progress-reporter thread deliberately blocked past the
  bound; `stop_and_join` still returns promptly.

**Dependencies:** Task 0
**Files likely touched:** `crates/mbv-core/src/player.rs`
**Estimated scope:** Small: 1 file

## Task 3: Add `report_stopped_for_shutdown`

**Description:** New function, e.g.
`report_stopped_for_shutdown(&self, timeout: Duration, ...) -> bool`, using
`timeout` as connect+total `ureq` budget, no retry. Additive only — the
existing `report_stopped` must not change signature or behavior.

**Acceptance criteria:**
- [ ] New function exists, distinct from `report_stopped`.
- [ ] No retry attempted under any failure.
- [ ] Existing `report_stopped` unchanged.

**Verification:**
- [ ] `cargo test -p mbv-core player`
- [ ] New test: stalling `TcpListener`, shutdown variant returns within its
  timeout.
- [ ] New test: same setup, exactly one request attempted (no retry).

**Dependencies:** Task 0
**Files likely touched:** `crates/mbv-core/src/api.rs`, `crates/mbv-core/src/player.rs`
**Estimated scope:** Small-Medium: 2 files

## Task 4: Thread `quit_timeout_secs` Through `Player::new`

**Description:** Add `quit_timeout_secs` as a new `Player::new` constructor
parameter (stored field, same pattern as `show_audio_window` etc.), landed
on `SessionReporter` at session-spawn time (not `MpvSessionConfig`). Update
all 5 known call sites: `daemon.rs::run_with_options`, `App::new`
(`src/app/mod.rs`), and 3 call sites inside `player.rs` (1 production, 2
test helpers — `cold_player`,
`set_initial_queue_seeds_status_without_starting_playback`, plus
`PlayerProxy::stub`).

**Acceptance criteria:**
- [ ] `Player::new` signature updated.
- [ ] All 5 call sites updated and compile.
- [ ] `SessionReporter` carries the value where `on_shutdown` can reach it.

**Verification:**
- [ ] `cargo build --workspace`
- [ ] `cargo test -p mbv-core player`

**Dependencies:** Task 1, Task 3
**Files likely touched:**
- `crates/mbv-core/src/player.rs`
- `crates/mbv-core/src/daemon.rs`
- `src/app/mod.rs`

**Estimated scope:** Small-Medium: 3-4 files

## Task 5: Branch `on_shutdown` on `self.quit_at.is_some()`

**Description:** `on_shutdown` (`player.rs`, currently around line 2170)
currently calls `report_stopped` unconditionally, but it fires both on a
real quit (`quit_at.is_some()`) and when mpv exits independently
(`quit_at.is_none()`, app keeps running — confirmed `PlayerEvent::MpvQuit`'s
handler does not break the event loop despite its own stale inline
comment claiming otherwise). Branch: real-quit case calls
`report_stopped_for_shutdown`; independent-death case keeps ordinary
`report_stopped`. Also fix the stale comment above the `MpvQuit` send.

**Acceptance criteria:**
- [ ] Real-quit path uses the shutdown-aware call.
- [ ] Independent-death path (`quit_at.is_none()`) unchanged — full
  budget, full retry.
- [ ] Stale "tell the app to quit" comment corrected.

**Verification:**
- [ ] `cargo test -p mbv-core player`
- [ ] New test proving both branches of the `quit_at` check dispatch to the
  correct report function.

**Dependencies:** Task 3, Task 4
**Files likely touched:** `crates/mbv-core/src/player.rs`
**Estimated scope:** Small: 1 file

## Task 6: Extract Teardown Into a Standalone Method

**Description:** Extract `App::run()`'s teardown tail into its own method
(e.g. `fn teardown(&mut self, ...)`), independent of `run()`'s
terminal/event-loop setup (`enable_raw_mode()` etc.), so it's callable
against a stubbed `App` in tests without a real tty. `run()` has zero
existing test coverage and can't be unit-tested as currently written —
confirmed during grilling review.

**Acceptance criteria:**
- [ ] Teardown logic lives in a method callable independently of `run()`.
- [ ] `run()`'s tail becomes a thin call into it.
- [ ] `run()` itself remains uncalled by tests (unchanged status quo).

**Verification:**
- [ ] `cargo build --workspace`
- [ ] New test calls the extracted method directly via `make_app_stub` or
  equivalent.

**Dependencies:** Task 0
**Files likely touched:** `src/app/mod.rs`
**Estimated scope:** Medium: 1 file, structural

## Task 7: Unify the Two Teardown Branches

**Description:** Within the Task 6 method, replace the two separate
branches (`QUIT_REQUESTED` signal path, normal fallthrough path) with one
shared sequence: read `last_valid_pos` (standardized, not
`position_ticks`), save queue state, stop + bounded join with real
headroom over the now-bounded `stop_and_join`/`report_stopped_for_shutdown`
budgets nested inside it, restore terminal. Only the log line explaining
*why* we're quitting should still differ between the two trigger
conditions.

**Acceptance criteria:**
- [ ] One shared sequence, not two.
- [ ] Both trigger conditions use `last_valid_pos`.
- [ ] Outer bound has real headroom over the nested bounded calls — not an
  identical `Duration` racing the same clock (see plan's Architecture
  Decisions for the composition approach).

**Verification:**
- [ ] `cargo test --lib` (closest existing focused quit/teardown target)
- [ ] New test: normal in-app quit-key entry point returns within a
  bounded window when the player thread is simulated as slow/hung
  (previously unbounded — the actual bug).
- [ ] Existing signal-path behavior unchanged apart from `last_valid_pos`.

**Dependencies:** Task 1, Task 2, Task 5, Task 6
**Files likely touched:** `src/app/mod.rs`
**Estimated scope:** Medium: 1 file, structural

## Task 8: Diagnostic Logging

**Description:** Add timestamped logging (start, duration, outcome) around
every join/report call in the unified sequence: the outer join, the bounded
`stop_and_join`, and the shutdown-aware report call.

**Acceptance criteria:**
- [ ] Each call logs start, duration, and outcome.

**Verification:**
- [ ] Manual: quit `mbv`, inspect `~/.local/state/mbv/mbv.log`.

**Dependencies:** Task 7
**Files likely touched:** `src/app/mod.rs`, `crates/mbv-core/src/player.rs`
**Estimated scope:** Small: 2 files, additive

## Task 9: Log Lock PID on `Resolution::Refuse`

**Description:** In `src/main.rs`'s `Resolution::Refuse` arm, call
`single_instance::read_pid(&lock_path)` and include the PID (if present)
in the printed refusal message.

**Acceptance criteria:**
- [ ] Message includes PID when readable.
- [ ] Degrades gracefully when not.

**Verification:**
- [ ] Manual: hold the lock, attempt a second launch, confirm PID appears
  and matches.

**Dependencies:** Task 0
**Files likely touched:** `src/main.rs`
**Estimated scope:** XS: 1 file, additive

## Task 10-13: Full Verification

**Description:** `cargo build --workspace`, `cargo test --workspace`,
`cargo clippy --all-targets -- -D warnings`, `cargo fmt --all -- --check`
— all clean.

**Dependencies:** Task 1-9
**Estimated scope:** N/A

## Task 14: `detect_changes` and PR

**Description:** Run `detect_changes({scope: "compare", base_ref: "main"})`,
confirm the affected-symbol/flow set matches the plan's Blast Radius
section, then commit, push, and open a PR referencing #202. **Do not
merge** — stop at the opened PR; merging requires explicit human
go-ahead in a separate step.

**Acceptance criteria:**
- [ ] `detect_changes` output matches expectations or surprises explained.
- [ ] PR description covers: the asymmetry, both upstream unbounded-join
  findings, the `on_shutdown` branch, the teardown extraction, the config
  field/default, links to #202 and both spec/plan docs.
- [ ] No `Co-Authored-By:` trailer on any commit.

**Verification:**
- [ ] `detect_changes(...)`
- [ ] `gh pr create ...`

**Dependencies:** Task 10-13
**Estimated scope:** N/A
