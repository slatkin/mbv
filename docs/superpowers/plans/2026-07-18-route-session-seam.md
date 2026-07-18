# Route / Sessions Panel Seam Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make F3 disconnect act only on Sessions-panel-owned connections, so route-owned playback cannot be disconnected or de-routed by pressing `d` in F3.

**Architecture:** Preserve the existing remote transport and suspend/restore machinery. Narrow F3 action authority at the `App` caller level: `RemoteSlotState` remains display-oriented, while `can_disconnect_remote()` and `disconnect_remote()` use explicit Sessions-panel-owned state (`connected_session_*` or `direct_remote_connected`) instead of inferred remote transport shape. `direct_remote_label` is display/persistence metadata and must not be used as the authority signal.

**Tech Stack:** Rust, existing `src/app/mod.rs` app-level tests, existing `cargo test -p mbv <filter>` workflow, manual mbv runtime repro for #249.

## Global Constraints

- Follow `docs/agents/debugging-regressions.md`: confirm the product symptom and active runtime path before treating tests as proof.
- Do not refactor unrelated remote/session code.
- Do not change `restore_local_mode` semantics; it remains the broad teardown primitive.
- F3 `Enter` may still tear down an active route before connecting to the selected session.
- F3 `d` must report `No session connected` when no Sessions-panel connection is active.
- Route status rendering must remain `route:<library>` during routed playback.
- Documentation decision already exists in ADR 0012 and `docs/superpowers/specs/2026-07-18-route-session-seam-design.md`.

---

## File Structure

- Modify: `src/app/mod.rs`
  - Add a private Sessions-panel ownership predicate near `remote_slot_state()`.
  - Change `can_disconnect_remote()` and `disconnect_remote()` to use the predicate instead of `RemoteSlotState::DirectRemote`.
  - Update/add focused tests in the existing remote/session test cluster around `remote_slot_state_*`, `switch_to_library_route_*`, and `disconnect_remote_*`.
- No docs changes are required in the implementation task unless the code reveals a contradiction with ADR 0012.

---

### Task 1: Pin F3 Disconnect Ownership In App Tests

**Files:**
- Modify: `src/app/mod.rs`

**Interfaces:**
- Consumes: existing test helpers `make_app_stub()`, `make_remote_app_stub()`, `make_local_daemon_app_stub()`, `make_items()`, `make_session()`, `mbv_core::remote_player::RemotePlayer::stub()`.
- Produces: failing tests that define the implementation boundary for Task 2.

- [ ] **Step 1: Update the generic remote display test so display state no longer implies disconnect authority**

In `src/app/mod.rs`, replace the existing test named:

```rust
fn remote_slot_state_is_direct_remote_for_network_daemon_mode()
```

with:

```rust
#[test]
fn remote_slot_state_direct_remote_display_does_not_imply_sessions_panel_disconnect() {
    let app = make_remote_app_stub(make_items(2), make_items(3));

    assert_eq!(app.remote_slot_state(), RemoteSlotState::DirectRemote);
    assert!(!app.can_disconnect_remote());
    assert_eq!(
        app.sessions_overlay_footer(),
        "[↵]conn [r]refresh [Esc]close"
    );
}
```

- [ ] **Step 2: Add a regression test for route-owned transport**

Add this test near `switch_to_library_route_sets_active_route_and_suspends_local()`:

```rust
#[test]
fn route_owned_transport_is_not_sessions_panel_disconnectable() {
    let mut app = make_app_stub();
    let (remote, remote_rx) = mbv_core::remote_player::RemotePlayer::stub(make_items(1), 0);

    app.switch_to_library_route("music", remote, remote_rx);
    app.status.clear();

    assert_eq!(app.remote_slot_state(), RemoteSlotState::DirectRemote);
    assert!(!app.can_disconnect_remote());
    assert_eq!(
        app.sessions_overlay_footer(),
        "[↵]conn [r]refresh [Esc]close"
    );

    app.disconnect_remote();

    assert_eq!(app.active_route.as_deref(), Some("music"));
    assert!(app.player.is_remote());
    assert!(app.suspended_local.is_some());
    assert!(app.remote_player_tab.is_some());
    assert_eq!(app.status, "No session connected");
}
```

- [ ] **Step 3: Add a positive control for F3-created direct remote**

Add this test near `disconnect_remote_clears_attached_remote_session()`:

```rust
#[test]
fn disconnect_remote_restores_local_for_sessions_panel_direct_remote() {
    let mut app = make_app_stub();
    let (remote, remote_rx) = mbv_core::remote_player::RemotePlayer::stub(make_items(1), 0);
    let sess = make_session("music", "mbv");

    app.switch_to_direct_remote(&sess, remote, remote_rx);

    assert_eq!(app.direct_remote_label.as_deref(), Some("music"));
    assert!(app.can_disconnect_remote());

    app.disconnect_remote();

    assert!(app.direct_remote_label.is_none());
    assert!(app.active_route.is_none());
    assert!(!app.player.is_remote());
    assert_eq!(app.status, "Disconnected from direct remote session");
}
```

- [ ] **Step 4: Update local-daemon no-session wording**

Replace the final assertion in `disconnect_remote_does_not_exit_local_daemon_mode()`:

```rust
assert_eq!(app.status, "Local daemon mode stays connected");
```

with:

```rust
assert_eq!(app.status, "No session connected");
```

- [ ] **Step 5: Run the focused tests and verify failure**

Run:

```bash
cargo test -p mbv remote_slot_state_direct_remote_display_does_not_imply_sessions_panel_disconnect
cargo test -p mbv route_owned_transport_is_not_sessions_panel_disconnectable
cargo test -p mbv disconnect_remote_restores_local_for_sessions_panel_direct_remote
cargo test -p mbv disconnect_remote_does_not_exit_local_daemon_mode
```

Expected: at least `remote_slot_state_direct_remote_display_does_not_imply_sessions_panel_disconnect` and `route_owned_transport_is_not_sessions_panel_disconnectable` fail because `can_disconnect_remote()` still treats `RemoteSlotState::DirectRemote` as disconnectable. `disconnect_remote_does_not_exit_local_daemon_mode` should fail on the old status string.

- [ ] **Step 6: Commit the failing tests only**

```bash
git add src/app/mod.rs
git commit -m "test: pin sessions panel disconnect ownership"
```

---

### Task 2: Narrow F3 Disconnect Authority

**Files:**
- Modify: `src/app/mod.rs`

**Interfaces:**
- Consumes: Task 1 tests.
- Produces:
  - `fn has_sessions_panel_connection(&self) -> bool`
  - `fn can_disconnect_remote(&self) -> bool`
  - `fn disconnect_remote(&mut self)`
  - `direct_remote_connected: bool` as the explicit Sessions-panel ownership marker for the current direct remote transport.

- [ ] **Step 1: Add a Sessions-panel ownership predicate**

Add this helper immediately after `remote_slot_state()`:

```rust
fn has_sessions_panel_connection(&self) -> bool {
    self.connected_session_id.is_some()
        || self.connected_session_state.is_some()
        || self.direct_remote_connected
}
```

Rationale: attached Emby sessions use `connected_session_*`; F3 direct remote upgrades set `direct_remote_connected`. Route-owned transport sets `active_route` and intentionally leaves all Sessions-panel authority fields empty. `direct_remote_label` may be empty or missing for display reasons, so it cannot determine disconnect authority.

- [ ] **Step 2: Replace `can_disconnect_remote()`**

Replace the existing body:

```rust
fn can_disconnect_remote(&self) -> bool {
    !matches!(
        self.remote_slot_state(),
        RemoteSlotState::Off | RemoteSlotState::LocalDaemon
    )
}
```

with:

```rust
fn can_disconnect_remote(&self) -> bool {
    self.has_sessions_panel_connection()
}
```

- [ ] **Step 3: Replace `disconnect_remote()`**

Replace the existing `disconnect_remote()` body with:

```rust
fn disconnect_remote(&mut self) {
    if self.connected_session_id.is_some() || self.connected_session_state.is_some() {
        self.connected_session_id = None;
        self.connected_session_state = None;
        self.session_miss_count = 0;
        self.remote_pos_s = 0;
        self.flash_status("Disconnected from remote session".to_string());
    } else if self.direct_remote_connected {
        self.restore_local_mode("Disconnected from direct remote session");
    } else {
        self.flash_status("No session connected".to_string());
    }
}
```

Do not call `restore_local_mode()` from the final `else` branch. That branch covers route-owned transport, local-daemon mode, and fully local playback; none is a Sessions-panel connection.

Keep `direct_remote_connected` true when an attached-session overlay is connected or disconnected on top of an existing F3-created direct remote transport. Clear it only when the direct transport is replaced, routed, explicitly disconnected, or restored to local mode.

- [ ] **Step 3a: Add transition coverage for attached-session overlays**

Add a focused regression test for this sequence:

```rust
#[test]
fn disconnecting_attached_session_preserves_underlying_sessions_panel_direct_remote() {
    let mut app = make_app_stub();
    let (remote, remote_rx) = mbv_core::remote_player::RemotePlayer::stub(make_items(1), 0);
    let direct = make_session("music", "mbv");
    let attached = make_session("phone", "Emby");

    app.switch_to_direct_remote(&direct, remote, remote_rx);
    assert!(app.direct_remote_connected);
    assert!(app.can_disconnect_remote());

    app.connect_to_session(&attached);
    assert!(app.direct_remote_connected);
    assert!(app.connected_session_id.is_some() || app.connected_session_state.is_some());
    assert!(app.can_disconnect_remote());

    app.disconnect_remote();
    assert!(app.direct_remote_connected);
    assert!(app.player.is_remote());
    assert!(app.can_disconnect_remote());

    app.disconnect_remote();
    assert!(!app.direct_remote_connected);
    assert!(!app.player.is_remote());
}
```

Also retain coverage that F3 direct remote remains disconnectable when the daemon reports an empty device name; that is the regression guard proving the authority bit is not inferred from `direct_remote_label`.

- [ ] **Step 4: Run the focused test set**

Run:

```bash
cargo test -p mbv remote_slot_state_direct_remote_display_does_not_imply_sessions_panel_disconnect
cargo test -p mbv route_owned_transport_is_not_sessions_panel_disconnectable
cargo test -p mbv disconnect_remote_restores_local_for_sessions_panel_direct_remote
cargo test -p mbv disconnecting_attached_session_preserves_underlying_sessions_panel_direct_remote
cargo test -p mbv disconnect_remote_does_not_exit_local_daemon_mode
cargo test -p mbv disconnect_remote_clears_attached_remote_session
cargo test -p mbv attached_session_state_wins_over_local_daemon_indicator
```

Expected: all selected tests pass.

- [ ] **Step 5: Run the containing app test target**

Run:

```bash
cargo test -p mbv disconnect_remote
cargo test -p mbv remote_slot_state
cargo test -p mbv switch_to_library_route
```

Expected: all selected tests pass.

- [ ] **Step 6: Run formatting and static checks**

Run:

```bash
cargo fmt --all -- --check
git diff --check
```

Expected: both commands pass.

Run:

```bash
cargo clippy --all-targets -- -D warnings
```

Expected: pass, or fail only on pre-existing warnings unrelated to `src/app/mod.rs`. If it fails, record the exact first unrelated warning in the task handoff and do not hide it.

- [ ] **Step 7: Commit the implementation**

```bash
git add src/app/mod.rs
git commit -m "fix: keep f3 disconnect scoped to sessions"
```

---

### Task 3: Runtime Repro Verification

**Files:**
- No source edits expected.
- If runtime notes are captured, update the PR body or final report, not code.

**Interfaces:**
- Consumes: Task 2 implementation.
- Produces: real product evidence that #249's route/session disconnect path is fixed.

- [ ] **Step 1: Build the implementation branch binary**

Run:

```bash
cargo build -p mbv
```

Expected: build succeeds.

- [ ] **Step 2: Run the live route scenario on the user's machine**

Use the user's existing local config with `[library_routes] music = ...`.

Manual actions:

1. Launch the implementation branch's mbv binary.
2. Start playback from the Music library.
3. Confirm the status pill shows `route:music`.
4. Press F3.
5. Press `d`.
6. Confirm the route-owned playback keeps playing remotely.
7. Confirm the status/message reports `No session connected`.
8. Confirm F2 still shows `Library routes music -> ...`.
9. Confirm the feed view setting is not blanked by this scenario.

Expected: F3 `d` does not restore local playback, does not clear route state, and does not mutate feed-view config.

- [ ] **Step 3: Run the positive F3 session scenario**

Manual actions:

1. Press F3.
2. Select a real discovered session with Enter.
3. Confirm F3/session status indicates the selected session or direct remote.
4. Press F3.
5. Press `d`.

Expected: the Sessions-panel connection disconnects. If it was a direct mbv remote, local playback mode is restored. If it was an attached Emby session, attached-session fields clear and the UI reports disconnect.

- [ ] **Step 4: Final regression sweep**

Run:

```bash
cargo test -p mbv
```

Expected: all `mbv` package tests pass. If runtime verification passed but the full package test suite exposes unrelated failures, report exact failures and do not claim full automated pass.

- [ ] **Step 5: Commit or amend only if runtime verification required wording/test adjustments**

If Task 3 required no edits, do not make an empty commit.

If it required a narrow test/status adjustment:

```bash
git add src/app/mod.rs
git commit -m "test: cover route disconnect runtime boundary"
```

---

## Self-Review

- Spec coverage: Task 1 and Task 2 cover F3 `d` not clearing `active_route`, reporting `No session connected`, preserving F3 direct disconnect, and preserving F3 `Enter` takeover semantics. Task 3 covers the required real runtime repro check.
- Placeholder scan: no incomplete markers or fill-in instructions remain; code snippets and commands are explicit.
- Type consistency: helper names and field names match existing `src/app/mod.rs` state (`connected_session_id`, `connected_session_state`, `direct_remote_connected`, `direct_remote_label`, `active_route`, `remote_slot_state`, `restore_local_mode`). `direct_remote_label` is display/persistence metadata only, not disconnect authority.
