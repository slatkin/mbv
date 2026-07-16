# Spec: Intermittent Slow Exit Leaves Foreground-Instance Lock Held

Issue: #202. Related: #192 (all Emby server calls share one blanket
connect/total timeout with no shutdown-awareness — this spec's real fix
turns out to be a shutdown-scoped instance of that same problem, not an
independent one). #191/PR #216 already addressed the *startup* instance of
#192 (bounded `authenticate_bounded`, 5s connect / 30s total split on the
shared `ureq::Agent`); this spec addresses the *shutdown* instance.

This spec was revised after an independent code-grounded review and an
adversarial "grilling" interview surfaced that the original version's
proposed fix — just adding a `join_or_timeout(5s)` call to the previously-
unbounded quit path — would have shipped a real regression. See
"Revision History" at the bottom for what changed and why.

## Assumptions

- This is a shutdown-path bug fix, not a redesign of `single_instance.rs`'s
  flock/socket resolution model (ADR 0006). `Resolution::Refuse` correctly
  means "a live process still holds the lock" — the fix is to make sure the
  process actually finishes exiting promptly, not to change how refusal is
  detected.
- **The teardown-branch asymmetry is a confirmed fact, not a hypothesis**
  (independently verified against the real source, file:line references
  below). Whether *this specific asymmetry* caused *this specific user's
  one-time report* is a strong, code-grounded working hypothesis, not
  settled proof — the user saw this once, and their own follow-up comment
  raises a second plausible cause (terminal close) that must be checked,
  not assumed away. Diagnosis should aim to demonstrate the mechanism is
  real and capable of producing this class of delay, not to retroactively
  claim it explains the one observed incident.
- No user-visible UI is required beyond what already exists (status/log
  lines); this is a correctness and diagnosability fix, not a new feature.
- `mbv -q` / SIGTERM-based quit and terminal-close/SIGHUP are both in scope
  for *diagnosis* (per the human's explicit follow-up comment on the
  issue), not just the in-app quit-key path.

## Objective

After a normal, non-stay-alive quit — pressing the in-app quit key, or the
terminal closing (SIGHUP) — `mbv` should release the
`$XDG_RUNTIME_DIR/mbv.lock` advisory flock promptly, so a later launch never
intermittently hits `single_instance::Resolution::Refuse` because a
previous, already-quit session is still technically alive and holding the
lock — **without** trading that reliability for routinely-incomplete
session termination on the Emby server.

### What's actually happening today

`App::run()` (`src/app/mod.rs`) has two teardown branches at the end of its
event loop:

- **Signal path** (`QUIT_REQUESTED` true — set only by SIGHUP/SIGTERM, i.e.
  terminal closed or `mbv -q`): calls
  `self.player.join_or_timeout(Duration::from_secs(5))` (`:3123`) — bounded.
- **Normal path** (in-app quit key, via `App::try_quit` in
  `src/app/actions.rs:4589-4626`, which never touches `QUIT_REQUESTED`):
  calls `self.player.join()` (`:3138`) — **unbounded**.

`LockGuard` (`src/main.rs:402-408`) only drops once `App::new(client).run()`
returns, so an unbounded join really does keep the flock held for as long
as the player thread's shutdown work takes.

**The signal-context-safety rationale for keeping these two branches
separate does not hold.** `handle_quit_signal` is nothing but two atomic
stores (`QUIT_REQUESTED`/`TERMINAL_GONE`); the actual `join`/`join_or_timeout`
call runs later, in ordinary thread context, identically in both branches.
There is no structural reason left for these to differ.

### Why "just add the same 5s bound to the normal path" is not the fix

The player-thread quit sequence calls `SessionReporter::report_stopped`
(`crates/mbv-core/src/player.rs`, e.g. the `quit_at`-elapsed watchdog path
at `:2244-2251`, and/or the corresponding `EndFile`-driven quit handling —
implementation should confirm exactly which call site(s) fire on a normal
player-thread quit vs. mid-playback track transitions, since `report_stopped`
has many call sites in `player.rs` and most of them are unrelated to
shutdown). `report_stopped` (`crates/mbv-core/src/api.rs:1207-1252`) POSTs
to `/Sessions/Playing/Stopped` — a **session-terminating** call, not a
progress ping — and has its own built-in retry: on failure, sleep 500ms and
retry once. Against the shared `ureq::Agent`'s 5s-connect/30s-total config
(`crates/mbv-core/src/api.rs:600-614`), its designed worst-case completion
time is roughly 5+30+0.5+30 ≈ 65s.

Today, the normal-quit path's unbounded `join()` means this call — including
its retry — almost always gets to finish. A bare external 5s bound doesn't
fix a rare failure mode; it **guarantees** that call gets cut off on any
real-world Emby slowness, converting "occasionally slow exit" into
"routinely incomplete session termination" — leaving `mbv` looking like it's
still playing ("phantom Now Playing") on the Emby server's session list,
for however long Emby's own idle-session handling takes to notice, which
this codebase doesn't control. The periodic `spawn_progress_reporter`
(10s-interval `TimeUpdate` pings, `crates/mbv-core/src/config.rs`) does
**not** mitigate this — pings and session-termination are different Emby
message types; a ping keeps a session looking active, it doesn't end one.

The fix therefore needs `report_stopped`'s call site(s) reached during
player-thread shutdown to be **shutdown-aware internally** — using a
tighter connect/total budget and skipping the 500ms-retry when called from
a teardown context — so the *whole* call, retry included, can realistically
complete inside whatever external bound we pick, rather than an unaware
external timeout fighting the function's own internal retry logic. This is
a real scope increase beyond "add a timeout constant," and is the same
class of problem #192 describes at the general level (one blanket timeout,
no fail-fast distinction) — just encountered here specifically during
shutdown rather than at connect time.

### What "success" means

1. Diagnosis (logging around both join call sites and around
   `report_stopped`) demonstrates the unbounded-join mechanism is real and
   capable of producing this class of delay — not a retroactive claim that
   it explains the user's one observed incident.
2. Both the normal-quit and terminal-close/SIGHUP paths are explicitly
   verified, per the human's follow-up comment — not just the one already
   suspected.
3. Both teardown branches are unified into one bounded-join sequence (the
   signal-context-safety reason to keep them separate doesn't hold), and
   both standardize on `last_valid_pos` over `position_ticks` for the final
   position save (see Open Question resolved below — the comment
   justifying `last_valid_pos`, "never zeroed during track transitions,"
   applies equally to both branches, not just the one that currently uses
   it).
4. The quit-triggered `report_stopped` call becomes shutdown-aware
   internally (tighter budget, retry skipped in teardown context) rather
   than being externally guillotined by an unaware timeout — so shutdown is
   both bounded *and* actually completes the Emby session-termination call
   in the overwhelming majority of real-world cases.
5. A restart attempted shortly after either quit path no longer
   intermittently hits `single_instance::Resolution::Refuse`.

## Tech Stack

- Rust 2021 workspace, no new dependencies expected.
- `src/app/mod.rs` owns the TUI event loop and the (to-be-unified) teardown
  sequence.
- `crates/mbv-core/src/player.rs` owns `Player::join`/`join_or_timeout`, the
  player thread's quit-triggered `report_stopped` call site(s).
- `crates/mbv-core/src/api.rs` owns `EmbyClient::report_stopped` and the
  shared `ureq::Agent` — needs a shutdown-aware variant or parameter, not
  just a caller-side timeout.
- `src/single_instance.rs` and `src/main.rs` own flock acquisition/release —
  expected to need no changes, just to be the thing this fix is verified
  against.
- Tests are ordinary Rust unit tests plus the existing GitNexus-tracked
  `detect_changes` / `impact` workflow this repo requires before committing.

## Commands

- Build: `cargo build --workspace`
- Format check: `cargo fmt --all -- --check`
- Lint: `cargo clippy --all-targets -- -D warnings`
- Focused player/shutdown tests: `cargo test -p mbv-core player`
- Focused app/quit tests: `cargo test --lib quit`
- Full test suite: `cargo test`

## Project Structure

- `src/app/mod.rs` -> `App::run()`'s teardown sequence (to be unified),
  `install_signal_handlers`, `start_quit_watchdog`,
  `QUIT_REQUESTED`/`TERMINAL_GONE` statics.
- `src/app/actions.rs` -> `App::try_quit` — confirmed entry point for the
  in-app quit key; does not set `QUIT_REQUESTED`.
- `crates/mbv-core/src/player.rs` -> `Player::join`, `Player::join_or_timeout`,
  the player thread's `run()` loop and its quit-triggered `report_stopped`
  call site(s) (distinct from the many mid-playback call sites elsewhere in
  this file — do not touch those).
- `crates/mbv-core/src/api.rs` -> `EmbyClient::report_stopped`,
  `EmbyClient::new`'s shared `ureq::Agent` config — needs a shutdown-scoped
  variant/parameter.
- `crates/mbv-core/src/config.rs` -> `spawn_progress_reporter`'s interval
  (context on why periodic pings don't substitute for `report_stopped`),
  and the new `quit_timeout_secs: u64` field (default `5`) added to
  `Config` following the existing `progress_interval_secs` pattern.
- `src/single_instance.rs` -> `resolve()`, `Resolution`, `LockGuard` — the
  detection side; not expected to change, but the source of truth for what
  "held" vs. "released promptly" means.
- `src/main.rs` -> where `LockGuard` actually drops and the
  `Resolution::Refuse` message the user sees on a blocked restart.

## Code Style

Unify the teardown sequence rather than adding a second bounded-join call
alongside the existing one:

```rust
// Both QUIT_REQUESTED and the normal in-app quit path should converge on
// one sequence: read last_valid_pos, save queue state, stop + bounded join,
// restore terminal. The only thing that should still differ between them
// is which log line explains *why* we're quitting.
```

For the shutdown-aware `report_stopped`, prefer a variant or parameter over
a caller-side wrapper that races the function externally — e.g. something
in the shape of `report_stopped_for_shutdown(&self, timeout: Duration, ...)`
that uses a tighter internal `ureq` budget (derived from the same
`quit_timeout_secs` config value below) and skips the 500ms-retry, rather
than `join_or_timeout` racing the existing `report_stopped` from outside.
The exact shape is an implementation decision, not dictated here — but it
must give the call a real chance to complete within the shutdown bound, not
just cut off whatever the ordinary function happens to be doing at
deadline.

**Timeout value — resolved.** One config value, `quit_timeout_secs: u64`
(default `5`), added to `Config` in `crates/mbv-core/src/config.rs`
following the existing `progress_interval_secs` pattern (manual TOML parse
from the `[general]` section, `#[serde(default)]`-equivalent fallback to
`5` when absent). This single value serves both roles:

- The outer bound passed to the unified teardown's `join_or_timeout(...)`.
- The internal budget for `report_stopped_for_shutdown`'s `ureq` request
  (connect + total), with the 500ms-retry skipped entirely — retrying
  within a 5s window after any real first-attempt latency isn't meaningful
  anyway.

Rationale for a single shared value rather than two independent knobs:
this is a local-network deployment (Emby server on the same LAN/host in the
overwhelmingly common case), so 5s is generous for a healthy connection and
short enough to keep quit feeling responsive when the server is actually
down or hung — and one number is simpler to reason about and document than
two separately-tunable ones for what is, from the user's perspective, one
"how long should quit be willing to wait" setting.

**Relationship to #192 — resolved, proceed independently.** This spec's
fix does not depend on `EmbyClient`'s shared `ureq::Agent` config (the
thing #192 is about) — `report_stopped_for_shutdown` gets its own
dedicated, explicitly-configured budget via `quit_timeout_secs`, decoupled
from whatever the general client timeout is or becomes. #202 can land
first, independently; #192's eventual resolution (further general-timeout
tightening) won't materially change what this fix needs, since the
shutdown path no longer inherits that shared config.

## Testing Strategy

- Add a `mbv-core` test proving the bounded-join mechanism itself returns
  promptly (well under its timeout) even when the player thread is
  deliberately blocked past the timeout.
- Add a test proving the unified teardown sequence in `App::run()` returns
  within a bounded window on both the signal-triggered and normal-quit
  entry points, when the player thread is simulated as slow/hung.
- Add a `mbv-core` test proving the shutdown-aware `report_stopped` variant
  actually attempts the call (doesn't just no-op) but respects its tighter
  internal budget — e.g. against a local `TcpListener` that stalls, similar
  in spirit to the stalled-listener tests added for #191/PR #216.
- Add a test proving the retry is skipped (not attempted) when invoked via
  the shutdown-aware path, vs. still attempted on the ordinary
  mid-playback path.
- Add logging around `report_stopped`/its shutdown variant and both join
  call sites with timestamps, so a future recurrence is diagnosable from
  `~/.local/state/mbv/mbv.log` without needing to reproduce interactively.
- Do not attempt to test the real single-instance flock/timing end-to-end in
  CI — test the bounded-join and shutdown-aware-report mechanisms directly,
  and treat flock-release timing as a consequence that follows once both
  are correct.

## Boundaries

- Always: preserve `single_instance.rs`'s existing "never trust lock-file
  existence, only socket connectability + live flock" model (ADR 0006).
- Always: keep the terminal-close (SIGHUP) path's existing behavior working
  while unifying it with the normal-quit path — verify, don't just assume,
  that unification doesn't regress it.
- Always: run `impact()` before editing `Player::join`/`join_or_timeout`,
  `App::run`'s teardown sequence, or `EmbyClient::report_stopped`, and
  report risk level before proceeding, per this repo's GitNexus workflow.
- Always: run `detect_changes({scope: "compare", base_ref: "main"})` before
  committing.
- Always: confirm exactly which `report_stopped` call site(s) in
  `player.rs` actually fire during a player-thread quit sequence before
  making any of them shutdown-aware — do not touch the mid-playback
  track-transition call sites, which have no timing pressure and should
  keep their current (full retry, full budget) behavior.
- Always: add `quit_timeout_secs` to `config.toml` following the existing
  `progress_interval_secs` pattern (manual parse, default fallback), not a
  hardcoded constant — the value must be user-configurable per the
  resolved decision above.
- Never: silently swallow a `report_stopped`/shutdown-variant failure
  without logging it.
- Never: change `single_instance::resolve()`'s refusal semantics to "treat
  the lock as stale."
- Never: apply the shutdown-aware budget/retry-skip to `report_stopped`
  call sites outside the actual quit sequence — mid-playback reporting
  should be unaffected.

## Success Criteria

- Diagnosis logging demonstrates the teardown-branch asymmetry is real and
  capable of producing this class of delay (not a retroactive claim about
  the specific user report).
- Both shutdown paths — normal in-app quit and terminal-close/SIGHUP — are
  explicitly exercised and verified.
- Both teardown branches are unified into one bounded sequence using
  `last_valid_pos` consistently.
- The quit-triggered `report_stopped` call site(s) use a shutdown-aware
  variant (tighter budget, retry skipped) so the call reliably completes
  within the shutdown bound in the common case, instead of being routinely
  abandoned mid-attempt.
- Mid-playback `report_stopped` call sites are unaffected — same budget,
  same retry behavior as today.
- After the fix, a restart attempted shortly after either quit path no
  longer intermittently hits `single_instance::Resolution::Refuse`.
- If a genuinely stuck live process is ever detected, the
  `Resolution::Refuse` path is diagnosable — at minimum by logging the
  lock's PID (`single_instance::read_pid`) at refusal time.
- Regression tests cover: the bounded-join mechanism itself, the unified
  teardown sequence on both entry points, and the shutdown-aware
  `report_stopped` variant's tighter budget + skipped retry.

## Open Questions

**Resolved (human decision, this round):**

- **Timeout value**: `quit_timeout_secs: u64`, default `5`, configurable in
  `config.toml`, one value shared by both the outer join bound and
  `report_stopped_for_shutdown`'s internal budget. See Code Style for the
  full rationale (local-network deployment, one user-facing knob).
- **Relationship to #192**: proceed independently. #202 lands first; no
  strong coupling once the shutdown path has its own dedicated
  `quit_timeout_secs` budget rather than inheriting the shared
  `EmbyClient` agent config that #192 is about.

**Still open, lower stakes:**

- Should `Resolution::Refuse`'s user-facing message include the lock PID
  proactively, so a future recurrence is self-diagnosing from the error
  message alone?
- Is there evidence from the user's original occurrence (logs) pinning
  down which specific shutdown-phase call was slow, or does this need
  fresh instrumentation and a live repro attempt first?
- Does the terminal-close/SIGHUP path have any gaps of its own worth fixing
  even if it's not the cause of this specific report (tray/watchdog thread
  interaction, whether signal handlers reliably fire before a killed
  terminal tears down stdio)?
- Confirm exactly which `report_stopped` call site(s) in `player.rs` fire
  on quit (the `quit_at`-elapsed watchdog at `:2244-2251`, and/or an
  `EndFile`-driven path) before implementation — the spec identifies the
  watchdog path with confidence but hasn't exhaustively traced every
  `EndFile` reason variant.

## Revision History

- **Original version**: proposed adding `join_or_timeout(Duration::from_secs(5))`
  to the previously-unbounded normal-quit path, treating this as a
  low-risk, mostly-mechanical fix. Root cause was stated as settled fact.
- **After independent review**: confirmed every structural claim (branch
  line numbers, join semantics, `QUIT_REQUESTED` ownership, `report_stopped`
  being a synchronous ureq call, `LockGuard` drop timing) against the real
  source — no factual errors found, but flagged that truncating
  `report_stopped` could lose the final playback-position update, and that
  the spec's Testing Strategy didn't cover it.
- **After grilling interview**: found (1) the diagnostic-confidence
  language was overstated relative to what a single-incident report can
  actually prove; (2) the signal-context-safety rationale for keeping two
  teardown branches doesn't hold, since `handle_quit_signal` does nothing
  but atomic stores — leading to full unification plus a
  `last_valid_pos`-consistency fix; (3) most importantly, that
  `report_stopped` hits a session-terminating endpoint with its own
  65s-worst-case retry logic, and a naive external timeout wouldn't just
  risk losing a few seconds of position — it would make `report_stopped`
  routinely fail to complete on every quit, leaving phantom "Now Playing"
  sessions on the Emby server. This is why the fix now requires a
  shutdown-aware `report_stopped` variant rather than an external timeout
  wrapper, and why the timeout values are called out as an explicit
  human decision rather than inherited by precedent.
