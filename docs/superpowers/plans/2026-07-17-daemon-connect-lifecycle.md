# Daemon Connect Lifecycle Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give mbv a lazy, fallback-with-warning, no-retry, no-parking daemon-route connect primitive (issue #222) that a future per-library routing trigger (issue #223) can call from inside a suspend/connect/restore swap, without wiring any UI trigger itself.

**Architecture:** Add a small, independently testable pair of `App` methods in `src/app/mod.rs`, modeled directly on the existing `connect_direct_endpoint` / `DIRECT_CONNECT_OVERRIDE` test-injection pattern used by the Sessions-panel "Direct Remote" upgrade (`connect_to_session`, `switch_to_direct_remote`): a thin connect wrapper (`connect_daemon_route_endpoint`, with its own `DAEMON_ROUTE_CONNECT_OVERRIDE` test seam, logging that a successful connect takes driving-client authority) and a fallback-aware caller (`try_daemon_route_connect`) that returns `Result<(RemotePlayer, Receiver<PlayerEvent>), String>` — `Ok` on a successful connect, or on failure an `Err(String)` holding a fully-formatted, ready-to-display status-bar warning (the raw connect error is logged, not returned) — with no retry scheduled anywhere. This deliberately mirrors `connect_direct_endpoint`'s existing shape (also `Result`-returning, leaving display/flashing to its caller `connect_to_session`) rather than flashing internally: `try_daemon_route_connect`'s one real consumer (#223's per-library swap function) needs to choose *how* to fall back — a plain `flash_status_high` when it was already local, or routing the same message through a `restore_local_mode`-style teardown when swapping away from a previously active *different* route — and a primitive that flashed unconditionally would risk a second, conflicting flash on top of that teardown path. The primitive still makes the warning impossible to forget or reword differently at each call site, since `Result`'s `Err` arm must be handled and already carries the exact canonical text — only *where* it gets displayed is the caller's call. The existing `--connect-daemon` / `daemon_client_endpoint` startup path in `main.rs` is untouched. No config schema changes are needed for this issue (routing config is #223's addition). Document the lifecycle rules in a new ADR and in `CONTEXT.md` so #223 can build its per-library swap function on top of vocabulary and primitives that already exist.

**Tech Stack:** Rust (2021 edition), `cargo test` workspace (`mbv` binary crate at repo root, `mbv-core` lib crate, `mbvd` daemon crate), std `mpsc`/`Mutex`/`AtomicBool` — no external test or mocking framework.

## Global Constraints

- `main.rs`'s `explicit_daemon_endpoint` branch (`--connect-daemon` / config `daemon_client_endpoint`) must not be modified or behaviorally changed — issue #222 states this path is unaffected.
- No startup-time daemon connection may be introduced anywhere. The new primitive must have zero production call sites in this plan (the trigger is #223's job) and must not be invoked from `App::new`, `App::new_remote`, or `App::build`.
- On a failed connect attempt: fall back to (or stay on) local playback. `try_daemon_route_connect` never hard-fails/exits; it logs the raw failure via `log::warn!` and returns `Err(String)` holding a ready-to-display warning -- the caller is responsible for actually surfacing it via the existing `flash_status_high` mechanism (`src/app/actions.rs:2242-2247`, 5s expiry), either directly or by threading the message through its own state-teardown path (e.g. #223's `restore_local_mode`).
- No background retry: a failed attempt schedules nothing. The next attempt only happens when a caller invokes the primitive again on its own natural trigger.
- No connection parking: swapping away from a route disconnects cleanly (the `RemotePlayer` is simply dropped, never stashed for reuse the way `SuspendedLocalSession` parks a local `Player`).
- Never call the daemon/`remote_player.rs` mechanism a "remote session" in code comments, docs, or log messages — that term is reserved for the Sessions-panel (`connected_session_id`) feature. See `mem:feedback_remote_session_terminology`.
- Never say "Jellyfin" — Emby only.
- Design the primitive so it is parameterized by a route label (not hardcoded to one global endpoint), so #223 can pass a library name without modifying this code — per the parent brief's explicit instruction not to paint #223 into a corner.

---

### Task 1: Daemon-route connect wrapper with lazy-connect, fallback, and no-retry behavior

**Files:**
- Modify: `src/app/mod.rs:36-39` (add new test-injection statics next to `DIRECT_CONNECT_OVERRIDE`)
- Modify: `src/app/mod.rs:2316-2333` (add new methods after `connect_direct_endpoint`)
- Test: `src/app/mod.rs` (`mod tests`, after `connect_to_session_preserves_direct_upgrade_failure_status_after_fallback`, currently ending around line 6233)

**Interfaces:**
- Consumes: `mbv_core::remote_player::{DaemonEndpoint, RemotePlayer}` (existing), `PlayerEvent` (existing import at `src/app/mod.rs:140`), the existing `DirectConnectFn` type alias (`src/app/mod.rs:25-34`). `App::flash_status_high` (`src/app/actions.rs:2242-2247`) is NOT consumed by this primitive -- it is deliberately left to the caller (see Architecture above), so it is a downstream consumer of `try_daemon_route_connect`'s `Err` payload instead.
- Produces: `App::connect_daemon_route_endpoint(&self, endpoint: &DaemonEndpoint, auth_token: &str) -> Result<(RemotePlayer, mpsc::Receiver<PlayerEvent>), String>` (private) and `App::try_daemon_route_connect(&mut self, endpoint: &DaemonEndpoint, route_label: &str) -> Result<(RemotePlayer, mpsc::Receiver<PlayerEvent>), String>` (`pub(super)`) — `Ok` on a successful connect; `Err(message)` on failure, where `message` is a fully-formatted, ready-to-display status-bar warning (the raw connect error is logged internally under `target: "daemon_route"`, not returned) for the caller to surface however fits its own state: a direct `flash_status_high(message)`, or threaded through a state-teardown path like issue #223's `restore_local_mode`. Later tasks and issue #223's per-library swap function call `try_daemon_route_connect` and must handle both arms; the primitive itself never calls `flash_status_high`. Also `#[cfg(test)] static DAEMON_ROUTE_CONNECT_OVERRIDE` / `DAEMON_ROUTE_CONNECT_TEST_LOCK`, used by Task 2's regression test.

- [ ] **Step 1: Write the failing tests**

Add these two tests to `src/app/mod.rs`'s `mod tests` block, directly after the closing `}` of `connect_to_session_preserves_direct_upgrade_failure_status_after_fallback` (which currently ends just before `fn remote_position_extrapolation_does_not_round_up_partial_seconds`):

```rust
    #[test]
    fn try_daemon_route_connect_returns_remote_player_on_successful_connect() {
        let _guard = crate::config::TestStateDirGuard::new();
        let _connect_guard = DAEMON_ROUTE_CONNECT_TEST_LOCK.lock().unwrap();
        fn route_connect_success(
            _endpoint: &mbv_core::remote_player::DaemonEndpoint,
            _auth_token: &str,
        ) -> Result<
            (
                mbv_core::remote_player::RemotePlayer,
                mpsc::Receiver<PlayerEvent>,
            ),
            String,
        > {
            Ok(mbv_core::remote_player::RemotePlayer::stub(
                make_items(1),
                0,
            ))
        }

        *DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = Some(route_connect_success);
        let mut app = make_app_stub();
        let endpoint =
            mbv_core::remote_player::DaemonEndpoint::Unix(std::path::PathBuf::from(
                "/tmp/mbv-music.sock",
            ));

        let result = app.try_daemon_route_connect(&endpoint, "Music");

        *DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = None;
        assert!(result.is_ok());
    }

    #[test]
    fn try_daemon_route_connect_returns_a_ready_to_display_warning_without_flashing_on_failure() {
        let _guard = crate::config::TestStateDirGuard::new();
        let _connect_guard = DAEMON_ROUTE_CONNECT_TEST_LOCK.lock().unwrap();
        fn route_connect_failure(
            _endpoint: &mbv_core::remote_player::DaemonEndpoint,
            _auth_token: &str,
        ) -> Result<
            (
                mbv_core::remote_player::RemotePlayer,
                mpsc::Receiver<PlayerEvent>,
            ),
            String,
        > {
            Err("connection refused".to_string())
        }

        *DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = Some(route_connect_failure);
        let mut app = make_app_stub();
        let endpoint =
            mbv_core::remote_player::DaemonEndpoint::Unix(std::path::PathBuf::from(
                "/tmp/mbv-music.sock",
            ));

        let result = app.try_daemon_route_connect(&endpoint, "Music");

        *DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = None;
        // `RemotePlayer` derives only `Clone` (no `PartialEq`/`Debug` --
        // confirmed against `crates/mbv-core/src/remote_player.rs`), so the
        // whole `Result` can't go through `assert_eq!` directly; match out
        // the `Err` payload instead.
        match result {
            Ok(_) => panic!("expected a connect failure to return Err, got Ok"),
            Err(message) => {
                assert_eq!(
                    message,
                    "\u{26a0} Music route unreachable, using local playback (mbv.log)"
                );
            }
        }
        // The primitive itself must never flash -- that is the caller's
        // job (see Architecture). `make_app_stub()` starts with an empty
        // status, so this pins down that `try_daemon_route_connect` left
        // it untouched.
        assert!(app.status.is_empty());
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p mbv try_daemon_route_connect -- --exact`
Expected: FAIL to compile — `error[E0425]: cannot find value \`DAEMON_ROUTE_CONNECT_OVERRIDE\` in this scope` (and/or `no method named \`try_daemon_route_connect\` found for struct \`App\``), since neither the statics nor the methods exist yet.

- [ ] **Step 3: Add the test-injection statics**

In `src/app/mod.rs`, immediately after the existing block:

```rust
#[cfg(test)]
static DIRECT_CONNECT_OVERRIDE: Mutex<Option<DirectConnectFn>> = Mutex::new(None);
#[cfg(test)]
static DIRECT_CONNECT_TEST_LOCK: Mutex<()> = Mutex::new(());
```

add:

```rust
// Separate from DIRECT_CONNECT_OVERRIDE above (Sessions-panel "Direct
// Remote" upgrade, keyed off a discovered SessionInfo): this is issue
// #222's lazy daemon-route connect primitive, targeting a statically
// configured DaemonEndpoint with no session discovery. Kept as its own
// override/lock pair so the two connect paths -- and the App state they
// eventually drive (`connected_session_id`/`direct_remote_label` vs. a
// future #223 `active_route`) -- stay independently testable and are
// never conflated, per #223's explicit "must not be conflated" rule.
#[cfg(test)]
static DAEMON_ROUTE_CONNECT_OVERRIDE: Mutex<Option<DirectConnectFn>> = Mutex::new(None);
#[cfg(test)]
static DAEMON_ROUTE_CONNECT_TEST_LOCK: Mutex<()> = Mutex::new(());
```

(`DirectConnectFn` is reused as-is: `fn(&DaemonEndpoint, &str) -> Result<(RemotePlayer, mpsc::Receiver<PlayerEvent>), String>` already matches this primitive's shape, so no new type alias is needed.)

- [ ] **Step 4: Add `connect_daemon_route_endpoint` and `try_daemon_route_connect`**

In `src/app/mod.rs`, immediately after `connect_direct_endpoint`'s closing `}` (and before `fn switch_to_direct_remote`), add:

```rust
    /// Lazy, on-demand connect to a daemon route endpoint (issue #222's
    /// lifecycle primitive). Unlike `connect_direct_endpoint` (Sessions-panel
    /// "Direct Remote" upgrade, keyed off a discovered `SessionInfo`), this
    /// targets a statically configured `DaemonEndpoint` with no session
    /// discovery involved -- the shape #223's per-library routing needs.
    ///
    /// Connecting **is** taking driving-client authority on that daemon
    /// (ADR 0003, ADR 0007, ADR 0010) -- logged here so it is diagnosable,
    /// not a hidden side effect.
    ///
    /// `#[allow(dead_code)]`: this repo's convention (`mem:conventions`) is
    /// "fix all compile warnings -- delete unused code, never
    /// `#[allow(unused)]`" -- but this primitive is a deliberate exception,
    /// not a suppressed mistake: issue #222's brief requires it to ship with
    /// *zero* production call sites (the trigger is #223's job, see
    /// Architecture above), so a plain `cargo build --workspace` (which
    /// strips `#[cfg(test)]` code, its only current caller) would otherwise
    /// warn `associated function is never used`. Deleting the primitive to
    /// silence that would defeat the entire point of this plan -- shipping
    /// a complete, tested, reusable connect primitive ahead of the issue
    /// that wires it up. Remove this attribute in the same change that adds
    /// #223's first call site (`apply_route_for_playback` or equivalent).
    #[allow(dead_code)]
    fn connect_daemon_route_endpoint(
        &self,
        endpoint: &mbv_core::remote_player::DaemonEndpoint,
        auth_token: &str,
    ) -> Result<
        (
            mbv_core::remote_player::RemotePlayer,
            mpsc::Receiver<PlayerEvent>,
        ),
        String,
    > {
        #[cfg(test)]
        if let Some(connect) = *DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() {
            return connect(endpoint, auth_token);
        }

        log::info!(
            target: "daemon_route",
            "connecting to daemon route endpoint {endpoint}; this takes driving-client authority on that daemon (see ADR 0003, ADR 0007, ADR 0010)"
        );
        mbv_core::remote_player::RemotePlayer::connect_endpoint(endpoint, auth_token)
    }

    /// Attempts a lazy connect to `endpoint` for the route named
    /// `route_label` (e.g. a library name from #223's `daemon_routes`, or a
    /// generic label for the wildcard "route everything" case). On success,
    /// returns `Ok` with the connected `RemotePlayer` and its event receiver
    /// for the caller to swap in (mirroring `switch_to_direct_remote`'s
    /// shape). On failure, per #222: falls back to (stays on) local
    /// playback and schedules no retry -- but this primitive does NOT flash
    /// the warning itself. It logs the raw connect error internally
    /// (`target: "daemon_route"`), then returns `Err(message)` where
    /// `message` is the fully-formatted, ready-to-display status-bar
    /// warning text. Flashing is left to the caller deliberately: #223's
    /// per-library swap function needs to choose *how* to fall back --
    /// `flash_status_high(message)` directly when it was already local, or
    /// threading `message` through a `restore_local_mode`-style teardown
    /// when swapping away from a previously active *different* route -- and
    /// having this primitive flash unconditionally would risk a second,
    /// conflicting flash on top of that teardown path's own flash. The
    /// caller is expected to try again only on its own next natural trigger
    /// (e.g. the next play/enqueue into this route), never from a
    /// background timer. See the same `#[allow(dead_code)]` rationale as
    /// `connect_daemon_route_endpoint` above -- remove both attributes
    /// together when #223 adds its first call site.
    #[allow(dead_code)]
    pub(super) fn try_daemon_route_connect(
        &mut self,
        endpoint: &mbv_core::remote_player::DaemonEndpoint,
        route_label: &str,
    ) -> Result<
        (
            mbv_core::remote_player::RemotePlayer,
            mpsc::Receiver<PlayerEvent>,
        ),
        String,
    > {
        let auth_token = self.client.lock().unwrap().token.clone();
        match self.connect_daemon_route_endpoint(endpoint, &auth_token) {
            Ok((remote, remote_rx)) => Ok((remote, remote_rx)),
            Err(e) => {
                log::warn!(
                    target: "daemon_route",
                    "daemon route connect failed for route={route_label:?} endpoint={endpoint}: {e}"
                );
                Err(format!(
                    "\u{26a0} {route_label} route unreachable, using local playback (mbv.log)"
                ))
            }
        }
    }
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p mbv try_daemon_route_connect -- --exact`
Expected: PASS — both `try_daemon_route_connect_returns_remote_player_on_successful_connect` and `try_daemon_route_connect_returns_a_ready_to_display_warning_without_flashing_on_failure` pass.

- [ ] **Step 6: Commit**

```bash
git add src/app/mod.rs
git commit -m "feat: add lazy daemon-route connect primitive with fallback warning (#222)"
```

---

### Task 2: Regression test — no daemon-route connect attempt at App construction

**Files:**
- Test: `src/app/mod.rs` (`mod tests`, directly after Task 1's two new tests)

**Interfaces:**
- Consumes: `DAEMON_ROUTE_CONNECT_OVERRIDE`, `DAEMON_ROUTE_CONNECT_TEST_LOCK`, `make_app_stub()` (all from Task 1 / existing `src/app/mod.rs:5401`).
- Produces: nothing new — this is a pure regression guard pinning down the "no connect before an explicit trigger" acceptance criterion from #222.

- [ ] **Step 1: Write the failing test**

This test is expected to pass immediately once written (there is genuinely no production call site yet), but write it first anyway per TDD discipline — run it once before any further code changes to confirm the assertion is exercised, not vacuously true from a typo. Add directly after the two tests from Task 1:

```rust
    #[test]
    fn app_construction_never_attempts_a_daemon_route_connect() {
        // #222 acceptance criterion: "No connection attempt happens before
        // the first play/enqueue action that needs one." There is no
        // production call site wiring `try_daemon_route_connect` into
        // startup yet (that wiring is #223's job) -- this test pins the
        // invariant down as a regression guard so a future startup-time
        // call is caught immediately instead of silently reintroducing the
        // eager-connect behavior #222 replaces.
        static CALLS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let _guard = crate::config::TestStateDirGuard::new();
        let _connect_guard = DAEMON_ROUTE_CONNECT_TEST_LOCK.lock().unwrap();
        CALLS.store(0, std::sync::atomic::Ordering::SeqCst);
        fn counting_connect(
            _endpoint: &mbv_core::remote_player::DaemonEndpoint,
            _auth_token: &str,
        ) -> Result<
            (
                mbv_core::remote_player::RemotePlayer,
                mpsc::Receiver<PlayerEvent>,
            ),
            String,
        > {
            CALLS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(mbv_core::remote_player::RemotePlayer::stub(Vec::new(), 0))
        }

        *DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = Some(counting_connect);
        let _app = make_app_stub();
        *DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = None;

        assert_eq!(CALLS.load(std::sync::atomic::Ordering::SeqCst), 0);
    }
```

- [ ] **Step 2: Run the test to verify it fails or passes as expected**

Run: `cargo test -p mbv app_construction_never_attempts_a_daemon_route_connect -- --exact`
Expected: PASS immediately (no code change needed — `make_app_stub()`/`App::new`/`App::new_remote`/`App::build` have no call to `try_daemon_route_connect` or `connect_daemon_route_endpoint`). If this fails, something in the current tree already wires an eager connect and must be investigated before continuing — do not proceed to Task 3 with this test failing or removed.

- [ ] **Step 3: Commit**

```bash
git add src/app/mod.rs
git commit -m "test: pin down no-eager-daemon-route-connect invariant at App construction (#222)"
```

---

### Task 3: New ADR documenting the connect lifecycle rules

**Files:**
- Create: `docs/adr/0010-lazy-daemon-route-connect-lifecycle.md`

**Interfaces:**
- Consumes: ADR 0003 (`docs/adr/0003-exclusive-ctrl-connection.md`), ADR 0007 (`docs/adr/0007-daemon-control-authority.md`) — cross-referenced, not modified.
- Produces: a durable decision record for #223 (and any future route-adding issue) to cite instead of re-deciding these rules.

- [ ] **Step 1: Create the ADR file**

```markdown
# Lazy Daemon Route Connect: Fallback, No Retry, No Parking

## Decision

A daemon-route connect attempt (the general "route everything" wildcard case
in #222, and the per-library case #223 builds on top of it) follows four
rules, distinct from the existing startup-only `--connect-daemon` /
`daemon_client_endpoint` thin-client path (`main.rs`'s `explicit_daemon_endpoint`
branch), which is unaffected by this ADR:

1. **Lazy connect.** mbv never attempts a daemon-route connection at
   startup. The first play/enqueue action that resolves to a configured
   route is what triggers the first connect attempt.
2. **Fallback, not hard-fail.** A failed connect attempt falls back to (or
   stays on) local playback. It never hard-fails or exits the process. The
   raw failure is always logged (`log::warn!`, `target: "daemon_route"`).
   `App::try_daemon_route_connect` returns a fully-formatted, ready-to-
   display status-bar warning as its `Err` payload rather than flashing it
   directly -- the caller decides how to surface it (a direct
   `App::flash_status_high`, or threaded through a state-teardown path),
   since the right *mechanism* depends on caller-side state this primitive
   deliberately knows nothing about (e.g. #223's `active_route`). Every
   call site is still required to surface it somehow: `Result`'s `Err` arm
   must be handled, so the warning can't be silently dropped by omission.
3. **No background retry.** A failed attempt schedules nothing further. The
   next attempt happens only on the next natural trigger — the next
   play/enqueue action that resolves to that route (in practice, for the
   wildcard case, the next mbv restart).
4. **No connection parking.** When mbv swaps away from a daemon route back
   to local, it disconnects cleanly — the `RemotePlayer` is dropped, not
   parked the way `SuspendedLocalSession` parks a local `Player` during a
   direct-remote takeover. The next time that route is needed, it
   reconnects fresh.

Connecting to a daemon route **is** taking that daemon's driving-client
authority (ADR 0003, ADR 0007) — an accepted, explicit consequence, not a
hidden side effect. This matters most for rule 4: a daemon route connection
is not something mbv should hold open "just in case," because doing so
continues to occupy that daemon's single ctrl-connection slot (ADR 0003)
even while the route sits idle.

## Context

Issue #222 replaced mbv's only pre-existing daemon-connect failure mode —
`explicit_daemon_endpoint`'s hard error + `std::process::exit(1)` — with a
resilient model for a *new*, separate mechanism: routing ordinary play
actions to a configured daemon rather than the local `Player`. That new
mechanism needed its own connect-timing and failure-handling rules, because
copying the explicit-endpoint path's hard-fail-at-startup behavior would be
wrong for a mechanism that, per #223, may have many possible routes (one per
library) rather than one fixed endpoint decided once at launch.

The four rules above were settled in the design that produced #222 before
this ADR was written; #222's issue body treats them as already-decided
scope, not open questions. This ADR exists to give them a durable home
outside a single issue body, mirroring why ADR 0003 exists (see that ADR's
"Why this ADR exists" section) — so a future doc edit cannot quietly narrow
or reverse them without leaving a trace.

Implementation: `App::try_daemon_route_connect` /
`App::connect_daemon_route_endpoint` in `src/app/mod.rs` (issue #222). No
production call site exists yet — #223 wires the actual play/enqueue
trigger and the per-library swap (a sibling to `switch_to_direct_remote` /
`restore_local_mode`) that calls this primitive. Both methods carry a
scoped `#[allow(dead_code)]` until that call site lands (see the doc
comments on each in `src/app/mod.rs`) -- this repo's "fix all compile
warnings, never `#[allow(unused)]`" convention (`mem:conventions`) is
deliberately overridden in this one, narrow, self-documenting case, not
silently worked around.

## Consequences

- `App::try_daemon_route_connect` is the one place connect/fallback/
  no-retry logic and warning-message formatting live; #223's per-library
  swap function must call it and surface its `Err(message)` rather than
  re-implementing connect/fallback logic or re-deriving the warning
  wording inline. The primitive does not flash the warning itself --
  #223's swap function owns *where* it gets displayed (see rule 2 above),
  but not *what* it says.
- A daemon-route `RemotePlayer` is never stored anywhere that outlives the
  swap that created it (no new `SuspendedLocalSession`-style parking
  struct for daemon routes). #223's swap-back path should simply let the
  `RemotePlayer` drop.
- Multiple devices routing to the same music-only `mbvd` (the scenario
  #223's design doc calls out) will see ordinary ADR 0003 eviction
  behavior — the newest connecting client takes over, the previous one is
  evicted with the existing structured disconnect event. This ADR does not
  change that; it only makes explicit that *initiating* a route connect is
  choosing to trigger it.
- The existing bounded intra-attempt retry loop for `DaemonEndpoint::Local`
  in `DaemonEndpoint::connect_stream` (`crates/mbv-core/src/remote_player.rs`,
  `LOCAL_DAEMON_CONNECT_RETRY_TIMEOUT`/`LOCAL_DAEMON_CONNECT_RETRY_INTERVAL`)
  is unrelated to rule 3 above and is unaffected: it waits out a
  same-machine daemon's own startup race *within* one connect attempt, not
  a retry *after* a whole attempt already failed.
```

- [ ] **Step 2: Commit**

```bash
git add docs/adr/0010-lazy-daemon-route-connect-lifecycle.md
git commit -m "docs: add ADR 0010 for lazy daemon-route connect lifecycle (#222)"
```

---

### Task 4: `CONTEXT.md` glossary updates

**Files:**
- Modify: `CONTEXT.md` (Daemon/TUI control seam section, after the existing **Local daemon** entry and before **Daemon responsibility boundary**, currently around line 135-137)

**Interfaces:**
- Consumes: existing terms **Thin client** (`CONTEXT.md:27-29`), **Local daemon** (`CONTEXT.md:133-135`), **Driving client** (`CONTEXT.md:121-123`), **Daemon contract** (`CONTEXT.md:125-127`), **Suspended local session** (`CONTEXT.md:31-33`). (Line numbers re-verified against current `main` during cross-review -- the original draft had **Driving client**/**Daemon contract** off by 6 lines each, pointing at **Daemon contract**'s `_Avoid_` line and **Cold daemon**'s body respectively; corrected here.)
- Produces: four new glossary terms other docs/plans (including #223's) can cite by name: **Lazy daemon route connect**, **Fallback to local playback**, **No background retry**, **No connection parking**.

- [ ] **Step 1: Insert the new glossary entries**

In `CONTEXT.md`, find this existing paragraph (end of the **Local daemon** entry):

```markdown
**Local daemon**:
A daemon deployment relationship where the daemon is running on the same machine as the TUI instance. This describes location, not whether the TUI is operating as a thin client or in the separate direct-remote-queue model.
_Avoid_: using this as a full substitute for **Thin client** — same-machine placement and queue/control semantics are different axes.
```

and insert immediately after it (before the `**Daemon responsibility boundary**` entry):

```markdown

**Lazy daemon route connect** (#222):
The connect-timing rule for the daemon-route lifecycle mbv builds beyond the existing startup-only `--connect-daemon`/`daemon_client_endpoint` **Thin client** path (unaffected — see ADR 0010): mbv never attempts a daemon connection at startup for a route. The first play/enqueue action that resolves to a configured route (the wildcard "route everything" case, or a per-library entry — see #223) is what triggers the first connect attempt, via `App::try_daemon_route_connect` (`src/app/mod.rs`).
_Avoid_: confusing this with the existing `explicit_daemon_endpoint` branch in `main.rs`, which still connects (or hard-exits) at startup and is untouched by this rule — the two are separate, additive mechanisms per #222.

**Fallback to local playback** (#222):
The on-failure behavior of a lazy daemon route connect attempt: stay on (or return to) the local `Player` rather than hard-failing/exiting. `App::try_daemon_route_connect` (`src/app/mod.rs`) always logs the raw failure (`log::warn!`) and returns a fully-formatted, ready-to-display warning as its `Err` payload (e.g. "⚠ Music route unreachable, using local playback (mbv.log)"); the caller decides how to surface it -- a direct `App::flash_status_high`, or threaded through a state-teardown path -- since only the caller knows its own routing state. Distinct from **Local daemon** — that term is about deployment location, not this failure-mode policy.
_Avoid_: treating a failed route connect as fatal, or falling back with no user-visible signal — both were true of the pre-#222 startup-time behavior this replaces for the new mechanism. Also avoid assuming `try_daemon_route_connect` itself calls `flash_status_high` — it deliberately does not (see ADR 0010).

**No background retry** (#222):
After a failed daemon route connect attempt, mbv does not schedule another attempt on a timer or in the background. The next attempt happens only on the next natural trigger — the next play/enqueue action that resolves to that route (which, for the wildcard case, in practice means the next mbv restart). Not to be confused with `DaemonEndpoint::connect_stream`'s existing bounded retry loop for `DaemonEndpoint::Local` (`crates/mbv-core/src/remote_player.rs`, `LOCAL_DAEMON_CONNECT_RETRY_TIMEOUT`), which waits out a same-machine daemon's startup race *within* one connect attempt and is unrelated/unaffected.
_Avoid_: conflating the existing intra-attempt local-daemon retry loop with this rule — this rule is about not scheduling a *new* attempt after a whole connect attempt has already failed.

**No connection parking** (#222):
When mbv swaps away from a daemon route back to local, it disconnects cleanly (drops the `RemotePlayer`, taking no action to keep it or its socket alive) rather than parking the connection the way **Suspended local session** parks a local `Player` during a direct-remote takeover. The next time that route is needed, it reconnects fresh. Chosen so mbv does not silently continue holding a daemon's **Driving client** authority (ADR 0003, ADR 0007) on a route that is not actively in use.
_Avoid_: reusing the `SuspendedLocalSession` pattern (or inventing an equivalent) to keep a disconnected daemon-route `RemotePlayer` alive for reuse — that is the "connection parking" this rule rules out.
```

- [ ] **Step 2: Verify the file still renders as valid markdown with no broken heading structure**

Run: `grep -n "^## \|^### " CONTEXT.md`
Expected: the same section headers as before the edit (`## Daemon/TUI control seam`, `### Language`, etc.) with no new `##`/`###` accidentally introduced — the four new entries must be plain `**Term**:` paragraphs like their neighbors, not new headings.

- [ ] **Step 3: Commit**

```bash
git add CONTEXT.md
git commit -m "docs: add lazy-connect/fallback/no-retry/no-parking glossary terms (#222)"
```

---

### Task 5: Full workspace verification

**Files:**
- None (verification only, no code changes).

**Interfaces:**
- Consumes: everything built in Tasks 1-4.
- Produces: confidence that the new code compiles cleanly across the workspace and that no existing test regressed.

- [ ] **Step 1: Build the full workspace**

Run: `cargo build --workspace`
Expected: builds successfully with zero warnings. `try_daemon_route_connect` (`pub(super)`) and `connect_daemon_route_endpoint` (private) are currently reachable only from `#[cfg(test)]` code, so a plain `cargo build` (which strips `#[cfg(test)]`) would otherwise warn `associated function is never used` for both -- Task 1 Step 4 already added a scoped `#[allow(dead_code)]` (with an inline comment explaining why, and when to remove it) to each, specifically to prevent this from landing as a build warning. If `cargo build --workspace` still warns about either method, the `#[allow(dead_code)]` was dropped or misplaced during Task 1 -- go back and fix Task 1's code, don't add a second suppression here. Any *other* warning is a real defect and must be fixed, per this repo's "fix all compile warnings" convention (`mem:conventions`).

- [ ] **Step 2: Run the full test suite**

Run: `cargo test --workspace`
Expected: PASS, including the three new tests from Tasks 1-2 (`try_daemon_route_connect_returns_remote_player_on_successful_connect`, `try_daemon_route_connect_returns_a_ready_to_display_warning_without_flashing_on_failure`, `app_construction_never_attempts_a_daemon_route_connect`) and all pre-existing tests (in particular `crates/mbv-core/src/remote_player.rs`'s existing suite and `src/app/mod.rs`'s existing `connect_to_session`/`switch_to_direct_remote` suite, to confirm the new `DAEMON_ROUTE_CONNECT_OVERRIDE` statics did not collide with or destabilize the existing `DIRECT_CONNECT_OVERRIDE` ones).

- [ ] **Step 3: Run clippy**

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: no new lint failures attributable to this plan's changes (`src/app/mod.rs`, `docs/adr/0010-lazy-daemon-route-connect-lifecycle.md`, `CONTEXT.md`). `--all-targets` compiles test code too, so `dead_code` is a non-issue here regardless (the primitive is reachable from `#[cfg(test)]` in this invocation) -- clippy should be clean with zero special-casing needed. If clippy flags `#[allow(dead_code)]` itself (e.g. `unused_attributes` because the code turns out to be reachable some other way you didn't anticipate), that means the attribute is unnecessary in this exact codebase configuration -- remove it and re-run Step 1's plain `cargo build --workspace` to confirm dead_code still doesn't fire without it before deciding it's safe to drop.

No commit for this task — it verifies the commits already made in Tasks 1-4.

---

### Task 6: Remove the legacy `--connect-daemon` / `daemon_client_endpoint` startup path

**Added in post-grilling review (2026-07-17), superseding Global Constraint 1 above.** The user has never used `--connect-daemon` in practice; keeping it around only to preserve an unused startup path is not worth the maintenance surface, and its removal also makes #223's design-item-8 question (precedence between this flag and the new `daemon_routes."*"` wildcard) moot — there is nothing left to conflict with. This task removes the flag entirely rather than deprecating it in place.

**Files:**
- Modify: `src/main.rs` — remove `connect_daemon_arg` (lines 41-55), `run_remote_app` (lines 15-39, only caller is the block being removed), the `cli_daemon_endpoint` binding and its match (lines 171-177), the `explicit_daemon_endpoint` construction (lines 235-245) and the `if let Some(endpoint) = explicit_daemon_endpoint { ... }` branch (lines 259-278), the `--connect-daemon` line in `print_usage` (lines 187-189), the stale "No `--connect-daemon` can be present here" comment near the stay-alive inferior-argv block (~line 336), and the two unit tests `connect_daemon_arg_accepts_split_and_equals_forms` / `connect_daemon_arg_requires_value`.
- Modify: `crates/mbv-core/src/config.rs` — remove `Config.daemon_client_endpoint` (struct field, `Default` impl, and its `[daemon.client] endpoint` TOML parsing in `parse_config`). A `[daemon.client]` section left over in an existing user's `config.toml` becomes inert (silently ignored, like any other unrecognized TOML key already is in this parser) — no migration warning, consistent with how this parser already treats unknown keys.
- Modify: `README.md`, `CONTEXT.md` — remove usage/glossary mentions of `--connect-daemon` / `daemon_client_endpoint`.
- Modify (light touch, not a rewrite): `docs/adr/0006-single-instance-flock-and-socket-detection.md` line 26 and this plan's own ADR 0010 (`docs/adr/0010-lazy-daemon-route-connect-lifecycle.md`) lines 7-8 — both mention `--connect-daemon` only in passing, as context for other decisions, not as their subject. Per explicit user direction, these are edited in place to drop the stale reference rather than superseded with a new ADR — the flag's presence in those docs was itself incidental noise from when the plans were drafted, not a decision worth preserving a historical trail for.

**Interfaces:**
- Removes: `main.rs::connect_daemon_arg`, `main.rs::run_remote_app`, `Config.daemon_client_endpoint`. No new public interface — this is a pure removal.
- After this task, `remote_player::RemotePlayer::connect_endpoint` and `remote_player::DaemonEndpoint::parse` remain in use (Sessions-panel `connect_direct_endpoint`, and this plan's own `connect_daemon_route_endpoint` from Task 1) — only the startup-flag call site goes away.

- [ ] **Step 1: Remove the CLI/config code path** in `src/main.rs` and `crates/mbv-core/src/config.rs` as scoped above.
- [ ] **Step 2: Update docs** (`README.md`, `CONTEXT.md`, the two ADR passing-mentions) to drop the removed flag.
- [ ] **Step 3: Run `cargo build --workspace` and `cargo test --workspace`** — expect a clean build (no dead-code warnings from the removal) and all remaining tests passing; the two flag-parser tests are deleted, not left failing.
- [ ] **Step 4: Run `cargo clippy --workspace --all-targets -- -D warnings`** — expect clean.
- [ ] **Step 5: Commit**

```bash
git add src/main.rs crates/mbv-core/src/config.rs README.md CONTEXT.md docs/adr/0006-single-instance-flock-and-socket-detection.md docs/adr/0010-lazy-daemon-route-connect-lifecycle.md
git commit -m "remove: legacy --connect-daemon / daemon_client_endpoint startup path (unused)"
```

---

## Self-Review

**1. Spec coverage against #222's acceptance criteria:**

- "A failed daemon connect attempt ... falls back to local playback instead of hard-failing/exiting." → Task 1 (`try_daemon_route_connect` returns `Err(message)` and never calls `std::process::exit`).
- "The fallback is surfaced via a status-bar warning, and logged." → Task 1: the raw failure is always logged (`log::warn!`), and a ready-to-display warning is always returned via `Err`, so every call site is forced (by the `Result` type) to handle and surface it -- the actual `flash_status_high` call happens at the caller (deliberately, so #223 can route it through a state-teardown path instead when needed; see Architecture and ADR 0010 rule 2). Task 1's failure test asserts the exact `Err` message text and that `App::status` is untouched by the primitive itself.
- "No connection attempt happens before the first play/enqueue action that needs one." → Task 1's primitive has no eager caller; Task 2 pins this down as an explicit regression test.
- "No background retry; failure is retried only on the next natural trigger." → Documented as a Global Constraint, in the ADR (Task 3), and structurally true since `try_daemon_route_connect` schedules nothing — there is no timer, thread, or loop anywhere in its implementation.
- "`--connect-daemon` / `daemon_client_endpoint` behavior is unchanged." → `main.rs` is never touched by this plan (verified by reading `main()` during research; no task modifies it).
- "Docs impact: `CONTEXT.md` ... New ADR (or amendment to ADR 0007)." → Tasks 3 and 4.
- Design note ("connecting takes driving-client authority ... explicit consequence, not a hidden side effect") → logged in `connect_daemon_route_endpoint` (Task 1) and stated in ADR 0010 (Task 3).
- Reusability for #223 (not hardcoded to the single global endpoint case) → `try_daemon_route_connect` takes `endpoint: &DaemonEndpoint` and `route_label: &str` as parameters rather than assuming a single wildcard case, and is a plain method #223's future per-library swap function can call directly, mirroring `switch_to_direct_remote`'s existing shape.

**2. Placeholder scan:** No "TBD"/"handle edge cases"/"similar to Task N" placeholders remain — every step shows complete, copy-pasteable code or an exact shell command with a concrete expected result.

**3. Type/signature consistency:** `try_daemon_route_connect(&mut self, endpoint: &mbv_core::remote_player::DaemonEndpoint, route_label: &str) -> Result<(RemotePlayer, mpsc::Receiver<PlayerEvent>), String>` is used identically across Task 1's implementation, Task 1's two tests, and Task 2's test (Task 2 only checks a call counter, so it is agnostic to `Option` vs `Result` and needed no change). `connect_daemon_route_endpoint`'s signature matches the existing `DirectConnectFn` type alias exactly (confirmed against `src/app/mod.rs:25-34`), which is why no new type alias was introduced -- note that `try_daemon_route_connect` itself is intentionally *not* `DirectConnectFn`-shaped (it takes an extra `route_label: &str` and is `&mut self`, not `&self`), so it was never a candidate for that alias. `DAEMON_ROUTE_CONNECT_OVERRIDE` / `DAEMON_ROUTE_CONNECT_TEST_LOCK` names are used consistently in Tasks 1 and 2. `RemotePlayer` derives only `Clone` (confirmed against `crates/mbv-core/src/remote_player.rs:33`), not `PartialEq`/`Debug` -- Task 1's failure test accounts for this by matching out the `Err` payload rather than `assert_eq!`-ing the whole `Result`.

## Open Questions / Assumptions (flag for follow-up review)

These were not fully pinned down by #222's issue body. This plan originally left them as open judgment calls; a subsequent cross-review pass (working from the actual #223 implementation plan -- library-scoped daemon routing, `docs/superpowers/plans/2026-07-17-library-scoped-daemon-routing.md`) resolved each definitively below, since #223's real call-site needs are now known rather than guessed at.

1. **Where the primitive lives -- RESOLVED: `src/app/mod.rs` is correct.** Confirmed against #223's actual plan: its per-library swap function (`App::switch_to_library_route`) is a sibling to `switch_to_direct_remote`, defined in the same file, and its orchestration function (`App::apply_route_for_playback`) calls the connect primitive directly as an `App` method. Both are App-level UI/state code, not something `mbvd` (a headless daemon with no `App`, no TUI state) would ever call. No further revisiting needed unless a genuinely new consumer (e.g. `mbvd` connecting *outbound* to another daemon) appears, which no current issue proposes.
2. **Primitive contract vs. #223's actual call site -- RESOLVED: changed `try_daemon_route_connect`'s return type from `Option<(RemotePlayer, Receiver<PlayerEvent>)>` to `Result<(RemotePlayer, Receiver<PlayerEvent>), String>`, and the primitive no longer calls `flash_status_high` itself.** Cross-checking against #223's actual `apply_route_for_playback` orchestration revealed a real mismatch: #223 needs to choose *how* to fall back depending on state the primitive has no business knowing (`active_route`) -- a plain `flash_status_high` when it was already local, versus routing the message through a `restore_local_mode`-style teardown (which itself already calls `flash_status_high`) when swapping away from a previously *active different* route. An unconditional internal flash risked a second, conflicting flash on top of that teardown path's own flash, and forced #223 to duplicate the "`{route} route unreachable...`" wording itself to keep messaging consistent. The fix: the primitive still owns computing the exact warning text and still unconditionally logs the raw error, but returns the formatted text as `Err(String)` for the caller to display via whichever mechanism fits -- exactly mirroring the precedent already set by `connect_direct_endpoint`/`connect_to_session` in this same file (`connect_direct_endpoint` also returns `Result` and lets its caller decide the message/flash). Task 1's implementation and both its tests were updated accordingly (Task 1 Step 4, Step 1 tests).
3. **Log target name -- RESOLVED: keep `target: "daemon_route"`, distinct from #223's `target: "library_route"`.** #223's plan uses its own `"library_route"` target for its own state-transition logging (`switch_to_library_route`, `restore_local_mode`, the enqueue-rejection guard) -- a deliberate, not accidental, split: `"daemon_route"` here covers only this primitive's own connect attempt/result (issue #222's scope), while `"library_route"` covers #223's higher-level routing decisions and queue-invariant enforcement (issue #223's scope). No consolidation needed; grepping `mbv.log` for either target answers a different question ("did a connect attempt happen and how did it go" vs. "why did mbv route/reject this play/enqueue").
4. **No new `disconnect_daemon_route` helper -- RESOLVED: confirmed sufficient, including for the route-to-route swap case.** Checked against #223's actual `switch_to_library_route`: when `!self.player.is_remote()` it suspends local exactly like `switch_to_direct_remote`; when already remote (i.e. swapping from one active route straight to another) it takes the simpler `else` branch -- `self.player = PlayerProxy::remote(remote, always_play_next);` -- which is a plain field reassignment. Rust drops the old `PlayerProxy` value (and the `RemotePlayer` `Arc`s/channels it owned) automatically as part of that assignment; there is no code path in #223's design that needs to explicitly tear down a daemon-route connection before replacing it. `RemotePlayer::join()` being a documented no-op confirms there's genuinely no cleanup logic being skipped. No dedicated teardown function is needed now or foreseeably.
5. **`cargo build --workspace` dead-code warning -- RESOLVED: confirmed it fires, and pre-empted it.** `try_daemon_route_connect` (`pub(super)`) and `connect_daemon_route_endpoint` (private) have no caller outside `#[cfg(test)]` code, which a plain `cargo build` (no `--tests`) strips entirely -- rustc's `dead_code` lint would fire. This repo's convention (`mem:conventions`) is "fix all compile warnings -- delete unused code, never `#[allow(unused)]`," which is in real tension with the parent brief's explicit requirement that this primitive ship with zero production call sites. Resolved in favor of keeping the primitive (deleting fully-designed, fully-tested code to dodge a warning would defeat this plan's purpose) with a narrow, explicitly-justified, temporary `#[allow(dead_code)]` on both methods (added in Task 1 Step 4), each carrying a doc comment explaining why and stating the removal condition: delete both attributes in the same change that adds #223's first call site. This is flagged as the one sanctioned exception to the "never `#[allow(unused)]`" rule in this plan, not a precedent for suppressing warnings generally.
6. **ADR numbering vs. #223's plan -- RESOLVED: this plan owns ADR 0010; #223's plan must use 0011.** At the time of writing, `docs/adr/` on `main` ends at `0009-v-key-controls-audio-visualizer.md`, so `0010` is the next free number -- and both this plan (`docs/adr/0010-lazy-daemon-route-connect-lifecycle.md`) and the independently-authored #223 plan (`docs/adr/0010-library-scoped-daemon-routing.md`) claimed it. Since #223 depends on #222 (not the reverse), this plan's ADR is logically the earlier decision and keeps `0010`; the #223 plan's ADR must be renumbered to `0011-library-scoped-daemon-routing.md` (including its own self-references and its `CONTEXT.md` task, if any) in that plan's own cross-review pass -- not edited here, per instruction to fix only this plan's file. No other collision was found between the two plans' `CONTEXT.md` glossary insertions: this plan adds **Lazy daemon route connect**, **Fallback to local playback**, **No background retry**, **No connection parking** to the *Daemon/TUI control seam* section (after **Local daemon**); #223's plan adds **Library route**/**Route table**, **Routed queue**, and amends **Suspended local session**, in the *Playback* section -- different terms, different insertion points, and #223's amendment to **Suspended local session** (noting it now has two callers) is consistent with, not contradicted by, anything in this plan.
7. **Post-grilling technical follow-ups (2026-07-19 review, process notes only — not re-applied to already-shipped code):** a later cross-review pass found two process gaps worth recording for future dependent-plan pairs: (a) sequencing relied on human memory of a cross-plan signature and was wrong once mid-review (the `Option`→`Result` change in item 2 above) — future dependent plans should pin the depended-upon signature with a compile-time assertion in the dependent plan's first consuming task, not just a prose precondition; (b) ADR numbering (item 6 above) had no reservation mechanism and collided once by luck of manual review — see the process fix added to `AGENTS.md`'s docs section (grep `docs/adr/` for the highest number against the merge-target branch at plan-authoring time, note the reservation in the plan header).
