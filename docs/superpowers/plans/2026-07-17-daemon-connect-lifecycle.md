# Daemon Connect Lifecycle Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give mbv a lazy, fallback-with-warning, no-retry, no-parking daemon-route connect primitive (issue #222) that a future per-library routing trigger (issue #223) can call from inside a suspend/connect/restore swap, without wiring any UI trigger itself.

**Architecture:** Add a small, independently testable pair of `App` methods in `src/app/mod.rs`, modeled directly on the existing `connect_direct_endpoint` / `DIRECT_CONNECT_OVERRIDE` test-injection pattern used by the Sessions-panel "Direct Remote" upgrade (`connect_to_session`, `switch_to_direct_remote`): a thin connect wrapper (`connect_daemon_route_endpoint`, with its own `DAEMON_ROUTE_CONNECT_OVERRIDE` test seam, logging that a successful connect takes driving-client authority) and a fallback-aware caller (`try_daemon_route_connect`) that returns `Option<(RemotePlayer, Receiver<PlayerEvent>)>` on success, or flashes a high-priority status-bar warning and returns `None` on failure — with no retry scheduled anywhere. The existing `--connect-daemon` / `daemon_client_endpoint` startup path in `main.rs` is untouched. No config schema changes are needed for this issue (routing config is #223's addition). Document the lifecycle rules in a new ADR and in `CONTEXT.md` so #223 can build its per-library swap function on top of vocabulary and primitives that already exist.

**Tech Stack:** Rust (2021 edition), `cargo test` workspace (`mbv` binary crate at repo root, `mbv-core` lib crate, `mbvd` daemon crate), std `mpsc`/`Mutex`/`AtomicBool` — no external test or mocking framework.

## Global Constraints

- `main.rs`'s `explicit_daemon_endpoint` branch (`--connect-daemon` / config `daemon_client_endpoint`) must not be modified or behaviorally changed — issue #222 states this path is unaffected.
- No startup-time daemon connection may be introduced anywhere. The new primitive must have zero production call sites in this plan (the trigger is #223's job) and must not be invoked from `App::new`, `App::new_remote`, or `App::build`.
- On a failed connect attempt: fall back to (or stay on) local playback, surface a status-bar warning via the existing `flash_status_high` mechanism (`src/app/actions.rs:2241-2246`, 5s expiry), and `log::warn!` the failure. Never hard-fail/exit.
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
- Consumes: `mbv_core::remote_player::{DaemonEndpoint, RemotePlayer}` (existing), `PlayerEvent` (existing import at `src/app/mod.rs:140`), `App::flash_status_high` (`src/app/actions.rs:2241-2246`), the existing `DirectConnectFn` type alias (`src/app/mod.rs:25-34`).
- Produces: `App::connect_daemon_route_endpoint(&self, endpoint: &DaemonEndpoint, auth_token: &str) -> Result<(RemotePlayer, mpsc::Receiver<PlayerEvent>), String>` (private) and `App::try_daemon_route_connect(&mut self, endpoint: &DaemonEndpoint, route_label: &str) -> Option<(RemotePlayer, mpsc::Receiver<PlayerEvent>)>` (`pub(super)`) — later tasks and issue #223's swap function call `try_daemon_route_connect`. Also `#[cfg(test)] static DAEMON_ROUTE_CONNECT_OVERRIDE` / `DAEMON_ROUTE_CONNECT_TEST_LOCK`, used by Task 2's regression test.

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
        assert!(result.is_some());
    }

    #[test]
    fn try_daemon_route_connect_falls_back_to_local_and_flashes_warning_on_failure() {
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
        assert!(result.is_none());
        assert_eq!(
            app.status,
            "\u{26a0} Music route unreachable, using local playback (mbv.log)"
        );
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
    /// returns the connected `RemotePlayer` and its event receiver for the
    /// caller to swap in (mirroring `switch_to_direct_remote`'s shape). On
    /// failure, per #222: falls back to (stays on) local playback, surfaces
    /// a status-bar warning via `flash_status_high`, and logs the failure --
    /// and schedules no retry. The caller is expected to try again only on
    /// its own next natural trigger (e.g. the next play/enqueue into this
    /// route), never from a background timer.
    pub(super) fn try_daemon_route_connect(
        &mut self,
        endpoint: &mbv_core::remote_player::DaemonEndpoint,
        route_label: &str,
    ) -> Option<(
        mbv_core::remote_player::RemotePlayer,
        mpsc::Receiver<PlayerEvent>,
    )> {
        let auth_token = self.client.lock().unwrap().token.clone();
        match self.connect_daemon_route_endpoint(endpoint, &auth_token) {
            Ok((remote, remote_rx)) => Some((remote, remote_rx)),
            Err(e) => {
                log::warn!(
                    target: "daemon_route",
                    "daemon route connect failed for route={route_label:?} endpoint={endpoint}: {e}"
                );
                self.flash_status_high(format!(
                    "\u{26a0} {route_label} route unreachable, using local playback (mbv.log)"
                ));
                None
            }
        }
    }
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p mbv try_daemon_route_connect -- --exact`
Expected: PASS — both `try_daemon_route_connect_returns_remote_player_on_successful_connect` and `try_daemon_route_connect_falls_back_to_local_and_flashes_warning_on_failure` pass.

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
   failure is surfaced via a status-bar warning (`App::flash_status_high`)
   and a `log::warn!` line.
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
`restore_local_mode`) that calls this primitive.

## Consequences

- `App::try_daemon_route_connect` is the one place fallback + warning +
  no-retry logic lives; #223's per-library swap function must call it
  rather than re-implementing connect/fallback logic inline.
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
- Consumes: existing terms **Thin client** (`CONTEXT.md:27-29`), **Local daemon** (`CONTEXT.md:133-135`), **Driving client** (`CONTEXT.md:127-128`), **Daemon contract** (`CONTEXT.md:129-130`), **Suspended local session** (`CONTEXT.md:31-33`).
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
The on-failure behavior of a lazy daemon route connect attempt: stay on (or return to) the local `Player` rather than hard-failing/exiting, and surface a status-bar warning (`App::flash_status_high`, e.g. "⚠ Music route unreachable, using local playback (mbv.log)") plus a `log::warn!` line. Distinct from **Local daemon** — that term is about deployment location, not this failure-mode policy.
_Avoid_: treating a failed route connect as fatal, or falling back with no user-visible signal — both were true of the pre-#222 startup-time behavior this replaces for the new mechanism.

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
Expected: builds successfully with no errors. `try_daemon_route_connect` is `pub(super)` and `connect_daemon_route_endpoint` is a private method it calls, and both are currently reachable only from `#[cfg(test)]` code — a plain `cargo build` (without compiling tests) may therefore warn that they are never used. If so, confirm the warning names only these two new methods and nothing else; this is expected and acceptable per this plan's explicit scope (a primitive with no UI trigger yet — that's #223's job), not a defect to silently work around.

- [ ] **Step 2: Run the full test suite**

Run: `cargo test --workspace`
Expected: PASS, including the three new tests from Tasks 1-2 (`try_daemon_route_connect_returns_remote_player_on_successful_connect`, `try_daemon_route_connect_falls_back_to_local_and_flashes_warning_on_failure`, `app_construction_never_attempts_a_daemon_route_connect`) and all pre-existing tests (in particular `crates/mbv-core/src/remote_player.rs`'s existing suite and `src/app/mod.rs`'s existing `connect_to_session`/`switch_to_direct_remote` suite, to confirm the new `DAEMON_ROUTE_CONNECT_OVERRIDE` statics did not collide with or destabilize the existing `DIRECT_CONNECT_OVERRIDE` ones).

- [ ] **Step 3: Run clippy**

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: no new lint failures attributable to this plan's changes (`src/app/mod.rs`, `docs/adr/0010-lazy-daemon-route-connect-lifecycle.md`, `CONTEXT.md`). `--all-targets` compiles test code too, so the `dead_code` risk noted in Step 1 should not reproduce here. If clippy does flag it, treat this as a real problem to resolve (most likely a short, well-commented `#[allow(dead_code)]` pointing at #223) rather than something to suppress blindly.

No commit for this task — it verifies the commits already made in Tasks 1-4.

---

## Self-Review

**1. Spec coverage against #222's acceptance criteria:**

- "A failed daemon connect attempt ... falls back to local playback instead of hard-failing/exiting." → Task 1 (`try_daemon_route_connect` returns `None` and never calls `std::process::exit`).
- "The fallback is surfaced via a status-bar warning, and logged." → Task 1 (`flash_status_high` + `log::warn!`), test asserts exact status text.
- "No connection attempt happens before the first play/enqueue action that needs one." → Task 1's primitive has no eager caller; Task 2 pins this down as an explicit regression test.
- "No background retry; failure is retried only on the next natural trigger." → Documented as a Global Constraint, in the ADR (Task 3), and structurally true since `try_daemon_route_connect` schedules nothing — there is no timer, thread, or loop anywhere in its implementation.
- "`--connect-daemon` / `daemon_client_endpoint` behavior is unchanged." → `main.rs` is never touched by this plan (verified by reading `main()` during research; no task modifies it).
- "Docs impact: `CONTEXT.md` ... New ADR (or amendment to ADR 0007)." → Tasks 3 and 4.
- Design note ("connecting takes driving-client authority ... explicit consequence, not a hidden side effect") → logged in `connect_daemon_route_endpoint` (Task 1) and stated in ADR 0010 (Task 3).
- Reusability for #223 (not hardcoded to the single global endpoint case) → `try_daemon_route_connect` takes `endpoint: &DaemonEndpoint` and `route_label: &str` as parameters rather than assuming a single wildcard case, and is a plain method #223's future per-library swap function can call directly, mirroring `switch_to_direct_remote`'s existing shape.

**2. Placeholder scan:** No "TBD"/"handle edge cases"/"similar to Task N" placeholders remain — every step shows complete, copy-pasteable code or an exact shell command with a concrete expected result.

**3. Type/signature consistency:** `try_daemon_route_connect(&mut self, endpoint: &mbv_core::remote_player::DaemonEndpoint, route_label: &str) -> Option<(RemotePlayer, mpsc::Receiver<PlayerEvent>)>` is used identically across Task 1's implementation, Task 1's two tests, and Task 2's test. `connect_daemon_route_endpoint`'s signature matches the existing `DirectConnectFn` type alias exactly (confirmed against `src/app/mod.rs:25-34`), which is why no new type alias was introduced. `DAEMON_ROUTE_CONNECT_OVERRIDE` / `DAEMON_ROUTE_CONNECT_TEST_LOCK` names are used consistently in Tasks 1 and 2.

## Open Questions / Assumptions (flag for follow-up review)

These were not fully pinned down by #222's issue body and required a judgment call during planning:

1. **Where the primitive lives.** The issue doesn't say whether the lifecycle primitive should live in `mbv-core` (reusable by `mbvd` too) or in `src/app` (TUI-only). I placed it in `src/app/mod.rs` because #223's design doc explicitly says the per-library swap mechanism it will build on top of this "extends the existing suspend/restore machinery ... in `src/app/mod.rs`" — i.e., the consumer is App-level UI code, mirroring `switch_to_direct_remote`. If a future issue needs `mbvd` itself to lazily connect to another daemon, this placement would need revisiting.
2. **Exact warning message wording for the wildcard case.** The issue's example message is library-specific ("⚠ Music route unreachable..."). Since #222 introduces no actual trigger (and thus no wildcard-specific caller), I designed `try_daemon_route_connect` to take a `route_label: &str` parameter rather than hardcoding "Music" or inventing wording for the wildcard case — the caller (#223, or a future wildcard trigger) decides the label. This was a judgment call to keep the primitive generic per the parent brief's explicit instruction.
3. **Log target name.** I used `target: "daemon_route"` for the new log lines, distinct from the existing `target: "remote"` used in `remote_player.rs` and `target: "sessions"` used by `connect_to_session`. Not specified by the issue; chosen for grep-ability and to avoid conflating this with either existing target.
4. **No new `disconnect_daemon_route` helper.** The issue's "no connection parking" rule is satisfied structurally by never storing a daemon-route `RemotePlayer` anywhere (it simply drops when the caller's local variable goes out of scope) — `RemotePlayer::join()` is already a documented no-op ("daemon keeps running when TUI exits"). I did not add a dedicated teardown function because there is nothing for it to do beyond an ordinary drop, and inventing one with no real logic would be dead ceremony; #223's swap-back function should just let the value drop. Flagging this in case a reviewer believes an explicit named teardown step (for symmetry with `switch_to_direct_remote`/`restore_local_mode`, or for a future explicit-disconnect log line) is wanted instead.
5. **`cargo build --workspace` dead-code warning risk.** Since this plan intentionally produces a primitive with no production call site (by the parent brief's design), Task 5 Step 1 calls out that a bare `cargo build` (without compiling tests) may warn that `try_daemon_route_connect`/`connect_daemon_route_endpoint` are unused. I did not add `#[allow(dead_code)]` preemptively since I could not verify from static reading alone whether `pub(super)` plus test-only usage actually triggers the warning in this codebase's clippy/rustc configuration — flagged as a concrete thing for the plan executor to check in Task 5 and resolve (most likely by confirming it's test-reachable and therefore not warned; if a warning does appear on plain `cargo build`, the right fix is almost certainly a short `#[allow(dead_code)]` with a comment pointing at #223, not deleting the primitive).
