# Library-Scoped Daemon Routing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let mbv route play/enqueue actions to a per-library daemon (`[daemon_routes]` in `config.toml`), swapping the active player to that daemon while other libraries keep playing locally — per GitHub issue #223.

**Architecture:** A new `daemon_routes: HashMap<String, String>` config table (library name, lowercased -> endpoint string) is parsed alongside the existing `hidden_libraries`/`feed_view_libraries` convention. `App` gains `daemon_routes` (copied from config), `active_route: Option<String>` (the lowercased library name currently driving playback via a routed daemon, kept separate from the Sessions-panel `connected_session_id`/`direct_remote_label` concept), and `library_route_cache: HashMap<String, Option<String>>` (per-item ancestor-lookup memoization for cross-library aggregate views). A new `switch_to_library_route` method is a sibling to the existing `switch_to_direct_remote` — same suspend-local/connect-remote shape, reusing `SuspendedLocalSession`/`self.suspended_local`, but targeting a statically configured `DaemonEndpoint` instead of a discovered `SessionInfo`. `restore_local_mode` is extended (not duplicated) to also clear `active_route`, since it is already the single "go back to local" tail for every thin-client path. Route resolution has two paths per the issue: nav-scoped (library tab already known, no network call) and ancestor-lookup (`EmbyClient::get_ancestors`, cached per item) for cross-library aggregate views (Home tab). Both play and enqueue call through a shared resolver; play performs the swap, enqueue enforces the no-mixed-route queue invariant with a rejection toast.

**Tech Stack:** Rust workspace (`crates/mbv-core` shared lib, `src/` `mbv` TUI binary), `cargo test`, plain `#[test]` functions, no mocking framework — reuses the existing `DIRECT_CONNECT_OVERRIDE` test seam in `src/app/mod.rs`.

## Global Constraints

- Case-insensitive library-name matching, same convention as `hidden_libraries`/`feed_view_libraries`: config values lowercased at parse time, lookups lowercase the query side too (`crates/mbv-core/src/config.rs`).
- `daemon_routes` is TOML-only for v1 — no config-UI/settings-panel write-back (issue #223's explicit out-of-scope list). It is still read into `Config` and copied into `App` exactly like `hidden_libraries`.
- `active_route: Option<String>` on `App` must stay independent of `connected_session_id` / `connected_session_state` / `direct_remote_label` — never read or write library-route state through those fields, and vice versa (issue #223's explicit instruction).
- Connection lifecycle for a routed daemon: lazy connect (only attempted at the first play/enqueue that resolves to that route), fallback-to-local-with-a-status-bar-warning on failure, no retry loop, no connection parking (issue #222's rules, which #223 depends on — see Assumptions below on sequencing).
- Rejection toast text for a mixed-route enqueue, verbatim from the issue: `"Can't mix libraries in a routed queue -- clear queue first"`.
- New log target for library-routing diagnostics: `"library_route"` (distinct from the existing `"sessions"` target used by `switch_to_direct_remote`/`connect_to_session`).
- `switch_to_library_route` is a new sibling method to `switch_to_direct_remote` — do not modify `switch_to_direct_remote` itself.

## Assumptions (open questions carried forward — see final report)

1. **#222 sequencing risk:** as of this plan, no branch or merged code implements #222's connection-lifecycle primitive; `git branch -a` shows no `222`/`lazy-connect`-named branch. This plan does not block on #222 landing first — it reuses the existing `App::connect_direct_endpoint` helper (`src/app/mod.rs:2315-2332`), which already has the exact `fn(&DaemonEndpoint, &str) -> Result<(RemotePlayer, Receiver<PlayerEvent>), String>` shape the task brief described as #222's expected primitive, and already fails over to a caller-driven fallback (no retry loop, no parking) matching #222's rules. If #222 lands first with a differently-named wrapper, swapping the call site in Task 8 is a one-line change.
2. **Sessions-panel / library-route interaction:** the issue does not say what happens if a Sessions-panel direct-remote (`connected_session_id`) is connected when a play/enqueue would otherwise resolve to a library route. This plan takes the conservative default: library routing is skipped entirely while `connected_session_id.is_some()` (the Sessions-panel path wins). Flagged for follow-up review.
3. **Enqueuing a library-root folder item itself:** `do_enqueue_folder` resolves its route via ancestor lookup of the folder item's own id. If the folder *is* a library root (`CollectionFolder`), `get_ancestors` returns no ancestors above it, so route resolution yields `None` (treated as local) rather than matching the folder's own name. Not addressed by the issue text; flagged for follow-up review.
4. **Status-bar visibility:** issue #223's acceptance criteria require the effective route to be "visible in app status and diagnosable in logs." This plan satisfies it by (a) reusing the existing remote-pill mechanism (`remote_status_spans` in `src/app/render/mod.rs`, already shows a label for `RemoteSlotState::DirectRemote`) to prefer `active_route` when set, and (b) `log::info!`/`log::warn!` at every swap/restore/fallback point under the `"library_route"` target — matching the level of persistence `direct_remote_label` already gets today (a pill label, not a dedicated new divider indicator).

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
```

Note: `LibraryTab` field list must match its current definition at `src/app/mod.rs:824-840` exactly (`library`, `nav_stack`, `search`, `feed_home_video`, `power_detail_scroll`, `album_track_focus`, `artist_header_focus`) — if any field's type doesn't accept the literal shown (e.g. `nav_stack: Vec::new()` vs a different default), match the existing test helper that already builds a `LibraryTab` elsewhere in `mod tests` and copy its exact construction instead of the literal above.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test route_for_active_library_view route_for_item_via_ancestors`
Expected: FAIL with `no method named 'route_for_active_library_view' found for struct 'App'` (and similarly for the other two).

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
    fn resolve_route_for_play(
        &mut self,
        item: &mbv_core::api::MediaItem,
    ) -> Option<(String, mbv_core::remote_player::DaemonEndpoint)> {
        if self.tab_idx == 0 {
            self.route_for_item_via_ancestors(&item.id)
        } else {
            let lib_idx = self.tab_idx - self.lib_tab_offset();
            self.route_for_active_library_view(lib_idx)
        }
    }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test route_for_active_library_view route_for_item_via_ancestors`
Expected: PASS (3 passed)

- [ ] **Step 5: Commit**

```bash
git add src/app/mod.rs
git commit -m "app: add nav-scoped and ancestor-lookup route resolvers"
```

---

### Task 8: App — `apply_route_for_playback` orchestration (connect/fallback)

**Files:**
- Modify: `src/app/mod.rs` — new method near `connect_to_session` (~2441)

**Interfaces:**
- Consumes: `resolve_route_for_play` (Task 7), `App.active_route` (Task 3), `switch_to_library_route` (Task 5), `restore_local_mode` (Task 6), `connect_direct_endpoint` (existing, `src/app/mod.rs:2315-2332`, reusing the `DIRECT_CONNECT_OVERRIDE` test seam).
- Produces: `fn apply_route_for_playback(&mut self, item: &mbv_core::api::MediaItem)` — Task 9 calls this from `play_item`/`play_items_routed`.

- [ ] **Step 1: Write the failing tests**

Add to `mod tests` in `src/app/mod.rs`:

```rust
    #[test]
    fn apply_route_for_playback_swaps_to_routed_daemon_on_success() {
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
            Ok(mbv_core::remote_player::RemotePlayer::stub(make_items(1), 0))
        }
        *DIRECT_CONNECT_OVERRIDE.lock().unwrap() = Some(direct_success);

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

        *DIRECT_CONNECT_OVERRIDE.lock().unwrap() = None;
        assert_eq!(app.active_route.as_deref(), Some("music"));
        assert!(app.player.is_remote());
    }

    #[test]
    fn apply_route_for_playback_falls_back_to_local_with_warning_on_connect_failure() {
        let _guard = crate::config::TestStateDirGuard::new();
        let _connect_guard = DIRECT_CONNECT_TEST_LOCK.lock().unwrap();
        fn direct_failure(
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
        *DIRECT_CONNECT_OVERRIDE.lock().unwrap() = Some(direct_failure);

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

        *DIRECT_CONNECT_OVERRIDE.lock().unwrap() = None;
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

        // No connect attempt was needed (no DIRECT_CONNECT_OVERRIDE set,
        // so a real connect attempt would panic/hang if this weren't a
        // no-op) -- active_route and local-ness are unchanged.
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
    /// Connection failure falls back to local playback with a
    /// status-bar warning and a log line -- never a hard error, per
    /// #222's fallback rule.
    fn apply_route_for_playback(&mut self, item: &mbv_core::api::MediaItem) {
        let resolved = self.resolve_route_for_play(item);
        match (resolved, self.active_route.clone()) {
            (Some((name, _)), Some(current)) if name == current => {}
            (Some((name, endpoint)), _) => {
                let auth_token = self.client.lock().unwrap().token.clone();
                match self.connect_direct_endpoint(&endpoint, &auth_token) {
                    Ok((remote, remote_rx)) => {
                        self.switch_to_library_route(&name, remote, remote_rx);
                    }
                    Err(e) => {
                        log::warn!(
                            target: "library_route",
                            "connect to library route {name:?} endpoint {endpoint} failed: {e}"
                        );
                        let warning =
                            format!("{name} route unreachable, using local playback (mbv.log)");
                        if self.active_route.is_some() {
                            self.restore_local_mode(&warning);
                        } else {
                            self.flash_status_high(warning);
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

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test apply_route_for_playback`
Expected: PASS (4 passed)

- [ ] **Step 5: Commit**

```bash
git add src/app/mod.rs
git commit -m "app: add apply_route_for_playback (connect/fallback orchestration)"
```

---

### Task 9: Wire route swap into `play_item` and `play_items_routed`

**Files:**
- Modify: `src/app/actions.rs:2327-2368` (`play_item`), `src/app/actions.rs:2291-2325` (`play_items_routed`)

**Interfaces:**
- Consumes: `apply_route_for_playback` (Task 8), `App.connected_session_id` (existing).

- [ ] **Step 1: Run impact analysis before editing**

```
impact({target: "play_item", direction: "upstream"})
impact({target: "play_items_routed", direction: "upstream"})
```

Report the caller lists and any HIGH/CRITICAL risk before proceeding — both are called from many nav/input-handling call sites across `src/app/`, so this is an additive guard at the top of each, not a signature or control-flow change for existing callers when no route applies.

- [ ] **Step 2: Write the failing tests**

Add to `mod tests` in `src/app/actions.rs`:

```rust
    #[test]
    fn play_item_swaps_to_library_route_before_replacing_queue() {
        let _guard = crate::config::TestStateDirGuard::new();
        let _connect_guard = crate::app::DIRECT_CONNECT_TEST_LOCK.lock().unwrap();
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
            Ok(mbv_core::remote_player::RemotePlayer::stub(make_items(1), 0))
        }
        *crate::app::DIRECT_CONNECT_OVERRIDE.lock().unwrap() = Some(direct_success);

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

        *crate::app::DIRECT_CONNECT_OVERRIDE.lock().unwrap() = None;
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

        // No DIRECT_CONNECT_OVERRIDE set -- if library routing engaged
        // here it would attempt a real connection and this test would
        // hang/fail rather than reach the assertion below.
        app.play_item(item);

        assert!(app.active_route.is_none());
    }
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test play_item_swaps_to_library_route_before_replacing_queue play_item_skips_library_routing_when_attached_to_a_session`
Expected: FAIL — `active_route` stays `None` after `play_item` in the first test (no wiring yet).

- [ ] **Step 4: Wire `apply_route_for_playback` into both functions**

In `src/app/actions.rs`, `play_item` (~line 2327), add the guard as the very first line of the function body:

```rust
    pub(super) fn play_item(&mut self, item: MediaItem) {
        if self.connected_session_id.is_none() {
            self.apply_route_for_playback(&item);
        }
        self.on_queue_replace_silent();
```

In `src/app/actions.rs`, `play_items_routed` (~line 2291), add the guard as the very first line of the function body:

```rust
    pub(super) fn play_items_routed(&mut self, items: Vec<MediaItem>, start_idx: usize) {
        if self.connected_session_id.is_none() {
            if let Some(item) = items.get(start_idx).or_else(|| items.first()) {
                let item = item.clone();
                self.apply_route_for_playback(&item);
            }
        }
        self.on_queue_replace_silent();
```

Also add `use mbv_core::remote_player::DaemonEndpoint;` style imports only if `src/app/actions.rs` doesn't already reach `App`'s methods without them — since `apply_route_for_playback` is a method on `App` (defined in `mod.rs`, same `impl App` type, different file), no additional import is needed; Rust resolves inherent methods across `impl` blocks in different files automatically for the same crate.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test play_item_swaps_to_library_route_before_replacing_queue play_item_skips_library_routing_when_attached_to_a_session`
Expected: PASS (2 passed)

- [ ] **Step 6: Run the full actions.rs test suite to check for regressions**

Run: `cargo test --lib app::actions::tests`
Expected: all existing tests still PASS (in particular any existing `play_item`/`play_items_routed` tests that don't configure `daemon_routes` — `apply_route_for_playback` must be a true no-op when `daemon_routes` is empty).

- [ ] **Step 7: Run `detect_changes` before committing**

```
detect_changes({scope: "compare", base_ref: "main"})
```

Confirm the only behavior-affecting flows touched are `play_item` and `play_items_routed`'s own execution paths, not unrelated ones.

- [ ] **Step 8: Commit**

```bash
git add src/app/actions.rs
git commit -m "actions: swap to library route before play_item/play_items_routed replace queue"
```

---

### Task 10: Enforce the no-mixed-route queue invariant on enqueue

**Files:**
- Modify: `src/app/actions.rs:2370-2435` (`enqueue_selected`), `src/app/actions.rs:2473-2514` (`enqueue_artist_header_selection`), `src/app/actions.rs:2576-2616` (`do_enqueue_folder`)

**Interfaces:**
- Consumes: `resolve_route_for_play`, `route_for_active_library_view`, `route_for_item_via_ancestors` (Task 7), `App.active_route` (Task 3), `flash_status_high` (existing, `src/app/actions.rs:2241-2246`).
- Produces: `fn enqueue_route_conflict(&mut self, resolved_name: Option<String>) -> bool` — `true` means the caller must abort without mutating the queue.

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
        let queue_before = app.queue_for_scope(app.visible_queue_scope()).clone();

        app.enqueue_selected();

        assert_eq!(app.queue_for_scope(app.visible_queue_scope()), &queue_before);
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
```

Note: `queue_for_scope` must return a `&PlaybackQueue`-family type implementing `PartialEq` for the comparison above to compile; if it does not, replace `assert_eq!(app.queue_for_scope(...), &queue_before)` with an equivalent field-by-field comparison already used elsewhere in this test module for queue-unchanged assertions (e.g. compare `.items` and `.len()` the way `enqueue_selected`'s existing rollback-path tests do) — check `src/app/actions.rs`'s existing enqueue tests for the established idiom and match it exactly rather than assuming `PartialEq`.

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test enqueue_selected_rejects_item_from_a_different_route enqueue_route_conflict`
Expected: FAIL — `enqueue_route_conflict` doesn't exist yet (compile error), and the reject test doesn't see the toast.

- [ ] **Step 4: Implement `enqueue_route_conflict` and wire it in**

Add to `src/app/actions.rs`, near `flash_status_high` (~line 2241):

```rust
    /// Enforces #223's queue-route invariant: an item whose resolved
    /// route differs from the queue's current route (`active_route`) is
    /// rejected with a toast instead of being appended or silently
    /// swapping the player. Returns `true` if the enqueue was rejected --
    /// the caller must abort without mutating the queue.
    pub(super) fn enqueue_route_conflict(&mut self, resolved_name: Option<String>) -> bool {
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

In `enqueue_selected` (~line 2370), guard the home-tab branch: immediately after `let Some(item) = self.current_home_item() else { return; };` and the `is_folder`/`is_playable` checks resolve to the non-folder append path, add the guard right before `let name = item.display_name();` in that branch:

```rust
            if !is_playable(&item) {
                return;
            }
            let resolved = self.route_for_item_via_ancestors(&item.id).map(|(n, _)| n);
            if self.enqueue_route_conflict(resolved) {
                return;
            }
            let name = item.display_name();
```

And guard the library-tab branch (`tab_idx >= 2`) the same way, using the nav-scoped resolver instead, immediately before its `let name = item.display_name();`:

```rust
            if !is_playable(&item) {
                return;
            }
            let lib_idx = self.tab_idx - self.lib_tab_offset();
            let resolved = self.route_for_active_library_view(lib_idx).map(|(n, _)| n);
            if self.enqueue_route_conflict(resolved) {
                return;
            }
            let name = item.display_name();
```

In `do_enqueue_folder` (~line 2576), add the guard as the first line of the function body:

```rust
    pub(super) fn do_enqueue_folder(&mut self, item: mbv_core::api::MediaItem) {
        let resolved = self.route_for_item_via_ancestors(&item.id).map(|(n, _)| n);
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

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test enqueue_selected_rejects_item_from_a_different_route enqueue_route_conflict`
Expected: PASS (4 passed)

- [ ] **Step 6: Run the full actions.rs test suite to check for regressions**

Run: `cargo test --lib app::actions::tests`
Expected: all existing enqueue tests still PASS (when `active_route` is `None` and `daemon_routes` is empty, `resolved_name` is always `None`, matching `self.active_route == None`, so `enqueue_route_conflict` never fires for existing local-only flows).

- [ ] **Step 7: Run `detect_changes` before committing**

```
detect_changes({scope: "compare", base_ref: "main"})
```

- [ ] **Step 8: Commit**

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

**Files:**
- Create: `docs/adr/0010-library-scoped-daemon-routing.md`

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
   item id for the session.
3. No match in either case means local playback.

Connecting to a routed library's daemon **is** takeover of that daemon's
driving-client slot (ADR 0003/0007) -- an accepted, explicit consequence,
not a hidden side effect. This matters if multiple devices route to the
same music-only daemon.

Library routing (`active_route`) is tracked independently of the
Sessions-panel direct-remote concept (`connected_session_id` /
`direct_remote_label`); they are two separate ways to end up thin-client
and must never be conflated in `App` state, even though both reuse the
same suspend/restore machinery (`SuspendedLocalSession`,
`switch_to_direct_remote`/`switch_to_library_route`,
`restore_local_mode`).

## Context

#222 established the connection lifecycle this depends on: fully lazy
connect (never at startup), fallback to local playback with a status-bar
warning on a failed connect, no background retry, no connection parking
(disconnect cleanly on swap-away; reconnect fresh next time). #223 reuses
that lifecycle for per-library routing rather than only the wildcard
"route everything" case #222 introduced.

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
git add docs/adr/0010-library-scoped-daemon-routing.md
git commit -m "docs: add ADR 0010 for library-scoped daemon routing"
```

---

## Self-Review

**1. Spec coverage against issue #223's acceptance criteria:**
- "Library-specific remote routing is configurable via `daemon_routes` in `config.toml`" -> Task 1.
- "Playback/enqueue started from a library-scoped view resolves its route from nav context with no network call; cross-library aggregate views resolve via `get_ancestors`" -> Task 7 (`route_for_active_library_view` vs `route_for_item_via_ancestors`), wired in Tasks 9-10.
- "Starting playback from a routed library swaps to that daemon (per #222's connect/fallback rules); starting playback from a non-routed library uses local playback, unaffected" -> Tasks 5, 8, 9.
- "Enqueuing an item whose route conflicts with the current queue's route is rejected with an explanatory toast, queue left unchanged" -> Task 10.
- "The effective route (local vs. which library route) is visible in app status and diagnosable in logs" -> Task 11 (status pill) + `log::info!`/`log::warn!` calls under the `"library_route"` target added in Tasks 4, 5, 7, 8.
- Docs impact (`CONTEXT.md`, new ADR) -> Tasks 12, 13.
- Explicitly out-of-scope items (mid-queue per-track swap, connection parking, config UI, migrating `daemon_client_endpoint`) -> none of the tasks implement these; Task 13's ADR states them explicitly as non-goals.

**2. Placeholder scan:** No "TBD"/"handle appropriately"/"add validation later" language appears in any task. Every step shows the actual code to write. The one caveat is Task 7 Step 1 and Task 10 Step 2's notes about matching `LibraryTab`'s exact field list / the existing queue-comparison idiom if it doesn't line up character-for-character with what's shown -- these are instructions to copy an existing, already-written pattern from the codebase (not "figure it out"), which is consistent with the file paths and existing symbols already cited throughout the plan, not a deferred design decision.

**3. Type/signature consistency check:**
- `resolve_daemon_route(routes: &HashMap<String, String>, library_name: &str) -> Option<&str>` (Task 2) is called consistently as `mbv_core::config::resolve_daemon_route(&self.daemon_routes, name)` in Task 4 -- same argument order and types.
- `resolve_route_for_library(&self, library_name: &str) -> Option<(String, DaemonEndpoint)>` (Task 4) is called identically in Task 7's `route_for_active_library_view` and `route_for_item_via_ancestors`, and in Task 8's `resolve_route_for_play`.
- `resolve_route_for_play(&mut self, item: &MediaItem) -> Option<(String, DaemonEndpoint)>` (Task 7) is called identically in Task 8's `apply_route_for_playback`.
- `switch_to_library_route(&mut self, library_name: &str, remote: RemotePlayer, remote_rx: Receiver<PlayerEvent>)` (Task 5) is called identically in Task 8's `apply_route_for_playback` (`self.switch_to_library_route(&name, remote, remote_rx)`), matching the `(String, DaemonEndpoint)` destructure's `name: String` (borrowed as `&name`) against the `&str` parameter.
- `enqueue_route_conflict(&mut self, resolved_name: Option<String>) -> bool` (Task 10) is called consistently everywhere it's wired in, always passed `Option<String>` (from `.map(|(n, _)| n)` on a `(String, DaemonEndpoint)` resolver result), matching `active_route: Option<String>`'s type for the `!=` comparison.
- `active_route` is read/written only as `Option<String>` everywhere (Tasks 3, 5, 6, 7, 8, 9, 10, 11) -- no task treats it as `Option<&str>` or a different shape.
- Field name `daemon_routes` used consistently on both `Config` (Task 1) and `App`/`AppInit` (Task 3) -- no `App` field is ever called `routes` or `library_routes` elsewhere in the plan.
