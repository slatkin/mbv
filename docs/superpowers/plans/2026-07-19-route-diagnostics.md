# Endpoint-Cached Library Route Diagnostics Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:test-driven-development to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make production `mbv.log` fully explain #256 route configuration, resolution, connection/fallback, auto-reconnect, and shutdown-persistence decisions without changing behavior.

**Architecture:** Preserve the existing config, F2 overlay, resolver, and app lifecycle seams. Make persistence/load outcomes explicit where errors are currently erased, initialize the logger before config parsing, and add focused `library_route`, `daemon_route`, and `auto_reconnect` events at existing branch points.

**Tech Stack:** Rust, `log`, existing `AppLog`, TOML/JSON config helpers, existing app test seams.

## Global Constraints

- #256 endpoint-cached semantics are authoritative: `[library_routes]` values are `tcp://host:port`; play/enqueue resolution is a pure config read with no `/Sessions` lookup or endpoint rediscovery.
- Preserve #222 fallback-to-local behavior and its user-visible high-priority warning on explicitly configured route connection failure.
- Diagnostics and persistence failures remain non-fatal.
- Production lifecycle evidence uses `info` and `warn`, not debug-only logs.
- Never log authentication tokens or media URLs.
- Do not add an ADR; this change documents and diagnoses existing decisions.
- Unit tests do not complete the issue: perform the manual #256 flow after implementation.

---

### Task 1: Add complete route and reconnect diagnostics

**Files:**
- Modify: `src/main.rs`
- Modify: `src/config.rs`
- Modify: `crates/mbv-core/src/config.rs`
- Modify: `src/app/render/overlays/library_routes.rs`
- Modify: `src/app/library_route.rs`
- Modify: `src/app/actions.rs`
- Modify: `src/app/mod.rs`
- Modify: `CONTEXT.md`
- Modify: `dist/config.toml`

**Interfaces:**
- `save_config_settings(&Config)` must expose write failure to callers rather than returning `()` and suppressing it.
- reconnect-state loading must distinguish a missing file from a read/parse failure so startup can log the correct decision.
- reconnect-state saving must expose create/write/rename/remove failures to teardown while leaving them non-fatal.
- Existing route resolver return values, retry policy, and user-visible fallback strings remain unchanged.

- [ ] **Step 1: Add failing persistence-outcome tests**

In `crates/mbv-core/src/config.rs`, update/add tests proving:

```rust
assert!(save_last_remote_connection(Some(&conn)).is_ok());
assert_eq!(load_last_remote_connection().unwrap(), Some(conn));
assert!(save_last_remote_connection(None).is_ok());
```

Add a path-parameterized private helper used by tests so a directory placed where the destination file should be produces an `Err` for save/remove/read. Assert the error text names both the failed filesystem operation and path. Add the same kind of path seam for `save_config_settings` and prove write/rename failure is returned.

- [ ] **Step 2: Verify the persistence tests fail for erased errors**

Run:

```bash
cargo test -p mbv-core config::tests::save_last_remote_connection -- --nocapture
cargo test -p mbv-core config::tests::load_last_remote_connection -- --nocapture
cargo test -p mbv-core config::tests::save_config_settings -- --nocapture
```

Expected: compilation or assertion failure because the helpers currently return `()`/`Option` and discard filesystem errors.

- [ ] **Step 3: Return explicit persistence and load outcomes**

Refactor private path-taking helpers around the current atomic-write flow and expose these public shapes:

```rust
pub fn save_config_settings(cfg: &Config) -> Result<(), String>;
pub fn save_last_remote_connection(
    conn: Option<&LastRemoteConnection>,
) -> Result<(), String>;
pub fn load_last_remote_connection() -> Result<Option<LastRemoteConnection>, String>;
```

Use `map_err` at create-directory, serialize, write, rename, read, and removal operations with operation and path in each message. Treat `NotFound` as `Ok(None)` when loading and `Ok(())` when clearing. On corrupt JSON, attempt removal; include a removal failure in the returned error rather than suppressing it. Update `src/config.rs` wrappers and every caller. Callers log failures at `warn` and keep current non-fatal behavior.

- [ ] **Step 4: Make startup config warnings durable**

In `src/main.rs`, initialize `applog` before `load_config()` using `config::is_system_instance()` and `state_dir().join("mbv.log")`. Do not initialize it a second time later. After successful load, log at `info`:

```text
config loaded: auto_reconnect=<bool> library_routes=<count> entries=[<library>=<endpoint>, ...]
```

Sort route entries before formatting for deterministic diagnostics. Keep config-load terminal error behavior unchanged. Add a focused test/helper test proving formatting is deterministic and includes accepted endpoint values without unrelated configuration secrets.

- [ ] **Step 5: Add failing F2 failure-path tests**

Use existing test overrides or introduce the smallest equivalent seams for `get_views()` and `fetch_sessions_blocking()`. Add tests proving a failed F2 library fetch and failed session fetch leave an explanatory high-priority status instead of presenting an unexplained empty selector. Preserve the legitimate empty-result UI when the request succeeds with an empty vector.

- [ ] **Step 6: Verify the F2 tests fail**

Run the exact new test names with:

```bash
cargo test -p mbv --bin mbv library_routes -- --nocapture
```

Expected: FAIL because `unwrap_or_default()` currently erases both failures.

- [ ] **Step 7: Instrument F2 discovery and persistence**

In `src/app/render/overlays/library_routes.rs`:

- replace failure-erasing `unwrap_or_default()` calls in the F2 route flow with explicit matches;
- log picker open, library-fetch success/failure, session-fetch success/failure, candidate count, endpoint eligibility/rejection with device name and reason, commit/removal decision, endpoint persisted, runtime route-table count, and config-save success/failure;
- show a five-second high-priority warning for fetch and save failures;
- retain the current greyed-out unresolvable device behavior and endpoint-only persistence.

- [ ] **Step 8: Add route-resolution lifecycle diagnostics**

In `src/app/library_route.rs` and the play/enqueue callers in `src/app/actions.rs`/`src/app/mod.rs`, log enough context to reconstruct one decision:

- user action (`play`, queue replace, or enqueue) and item id/name;
- bypass because a non-library thin-client owns playback;
- route table empty;
- active-library, queue, or ancestor resolution path;
- configured library missing, accepted endpoint, or malformed endpoint (the warning must say accepted shape is `tcp://host:port`);
- ancestor cache hit, miss, expired, and successful no-library result; retain the current warning and non-caching behavior for fetch failure;
- already-active route no-op;
- no-route while local versus restoration from a prior active route.

Avoid introducing a correlation-id abstraction: the item id, action, library, route, and endpoint fields are sufficient for this single-threaded decision flow.

- [ ] **Step 9: Complete connection, fallback, reconnect, and teardown diagnostics**

In `src/app/mod.rs`:

- log daemon route attempt start, label, endpoint, success, and existing failure/fallback;
- log switch success and local restoration with previous route and reason;
- log auto-reconnect enabled/disabled, state missing/load failure/loaded variant, resolution, connection outcome, and local fallback;
- log teardown skip because launched-as-remote or disabled, then the exact save-library-route/save-direct-session/clear decision and persistence success/failure;
- retain current warning text and no-retry behavior.

- [ ] **Step 10: Correct user-facing documentation**

Update `CONTEXT.md`’s `Library route` definition and lazy-connect language to say F2 persists endpoint-cached `tcp://host:port` values, runtime resolution is a pure config read, and stale endpoints fall back locally without rediscovery. Update `dist/config.toml` with a concise commented example/help block showing:

```toml
# [library_routes]
# music = "tcp://192.0.2.10:17831"
```

State that F2 is the normal way to select a friendly device and write this endpoint.

- [ ] **Step 11: Run focused and full automated verification**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test -p mbv-core config::tests -- --nocapture
cargo test -p mbv --bin mbv library_route -- --nocapture
cargo test -p mbv --bin mbv auto_reconnect -- --nocapture
cargo test --workspace
git diff --check
```

Expected: all checks pass. If sandboxed socket/terminal tests fail with `Operation not permitted`, rerun `cargo test --workspace` with the required elevated test permission and record both outcomes.

- [ ] **Step 12: Prepare the manual verification checklist**

Do not claim manual verification from tests. Report these exact remaining human steps:

1. Start a routable mbv daemon on the target device.
2. Open F2, assign a library to the friendly target, and confirm `config.toml` contains `tcp://host:port`.
3. Play from that library and confirm `mbv.log` explains action, context, resolution, attempt, and success/fallback plus the visible warning on failure.
4. Quit while routed with `auto_reconnect = true`; confirm the log explains the persisted state operation.
5. Restart; confirm the log explains the startup gate, state load, re-resolution, connection, and outcome.
6. Repeat once with an unreachable cached endpoint and confirm fallback is visible and fully explained.

