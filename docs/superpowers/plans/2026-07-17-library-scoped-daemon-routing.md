# Library-Scoped Daemon Routing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let mbv route play/enqueue actions to a per-library daemon (`[daemon_routes]` in `config.toml`), swapping the active player to that daemon while other libraries keep playing locally — per GitHub issue #223.

**Architecture:** A new `daemon_routes: HashMap<String, String>` config table (library name, lowercased -> endpoint string) is parsed alongside the existing `hidden_libraries`/`feed_view_libraries` convention. `App` gains `daemon_routes` (copied from config), `active_route: Option<String>` (the lowercased library name currently driving playback via a routed daemon, kept separate from the Sessions-panel `connected_session_id`/`direct_remote_label` concept), and `library_route_cache: HashMap<String, Option<String>>` (per-item ancestor-lookup memoization for cross-library aggregate views). A new `switch_to_library_route` method is a sibling to the existing `switch_to_direct_remote` — same suspend-local/connect-remote shape, reusing `SuspendedLocalSession`/`self.suspended_local`, but targeting a statically configured `DaemonEndpoint` instead of a discovered `SessionInfo`. `restore_local_mode` is extended (not duplicated) to also clear `active_route`, since it is already the single "go back to local" tail for every thin-client path. Route resolution has two paths per the issue: nav-scoped (library tab already known, no network call) and ancestor-lookup (`EmbyClient::get_ancestors`, cached per item) for cross-library aggregate views (Home tab). Both play and enqueue call through a shared resolver; play performs the swap, enqueue enforces the no-mixed-route queue invariant with a rejection toast.

**Tech Stack:** Rust workspace (`crates/mbv-core` shared lib, `src/` `mbv` TUI binary), `cargo test`, plain `#[test]` functions, no mocking framework -- Tasks 8-9 reuse #222's `DAEMON_ROUTE_CONNECT_OVERRIDE` test seam in `src/app/mod.rs` for daemon-route connect attempts; the one Sessions-panel regression test added in Task 9 (Step 5) uses the pre-existing, separate `DIRECT_CONNECT_OVERRIDE` seam, deliberately kept apart from the daemon-route one.

## Global Constraints

- Case-insensitive library-name matching, same convention as `hidden_libraries`/`feed_view_libraries`: config values lowercased at parse time, lookups lowercase the query side too (`crates/mbv-core/src/config.rs`).
- `daemon_routes` is TOML-only for v1 — no config-UI/settings-panel write-back (issue #223's explicit out-of-scope list). It is still read into `Config` and copied into `App` exactly like `hidden_libraries`.
- `active_route: Option<String>` on `App` must stay independent of `connected_session_id` / `connected_session_state` / `direct_remote_label` — never read or write library-route state through those fields, and vice versa (issue #223's explicit instruction).
- Connection lifecycle for a routed daemon: lazy connect (only attempted at the first play/enqueue that resolves to that route), fallback-to-local-with-a-status-bar-warning on failure, no retry loop, no connection parking (issue #222's rules, which #223 depends on — see Assumptions below on sequencing).
- Rejection toast text for a mixed-route enqueue, verbatim from the issue: `"Can't mix libraries in a routed queue -- clear queue first"`.
- New log target for library-routing diagnostics: `"library_route"` (distinct from the existing `"sessions"` target used by `switch_to_direct_remote`/`connect_to_session`).
- `switch_to_library_route` is a new sibling method to `switch_to_direct_remote` — do not modify `switch_to_direct_remote` itself.

## Design decisions carried forward from review

This plan was cross-reviewed against issue #223's text, the actual current codebase (via Serena, not assumed), and the sibling #222 plan (`docs/superpowers/plans/2026-07-17-daemon-connect-lifecycle.md` in a separate worktree, which by the time of this review had already been written). Four points the issue text left open are resolved definitively below, and the review surfaced additional gaps (a `tab_idx` underflow panic, Sessions-panel/direct-remote conflation, a duplicate-text edit hazard, a non-`PartialEq` test assertion, a missed `AppInit` construction site) that are fixed inline in the affected tasks rather than listed here — see each task's own comments for those.

1. **#222 integration (no longer an open question):** #222's plan now exists and defines exactly the primitive this plan needs: `App::try_daemon_route_connect(&mut self, endpoint: &DaemonEndpoint, route_label: &str) -> Result<(RemotePlayer, Receiver<PlayerEvent>), String>` (`src/app/mod.rs`, #222's Task 1). It performs the lazy-connect attempt and, on failure, logs the raw error internally and returns `Err(message)` where `message` is a fully-formatted, ready-to-display status-bar warning -- deliberately *not* flashing it itself, since only the caller (this plan's `apply_route_for_playback`) knows whether to flash directly or route the message through a `restore_local_mode`-style teardown. No retry is ever scheduled -- the exact fallback/no-retry behavior #222 was built to centralize. This plan's Task 8 (`apply_route_for_playback`) calls that primitive directly instead of reusing the Sessions-panel's `connect_direct_endpoint`/`DIRECT_CONNECT_OVERRIDE` seam, and Task 9's tests use the matching `DAEMON_ROUTE_CONNECT_OVERRIDE`/`DAEMON_ROUTE_CONNECT_TEST_LOCK` seam instead -- per #222's own design note, the two connect paths (Sessions-panel direct-remote vs. daemon-route) are kept on independent test seams so neither plan's tests can silently interfere with the other's. **Sequencing dependency:** Tasks 8 and 9 in this plan cannot compile until #222's Task 1 (the `try_daemon_route_connect`/`connect_daemon_route_endpoint` methods and the `DAEMON_ROUTE_CONNECT_OVERRIDE`/`DAEMON_ROUTE_CONNECT_TEST_LOCK` statics) has been implemented and merged into this branch. Tasks 1-7 have no such dependency and can proceed independently.
2. **Sessions-panel / library-route interaction (resolved):** there are actually *two* directions to this, not one, and the codebase's current behavior only protects one of them:
   - **Library route while an Emby-remote "attached session" is active** (`connected_session_id.is_some()`): during this mode `self.player` stays local (`play_items_routed`/`play_item` route commands through `do_session_command`/the Emby websocket API, never touching `self.player`), so library routing must be skipped outright -- it must never swap `self.player` while an attached session is being driven via the Emby API.
   - **Library route while already thin-client for a reason *other* than library routing** (a Sessions-panel "Direct Remote" ctrl-socket upgrade, or local-daemon mode -- both leave `connected_session_id` as `None` but `self.player.is_remote()` `true` and `active_route` `None`): the original plan's guard (`self.connected_session_id.is_none()`) does **not** catch this case, so a play/enqueue action would have silently swapped `self.player` away from an active Sessions-panel direct-remote connection into a library route without ever clearing `direct_remote_label`, conflating the two thin-client concepts exactly as Global Constraint #15 says must never happen. Fixed in Tasks 9 and 10 by gating on `self.connected_session_id.is_some() || (self.player.is_remote() && self.active_route.is_none())` instead of `connected_session_id` alone -- this correctly still *allows* library routing to run when `active_route` is already `Some(..)` (so it can re-evaluate/swap/restore, which is its job), while skipping it whenever the current remote state belongs to a different mechanism entirely.
   - Additionally, the reverse direction was checked: `switch_to_direct_remote`'s `else` branch (taken when `self.player.is_remote()` is already `true`, i.e. exactly the "was on a library route, now upgrading via Sessions-panel" case) overwrites `self.player`/`self.player_rx` directly without going through `restore_local_mode`, so it would leave a stale `active_route` behind. Per this plan's Global Constraint (do not modify `switch_to_direct_remote` itself), the fix is at its sole caller, `connect_to_session` -- Task 9 adds `self.active_route = None;` there, immediately before the call to `switch_to_direct_remote`, so the Sessions-panel path always starts from a clean slate regardless of which internal branch `switch_to_direct_remote` takes.
3. **Enqueuing a library-root folder item itself (resolved):** `do_enqueue_folder` receives the full `MediaItem`, not just an id, so it does not need to rely solely on ancestor lookup (which correctly returns no match for a library root, since a library root has no `CollectionFolder` ancestor above it). Task 7 adds `resolve_route_for_enqueue_folder(&mut self, item: &MediaItem) -> Option<String>`, which checks `item.item_type == "CollectionFolder"` first and resolves directly via `resolve_route_for_library(&item.name)` in that case, falling back to the ancestor-lookup resolver otherwise. Task 10 wires `do_enqueue_folder` to call this instead of `route_for_item_via_ancestors` directly.
4. **Status-bar visibility (resolved, unchanged from the original plan):** satisfied by (a) reusing the existing remote-pill mechanism (`remote_status_spans` in `src/app/render/mod.rs`, already shows a label for `RemoteSlotState::DirectRemote`) to prefer `active_route` when set, and (b) `log::info!`/`log::warn!` at every swap/restore/fallback point under the `"library_route"` target (plus #222's own `"daemon_route"`-target logging inside `try_daemon_route_connect` for the connect attempt itself) -- matching the level of persistence `direct_remote_label` already gets today (a pill label, not a dedicated new divider indicator). No further design work was needed here; this item is settled, not carried forward as open.

## Post-grilling revisions (2026-07-19 review)

A follow-up adversarial review (subagent-driven grill against the merged codebase, cross-checked against #222) surfaced further gaps. Product/design calls were escalated to the user; the rest were resolved technically. Both categories are folded in as concrete task deltas below rather than left as prose commentary.

**Design decisions (user-approved):**

1. **Failed ancestor-lookup caching → don't cache failures, only cache confirmed results.** Task 7's `route_for_item_via_ancestors` must not insert into `library_route_cache` when `get_ancestors` itself errors (transient failure) — only a *successful* `get_ancestors` call (whether it finds an owning library or confirms there is none) gets cached. A failed lookup retries on the item's next play/enqueue attempt. Delta to Task 7 Step 3: split the `Err(e)` arm of the `match ancestors` block so it returns `None` directly without touching `self.library_route_cache`, instead of falling through to the shared `self.library_route_cache.insert(...)` call that follows the match today.
2. **Sessions-panel/direct-remote route-conflict bypass → leave silent, ship as designed.** No change to Task 9/10's `in_other_thin_client_mode` short-circuit — confirmed as intended behavior, not a gap.
3. **Malformed `daemon_routes` entry → startup warning banner, not log-only.** Delta to Task 4 (`resolve_route_for_library`): in addition to the existing `log::warn!`, the first malformed entry encountered should surface via `flash_status_high` once at startup (not per-lookup — `resolve_route_for_library` is called repeatedly, so gate this on a new `App` field, e.g. `daemon_routes_warned: std::cell::RefCell<std::collections::HashSet<String>>` or equivalent, to flash each bad library name only once per process). Satisfies the issue's own "effective route is visible in app status" acceptance criterion for the failure case, not just the success case Task 11 already covers.
4. **Multi-client driving-slot eviction → generic disconnect handling, ship as designed.** No distinct "kicked off" toast. The evicted client's existing generic connection-lost handling applies unchanged.
5. **Route cache staleness on mid-session library reorg → add a TTL.** Delta to Task 3 / Task 7: `library_route_cache` becomes `HashMap<String, (Option<String>, Instant)>` (or a small wrapper struct) instead of `HashMap<String, Option<String>>`, and `route_for_item_via_ancestors`'s cache-hit check must also verify `Instant::now().duration_since(cached_at) < LIBRARY_ROUTE_CACHE_TTL` before trusting the cached value, re-resolving (as a normal cache-miss) if expired. Exact TTL value to be picked during implementation (candidate: 15-30 minutes) — not fixed by this review.
6. **Reject-toast remediation → text-only, ship as designed.** No change to Task 10's toast text or behavior.
7. **Home-tab latency from the new `get_ancestors` call → accept as-is, no budget or new indicator.** No change to Task 7/8/9; reuses the existing playback-starting UI state.
8. **`--connect-daemon` vs. `daemon_routes."*"` precedence → moot.** The user has separately directed removal of the legacy `--connect-daemon` flag entirely (see #222's plan, newly-added Task 6). Once that lands, there is nothing for the wildcard route to conflict with, and no code change is needed in this plan for that reason. If #222's Task 6 has not yet landed when this plan's Tasks 8-9 are implemented, both mechanisms can coexist only in the window between merges; no interim mitigation is added here since the removal is expected to land first (no dependency ordering constraint the other direction).

**Technical resolutions (investigated against the actual codebase, not re-asserted from the plan text):**

9. **Sequencing enforcement between #222 and #223 (applies to Task 8's prerequisite note):** rewrite the prerequisite note in Task 8 from a state description ("must already exist") to an explicit instruction ("merge #222's branch into this one before starting Task 8"), and add a compile-time signature pin immediately before `apply_route_for_playback`'s body: `#[cfg(test)] const _: fn() = || { let _: fn(&mut App, &mbv_core::remote_player::DaemonEndpoint, &str) -> Result<(mbv_core::remote_player::RemotePlayer, mpsc::Receiver<PlayerEvent>), String> = App::try_daemon_route_connect; };`. This fails the build immediately if #222's branch merges in with a drifted signature, rather than relying on a human re-reading #222's plan file a third time (which is exactly what happened during this plan's original cross-review — see "Design decisions carried forward from review" item 1 above).
10. **Queue-tab fallback (`resolve_route_for_play`, `tab_idx == 1` branch) — confirmed already covered, no change.** Task 7's own test `resolve_route_for_play_does_not_panic_from_the_queue_tab` already exercises both the no-route and already-routed cases. No gap.
11. **Duplicated thin-client-mode boolean logic — consolidate into one method.** The expression `self.connected_session_id.is_some() || (self.player.is_remote() && self.active_route.is_none())` appears verbatim at three call sites across Tasks 9 and 10 (`play_item`, `play_items_routed`, and inline inside `enqueue_route_conflict` as `in_other_thin_client_mode`). Delta: add to Task 7 (alongside the other resolvers, since Tasks 8/9/10 all consume it):
   ```rust
   /// True when self.player is remote for a reason other than library
   /// routing (#223): a Sessions-panel attached session, or a non-library-
   /// route direct-remote/local-daemon connection. Library routing must
   /// never engage -- for play or enqueue -- while this is true.
   fn in_non_library_thin_client_mode(&self) -> bool {
       self.connected_session_id.is_some()
           || (self.player.is_remote() && self.active_route.is_none())
   }
   ```
   Tasks 9's `play_item`/`play_items_routed` guards and Task 10's `enqueue_route_conflict` must call `self.in_non_library_thin_client_mode()` instead of inlining the expression, so a future fix to the condition only needs to change in one place.
12. **`RemotePlayer` route-to-route swap leaks a reader thread and a half-open socket — real bug, needs a fix, not just a doc note.** Confirmed against `crates/mbv-core/src/remote_player.rs`: `RemotePlayer` has no `Drop` impl; `connect_endpoint` spawns a reader thread and a writer thread, each holding a separate `try_clone()`'d duplicate of the socket fd; `RemotePlayer::join()` (line 551) is a documented no-op. When `switch_to_library_route`'s already-remote branch (Task 5, the `else` arm) does `self.player = PlayerProxy::remote(remote, ...)`, the old `RemotePlayer` drops — closing the *writer* thread's fd duplicate, but not the *reader* thread's separate duplicate, which stays blocked in `reader.lines()` indefinitely. This contradicts ADR 0010's "disconnects cleanly... reconnects fresh" framing and is a **pre-existing bug** (also affects today's Sessions-panel `switch_to_direct_remote` already-remote branch), not something this plan introduces — but this plan's route-to-route swaps (music→video→music route flips) are the first place it gets exercised repeatedly in normal use. Delta: add a new Task 5b (or fold into Task 5) to give `RemotePlayer` a real `disconnect()` method that shuts down the shared socket (`.shutdown(Shutdown::Both)` for TCP, or equivalent) before/instead of relying on `Drop`, threading a handle to the underlying stream through to `RemotePlayer` alongside `cmd_tx`. `switch_to_library_route`'s already-remote branch (and, ideally, `switch_to_direct_remote`'s matching branch, filed as a separate fix outside this plan's scope since it's pre-existing) should call it explicitly before reassigning `self.player`. This should land before or alongside Task 5, not deferred past this plan's completion, since Task 5 is exactly what starts exercising the leak repeatedly.
13. **Config hot-reload — confirmed moot, no change needed.** `load_config()` is called once at startup only; no watcher exists anywhere in the tree. `daemon_routes` changes only take effect on restart, same as every other config value. No task in this plan needs to handle a live config change.
14. **Async/blocking connect — confirmed pre-existing, not worsened by this plan, no change needed.** `try_daemon_route_connect` (from #222) calls the same synchronous `RemotePlayer::connect_endpoint` primitive the existing Sessions-panel path already uses, bounded by the existing `DAEMON_TCP_CONNECT_TIMEOUT`/`DAEMON_HANDSHAKE_HARD_BOUND` constants. This plan triggers that same bounded block from more call sites (any library-scoped play/enqueue) but does not introduce a new or worse bound.
15. **ADR numbering reservation — process fix added to `AGENTS.md`, not to this plan's tasks.** No existing convention caught the #222/#223 ADR-0010 collision before manual review. See `AGENTS.md`'s docs section for the added process note (check the highest `docs/adr/` number against the merge-target branch, not just the authoring worktree, at plan-authoring time).

---

### Task 1: Config — `daemon_routes` table parsing

**Files:**
- Modify: `crates/mbv-core/src/config.rs:4-51` (`Config` struct), `crates/mbv-core/src/config.rs:55-97` (`impl Default for Config`), `crates/mbv-core/src/config.rs:494-773` (`parse_config`)
- Test: `crates/mbv-core/src/config.rs` `mod tests` (same file, lines ~1017+)

**Interfaces:**
- Produces: `Config.daemon_routes: std::collections::HashMap<String, String>` — library name (lowercased) -> raw daemon endpoint string, ready to be parsed by `mbv_core::remote_player::DaemonEndpoint::parse`. `"*"` is a valid key (wildcard/route-everything, consumed by a later task's resolver, not this one).

- [ ] **Step 1: Write the failing tests**

Add to `crates/mbv-core/src/config.rs`'s `mod tests` (near `parse_hidden_libraries_lowercased`, ~line 1194):

```rust
    #[test]
    fn parse_daemon_routes_lowercased_keys() {
        let toml = r#"
[server]
url = "http://host"
[daemon_routes]
Music = "tcp://musicserver.local:9000"
"*" = "unix:///run/mbvd.sock"
"#;
        let cfg = parse_config(toml).unwrap();
        assert_eq!(
            cfg.daemon_routes.get("music").map(String::as_str),
            Some("tcp://musicserver.local:9000")
        );
        assert_eq!(
            cfg.daemon_routes.get("*").map(String::as_str),
            Some("unix:///run/mbvd.sock")
        );
    }

    #[test]
    fn parse_default_daemon_routes_when_absent() {
        let toml = r#"
[server]
url = "http://host"
"#;
        let cfg = parse_config(toml).unwrap();
        assert!(cfg.daemon_routes.is_empty());
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p mbv-core parse_daemon_routes_lowercased_keys parse_default_daemon_routes_when_absent`
Expected: FAIL with `no field 'daemon_routes' on type 'Config'` (compile error).

- [ ] **Step 3: Add the field, default, and parsing**

In `Config` (after `pub feed_view_libraries: Vec<String>,` at line 43, before `pub config_version: u32,`):

```rust
    pub feed_view_libraries: Vec<String>, // libraries treated as feed view (unplayed, date-sorted)
    /// Library name (lowercased) -> daemon endpoint string, from
    /// `[daemon_routes]` (#223). Playback/enqueue resolved to one of these
    /// libraries swaps the active player to that daemon. `"*"` is a
    /// wildcard "route everything" entry (#222). TOML-only for v1 -- no
    /// settings-panel write-back, matching the `hidden_libraries` value
    /// precedent but without exposing it for in-app editing.
    pub daemon_routes: std::collections::HashMap<String, String>,
    pub config_version: u32,   // schema version for future migrations (0 = unversioned)
```

In `impl Default for Config` (after `feed_view_libraries: vec![],` at line ~89):

```rust
            feed_view_libraries: vec![],
            daemon_routes: std::collections::HashMap::new(),
            config_version: 0,
```

In `parse_config`, after the `feed_view_libraries` block (~line 737-746):

```rust
    let daemon_routes: std::collections::HashMap<String, String> = doc
        .get("daemon_routes")
        .and_then(|v| v.as_table())
        .map(|table| {
            table
                .iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.to_lowercase(), s.to_string())))
                .collect()
        })
        .unwrap_or_default();
```

And add `daemon_routes,` to the `Ok(Config { ... })` construction, immediately after `feed_view_libraries,`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p mbv-core parse_daemon_routes_lowercased_keys parse_default_daemon_routes_when_absent`
Expected: PASS (2 passed)

- [ ] **Step 5: Run the full config test module to check for regressions**

Run: `cargo test -p mbv-core config::tests`
Expected: all existing tests still PASS (struct-literal `Config { .. }` construction sites elsewhere in the file, e.g. `Default`, must compile with the new field already added in this step).

- [ ] **Step 6: Commit**

```bash
git add crates/mbv-core/src/config.rs
git commit -m "config: parse [daemon_routes] table for library-scoped daemon routing"
```

---

### Task 2: Config — `resolve_daemon_route` lookup helper

**Files:**
- Modify: `crates/mbv-core/src/config.rs` (new free function, placed near other free functions e.g. `is_system_instance`)
- Test: `crates/mbv-core/src/config.rs` `mod tests`

**Interfaces:**
- Consumes: `Config.daemon_routes: HashMap<String, String>` (Task 1).
- Produces: `pub fn resolve_daemon_route<'a>(routes: &'a HashMap<String, String>, library_name: &str) -> Option<&'a str>` — later tasks call this from `App` to get the raw endpoint string for a library name before parsing it with `DaemonEndpoint::parse`.

- [ ] **Step 1: Write the failing tests**

Add to `mod tests`:

```rust
    #[test]
    fn resolve_daemon_route_matches_case_insensitively() {
        let mut routes = std::collections::HashMap::new();
        routes.insert("music".to_string(), "tcp://musicserver.local:9000".to_string());
        assert_eq!(
            resolve_daemon_route(&routes, "Music"),
            Some("tcp://musicserver.local:9000")
        );
    }

    #[test]
    fn resolve_daemon_route_falls_back_to_wildcard() {
        let mut routes = std::collections::HashMap::new();
        routes.insert("*".to_string(), "unix:///run/mbvd.sock".to_string());
        assert_eq!(
            resolve_daemon_route(&routes, "Movies"),
            Some("unix:///run/mbvd.sock")
        );
    }

    #[test]
    fn resolve_daemon_route_prefers_exact_match_over_wildcard() {
        let mut routes = std::collections::HashMap::new();
        routes.insert("*".to_string(), "unix:///run/mbvd.sock".to_string());
        routes.insert("music".to_string(), "tcp://musicserver.local:9000".to_string());
        assert_eq!(
            resolve_daemon_route(&routes, "Music"),
            Some("tcp://musicserver.local:9000")
        );
    }

    #[test]
    fn resolve_daemon_route_returns_none_when_unconfigured() {
        let routes = std::collections::HashMap::new();
        assert_eq!(resolve_daemon_route(&routes, "Movies"), None);
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p mbv-core resolve_daemon_route`
Expected: FAIL with `cannot find function 'resolve_daemon_route' in this scope`.

- [ ] **Step 3: Implement the helper**

Add near the other free functions in `crates/mbv-core/src/config.rs` (e.g. directly above `pub fn is_system_instance`):

```rust
/// Resolves the configured daemon endpoint string for a library name
/// (#223). Matches case-insensitively (the query is lowercased before
/// lookup; `routes`' keys are already lowercased by `parse_config`), then
/// falls back to the `"*"` wildcard "route everything" entry (#222) if
/// present. Returns `None` if neither matches -- the caller stays local.
pub fn resolve_daemon_route<'a>(
    routes: &'a std::collections::HashMap<String, String>,
    library_name: &str,
) -> Option<&'a str> {
    routes
        .get(&library_name.to_lowercase())
        .or_else(|| routes.get("*"))
        .map(|s| s.as_str())
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p mbv-core resolve_daemon_route`
Expected: PASS (4 passed)

- [ ] **Step 5: Commit**

```bash
git add crates/mbv-core/src/config.rs
git commit -m "config: add resolve_daemon_route lookup helper for library routing"
```

---

### Task 3: App state — `daemon_routes`, `active_route`, `library_route_cache` fields

**Files:**
- Modify: `src/app/mod.rs` — `App` struct (~896-1095), `AppInit` struct (~1097-1127), `App::build` (~1585-1750), `App::new` (~1752-1860), `App::new_remote` (~1875-2009), `make_app_stub` test helper (~5401+)

**Interfaces:**
- Consumes: `Config.daemon_routes` (Task 1).
- Produces: `App.daemon_routes: HashMap<String, String>`, `App.active_route: Option<String>`, `App.library_route_cache: HashMap<String, Option<String>>` — all subsequent tasks read/write these.

- [ ] **Step 1: Write the failing test**

Add to `src/app/mod.rs`'s `mod tests` (near other `make_app_stub`-based smoke tests, e.g. after `remote_slot_state_is_off_for_local_only_app`):

```rust
    #[test]
    fn app_stub_starts_with_no_active_library_route() {
        let app = make_app_stub();
        assert!(app.active_route.is_none());
        assert!(app.daemon_routes.is_empty());
        assert!(app.library_route_cache.is_empty());
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test app_stub_starts_with_no_active_library_route`
Expected: FAIL with a compile error (`no field 'active_route' on type 'App'`).

- [ ] **Step 3: Add the fields**

In the `App` struct, immediately after the `hidden_latest: Vec<String>,` field (line 920):

```rust
    hidden_latest: Vec<String>,
    /// Library name (lowercased) -> daemon endpoint string, copied from
    /// `Config.daemon_routes` at startup (#223). Endpoints are parsed
    /// lazily via `DaemonEndpoint::parse` at connect time, not eagerly, so
    /// one malformed entry never blocks startup or other routes.
    daemon_routes: std::collections::HashMap<String, String>,
```

Immediately after the `suspended_local: Option<SuspendedLocalSession>,` field (line ~1043), before `force_clear`:

```rust
    suspended_local: Option<SuspendedLocalSession>,
    /// The library route currently driving playback, if any (#223):
    /// `Some(name)` holds the lowercased library name whose configured
    /// daemon is the active player target. `None` means local playback,
    /// or a Sessions-panel direct remote (`connected_session_id` /
    /// `direct_remote_label`) -- a separate concept, never conflated with
    /// this one. Fixed for the life of the current queue: a *new* queue
    /// re-evaluates it (see `apply_route_for_playback`), but enqueuing
    /// into the existing queue must match it or be rejected (see
    /// `enqueue_route_conflict`).
    active_route: Option<String>,
    /// Per-item cache of ancestor-lookup library-route resolution for
    /// cross-library aggregate views (Continue Watching/Next Up,
    /// Favorites), keyed by item id. `Some(name)` = resolved to that
    /// library (lowercased); `None` = resolved, no owning library route.
    /// Avoids a repeat `get_ancestors` round-trip for the same item
    /// within a session (#223).
    library_route_cache: std::collections::HashMap<String, Option<String>>,
```

In `AppInit`, immediately after `hidden_libraries: Vec<String>,` (line 1109):

```rust
    hidden_libraries: Vec<String>,
    daemon_routes: std::collections::HashMap<String, String>,
```

In `App::build`, immediately after `hidden_libraries: init.hidden_libraries,` (line 1603):

```rust
            hidden_libraries: init.hidden_libraries,
            daemon_routes: init.daemon_routes,
```

And immediately after `suspended_local: None,` (line ~1728):

```rust
            suspended_local: None,
            active_route: None,
            library_route_cache: std::collections::HashMap::new(),
```

In `App::new`, after `let hidden_libraries = client.config.hidden_libraries.clone();` (~line 1766):

```rust
        let hidden_libraries = client.config.hidden_libraries.clone();
        let daemon_routes = client.config.daemon_routes.clone();
```

...and add `daemon_routes,` to the `AppInit { ... }` construction, immediately after `hidden_libraries,`.

In `App::new_remote`, after `let hidden_libraries = client.config.hidden_libraries.clone();` (line 1889):

```rust
        let hidden_libraries = client.config.hidden_libraries.clone();
        let daemon_routes = client.config.daemon_routes.clone();
```

...and add `daemon_routes,` to its `AppInit { ... }` construction the same way (find the matching `hidden_libraries,` line in that function's `Self::build(AppInit { ... })` call and add `daemon_routes,` immediately after it).

In `make_app_stub` (`src/app/mod.rs` ~5401+), add to the `App { ... }` struct literal, immediately after `hidden_libraries: Vec::new(),` (line 5437):

```rust
            hidden_libraries: Vec::new(),
            daemon_routes: std::collections::HashMap::new(),
```

...and immediately after `suspended_local: None,` (line ~5559):

```rust
            suspended_local: None,
            active_route: None,
            library_route_cache: std::collections::HashMap::new(),
```

There is a **second** test helper that constructs an `App` via `AppInit`, distinct from `make_app_stub`'s raw `App { ... }` literal: `pub(crate) fn make_built_app()` (`src/app/mod.rs`, currently starting at line 5633), which calls `App::build(AppInit { ... })` directly. `AppInit` gained the new `daemon_routes` field earlier in this step, so `make_built_app`'s `AppInit { ... }` literal must also be updated or the crate will fail to compile with a "missing field `daemon_routes`" error -- `cargo build --workspace` in Step 5 below will not silently pass this over; catch it here instead of discovering it as a surprise build failure. In `make_built_app`, immediately after its own `hidden_libraries: Vec::new(),` line (currently line 5668):

```rust
            hidden_libraries: Vec::new(),
            daemon_routes: std::collections::HashMap::new(),
```

(Confirmed via GitNexus: `AppInit` is constructed from exactly three call sites in this codebase -- `App::new`, `App::new_remote`, and `make_built_app` -- all three are covered by this step; there is no fourth site anywhere else in `src/`.)

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test app_stub_starts_with_no_active_library_route`
Expected: PASS

- [ ] **Step 5: Run the full workspace build to catch any other struct-literal sites**

Run: `cargo build --workspace`
Expected: no errors. (`App` is only constructed via `build`/`make_app_stub` in this codebase per the `mem:core` convention — "New App fields: add to struct, set default in build(), add to AppInit only if constructors need different values" — but this build step is the safety net if another construction site exists.)

- [ ] **Step 6: Commit**

```bash
git add src/app/mod.rs
git commit -m "app: add daemon_routes/active_route/library_route_cache state for library routing"
```

---

### Task 4: App — `resolve_route_for_library` method

**Files:**
- Modify: `src/app/mod.rs` — new method in the `impl App` block containing `session_direct_endpoint` (~2288)

**Interfaces:**
- Consumes: `App.daemon_routes` (Task 3), `mbv_core::config::resolve_daemon_route` (Task 2), `mbv_core::remote_player::DaemonEndpoint::parse`.
- Produces: `fn resolve_route_for_library(&self, library_name: &str) -> Option<(String, mbv_core::remote_player::DaemonEndpoint)>` — later tasks (route-for-view resolvers, Task 7) call this.

- [ ] **Step 1: Write the failing tests**

Add to `mod tests` in `src/app/mod.rs`:

```rust
    #[test]
    fn resolve_route_for_library_matches_case_insensitively() {
        let mut app = make_app_stub();
        app.daemon_routes.insert(
            "music".to_string(),
            "tcp://127.0.0.1:9000".to_string(),
        );
        let resolved = app.resolve_route_for_library("Music");
        assert_eq!(
            resolved,
            Some((
                "music".to_string(),
                mbv_core::remote_player::DaemonEndpoint::Tcp("127.0.0.1:9000".parse().unwrap())
            ))
        );
    }

    #[test]
    fn resolve_route_for_library_returns_none_when_unconfigured() {
        let app = make_app_stub();
        assert_eq!(app.resolve_route_for_library("Movies"), None);
    }

    #[test]
    fn resolve_route_for_library_skips_invalid_endpoint() {
        let mut app = make_app_stub();
        app.daemon_routes.insert(
            "music".to_string(),
            "notascheme://x".to_string(),
        );
        assert_eq!(app.resolve_route_for_library("Music"), None);
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test resolve_route_for_library`
Expected: FAIL with `no method named 'resolve_route_for_library' found for struct 'App'`.

- [ ] **Step 3: Implement the method**

Add to the `impl App` block in `src/app/mod.rs`, near `session_direct_endpoint` (~line 2288):

```rust
    /// Resolves the configured daemon route for a library name (#223):
    /// looks up `daemon_routes` (exact match, then `"*"` wildcard) and
    /// parses the endpoint string. Returns `(lowercased_library_name,
    /// endpoint)` on a match with a valid endpoint. A malformed endpoint
    /// string is logged and treated as no match, rather than failing the
    /// whole app -- one bad `daemon_routes` entry never blocks other
    /// routes or local playback.
    fn resolve_route_for_library(
        &self,
        library_name: &str,
    ) -> Option<(String, mbv_core::remote_player::DaemonEndpoint)> {
        let name = library_name.trim();
        if name.is_empty() {
            return None;
        }
        let raw = mbv_core::config::resolve_daemon_route(&self.daemon_routes, name)?;
        match mbv_core::remote_player::DaemonEndpoint::parse(raw) {
            Ok(endpoint) => Some((name.to_lowercase(), endpoint)),
            Err(e) => {
                log::warn!(
                    target: "library_route",
                    "daemon_routes entry for library {name:?} has an invalid endpoint {raw:?}: {e}"
                );
                None
            }
        }
    }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test resolve_route_for_library`
Expected: PASS (3 passed)

- [ ] **Step 5: Commit**

```bash
git add src/app/mod.rs
git commit -m "app: add resolve_route_for_library helper"
```

---

### Task 5: App — `switch_to_library_route` (sibling to `switch_to_direct_remote`)

**Files:**
- Modify: `src/app/mod.rs` — new method placed immediately after `switch_to_direct_remote` (~2403)

**Interfaces:**
- Consumes: `SuspendedLocalSession` (existing, `src/app/mod.rs:889-894`), `App.suspended_local`, `App.active_route` (Task 3), `mbv_core::remote_player::RemotePlayer`.
- Produces: `fn switch_to_library_route(&mut self, library_name: &str, remote: mbv_core::remote_player::RemotePlayer, remote_rx: mpsc::Receiver<PlayerEvent>)` — Task 8's orchestration calls this.

- [ ] **Step 1: Run impact analysis before touching `SuspendedLocalSession` reuse**

Per this repo's `CLAUDE.md`, before adding a new caller of a shared symbol, check its blast radius:

```
impact({target: "SuspendedLocalSession", direction: "upstream"})
```

Expected finding (already surfaced by the indexer): only `switch_to_direct_remote` currently constructs it. Risk is LOW — this task adds a second, independent constructor site; it does not modify the struct or the existing call site.

- [ ] **Step 2: Write the failing test**

Add to `mod tests` in `src/app/mod.rs`:

```rust
    #[test]
    fn switch_to_library_route_sets_active_route_and_suspends_local() {
        let mut app = make_app_stub();
        let (remote, remote_rx) = mbv_core::remote_player::RemotePlayer::stub(make_items(1), 0);

        app.switch_to_library_route("music", remote, remote_rx);

        assert_eq!(app.active_route.as_deref(), Some("music"));
        assert!(app.player.is_remote());
        assert!(app.suspended_local.is_some());
        assert!(app.remote_player_tab.is_some());
        // Must stay independent of the Sessions-panel direct-remote fields.
        assert!(app.connected_session_id.is_none());
        assert!(app.direct_remote_label.is_none());
    }

    #[test]
    fn switch_to_library_route_sets_remote_queue_scope_when_daemon_has_items() {
        let mut app = make_app_stub();
        let (remote, remote_rx) = mbv_core::remote_player::RemotePlayer::stub(make_items(2), 0);

        app.switch_to_library_route("music", remote, remote_rx);

        assert!(app.has_direct_remote_queue());
    }
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test switch_to_library_route`
Expected: FAIL with `no method named 'switch_to_library_route' found for struct 'App'`.

- [ ] **Step 4: Implement `switch_to_library_route`**

Add to `src/app/mod.rs`, immediately after `switch_to_direct_remote` (~line 2403):

```rust
    /// Sibling to `switch_to_direct_remote` for library-scoped daemon
    /// routing (#223): same suspend-local/connect-remote shape, but
    /// targets a statically configured `DaemonEndpoint` from
    /// `daemon_routes` instead of a discovered `SessionInfo`, and tracks
    /// `active_route` instead of `connected_session_id`/
    /// `direct_remote_label` -- library routing and the Sessions-panel
    /// direct-remote flow are two independent ways to end up thin-client
    /// and must not be conflated in App state. This is a new sibling
    /// method, not a modification of `switch_to_direct_remote`.
    fn switch_to_library_route(
        &mut self,
        library_name: &str,
        remote: mbv_core::remote_player::RemotePlayer,
        remote_rx: mpsc::Receiver<PlayerEvent>,
    ) {
        let initial_items = remote.items.lock().unwrap().clone();
        let has_initial_items = !initial_items.is_empty();
        let initial_cursor = remote.status.lock().unwrap().current_idx;
        let always_play_next = self.client.lock().unwrap().config.always_play_next;
        // Cloned before `remote` is moved into `PlayerProxy::remote` below,
        // mirroring `switch_to_direct_remote`'s #175 MPRIS rebind.
        let mpris_remote = remote.clone();

        if !self.player.is_remote() {
            self.player.stop();
            self.player.join_or_timeout(Duration::from_secs(5));
            let (_dummy_ws_tx, dummy_ws_rx) = mpsc::channel::<WsEvent>();
            let suspended = SuspendedLocalSession {
                player: std::mem::replace(
                    &mut self.player,
                    PlayerProxy::remote(remote, always_play_next),
                ),
                player_rx: std::mem::replace(&mut self.player_rx, remote_rx),
                ws_rx: std::mem::replace(&mut self.ws_rx, dummy_ws_rx),
                ws_send_tx: self.ws_send_tx.take(),
            };
            self.suspended_local = Some(suspended);
        } else {
            self.player = PlayerProxy::remote(remote, always_play_next);
            self.player_rx = remote_rx;
        }

        if let Some(handle) = &self.mpris {
            let disconnected = mpris_remote.disconnected_flag();
            crate::mpris::rebind(
                handle,
                mpris_remote.status.clone(),
                move |cmd| {
                    mpris_remote.send_command(cmd);
                },
                Some(disconnected),
            );
        }

        self.remote_player_tab = Some(PlayerTab::new(initial_items, initial_cursor));
        self.active_route = Some(library_name.to_string());
        self.remote_pos_s = 0;
        self.remote_pos_at = Instant::now();
        self.remote_api_pos_advanced_at = Instant::now() - Duration::from_secs(60);
        self.remote_seek_pending_until = Instant::now() - Duration::from_secs(1);
        self.runtime_zero_since = None;
        self.next_up_item = None;
        self.skip_intro_end_ticks = None;
        if has_initial_items {
            self.set_queue_scope(QueueScope::Remote);
        } else {
            self.set_queue_scope(QueueScope::Local);
        }
        log::info!(
            target: "library_route",
            "switched playback to library route {library_name:?}"
        );
        self.flash_status(format!("Routed to {library_name} daemon"));
    }
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test switch_to_library_route`
Expected: PASS (2 passed)

- [ ] **Step 6: Run `detect_changes` before committing**

Per this repo's `CLAUDE.md`:

```
detect_changes({scope: "compare", base_ref: "main"})
```

Expected: only `switch_to_library_route` (new) and its two new tests appear as affected symbols; `switch_to_direct_remote` and its existing tests must NOT show up as changed.

- [ ] **Step 7: Commit**

```bash
git add src/app/mod.rs
git commit -m "app: add switch_to_library_route, sibling to switch_to_direct_remote"
```

---

### Task 6: App — extend `restore_local_mode` to clear `active_route`

**Files:**
- Modify: `src/app/mod.rs:2405-2439` (`restore_local_mode`)

**Interfaces:**
- Consumes: `App.active_route` (Task 3).
- Produces: `restore_local_mode` now clears `active_route` in addition to its existing resets, so it is the single shared "go back to local" tail for both the Sessions-panel direct-remote path and the new library-route path.

- [ ] **Step 1: Run impact analysis before editing**

```
impact({target: "restore_local_mode", direction: "upstream"})
```

Read the caller list and report any HIGH/CRITICAL risk before proceeding — `restore_local_mode` is called from several places (e.g. `disconnect_remote`'s `RemoteSlotState::DirectRemote` arm), so confirm none of them assert `active_route` stays unset in a way this change would break (it currently doesn't exist, so no caller can be asserting on it yet).

- [ ] **Step 2: Write the failing test**

Add to `mod tests` in `src/app/mod.rs`:

```rust
    #[test]
    fn restore_local_mode_clears_active_route() {
        let mut app = make_app_stub();
        let (remote, remote_rx) = mbv_core::remote_player::RemotePlayer::stub(make_items(1), 0);
        app.switch_to_library_route("music", remote, remote_rx);
        assert_eq!(app.active_route.as_deref(), Some("music"));

        app.restore_local_mode("Local playback restored");

        assert!(app.active_route.is_none());
        assert!(!app.player.is_remote());
    }
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test restore_local_mode_clears_active_route`
Expected: FAIL — `active_route` is still `Some("music")` after `restore_local_mode` (assertion failure, not a compile error).

- [ ] **Step 4: Extend `restore_local_mode`**

In `src/app/mod.rs`, in `restore_local_mode` (~line 2405), add `self.active_route = None;` immediately after `self.direct_remote_label = None;`:

```rust
        self.remote_player_tab = None;
        self.connected_session_id = None;
        self.connected_session_state = None;
        self.direct_remote_label = None;
        self.active_route = None;
        self.session_miss_count = 0;
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test restore_local_mode_clears_active_route`
Expected: PASS

- [ ] **Step 6: Run the full mod.rs test suite to check for regressions**

Run: `cargo test --lib app::mod::tests`
Expected: all existing tests (including `disconnect_remote_*`, `remote_slot_state_*`) still PASS.

- [ ] **Step 7: Commit**

```bash
git add src/app/mod.rs
git commit -m "app: restore_local_mode also clears active_route (shared local-restore tail)"
```

---

### Task 7: App — route resolution: nav-scoped view, ancestor-lookup view, combined resolver

**Files:**
- Modify: `src/app/mod.rs` — new methods near `resolve_route_for_library` (Task 4)

**Interfaces:**
- Consumes: `App.libs: Vec<LibraryTab>` (existing, `LibraryTab.library: MediaItem`), `App.tab_idx`, `App.lib_tab_offset()` (existing, used by `current_lib_item`), `App.library_route_cache` (Task 3), `EmbyClient::get_ancestors` (`crates/mbv-core/src/api.rs:1567-1579`), `resolve_route_for_library` (Task 4).
- Produces:
  - `fn route_for_active_library_view(&self, lib_idx: usize) -> Option<(String, DaemonEndpoint)>`
  - `fn route_for_item_via_ancestors(&mut self, item_id: &str) -> Option<(String, DaemonEndpoint)>`
  - `fn resolve_route_for_play(&mut self, item: &mbv_core::api::MediaItem) -> Option<(String, DaemonEndpoint)>`
  - `fn resolve_route_for_enqueue_folder(&mut self, item: &mbv_core::api::MediaItem) -> Option<String>`

  Tasks 8-10 call these.

- [ ] **Step 1: Write the failing tests**

Add to `mod tests` in `src/app/mod.rs`:

```rust
    #[test]
    fn route_for_active_library_view_uses_nav_state_no_network() {
        let mut app = make_app_stub();
        app.daemon_routes.insert(
            "music".to_string(),
            "tcp://127.0.0.1:9000".to_string(),
        );
        let mut lib_item = make_item("Music", "CollectionFolder");
        lib_item.id = "lib-music".to_string();
        app.libs.push(LibraryTab {
            library: lib_item,
            nav_stack: Vec::new(),
            search: None,
            feed_home_video: None,
            power_detail_scroll: Default::default(),
            album_track_focus: None,
            artist_header_focus: None,
        });

        let resolved = app.route_for_active_library_view(0);

        assert_eq!(resolved.map(|(name, _)| name), Some("music".to_string()));
    }

    #[test]
    fn route_for_active_library_view_none_for_unrouted_library() {
        let mut app = make_app_stub();
        let mut lib_item = make_item("Movies", "CollectionFolder");
        lib_item.id = "lib-movies".to_string();
        app.libs.push(LibraryTab {
            library: lib_item,
            nav_stack: Vec::new(),
            search: None,
            feed_home_video: None,
            power_detail_scroll: Default::default(),
            album_track_focus: None,
            artist_header_focus: None,
        });

        assert_eq!(app.route_for_active_library_view(0), None);
    }

    #[test]
    fn route_for_item_via_ancestors_caches_after_first_lookup() {
        let mut app = make_app_stub();
        app.daemon_routes.insert(
            "music".to_string(),
            "tcp://127.0.0.1:9000".to_string(),
        );
        // No live server in this stub -- `get_ancestors` will error and the
        // resolver caches a `None` result rather than retrying every call.
        let first = app.route_for_item_via_ancestors("item-1");
        assert_eq!(first, None);
        assert!(app.library_route_cache.contains_key("item-1"));
        // Second call must not attempt another network round-trip; the
        // cached `None` short-circuits before any client call, so the
        // result is stable and deterministic in a stub with no server.
        let second = app.route_for_item_via_ancestors("item-1");
        assert_eq!(second, None);
    }

    #[test]
    fn resolve_route_for_play_does_not_panic_from_the_queue_tab() {
        // Regression guard: `tab_idx` values are 0 = Home, 1 = Queue tab,
        // 2.. = library tabs (`lib_tab_offset() == 2`, confirmed against
        // `src/app/input.rs`). An `if tab_idx == 0 { .. } else { lib_idx =
        // tab_idx - lib_tab_offset() }` shape (as opposed to `enqueue_selected`'s
        // existing `tab_idx == 0` / `tab_idx >= 2` split) underflows a `usize`
        // subtraction (1 - 2) and panics when called from the Queue tab. The
        // Queue tab has no library of its own -- the item being played is
        // already part of whatever queue is current, so `resolve_route_for_play`
        // must fall through to "keep the current `active_route`" instead of
        // either panicking or wrongly resolving a nav-scoped library.
        let mut app = make_app_stub();
        app.tab_idx = 1;
        let mut item = make_item("Song", "Audio");
        item.id = "song-1".to_string();

        // Local queue: no route to keep.
        assert_eq!(app.resolve_route_for_play(&item), None);

        // Already routed: the Queue tab must not clear or re-resolve the
        // route out from under an in-progress routed queue.
        app.daemon_routes.insert(
            "music".to_string(),
            "tcp://127.0.0.1:9000".to_string(),
        );
        app.active_route = Some("music".to_string());
        assert_eq!(
            app.resolve_route_for_play(&item).map(|(name, _)| name),
            Some("music".to_string())
        );
    }

    #[test]
    fn resolve_route_for_enqueue_folder_matches_a_library_root_folder_by_its_own_name() {
        // #223 follow-up: `get_ancestors` on a library root returns no
        // `CollectionFolder` ancestor above it (there isn't one), so a plain
        // ancestor-lookup resolver always yields `None` for the library root
        // item itself. `do_enqueue_folder` can receive exactly that item (the
        // user enqueue-recursive's an entire library from its root), so this
        // helper checks the item's own type first.
        let mut app = make_app_stub();
        app.daemon_routes.insert(
            "music".to_string(),
            "tcp://127.0.0.1:9000".to_string(),
        );
        let mut lib_root = make_item("Music", "CollectionFolder");
        lib_root.id = "lib-music".to_string();

        assert_eq!(
            app.resolve_route_for_enqueue_folder(&lib_root),
            Some("music".to_string())
        );
    }

    #[test]
    fn resolve_route_for_enqueue_folder_falls_back_to_ancestor_lookup_for_a_non_root_folder() {
        let mut app = make_app_stub();
        let mut sub_folder = make_item("Some Album", "MusicAlbum");
        sub_folder.id = "album-1".to_string();
        sub_folder.is_folder = true;

        // No live server in this stub -- `get_ancestors` errors, so this
        // must fall through to the ancestor-lookup path (not treat every
        // folder as a library root) and resolve to `None`, not panic.
        assert_eq!(app.resolve_route_for_enqueue_folder(&sub_folder), None);
    }
```

Note: `LibraryTab` field list must match its current definition at `src/app/mod.rs:824-840` exactly (`library`, `nav_stack`, `search`, `feed_home_video`, `power_detail_scroll`, `album_track_focus`, `artist_header_focus`) — if any field's type doesn't accept the literal shown (e.g. `nav_stack: Vec::new()` vs a different default), match the existing test helper that already builds a `LibraryTab` elsewhere in `mod tests` and copy its exact construction instead of the literal above.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test route_for_active_library_view route_for_item_via_ancestors resolve_route_for_play_does_not_panic resolve_route_for_enqueue_folder`
Expected: FAIL with `no method named 'route_for_active_library_view' found for struct 'App'` (and similarly for the other three -- `resolve_route_for_play` and `resolve_route_for_enqueue_folder` do not exist yet either).

- [ ] **Step 3: Implement the resolvers**

Add to `src/app/mod.rs`, near `resolve_route_for_library` (Task 4):

```rust
    /// Nav-context route resolution for library-scoped views (Library
    /// tab, Power View, Album/Artist drill-down, in-library search) --
    /// the active library is already known from navigation state
    /// (`LibraryTab::library`), so no network call is needed (#223).
    fn route_for_active_library_view(
        &self,
        lib_idx: usize,
    ) -> Option<(String, mbv_core::remote_player::DaemonEndpoint)> {
        let lib = self.libs.get(lib_idx)?;
        self.resolve_route_for_library(&lib.library.name)
    }

    /// Cross-library aggregate view (Continue Watching/Next Up, Favorites)
    /// route resolution: walks the item's ancestor chain via
    /// `EmbyClient::get_ancestors` to find the owning library
    /// (`CollectionFolder`), then matches it against `daemon_routes`.
    /// Cached per item id for the session so a repeated play/enqueue of
    /// the same item never re-fetches (#223).
    fn route_for_item_via_ancestors(
        &mut self,
        item_id: &str,
    ) -> Option<(String, mbv_core::remote_player::DaemonEndpoint)> {
        if let Some(cached) = self.library_route_cache.get(item_id) {
            return cached
                .clone()
                .and_then(|name| self.resolve_route_for_library(&name));
        }
        let ancestors = {
            let client = self.client.lock().unwrap();
            client.get_ancestors(item_id)
        };
        let library_name = match ancestors {
            Ok(chain) => chain
                .into_iter()
                .find(|a| a.item_type == "CollectionFolder")
                .map(|a| a.name),
            Err(e) => {
                log::warn!(
                    target: "library_route",
                    "get_ancestors failed for item {item_id:?}: {e}"
                );
                None
            }
        };
        self.library_route_cache
            .insert(item_id.to_string(), library_name.clone());
        library_name.and_then(|name| self.resolve_route_for_library(&name))
    }

    /// Resolves the daemon route (if any) that a play/enqueue of `item`
    /// should target: nav-scoped lookup for library-scoped views
    /// (`tab_idx >= 2` -- Library/Power/Album/Artist/in-library search),
    /// ancestor-lookup for cross-library aggregate views (`tab_idx == 0`
    /// -- Home tab). No match in either case means local playback,
    /// unaffected (#223).
    ///
    /// `tab_idx == 1` is the Queue tab -- it has no library of its own
    /// (`lib_tab_offset()` is `2`, so a bare `tab_idx - lib_tab_offset()`
    /// would underflow and panic here, unlike `enqueue_selected`'s existing
    /// `tab_idx == 0` / `tab_idx >= 2` split which already avoids this).
    /// An item played from the Queue tab is already part of whatever queue
    /// is current, so this keeps whatever route is already active rather
    /// than re-resolving from nav context (there is none) or treating "no
    /// nav-scoped resolution" as "no route", which would incorrectly
    /// restore to local every time the Queue tab is used to play/jump
    /// within an already-routed queue.
    fn resolve_route_for_play(
        &mut self,
        item: &mbv_core::api::MediaItem,
    ) -> Option<(String, mbv_core::remote_player::DaemonEndpoint)> {
        if self.tab_idx == 0 {
            self.route_for_item_via_ancestors(&item.id)
        } else if self.tab_idx >= 2 {
            let lib_idx = self.tab_idx - self.lib_tab_offset();
            self.route_for_active_library_view(lib_idx)
        } else {
            self.active_route
                .clone()
                .and_then(|name| self.resolve_route_for_library(&name))
        }
    }

    /// Route resolution specifically for `do_enqueue_folder` (#223 follow-up,
    /// see "Design decisions carried forward from review" above): the item
    /// being enqueue-recursive'd may itself *be* a library root
    /// (`item_type == "CollectionFolder"`), in which case `get_ancestors`
    /// returns no ancestor above it and a plain ancestor-lookup resolver
    /// always yields `None`. Check the item's own type first; only fall
    /// back to ancestor lookup for a non-root folder.
    fn resolve_route_for_enqueue_folder(
        &mut self,
        item: &mbv_core::api::MediaItem,
    ) -> Option<String> {
        if item.item_type == "CollectionFolder" {
            return self
                .resolve_route_for_library(&item.name)
                .map(|(name, _)| name);
        }
        self.route_for_item_via_ancestors(&item.id)
            .map(|(name, _)| name)
    }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test route_for_active_library_view route_for_item_via_ancestors resolve_route_for_play_does_not_panic resolve_route_for_enqueue_folder`
Expected: PASS (5 passed)

- [ ] **Step 5: Commit**

```bash
git add src/app/mod.rs
git commit -m "app: add nav-scoped and ancestor-lookup route resolvers"
```

---

### Task 8: App — `apply_route_for_playback` orchestration (connect/fallback)

**Prerequisite:** This task calls `App::try_daemon_route_connect`, defined by #222's plan (`docs/superpowers/plans/2026-07-17-daemon-connect-lifecycle.md`, Task 1). That method -- along with `App::connect_daemon_route_endpoint` and the `DAEMON_ROUTE_CONNECT_OVERRIDE`/`DAEMON_ROUTE_CONNECT_TEST_LOCK` `#[cfg(test)]` statics it uses -- must already exist in `src/app/mod.rs` before this task's code will compile. Do not substitute `connect_direct_endpoint`/`DIRECT_CONNECT_OVERRIDE` (the Sessions-panel seam) here: keeping the two connect paths on independent test seams is a deliberate #222/#223 design choice (see "Design decisions carried forward from review" above), not an incidental implementation detail.

**Verified directly against #222's current plan file (not assumed):** `try_daemon_route_connect` returns `Result<(RemotePlayer, Receiver<PlayerEvent>), String>`, not `Option`. On `Err`, the `String` payload is already the fully-formatted, ready-to-display status-bar warning text (`"\u{26a0} {route_label} route unreachable, using local playback (mbv.log)"`) -- the primitive logs the raw connect error internally and deliberately does **not** call `flash_status_high` itself, leaving *how* to surface the message to the caller (a plain flash when already local, vs. threading it through `restore_local_mode` when swapping away from a previously active different route -- exactly the choice this task needs to make). This means `apply_route_for_playback` below needs no separate message-reconstruction helper: it just forwards the `Err` string verbatim to whichever display path applies.

**Files:**
- Modify: `src/app/mod.rs` — new method near `connect_to_session` (~2441)

**Interfaces:**
- Consumes: `resolve_route_for_play` (Task 7), `App.active_route` (Task 3), `switch_to_library_route` (Task 5), `restore_local_mode` (Task 6), `App::try_daemon_route_connect(&mut self, endpoint: &DaemonEndpoint, route_label: &str) -> Result<(RemotePlayer, mpsc::Receiver<PlayerEvent>), String>` (from #222's plan, Task 1 -- already does the connect attempt and the failure log; on `Err`, returns a ready-to-display warning string for this task to surface).
- Produces: `fn apply_route_for_playback(&mut self, item: &mbv_core::api::MediaItem)` — Task 9 calls this from `play_item`/`play_items_routed`.

- [ ] **Step 1: Write the failing tests**

Add to `mod tests` in `src/app/mod.rs`:

```rust
    #[test]
    fn apply_route_for_playback_swaps_to_routed_daemon_on_success() {
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
            Ok(mbv_core::remote_player::RemotePlayer::stub(make_items(1), 0))
        }
        *DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = Some(route_connect_success);

        let mut app = make_app_stub();
        app.daemon_routes.insert(
            "music".to_string(),
            "tcp://127.0.0.1:9000".to_string(),
        );
        let mut lib_item = make_item("Music", "CollectionFolder");
        lib_item.id = "lib-music".to_string();
        app.libs.push(LibraryTab {
            library: lib_item,
            nav_stack: Vec::new(),
            search: None,
            feed_home_video: None,
            power_detail_scroll: Default::default(),
            album_track_focus: None,
            artist_header_focus: None,
        });
        app.tab_idx = app.lib_tab_offset();
        let mut item = make_item("Song", "Audio");
        item.id = "song-1".to_string();

        app.apply_route_for_playback(&item);

        *DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = None;
        assert_eq!(app.active_route.as_deref(), Some("music"));
        assert!(app.player.is_remote());
    }

    #[test]
    fn apply_route_for_playback_falls_back_to_local_with_warning_on_connect_failure() {
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
        app.daemon_routes.insert(
            "music".to_string(),
            "tcp://127.0.0.1:9000".to_string(),
        );
        let mut lib_item = make_item("Music", "CollectionFolder");
        lib_item.id = "lib-music".to_string();
        app.libs.push(LibraryTab {
            library: lib_item,
            nav_stack: Vec::new(),
            search: None,
            feed_home_video: None,
            power_detail_scroll: Default::default(),
            album_track_focus: None,
            artist_header_focus: None,
        });
        app.tab_idx = app.lib_tab_offset();
        let mut item = make_item("Song", "Audio");
        item.id = "song-1".to_string();

        app.apply_route_for_playback(&item);

        *DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = None;
        assert!(app.active_route.is_none());
        assert!(!app.player.is_remote());
        assert!(app.status.contains("unreachable"));
    }

    #[test]
    fn apply_route_for_playback_is_noop_when_item_already_matches_active_route() {
        let mut app = make_app_stub();
        app.daemon_routes.insert(
            "music".to_string(),
            "tcp://127.0.0.1:9000".to_string(),
        );
        app.active_route = Some("music".to_string());
        let mut lib_item = make_item("Music", "CollectionFolder");
        lib_item.id = "lib-music".to_string();
        app.libs.push(LibraryTab {
            library: lib_item,
            nav_stack: Vec::new(),
            search: None,
            feed_home_video: None,
            power_detail_scroll: Default::default(),
            album_track_focus: None,
            artist_header_focus: None,
        });
        app.tab_idx = app.lib_tab_offset();
        let mut item = make_item("Song", "Audio");
        item.id = "song-1".to_string();

        app.apply_route_for_playback(&item);

        // No connect attempt was needed (no DAEMON_ROUTE_CONNECT_OVERRIDE
        // set, so a real connect attempt would panic/hang if this weren't
        // a no-op) -- active_route and local-ness are unchanged.
        assert_eq!(app.active_route.as_deref(), Some("music"));
        assert!(!app.player.is_remote());
    }

    #[test]
    fn apply_route_for_playback_restores_local_when_item_has_no_route() {
        let mut app = make_app_stub();
        let (remote, remote_rx) = mbv_core::remote_player::RemotePlayer::stub(make_items(1), 0);
        app.switch_to_library_route("music", remote, remote_rx);
        let mut movies_item = make_item("Movies", "CollectionFolder");
        movies_item.id = "lib-movies".to_string();
        app.libs.push(LibraryTab {
            library: movies_item,
            nav_stack: Vec::new(),
            search: None,
            feed_home_video: None,
            power_detail_scroll: Default::default(),
            album_track_focus: None,
            artist_header_focus: None,
        });
        app.tab_idx = app.lib_tab_offset();
        let mut item = make_item("Movie", "Movie");
        item.id = "movie-1".to_string();

        app.apply_route_for_playback(&item);

        assert!(app.active_route.is_none());
        assert!(!app.player.is_remote());
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test apply_route_for_playback`
Expected: FAIL with `no method named 'apply_route_for_playback' found for struct 'App'`.

- [ ] **Step 3: Implement `apply_route_for_playback`**

Add to `src/app/mod.rs`, near `connect_to_session` (~line 2441):

```rust
    /// Applies the route resolved by `resolve_route_for_play` before a
    /// queue replace (#223): swaps to the routed daemon, restores local,
    /// or leaves the current target alone if it already matches.
    /// Connection failure falls back to local playback -- never a hard
    /// error -- per #222's fallback rule, via `try_daemon_route_connect`
    /// (#222's plan, Task 1), which already logs the raw failure and
    /// returns a ready-to-display warning string as its `Err` payload;
    /// this method decides *where* to surface that string -- a direct
    /// flash when we were already local, or threaded through
    /// `restore_local_mode` when we were already on a *different* route
    /// and must actually swap the player back to local, not just show a
    /// warning while silently staying connected to the old route.
    fn apply_route_for_playback(&mut self, item: &mbv_core::api::MediaItem) {
        let resolved = self.resolve_route_for_play(item);
        match (resolved, self.active_route.clone()) {
            (Some((name, _)), Some(current)) if name == current => {}
            (Some((name, endpoint)), was_routed) => {
                match self.try_daemon_route_connect(&endpoint, &name) {
                    Ok((remote, remote_rx)) => {
                        self.switch_to_library_route(&name, remote, remote_rx);
                    }
                    Err(message) => {
                        log::warn!(
                            target: "library_route",
                            "connect to library route {name:?} endpoint {endpoint} failed: {message}"
                        );
                        if was_routed.is_some() {
                            self.restore_local_mode(&message);
                        } else {
                            self.flash_status_high(message);
                        }
                    }
                }
            }
            (None, Some(_)) => {
                self.restore_local_mode("Local playback restored");
            }
            (None, None) => {}
        }
    }
```

- [ ] **Step 4: Remove #222's now-obsolete `#[allow(dead_code)]` on the primitive**

#222's plan (Task 1) deliberately shipped `App::connect_daemon_route_endpoint` and `App::try_daemon_route_connect` with a narrow, explicitly-temporary `#[allow(dead_code)]` on each, because that plan produces the primitive with zero production call sites by design (the trigger was left for #223). Its own plan states the removal condition verbatim: "delete both attributes in the same change that adds #223's first call site." This task's `apply_route_for_playback` *is* that first call site, so remove both attributes now, as part of this task's edit (not a separate cleanup task) -- leaving them in place after this task lands would be shipping a stale suppression the sibling plan's own text says must go. In `src/app/mod.rs`, delete the `#[allow(dead_code)]` line immediately above `fn connect_daemon_route_endpoint(` and the one immediately above `pub(super) fn try_daemon_route_connect(`.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test apply_route_for_playback`
Expected: PASS (4 passed)

- [ ] **Step 6: Run `cargo build --workspace` to confirm removing `#[allow(dead_code)]` did not uncover a real warning**

Run: `cargo build --workspace`
Expected: no warnings. Both methods are now reachable from this task's real `apply_route_for_playback` call site (not just `#[cfg(test)]` code), so `dead_code` should not fire even without the attribute -- if it does, the call site is not actually wired the way this task intends and must be fixed before proceeding, not re-suppressed with the attribute.

- [ ] **Step 7: Commit**

```bash
git add src/app/mod.rs
git commit -m "app: add apply_route_for_playback (connect/fallback orchestration); remove #222's temporary dead_code allowances now that this is their first call site"
```

---

### Task 9: Wire route swap into `play_item` and `play_items_routed`

**Files:**
- Modify: `src/app/actions.rs:2327-2368` (`play_item`), `src/app/actions.rs:2291-2325` (`play_items_routed`)

**Prerequisite:** Same as Task 8 -- this task's tests depend on #222's `DAEMON_ROUTE_CONNECT_OVERRIDE`/`DAEMON_ROUTE_CONNECT_TEST_LOCK` statics already existing in `src/app/mod.rs`.

**Interfaces:**
- Consumes: `apply_route_for_playback` (Task 8), `App.connected_session_id` (existing), `App.player.is_remote()` (existing), `App.active_route` (Task 3).

- [ ] **Step 1: Run impact analysis before editing**

```
impact({target: "play_item", direction: "upstream"})
impact({target: "play_items_routed", direction: "upstream"})
impact({target: "connect_to_session", direction: "upstream"})
```

Report the caller lists and any HIGH/CRITICAL risk before proceeding — `play_item`/`play_items_routed` are called from many nav/input-handling call sites across `src/app/`, so this is an additive guard at the top of each, not a signature or control-flow change for existing callers when no route applies. `connect_to_session` gets one additional line (clearing `active_route`) with no signature change.

**Why the guard is not simply `self.connected_session_id.is_none()`:** during review against the actual `remote_slot_state()`/`switch_to_direct_remote` code (not just the issue text), two thin-client modes exist that `connected_session_id` alone does not detect: a Sessions-panel "Direct Remote" ctrl-socket upgrade and local-daemon mode both leave `connected_session_id` as `None` while `self.player.is_remote()` is `true` and `active_route` is `None`. Guarding on `connected_session_id` alone would let library routing silently swap `self.player` away from an active Sessions-panel direct-remote connection -- exactly the state conflation Global Constraint #15 forbids. The correct condition, used throughout this task, is:

```rust
let skip_library_routing = self.connected_session_id.is_some()
    || (self.player.is_remote() && self.active_route.is_none());
```

This still lets library routing run when `active_route` is already `Some(..)` (so `apply_route_for_playback` can re-evaluate/swap/restore -- that is its job), and only skips it when the current remote state belongs to a different, non-library-route mechanism.

- [ ] **Step 2: Write the failing tests**

Add to `mod tests` in `src/app/actions.rs`:

```rust
    #[test]
    fn play_item_swaps_to_library_route_before_replacing_queue() {
        let _guard = crate::config::TestStateDirGuard::new();
        let _connect_guard = crate::app::DAEMON_ROUTE_CONNECT_TEST_LOCK.lock().unwrap();
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
            Ok(mbv_core::remote_player::RemotePlayer::stub(make_items(1), 0))
        }
        *crate::app::DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = Some(route_connect_success);

        let mut app = make_app_stub();
        app.daemon_routes.insert(
            "music".to_string(),
            "tcp://127.0.0.1:9000".to_string(),
        );
        let mut lib_item = make_item("Music", "CollectionFolder");
        lib_item.id = "lib-music".to_string();
        app.libs.push(LibraryTab {
            library: lib_item,
            nav_stack: Vec::new(),
            search: None,
            feed_home_video: None,
            power_detail_scroll: Default::default(),
            album_track_focus: None,
            artist_header_focus: None,
        });
        app.tab_idx = app.lib_tab_offset();
        let mut item = make_item("Song", "Audio");
        item.id = "song-1".to_string();

        app.play_item(item);

        *crate::app::DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = None;
        assert_eq!(app.active_route.as_deref(), Some("music"));
    }

    #[test]
    fn play_item_skips_library_routing_when_attached_to_a_session() {
        let mut app = make_app_stub();
        app.daemon_routes.insert(
            "music".to_string(),
            "tcp://127.0.0.1:9000".to_string(),
        );
        app.connected_session_id = Some("sess-1".to_string());
        let mut lib_item = make_item("Music", "CollectionFolder");
        lib_item.id = "lib-music".to_string();
        app.libs.push(LibraryTab {
            library: lib_item,
            nav_stack: Vec::new(),
            search: None,
            feed_home_video: None,
            power_detail_scroll: Default::default(),
            album_track_focus: None,
            artist_header_focus: None,
        });
        app.tab_idx = app.lib_tab_offset();
        let mut item = make_item("Song", "Audio");
        item.id = "song-1".to_string();

        // No DAEMON_ROUTE_CONNECT_OVERRIDE set -- if library routing
        // engaged here it would attempt a real connection and this test
        // would hang/fail rather than reach the assertion below.
        app.play_item(item);

        assert!(app.active_route.is_none());
    }

    #[test]
    fn play_item_skips_library_routing_when_already_direct_remote_via_sessions_panel() {
        // Regression guard for the gap `connected_session_id.is_none()`
        // alone misses: a Sessions-panel "Direct Remote" ctrl-socket
        // upgrade leaves `connected_session_id` as `None` but
        // `self.player.is_remote()` `true` and `active_route` `None`.
        // Library routing must not engage here either -- it would swap
        // `self.player` out from under the active direct-remote
        // connection without ever clearing `direct_remote_label`.
        let mut app = make_app_stub();
        app.daemon_routes.insert(
            "music".to_string(),
            "tcp://127.0.0.1:9000".to_string(),
        );
        let (remote, remote_rx) = mbv_core::remote_player::RemotePlayer::stub(make_items(1), 0);
        let sess = crate::app::tests::make_session("other-mbv", "mbv");
        app.switch_to_direct_remote(&sess, remote, remote_rx);
        assert!(app.player.is_remote());
        assert!(app.active_route.is_none());

        let mut lib_item = make_item("Music", "CollectionFolder");
        lib_item.id = "lib-music".to_string();
        app.libs.push(LibraryTab {
            library: lib_item,
            nav_stack: Vec::new(),
            search: None,
            feed_home_video: None,
            power_detail_scroll: Default::default(),
            album_track_focus: None,
            artist_header_focus: None,
        });
        app.tab_idx = app.lib_tab_offset();
        let mut item = make_item("Song", "Audio");
        item.id = "song-1".to_string();

        // No DAEMON_ROUTE_CONNECT_OVERRIDE set -- if library routing
        // engaged here it would attempt a real connection and this test
        // would hang/fail rather than reach the assertion below.
        app.play_item(item);

        assert!(app.active_route.is_none());
    }
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test play_item_swaps_to_library_route_before_replacing_queue play_item_skips_library_routing_when_attached_to_a_session play_item_skips_library_routing_when_already_direct_remote_via_sessions_panel`
Expected: FAIL — `active_route` stays `None` after `play_item` in the first test (no wiring yet); the third test compiles but is not yet a meaningful regression guard until Step 4 lands (it would currently pass vacuously since nothing calls `apply_route_for_playback` at all yet -- re-run it after Step 4 together with the others to confirm it still passes for the *right* reason).

- [ ] **Step 4: Wire `apply_route_for_playback` into both functions, using the combined skip condition**

In `src/app/actions.rs`, `play_item` (~line 2327), add the guard as the very first line of the function body:

```rust
    pub(super) fn play_item(&mut self, item: MediaItem) {
        let skip_library_routing = self.connected_session_id.is_some()
            || (self.player.is_remote() && self.active_route.is_none());
        if !skip_library_routing {
            self.apply_route_for_playback(&item);
        }
        self.on_queue_replace_silent();
```

In `src/app/actions.rs`, `play_items_routed` (~line 2291), add the guard as the very first line of the function body:

```rust
    pub(super) fn play_items_routed(&mut self, items: Vec<MediaItem>, start_idx: usize) {
        let skip_library_routing = self.connected_session_id.is_some()
            || (self.player.is_remote() && self.active_route.is_none());
        if !skip_library_routing {
            if let Some(item) = items.get(start_idx).or_else(|| items.first()) {
                let item = item.clone();
                self.apply_route_for_playback(&item);
            }
        }
        self.on_queue_replace_silent();
```

Also add `use mbv_core::remote_player::DaemonEndpoint;` style imports only if `src/app/actions.rs` doesn't already reach `App`'s methods without them — since `apply_route_for_playback` is a method on `App` (defined in `mod.rs`, same `impl App` type, different file), no additional import is needed; Rust resolves inherent methods across `impl` blocks in different files automatically for the same crate.

- [ ] **Step 5: Fix the reverse conflation: clear `active_route` before a Sessions-panel direct-remote upgrade**

`switch_to_direct_remote`'s `else` branch (taken when `self.player.is_remote()` is already `true` -- exactly the "was on a library route, now upgrading via Sessions-panel" case) overwrites `self.player`/`self.player_rx` directly without going through `restore_local_mode`, so it never clears a stale `active_route`. Per this plan's Global Constraint against modifying `switch_to_direct_remote` itself, fix this at its sole caller instead. In `src/app/mod.rs`, `connect_to_session` (~line 2442), add `self.active_route = None;` immediately before the successful-upgrade call to `switch_to_direct_remote`:

```rust
                match self.connect_direct_endpoint(&endpoint, &auth_token) {
                    Ok((remote, remote_rx)) => {
                        self.active_route = None;
                        self.switch_to_direct_remote(sess, remote, remote_rx);
                        return;
                    }
```

Add a regression test for this to `mod tests` in `src/app/mod.rs`, near the existing `connect_to_session_uses_direct_upgrade_success` test:

```rust
    #[test]
    fn connect_to_session_clears_a_stale_active_route_on_direct_upgrade() {
        let _guard = crate::config::TestStateDirGuard::new();
        let _connect_guard = DIRECT_CONNECT_TEST_LOCK.lock().unwrap();
        fn direct_success(
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

        *DIRECT_CONNECT_OVERRIDE.lock().unwrap() = Some(direct_success);
        let mut app = make_app_stub();
        // Simulate already being on a library route (#223) by setting the
        // player remote directly, mirroring what `switch_to_library_route`
        // would have done, without depending on that method here.
        let (remote, remote_rx) = mbv_core::remote_player::RemotePlayer::stub(make_items(1), 0);
        app.player = mbv_core::player::PlayerProxy::remote(remote, false);
        app.player_rx = remote_rx;
        app.active_route = Some("music".to_string());
        let mut sess = make_session("remote-mbv", "mbv");
        sess.supported_commands = vec![mbv_core::api::mbv_direct_tcp_port_command(47788)];

        app.connect_to_session(&sess);

        *DIRECT_CONNECT_OVERRIDE.lock().unwrap() = None;
        assert!(app.active_route.is_none());
    }
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test play_item_swaps_to_library_route_before_replacing_queue play_item_skips_library_routing_when_attached_to_a_session play_item_skips_library_routing_when_already_direct_remote_via_sessions_panel connect_to_session_clears_a_stale_active_route_on_direct_upgrade`
Expected: PASS (4 passed)

- [ ] **Step 7: Run the full actions.rs and mod.rs test suites to check for regressions**

Run: `cargo test --lib app::actions::tests`
Run: `cargo test --lib app::mod::tests`
Expected: all existing tests still PASS (in particular any existing `play_item`/`play_items_routed` tests that don't configure `daemon_routes` — `apply_route_for_playback` must be a true no-op when `daemon_routes` is empty; and all existing `connect_to_session_*`/`switch_to_direct_remote_*` tests, since `active_route` starts `None` for every one of them and clearing an already-`None` value is a no-op).

- [ ] **Step 8: Run `detect_changes` before committing**

```
detect_changes({scope: "compare", base_ref: "main"})
```

Confirm the only behavior-affecting flows touched are `play_item`, `play_items_routed`, and `connect_to_session`'s own execution paths, not unrelated ones.

- [ ] **Step 9: Commit**

```bash
git add src/app/actions.rs src/app/mod.rs
git commit -m "actions: swap to library route before play_item/play_items_routed replace queue; clear stale active_route on Sessions-panel direct-remote upgrade"
```

---

### Task 10: Enforce the no-mixed-route queue invariant on enqueue

**Files:**
- Modify: `src/app/actions.rs:2370-2435` (`enqueue_selected`), `src/app/actions.rs:2473-2514` (`enqueue_artist_header_selection`), `src/app/actions.rs:2576-2616` (`do_enqueue_folder`)

**Interfaces:**
- Consumes: `resolve_route_for_play`, `route_for_active_library_view`, `route_for_item_via_ancestors`, `resolve_route_for_enqueue_folder` (Task 7), `App.active_route` (Task 3), `App.connected_session_id`, `App.player.is_remote()` (existing), `flash_status_high` (existing, `src/app/actions.rs:2241-2246`).
- Produces: `fn enqueue_route_conflict(&mut self, resolved_name: Option<String>) -> bool` — `true` means the caller must abort without mutating the queue.

**A second gap found during review, fixed here:** the same "which thin-client mode are we actually in" question from Task 9 applies to enqueue. Gating `enqueue_route_conflict` purely on `resolved_name != self.active_route` would fire the "Can't mix libraries" rejection toast even while attached to a Sessions-panel session or a non-library-route direct-remote connection -- both leave `active_route` at `None`, so *any* item that happens to resolve to a configured `daemon_routes` entry would be wrongly rejected, even though the real reason has nothing to do with library routing. `enqueue_route_conflict` itself is fixed below to short-circuit `false` (no conflict, allow the enqueue) whenever we are in one of those other thin-client modes -- centralizing the fix here means all three call sites (`enqueue_selected`'s two branches, `do_enqueue_folder`, `enqueue_artist_header_selection`) get it for free instead of needing the same check repeated at each site.

- [ ] **Step 1: Run impact analysis before editing**

```
impact({target: "enqueue_selected", direction: "upstream"})
impact({target: "do_enqueue_folder", direction: "upstream"})
impact({target: "enqueue_artist_header_selection", direction: "upstream"})
```

Report caller lists and risk level before proceeding.

- [ ] **Step 2: Write the failing tests**

Add to `mod tests` in `src/app/actions.rs`:

```rust
    #[test]
    fn enqueue_selected_rejects_item_from_a_different_route_than_active_queue() {
        let mut app = make_app_stub();
        app.daemon_routes.insert(
            "music".to_string(),
            "tcp://127.0.0.1:9000".to_string(),
        );
        app.active_route = Some("music".to_string());
        let mut movies_item = make_item("Movies", "CollectionFolder");
        movies_item.id = "lib-movies".to_string();
        app.libs.push(LibraryTab {
            library: movies_item,
            nav_stack: Vec::new(),
            search: None,
            feed_home_video: None,
            power_detail_scroll: Default::default(),
            album_track_focus: None,
            artist_header_focus: None,
        });
        app.tab_idx = app.lib_tab_offset();

        app.enqueue_selected();

        // `PlayerTab`/`PlaybackQueue`/`MediaItem` implement neither
        // `PartialEq` nor `Debug` in this codebase (confirmed: `MediaItem`
        // derives only `Debug, Clone, Serialize, Deserialize`, and
        // `PlayerTab` derives only `Clone, Default`), so a whole-struct
        // `assert_eq!` against a captured "before" clone will not compile.
        // The established idiom elsewhere in this test module (e.g. the
        // rollback-path tests) is to assert on `.items` directly instead
        // -- here that's simplest as "still empty", since `make_app_stub`
        // starts with an empty queue and a rejected enqueue must leave it
        // that way.
        assert!(app
            .queue_for_scope(app.visible_queue_scope())
            .items
            .is_empty());
        assert!(app.status.contains("Can't mix libraries in a routed queue"));
    }

    #[test]
    fn enqueue_route_conflict_allows_matching_route() {
        let mut app = make_app_stub();
        app.active_route = Some("music".to_string());
        assert!(!app.enqueue_route_conflict(Some("music".to_string())));
    }

    #[test]
    fn enqueue_route_conflict_allows_local_queue_local_item() {
        let mut app = make_app_stub();
        assert!(!app.enqueue_route_conflict(None));
    }

    #[test]
    fn enqueue_route_conflict_rejects_mismatched_route() {
        let mut app = make_app_stub();
        app.active_route = Some("music".to_string());
        assert!(app.enqueue_route_conflict(Some("movies".to_string())));
        assert!(app.status.contains("Can't mix libraries in a routed queue"));
    }

    #[test]
    fn enqueue_route_conflict_allows_enqueue_while_attached_to_a_session() {
        // A Sessions-panel attached session (`connected_session_id`) has
        // its own, separate queue-scope rules -- the library-routing
        // invariant must not fire a "Can't mix libraries" toast for a
        // reason unrelated to library routing.
        let mut app = make_app_stub();
        app.connected_session_id = Some("sess-1".to_string());
        assert!(!app.enqueue_route_conflict(Some("music".to_string())));
    }

    #[test]
    fn enqueue_route_conflict_allows_enqueue_while_on_a_non_route_direct_remote() {
        let mut app = make_app_stub();
        let (remote, remote_rx) = mbv_core::remote_player::RemotePlayer::stub(make_items(1), 0);
        app.player = mbv_core::player::PlayerProxy::remote(remote, false);
        app.player_rx = remote_rx;
        // active_route stays None: this is a Sessions-panel direct-remote
        // connection, not a library route.
        assert!(!app.enqueue_route_conflict(Some("music".to_string())));
    }
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test enqueue_selected_rejects_item_from_a_different_route enqueue_route_conflict`
Expected: FAIL — `enqueue_route_conflict` doesn't exist yet (compile error), and the reject test doesn't see the toast.

- [ ] **Step 4: Implement `enqueue_route_conflict`**

Add to `src/app/actions.rs`, near `flash_status_high` (~line 2241):

```rust
    /// Enforces #223's queue-route invariant: an item whose resolved
    /// route differs from the queue's current route (`active_route`) is
    /// rejected with a toast instead of being appended or silently
    /// swapping the player. Returns `true` if the enqueue was rejected --
    /// the caller must abort without mutating the queue.
    ///
    /// Short-circuits `false` (no conflict) whenever the app is currently
    /// in a thin-client mode that has nothing to do with library routing
    /// (a Sessions-panel attached session, or a non-library-route direct
    /// remote / local-daemon connection) -- both leave `active_route` at
    /// `None`, so without this check any item resolving to a configured
    /// `daemon_routes` entry would be wrongly rejected for a reason
    /// unrelated to library routing. Mirrors the same condition Task 9
    /// uses to gate `apply_route_for_playback`.
    pub(super) fn enqueue_route_conflict(&mut self, resolved_name: Option<String>) -> bool {
        let in_other_thin_client_mode = self.connected_session_id.is_some()
            || (self.player.is_remote() && self.active_route.is_none());
        if in_other_thin_client_mode {
            return false;
        }
        if resolved_name != self.active_route {
            self.flash_status_high(
                "Can't mix libraries in a routed queue -- clear queue first".to_string(),
            );
            true
        } else {
            false
        }
    }
```

- [ ] **Step 5: Wire the guard into all three enqueue call sites**

Per this repo's `CLAUDE.md`, use Serena's `replace_symbol_body` for these edits rather than a text-anchored `Edit`/`replace_content` call: `enqueue_selected`'s two branches both end their `is_playable`/`let name = item.display_name();` sequence with byte-identical text (same four lines, same indentation, differing only in how `item` was obtained a few lines earlier), so a plain string-anchored replace of just the `let name = item.display_name();` line is ambiguous -- both occurrences would match the same `old_string`. `replace_symbol_body` on the whole `enqueue_selected` method sidesteps this by targeting the symbol, not a text pattern. The complete new body (identical to the current one except for the two inserted guards) is:

```rust
    pub(super) fn enqueue_selected(&mut self) {
        if self.tab_idx == 0 {
            let Some(item) = self.current_home_item() else {
                return;
            };
            if item.is_folder {
                self.do_enqueue_folder(item);
                return;
            }
            if !is_playable(&item) {
                return;
            }
            let resolved = self.route_for_item_via_ancestors(&item.id).map(|(n, _)| n);
            if self.enqueue_route_conflict(resolved) {
                return;
            }
            let name = item.display_name();
            let scope = self.visible_queue_scope();
            let appended = item.clone();
            let previous_dirty = self.queue_dirty;
            let previous_queue = self.queue_for_scope(scope).clone();
            {
                self.queue_for_scope_mut(scope).append_item(item);
            }
            if self.local_queue_metadata_applies(scope) {
                self.queue_dirty = true;
            }
            self.flash_status(format!("Added: {name}"));
            if self.sync_playback_queue_after_append(scope, vec![appended]) {
                self.sync_direct_remote_queue_after_edit(scope);
                self.persist_local_queue_state_if_needed(scope);
            } else {
                self.queue_dirty = previous_dirty;
                *self.queue_for_scope_mut(scope) = previous_queue;
            }
        } else if self.tab_idx >= 2 {
            if self.enqueue_selected_artist_header() {
                return;
            }
            let Some(item) = self.current_lib_item() else {
                return;
            };
            if item.is_folder {
                self.do_enqueue_folder(item);
                return;
            }
            if !is_playable(&item) {
                return;
            }
            let lib_idx = self.tab_idx - self.lib_tab_offset();
            let resolved = self.route_for_active_library_view(lib_idx).map(|(n, _)| n);
            if self.enqueue_route_conflict(resolved) {
                return;
            }
            let name = item.display_name();
            let scope = self.visible_queue_scope();
            let appended = item.clone();
            let previous_dirty = self.queue_dirty;
            let previous_queue = self.queue_for_scope(scope).clone();
            {
                self.queue_for_scope_mut(scope).append_item(item);
            }
            if self.local_queue_metadata_applies(scope) {
                self.queue_dirty = true;
            }
            self.flash_status(format!("Added: {name}"));
            if self.sync_playback_queue_after_append(scope, vec![appended]) {
                self.sync_direct_remote_queue_after_edit(scope);
                self.persist_local_queue_state_if_needed(scope);
            } else {
                self.queue_dirty = previous_dirty;
                *self.queue_for_scope_mut(scope) = previous_queue;
            }
        }
    }
```

Use `mcp__serena__replace_symbol_body` with `name_path_pattern: "enqueue_selected"`, `relative_path: "src/app/actions.rs"`, and the body above (everything between, but not including, the outer `{ }` of the function -- follow Serena's existing convention for this tool, i.e. pass the full function including signature as shown, consistent with how this repo's other `replace_symbol_body` calls are made).

In `do_enqueue_folder` (~line 2576), add the guard as the first line of the function body, calling `resolve_route_for_enqueue_folder` (Task 7's library-root-aware resolver) instead of `route_for_item_via_ancestors` directly:

```rust
    pub(super) fn do_enqueue_folder(&mut self, item: mbv_core::api::MediaItem) {
        let resolved = self.resolve_route_for_enqueue_folder(&item);
        if self.enqueue_route_conflict(resolved) {
            return;
        }
        let client = self.client.lock().unwrap();
```

In `enqueue_artist_header_selection` (~line 2473), add the guard as the first line of the function body, using the `lib_idx` parameter already passed in:

```rust
    fn enqueue_artist_header_selection(
        &mut self,
        lib_idx: usize,
        selection: &ArtistHeaderSelection,
    ) -> bool {
        let resolved = self.route_for_active_library_view(lib_idx).map(|(n, _)| n);
        if self.enqueue_route_conflict(resolved) {
            return true;
        }
        let items = match self.resolve_artist_header_playable_items(lib_idx, selection) {
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test enqueue_selected_rejects_item_from_a_different_route enqueue_route_conflict`
Expected: PASS (6 passed)

- [ ] **Step 7: Run the full actions.rs test suite to check for regressions**

Run: `cargo test --lib app::actions::tests`
Expected: all existing enqueue tests still PASS (when `active_route` is `None`, `daemon_routes` is empty, `connected_session_id` is `None`, and `self.player.is_remote()` is `false` -- i.e. every existing local-only test's starting state -- `resolved_name` is always `None`, matching `self.active_route == None`, so `enqueue_route_conflict` never fires for existing local-only flows).

- [ ] **Step 8: Run `detect_changes` before committing**

```
detect_changes({scope: "compare", base_ref: "main"})
```

- [ ] **Step 9: Commit**

```bash
git add src/app/actions.rs
git commit -m "actions: reject enqueue that mixes library routes in one queue"
```

---

### Task 11: Status-bar visibility — prefer `active_route` in the remote pill

**Files:**
- Modify: `src/app/render/mod.rs:768-818` (`remote_status_spans`)

**Interfaces:**
- Consumes: `App.active_route` (Task 3), existing `RemoteSlotState::DirectRemote` classification (`src/app/mod.rs:1511-1523`).

- [ ] **Step 1: Run impact analysis before editing**

```
impact({target: "remote_status_spans", direction: "upstream"})
```

Report caller list and risk before proceeding — expected to be called only from the status-bar render path.

- [ ] **Step 2: Write the failing test**

Add to `mod tests` in `src/app/render/mod.rs` (near `status_bar_omits_alive_marker_when_overflow_chooses_without_alive`, ~line 1306, which already shows the harness pattern for calling `remote_status_spans` directly):

```rust
    #[test]
    fn remote_status_spans_prefers_active_route_label_over_daemon_endpoint() {
        let mut app = make_app_stub();
        app.active_route = Some("music".to_string());
        let spans = app.remote_status_spans(RemoteSlotState::DirectRemote, "tcp://127.0.0.1:9000");
        let text: String = spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains("music"));
    }
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test remote_status_spans_prefers_active_route_label_over_daemon_endpoint`
Expected: FAIL — label falls back to `daemon_endpoint_label("tcp://127.0.0.1:9000")` instead of showing `"music"`.

- [ ] **Step 4: Prefer `active_route` in the `DirectRemote` label branch**

In `src/app/render/mod.rs`, in `remote_status_spans` (~line 782), replace the `DirectRemote` match arm:

```rust
            super::RemoteSlotState::DirectRemote => self
                .direct_remote_label
                .clone()
                .or_else(|| daemon_endpoint_label(daemon_endpoint)),
```

with:

```rust
            super::RemoteSlotState::DirectRemote => self
                .active_route
                .as_ref()
                .map(|name| format!("route:{name}"))
                .or_else(|| self.direct_remote_label.clone())
                .or_else(|| daemon_endpoint_label(daemon_endpoint)),
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test remote_status_spans_prefers_active_route_label_over_daemon_endpoint`
Expected: PASS

- [ ] **Step 6: Run the render module test suite to check for regressions**

Run: `cargo test --lib app::render::mod::tests`
Expected: all existing tests still PASS (in particular any test asserting `direct_remote_label` shows through when `active_route` is `None`, which is unchanged by this `or_else` chain).

- [ ] **Step 7: Commit**

```bash
git add src/app/render/mod.rs
git commit -m "render: prefer active library route label in the remote status pill"
```

---

### Task 12: `CONTEXT.md` glossary additions

**Files:**
- Modify: `CONTEXT.md`

**Interfaces:** None (documentation only).

- [ ] **Step 1: Add the new terms**

In `CONTEXT.md`, after the **Suspended local session** entry (line 31-33), add:

```markdown
**Library route** / **Route table** (`daemon_routes`):
A `[daemon_routes]` config table mapping library name (matched case-insensitively, same convention as `hidden_libraries`/`feed_view_libraries`) to a daemon endpoint string. A play/enqueue action resolved to a routed library swaps the active player to that daemon via `switch_to_library_route`, a sibling to `switch_to_direct_remote` using the same **Suspended local session** mechanism -- tracked by its own `active_route` field, kept independent of the Sessions-panel direct-remote's `connected_session_id`/`direct_remote_label`. `"*"` is a wildcard "route everything" entry. TOML-only for v1; no settings-panel UI. See #223.
_Avoid_: conflating a library route with a Sessions-panel **Thin client** connection -- they are two independent ways to end up thin-client, and connecting to either takes driving-client authority over that daemon (ADR 0007) as an accepted consequence, not a hidden side effect.

**Routed queue**:
A queue whose route (local, or a specific library route) was decided once, from the item(s) that started it, and is fixed for that queue's lifetime (`App::active_route`). Enqueuing an item that resolves to a *different* route than the current queue's is rejected outright with a toast; no auto-clear, no auto-swap. Starting a brand-new queue (replace, not append) re-evaluates the route from scratch. Mid-queue per-track route swapping is explicitly out of scope. See #223.
```

Also amend the existing **Suspended local session** entry (line 31-33) to note it now has two callers:

```markdown
**Suspended local session**:
The local player and its event channels, parked in place when control is handed off to a direct remote, so that local playback can be resumed later without rebuilding it from scratch. Used by two independent callers: the Sessions-panel direct-remote upgrade (`switch_to_direct_remote`) and library-scoped daemon routing (`switch_to_library_route`, #223) -- both restore through the same `restore_local_mode`.
_Avoid_: conflating with remote slot state -- that's a classification of the current relationship; this is a stashed resource that exists only during part of one such relationship (direct remote or library route).
```

- [ ] **Step 2: Verify the file is still valid markdown**

Run: `grep -c "^##" CONTEXT.md`
Expected: same section count as before the edit (no headings were accidentally broken by the insertion).

- [ ] **Step 3: Commit**

```bash
git add CONTEXT.md
git commit -m "docs: add Library route / Route table / Routed queue glossary terms"
```

---

### Task 13: New ADR — library-scoped daemon routing

**Note on numbering:** #222's sibling plan (`docs/superpowers/plans/2026-07-17-daemon-connect-lifecycle.md`) already claims `docs/adr/0010-lazy-daemon-route-connect-lifecycle.md` for its own new ADR. This task uses `0011` to avoid the collision -- check `ls docs/adr/` immediately before writing this file in case a *different* ADR has landed at 0010 or 0011 in the meantime, and bump further if needed.

**Files:**
- Create: `docs/adr/0011-library-scoped-daemon-routing.md`

**Interfaces:** None (documentation only).

- [ ] **Step 1: Write the ADR**

```markdown
# Library-Scoped Daemon Routing

## Decision

Daemon routing can be decided per library via a `[daemon_routes]` config
table (library name, case-insensitive -> daemon endpoint string, plus an
optional `"*"` wildcard). Resolving a play/enqueue action to a routed
library swaps the active player to that daemon; other libraries keep
playing locally, unaffected.

Route resolution is queue-level, not per-track: a queue's route is decided
once from the item(s) that started it and held for that queue's lifetime.
Enqueuing an item whose resolved route differs from the queue's current
route is rejected with a toast, not auto-swapped or auto-cleared. Starting
a new queue (replace) re-evaluates the route from scratch.

Route resolution order:
1. Nav-scoped views (Library tab, Power View, Album/Artist drill-down,
   in-library search) resolve the active library directly from
   navigation state -- no network call.
2. Cross-library aggregate views (Home tab: Continue Watching/Next Up,
   Favorites) resolve via `EmbyClient::get_ancestors`, walking the
   item's ancestor chain to its owning `CollectionFolder`. Cached per
   item id for the session. `do_enqueue_folder` additionally checks
   whether the enqueued item is itself a library root before falling
   back to ancestor lookup, since a library root has no `CollectionFolder`
   ancestor above it for the lookup to find.
3. The Queue tab (no library context of its own) keeps whatever route is
   already active rather than re-resolving or clearing it.
4. No match in any case means local playback.

Connecting to a routed library's daemon **is** takeover of that daemon's
driving-client slot (ADR 0003/0007) -- an accepted, explicit consequence,
not a hidden side effect. This matters if multiple devices route to the
same music-only daemon. The connect attempt itself, including this
consequence, is delegated to `App::try_daemon_route_connect`
(ADR 0010, #222) rather than re-implemented here.

Library routing (`active_route`) is tracked independently of the
Sessions-panel direct-remote concept (`connected_session_id` /
`direct_remote_label`); they are two separate ways to end up thin-client
and must never be conflated in `App` state, even though both reuse the
same suspend/restore machinery (`SuspendedLocalSession`,
`switch_to_direct_remote`/`switch_to_library_route`,
`restore_local_mode`). Two specific conflation hazards were identified and
closed: (1) a play/enqueue action while already thin-client for a reason
*other* than library routing (Sessions-panel direct-remote, local-daemon
mode) must not let library routing swap the player out from under that
other connection; (2) a Sessions-panel direct-remote upgrade while already
on a library route must clear the stale `active_route`, since
`switch_to_direct_remote`'s already-remote branch does not itself go
through `restore_local_mode`.

## Context

ADR 0010 (#222) established the connection lifecycle this depends on:
fully lazy connect (never at startup), fallback to local playback with a
status-bar warning on a failed connect via `App::try_daemon_route_connect`,
no background retry, no connection parking (disconnect cleanly on
swap-away; reconnect fresh next time). This issue (#223) reuses that
lifecycle -- calling `try_daemon_route_connect` directly, on the same
`DAEMON_ROUTE_CONNECT_OVERRIDE` test seam ADR 0010 introduced -- for
per-library routing rather than only the wildcard "route everything" case
#222 introduced.

## Consequences

- `Config` gains a `daemon_routes: HashMap<String, String>` field,
  parsed like `hidden_libraries`/`feed_view_libraries` (lowercased keys),
  but with no settings-panel write-back for v1 -- TOML-only, matching the
  `hidden_libraries` value-editing precedent without exposing this table
  for in-app editing.
- `App` gains `active_route: Option<String>` and a per-item
  `library_route_cache` for ancestor-lookup memoization.
- `restore_local_mode` is the single shared "go back to local" tail for
  both the Sessions-panel and library-route thin-client paths; it clears
  `active_route` in addition to its existing resets.
- A malformed `daemon_routes` endpoint value is logged and skipped
  (treated as no route for that library) rather than failing startup or
  blocking other routes.
- Mid-queue per-track route swapping and connection parking/reuse across
  route switches remain explicitly out of scope, per #223.
```

- [ ] **Step 2: Commit**

```bash
git add docs/adr/0011-library-scoped-daemon-routing.md
git commit -m "docs: add ADR 0011 for library-scoped daemon routing"
```

---

## Self-Review

**1. Spec coverage against issue #223's acceptance criteria:**
- "Library-specific remote routing is configurable via `daemon_routes` in `config.toml`" -> Task 1.
- "Playback/enqueue started from a library-scoped view resolves its route from nav context with no network call; cross-library aggregate views resolve via `get_ancestors`" -> Task 7 (`route_for_active_library_view` vs `route_for_item_via_ancestors`), wired in Tasks 9-10.
- "Starting playback from a routed library swaps to that daemon (per #222's connect/fallback rules); starting playback from a non-routed library uses local playback, unaffected" -> Tasks 5, 8, 9, now via #222's actual `App::try_daemon_route_connect` primitive rather than a placeholder reuse of the Sessions-panel connect helper.
- "Enqueuing an item whose route conflicts with the current queue's route is rejected with an explanatory toast, queue left unchanged" -> Task 10.
- "The effective route (local vs. which library route) is visible in app status and diagnosable in logs" -> Task 11 (status pill) + `log::info!`/`log::warn!` calls under the `"library_route"` target added in Tasks 4, 5, 7, 8 (plus #222's own `"daemon_route"`-target logging for the connect attempt itself).
- Docs impact (`CONTEXT.md`, new ADR) -> Tasks 12, 13 (ADR now numbered 0011 to avoid colliding with #222's ADR 0010).
- Explicitly out-of-scope items (mid-queue per-track swap, connection parking, config UI, migrating `daemon_client_endpoint`) -> none of the tasks implement these; Task 13's ADR states them explicitly as non-goals.

**2. Placeholder scan:** No "TBD"/"handle appropriately"/"add validation later" language appears in any task. Every step shows the actual code to write. The one caveat is Task 7 Step 1's note about matching `LibraryTab`'s exact field list if it doesn't line up character-for-character with what's shown -- this is an instruction to copy an existing, already-written pattern from the codebase (not "figure it out"), consistent with the file paths and existing symbols already cited throughout the plan, not a deferred design decision. Task 10's original queue-comparison hedge ("if `PartialEq` doesn't hold...") was resolved definitively during this review rather than left conditional -- confirmed via Serena that neither `MediaItem` nor `PlayerTab` derive `PartialEq`/`Debug`, so Task 10's test now asserts on `.items.is_empty()` directly, matching this test module's established idiom, with no "if not" branch left in the plan.

**3. Type/signature consistency check:**
- `resolve_daemon_route(routes: &HashMap<String, String>, library_name: &str) -> Option<&str>` (Task 2) is called consistently as `mbv_core::config::resolve_daemon_route(&self.daemon_routes, name)` in Task 4 -- same argument order and types.
- `resolve_route_for_library(&self, library_name: &str) -> Option<(String, DaemonEndpoint)>` (Task 4) is called identically in Task 7's `route_for_active_library_view`, `route_for_item_via_ancestors`, `resolve_route_for_play`, and `resolve_route_for_enqueue_folder`.
- `resolve_route_for_play(&mut self, item: &MediaItem) -> Option<(String, DaemonEndpoint)>` (Task 7) is called identically in Task 8's `apply_route_for_playback`. Its `tab_idx == 1` (Queue tab) branch was added during this review to fix a `usize` underflow panic the original two-way `if tab_idx == 0 { .. } else { .. }` split had -- `lib_tab_offset()` is `2`, so `tab_idx - lib_tab_offset()` for `tab_idx == 1` would have subtracted `2` from `1`.
- `resolve_route_for_enqueue_folder(&mut self, item: &MediaItem) -> Option<String>` (added to Task 7 during this review) is called identically in Task 10's `do_enqueue_folder` guard, replacing the plan's original direct call to `route_for_item_via_ancestors` there.
- `App::try_daemon_route_connect(&mut self, endpoint: &DaemonEndpoint, route_label: &str) -> Result<(RemotePlayer, mpsc::Receiver<PlayerEvent>), String>` (#222's plan, Task 1 -- not defined in this plan, only consumed) is called identically in Task 8's `apply_route_for_playback` (`self.try_daemon_route_connect(&endpoint, &name)`), matched on `Ok(..)`/`Err(message)` -- verified directly against #222's current plan file, which itself was revised from an earlier `Option`-returning shape to this `Result`-returning one for exactly the reason this review's item 1 above describes (the caller needs the message text to route through different display paths depending on `active_route` state).
- `switch_to_library_route(&mut self, library_name: &str, remote: RemotePlayer, remote_rx: Receiver<PlayerEvent>)` (Task 5) is called identically in Task 8's `apply_route_for_playback` (`self.switch_to_library_route(&name, remote, remote_rx)`), matching the `(String, DaemonEndpoint)` destructure's `name: String` (borrowed as `&name`) against the `&str` parameter.
- `enqueue_route_conflict(&mut self, resolved_name: Option<String>) -> bool` (Task 10) is called consistently everywhere it's wired in, always passed `Option<String>` (from `.map(|(n, _)| n)` on a `(String, DaemonEndpoint)` resolver result, or directly from `resolve_route_for_enqueue_folder`'s own `Option<String>`), matching `active_route: Option<String>`'s type for the `!=` comparison.
- `active_route` is read/written only as `Option<String>` everywhere (Tasks 3, 5, 6, 7, 8, 9, 10, 11) -- no task treats it as `Option<&str>` or a different shape.
- Field name `daemon_routes` used consistently on both `Config` (Task 1) and `App`/`AppInit` (Task 3) -- no `App` field is ever called `routes` or `library_routes` elsewhere in the plan. Confirmed via GitNexus that `AppInit` has exactly three real construction sites (`App::new`, `App::new_remote`, `make_built_app`); Task 3 originally covered only the first two plus the separate raw-`App`-literal `make_app_stub` helper -- `make_built_app`'s own `AppInit { .. }` literal was a missed fourth site, fixed in Task 3 during this review.

**4. Cross-plan consistency with #222 (added during this review pass):** Verified against `docs/superpowers/plans/2026-07-17-daemon-connect-lifecycle.md` directly (not from memory, and re-verified a second time after an earlier pass of this same review had already drafted Task 8 against a stale, `Option`-returning recollection of the signature): the primitive's real, current signature is `App::try_daemon_route_connect(&mut self, endpoint: &DaemonEndpoint, route_label: &str) -> Result<(RemotePlayer, mpsc::Receiver<PlayerEvent>), String>`. It logs the raw failure internally and returns `Err(message)` -- a fully-formatted, ready-to-display warning (`\u{26a0} {route_label} route unreachable, using local playback (mbv.log)`) -- without flashing it itself, and it introduces its own `DAEMON_ROUTE_CONNECT_OVERRIDE`/`DAEMON_ROUTE_CONNECT_TEST_LOCK` test seam separate from the pre-existing `DIRECT_CONNECT_OVERRIDE`/`DIRECT_CONNECT_TEST_LOCK`. This plan's Task 8/9 now call that primitive directly (previously they reused `connect_direct_endpoint`/`DIRECT_CONNECT_OVERRIDE`, written before #222's plan existed) and match on `Ok`/`Err`, forwarding the `Err` message verbatim to whichever display path `apply_route_for_playback` chooses -- no separate message-formatting helper was needed once the real `Result` contract was confirmed. Task 13's ADR is renumbered `0011` since #222 claims `0010`.
