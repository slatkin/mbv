# Library Routes by Device Name Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace #223's `[daemon_routes]` config (library name -> raw
`tcp://`/`unix://` endpoint string) with `[library_routes]` (library name ->
device name), resolved at connect time the same way F3's Sessions panel
already resolves a device name to a live connection.

**Architecture:** `Config.daemon_routes: HashMap<String,String>` (endpoint
strings) becomes `Config.library_routes: HashMap<String,String>` (device
names), with no `"*"` wildcard. `App::resolve_route_for_library` no longer
calls `DaemonEndpoint::parse` on a static string; instead it looks up the
assigned device name and resolves it against the *live* session list using
the same matching `session_direct_endpoint` already applies for F3. A new
F2 Settings row ("Library routes") opens a two-stage picker: pick a
library, then pick a device from the live session list (or "Local (no
route)" to clear).

**Tech Stack:** Rust, `ratatui` (TUI rendering), `toml` (config
serialization), existing `EmbyClient` HTTP layer (`ureq`).

**Spec:** `docs/superpowers/specs/2026-07-18-library-routes-by-device-name-design.md`

## Global Constraints

- Section name is `[library_routes]` (top-level, not nested under
  `[general]`). `[daemon_routes]` is no longer recognized at all.
- Values are device names (plain strings), matched case-insensitively.
  Keys (library names) are also matched case-insensitively -- same
  convention as `hidden_libraries`/`feed_view_libraries`.
- No `"*"` wildcard key -- the parser has exactly one shape: library name
  -> device name.
- Breaking change, no migration. Old `tcp://`/`unix://`/`"*"` entries are
  simply not recognized under the new key.
- No same-host/Unix-socket routing -- device-name resolution only
  considers TCP-reachable mbv sessions (mirrors `session_direct_endpoint`
  exactly, including its "same device as me -> Local" case for when a
  routed library happens to point at the local machine).
- The F2 Settings picker only lets a user assign a device that is
  *currently live* in the session list. Existing assignments still display
  (and still round-trip through save/load) even when their device is
  currently offline -- only *assigning* requires liveness.
- No change to `active_route`, `switch_to_library_route`,
  `apply_route_for_playback`'s connect/fallback branching, or any #222
  connect-lifecycle behavior. This plan only changes how the
  `Option<(String, DaemonEndpoint)>` fed into that machinery gets produced.
- Every task ends with `cargo build`, `cargo test`, and
  `cargo clippy --all-targets -- -D warnings` passing, and
  `cargo fmt --all -- --check` clean, per `docs/CHECKIN.md`.

---

### Task 1: Add `EmbyClient::get_sessions_unfiltered()`

**Files:**
- Modify: `crates/mbv-core/src/api.rs:1713-1774`

**Interfaces:**
- Produces: `EmbyClient::get_sessions_unfiltered(&self) -> Result<Vec<SessionInfo>, String>` -- Task 3's device-resolution code calls this.
- Produces: `EmbyClient::get_sessions_with_active_within(&self, active_within_secs: Option<&str>) -> Result<Vec<SessionInfo>, String>` -- shared implementation.

The existing `get_sessions()` hardcodes `ActiveWithinSeconds=600`, which
would wrongly treat an idle-but-still-connected target device as gone.
Factor the query parameter out so an unfiltered variant can skip it
entirely.

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)] mod tests` block in `crates/mbv-core/src/api.rs`
(near the existing session-parsing tests):

```rust
    #[test]
    fn get_sessions_with_active_within_none_omits_query_param() {
        // get_sessions_unfiltered must not constrain ActiveWithinSeconds --
        // this is a compile-time/shape check since the real query param is
        // only observable over the wire; the behavioral guarantee is that
        // both methods exist and share one implementation.
        let client = EmbyClient::new_for_test("http://localhost:1", "tok", "dev", "did");
        // Both calls fail (no server listening on port 1) -- we only need
        // them to compile and return a network-error Result, proving the
        // unfiltered path exists and is reachable without a required
        // ActiveWithinSeconds argument.
        assert!(client.get_sessions_unfiltered().is_err());
        assert!(client.get_sessions().is_err());
    }
```

Check whether `EmbyClient::new_for_test` (or an equivalent test
constructor) already exists in `crates/mbv-core/src/api.rs` -- search for
`fn new_for_test` or how `auth_header_contains_device_name_and_id` (test at
line ~2209) constructs an `EmbyClient` and reuse that exact pattern instead
if the name differs.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p mbv-core get_sessions_with_active_within_none_omits_query_param`
Expected: FAIL with "no method named `get_sessions_unfiltered`"

- [ ] **Step 3: Implement `get_sessions_with_active_within` + `get_sessions_unfiltered`**

Replace the body at `crates/mbv-core/src/api.rs:1715-1774` (currently
`#[allow(dead_code)] pub fn get_sessions`) with:

```rust
    fn get_sessions_with_active_within(
        &self,
        active_within_secs: Option<&str>,
    ) -> Result<Vec<SessionInfo>, String> {
        let mut req = self.get("/Sessions");
        if let Some(secs) = active_within_secs {
            req = req.query("ActiveWithinSeconds", secs);
        }
        let arr: Value = req
            .call()
            .map_err(|e| e.to_string())?
            .into_json()
            .map_err(|e| e.to_string())?;
        let sessions = arr
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| {
                        if v["DeviceId"].as_str().unwrap_or("") == self.device_id {
                            return None;
                        }
                        if !v["SupportsRemoteControl"].as_bool().unwrap_or(false) {
                            return None;
                        }
                        let ps = &v["PlayState"];
                        let npi = &v["NowPlayingItem"];
                        let media_info = npi["MediaStreams"]
                            .as_array()
                            .map(|streams| parse_session_media_info(streams))
                            .unwrap_or_default();
                        let raw_host = v["RemoteEndPoint"].as_str().unwrap_or("");
                        let host = raw_host.rsplit(':').nth(1).unwrap_or(raw_host).to_string();
                        Some(SessionInfo {
                            id: v["Id"].as_str().unwrap_or("").to_string(),
                            device_name: v["DeviceName"].as_str().unwrap_or("").to_string(),
                            client: v["Client"].as_str().unwrap_or("").to_string(),
                            user_name: v["UserName"].as_str().unwrap_or("").to_string(),
                            host,
                            supported_commands: v["SupportedCommands"]
                                .as_array()
                                .map(|arr| {
                                    arr.iter()
                                        .filter_map(|value| value.as_str().map(str::to_string))
                                        .collect()
                                })
                                .unwrap_or_default(),
                            now_playing: npi["Name"].as_str().map(str::to_string),
                            now_playing_item_id: npi["Id"].as_str().map(str::to_string),
                            position_s: ps["PositionTicks"].as_i64().unwrap_or(0)
                                / TICKS_PER_SECOND,
                            runtime_s: npi["RunTimeTicks"].as_i64().unwrap_or(0) / TICKS_PER_SECOND,
                            is_paused: ps["IsPaused"].as_bool().unwrap_or(false),
                            volume: ps["VolumeLevel"].as_i64().unwrap_or(100),
                            sub_index: ps["SubtitleStreamIndex"].as_i64().unwrap_or(-1),
                            audio_index: ps["AudioStreamIndex"].as_i64().unwrap_or(0),
                            muted: ps["IsMuted"].as_bool().unwrap_or(false),
                            media_info,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        Ok(sessions)
    }

    #[allow(dead_code)]
    pub fn get_sessions(&self) -> Result<Vec<SessionInfo>, String> {
        self.get_sessions_with_active_within(Some("600"))
    }

    /// Unfiltered by `ActiveWithinSeconds` -- used by library-route device
    /// resolution (#239) so an idle-but-still-connected target device
    /// isn't wrongly treated as gone.
    pub fn get_sessions_unfiltered(&self) -> Result<Vec<SessionInfo>, String> {
        self.get_sessions_with_active_within(None)
    }
```

Only the function bodies at lines 1715-1774 change; nothing else in the
file moves.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p mbv-core get_sessions_with_active_within_none_omits_query_param`
Expected: PASS

- [ ] **Step 5: Run full crate test suite and clippy**

Run: `cargo test -p mbv-core && cargo clippy -p mbv-core --all-targets -- -D warnings`
Expected: all pass, no warnings

- [ ] **Step 6: Commit**

```bash
git add crates/mbv-core/src/api.rs
git commit -m "core: add get_sessions_unfiltered for library-route device resolution"
```

---

### Task 2: Rename config model `daemon_routes` -> `library_routes`, drop wildcard

**Files:**
- Modify: `crates/mbv-core/src/config.rs:47-52` (struct field + doc comment)
- Modify: `crates/mbv-core/src/config.rs:97` (`Default` impl)
- Modify: `crates/mbv-core/src/config.rs:120-133` (`resolve_daemon_route` function)
- Modify: `crates/mbv-core/src/config.rs:758-767` (`parse_config` TOML read)
- Modify: `crates/mbv-core/src/config.rs:801` (`Config { ... }` construction)
- Modify: `crates/mbv-core/src/config.rs:1268-1296` (tests)
- Modify: `crates/mbv-core/src/config.rs` (`save_config_settings`, add write-back -- there is currently none for this field)

**Interfaces:**
- Produces: `pub library_routes: std::collections::HashMap<String, String>` on `Config` (replaces `daemon_routes`).
- Produces: `pub fn resolve_library_route<'a>(routes: &'a HashMap<String, String>, library_name: &str) -> Option<&'a str>` (replaces `resolve_daemon_route`, no wildcard fallback).
- Consumes (Task 3): both of the above.

- [ ] **Step 1: Write the failing tests**

Replace the two tests at `crates/mbv-core/src/config.rs:1268-1296`
(`parse_daemon_routes_lowercased_keys`, `parse_default_daemon_routes_when_absent`) with:

```rust
    #[test]
    fn parse_library_routes_lowercased_keys() {
        let toml = r#"
[server]
url = "http://host"
[library_routes]
Music = "living-room-pc"
"#;
        let cfg = parse_config(toml).unwrap();
        assert_eq!(
            cfg.library_routes.get("music").map(String::as_str),
            Some("living-room-pc")
        );
    }

    #[test]
    fn parse_library_routes_ignores_legacy_wildcard_key() {
        // "*" is no longer a wildcard -- it's just an (unusable) library
        // name like any other, since #239 dropped the catch-all.
        let toml = r#"
[server]
url = "http://host"
[library_routes]
"*" = "living-room-pc"
"#;
        let cfg = parse_config(toml).unwrap();
        assert_eq!(
            cfg.library_routes.get("*").map(String::as_str),
            Some("living-room-pc")
        );
        assert_eq!(resolve_library_route(&cfg.library_routes, "movies"), None);
    }

    #[test]
    fn parse_default_library_routes_when_absent() {
        let toml = r#"
[server]
url = "http://host"
"#;
        let cfg = parse_config(toml).unwrap();
        assert!(cfg.library_routes.is_empty());
    }

    #[test]
    fn resolve_library_route_has_no_wildcard_fallback() {
        let mut routes = std::collections::HashMap::new();
        routes.insert("music".to_string(), "living-room-pc".to_string());
        assert_eq!(
            resolve_library_route(&routes, "Music"),
            Some("living-room-pc")
        );
        assert_eq!(resolve_library_route(&routes, "movies"), None);
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p mbv-core library_routes`
Expected: FAIL to compile (`daemon_routes`/`resolve_daemon_route` still the only names that exist)

- [ ] **Step 3: Rename the struct field**

In `crates/mbv-core/src/config.rs:44-52`, replace:

```rust
    pub feed_view_libraries: Vec<String>, // libraries treated as feed view (unplayed, date-sorted)
    /// Library name (lowercased) -> daemon endpoint string, from
    /// `[daemon_routes]` (#223). Playback/enqueue resolved to one of these
    /// libraries swaps the active player to that daemon. `"*"` is a
    /// wildcard "route everything" entry (#222). TOML-only for v1 -- no
    /// settings-panel write-back, matching the `hidden_libraries` value
    /// precedent but without exposing it for in-app editing.
    pub daemon_routes: std::collections::HashMap<String, String>,
```

with:

```rust
    pub feed_view_libraries: Vec<String>, // libraries treated as feed view (unplayed, date-sorted)
    /// Library name (lowercased) -> device name, from `[library_routes]`
    /// (#239, replacing #223's `[daemon_routes]`). Playback/enqueue
    /// resolved to one of these libraries swaps the active player to
    /// whichever live session currently has this device name -- resolved
    /// the same way F3's Sessions panel resolves a device to a
    /// connection (`App::session_direct_endpoint`). No `"*"` wildcard.
    /// Editable via the F2 Settings "Library routes" row, and
    /// hand-editable in `config.toml`.
    pub library_routes: std::collections::HashMap<String, String>,
```

- [ ] **Step 4: Rename the `Default` impl field**

At `crates/mbv-core/src/config.rs:97`, change:

```rust
            daemon_routes: std::collections::HashMap::new(),
```

to:

```rust
            library_routes: std::collections::HashMap::new(),
```

- [ ] **Step 5: Rewrite `resolve_daemon_route` -> `resolve_library_route`**

Replace the function at `crates/mbv-core/src/config.rs:120-133`:

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

with:

```rust
/// Resolves the configured device name for a library name (#239). Matches
/// case-insensitively (the query is lowercased before lookup; `routes`'
/// keys are already lowercased by `parse_config`). No wildcard fallback --
/// returns `None` if the library has no route, and the caller stays local.
pub fn resolve_library_route<'a>(
    routes: &'a std::collections::HashMap<String, String>,
    library_name: &str,
) -> Option<&'a str> {
    routes.get(&library_name.to_lowercase()).map(|s| s.as_str())
}
```

- [ ] **Step 6: Rename the TOML-read variable and section key**

At `crates/mbv-core/src/config.rs:758-767`, change:

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

to:

```rust
    let library_routes: std::collections::HashMap<String, String> = doc
        .get("library_routes")
        .and_then(|v| v.as_table())
        .map(|table| {
            table
                .iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.to_lowercase(), s.to_string())))
                .collect()
        })
        .unwrap_or_default();
```

Then at line 801, in the `Config { ... }` construction, change `daemon_routes,` to `library_routes,`.

- [ ] **Step 7: Add write-back in `save_config_settings`**

`daemon_routes` was TOML-only in v1 -- `save_config_settings` never wrote
it. Task 4's Settings UI needs edits to persist, so add a `[library_routes]`
write-back. In `crates/mbv-core/src/config.rs`, immediately after the
`feed_view_libraries` insert inside the `general` section (the block
ending around line ~103 in the function, right before `let queue = section!("queue");`),
add (as its own top-level section, NOT inside `general`):

```rust
    if cfg.library_routes.is_empty() {
        table.remove("library_routes");
    } else {
        let mut routes_table = toml::map::Map::new();
        for (library, device) in &cfg.library_routes {
            routes_table.insert(library.clone(), toml::Value::String(device.clone()));
        }
        table.insert(
            "library_routes".to_string(),
            toml::Value::Table(routes_table),
        );
    }
```

Place this directly on `table` (the top-level document map), not inside
any `section!(...)` block -- `library_routes` is a top-level TOML table,
sibling to `[general]`/`[queue]`/`[mpv]`, per the spec's explicit
"not nested under `[general]`" requirement.

- [ ] **Step 8: Run tests to verify they pass**

Run: `cargo test -p mbv-core`
Expected: all pass, including the four new/rewritten tests

- [ ] **Step 9: Run clippy and fmt check**

Run: `cargo clippy -p mbv-core --all-targets -- -D warnings && cargo fmt --all -- --check`
Expected: clean

- [ ] **Step 10: Commit**

```bash
git add crates/mbv-core/src/config.rs
git commit -m "core: rename daemon_routes to library_routes, drop wildcard, add write-back"
```

---

### Task 3: Resolve library routes via live device-name lookup

**Files:**
- Modify: `src/app/mod.rs:26-53` (test-seam statics, add a sessions-fetch seam)
- Modify: `src/app/mod.rs:937-940` (`App.daemon_routes` field)
- Modify: `src/app/mod.rs:1086-1096` (`App.daemon_routes_warned` field + doc comment)
- Modify: `src/app/mod.rs:1163` (init struct field)
- Modify: `src/app/mod.rs:1657, 1785, 1823, 1899, 1949, 2036, 5768, 5893, 6003` (construction/test-stub sites)
- Modify: `src/app/mod.rs:2349-2374` (add a new method near `session_direct_endpoint`)
- Modify: `src/app/mod.rs:6802, 6845, 6873, 6951, 6953, 10186` (tests referencing `app.daemon_routes`)
- Modify: `src/app/library_route.rs:55-99` (`resolve_route_for_library`)
- Modify: `src/login.rs:138` (`daemon_routes: base_config.daemon_routes.clone()`)
- Modify: `crates/mbv-core/src/config.rs` and `src/app/actions.rs:7231, 7331, 7357, 7393` (test references -- confirm exact content with `grep -n daemon_routes src/app/actions.rs` before editing; these may already be test-only mirrors of the mod.rs tests)

**Interfaces:**
- Consumes: `mbv_core::config::resolve_library_route` (Task 2), `EmbyClient::get_sessions_unfiltered` (Task 1), `App::session_direct_endpoint` (existing, `src/app/mod.rs:2349-2374`).
- Produces: `App::resolve_device_endpoint(&self, device_name: &str) -> Option<mbv_core::remote_player::DaemonEndpoint>` -- new method, used only by `resolve_route_for_library`.
- Produces: `App::fetch_sessions_blocking(&self) -> Result<Vec<mbv_core::api::SessionInfo>, String>` -- new test seam, mirrors the existing `DAEMON_ROUTE_CONNECT_OVERRIDE` pattern at `src/app/mod.rs:26-53`.
- Unchanged: `App::resolve_route_for_library(&mut self, library_name: &str) -> Option<(String, mbv_core::remote_player::DaemonEndpoint)>` signature (callers in `library_route.rs` and `apply_route_for_playback` at `src/app/mod.rs:2694-2721` need no changes).

This is the core behavior change: `resolve_route_for_library` currently
parses a static endpoint string with `DaemonEndpoint::parse`. It must
instead look up the configured *device name* and resolve it against the
live session list, the same way `session_direct_endpoint` already does
for F3.

- [ ] **Step 1: Write the failing tests**

First, read the full current body of `resolve_route_for_library` in
`src/app/library_route.rs:55-99` and the six existing
`app.daemon_routes.insert(...)` test call sites in `src/app/mod.rs`
(around lines 6802, 6845, 6873, 6951, 6953) -- these currently insert
raw endpoint strings like `"tcp://127.0.0.1:9000"` as the *value*. After
this task, `app.library_routes` values are device names, and the
connect-success/failure paths those tests exercise (via
`DAEMON_ROUTE_CONNECT_OVERRIDE`) need a *device* to resolve to a live
session first. Add a new test-seam static so tests can inject a fake
session list without a real network call, mirroring
`DAEMON_ROUTE_CONNECT_OVERRIDE` exactly.

Add near `src/app/mod.rs:50-53` (right after the existing
`DAEMON_ROUTE_CONNECT_OVERRIDE`/`DAEMON_ROUTE_CONNECT_TEST_LOCK` pair):

```rust
#[cfg(test)]
type SessionsLoadFn =
    fn(&mbv_core::api::EmbyClient) -> Result<Vec<mbv_core::api::SessionInfo>, String>;
#[cfg(test)]
static SESSIONS_LOAD_OVERRIDE: Mutex<Option<SessionsLoadFn>> = Mutex::new(None);
#[cfg(test)]
static SESSIONS_LOAD_TEST_LOCK: Mutex<()> = Mutex::new(());
```

Then add these tests to the `#[cfg(test)] mod tests` block in
`src/app/mod.rs`, near the existing `resolve_route_for_library`-adjacent
tests in `src/app/library_route.rs`'s own test module (check
`src/app/library_route.rs`'s bottom `mod tests` -- if
`resolve_route_for_library` already has tests there, add these
alongside them instead of in `mod.rs`):

```rust
    #[test]
    fn resolve_route_for_library_resolves_via_live_device_name() {
        let _guard = crate::config::TestStateDirGuard::new();
        let _sessions_guard = SESSIONS_LOAD_TEST_LOCK.lock().unwrap();
        fn fake_sessions(
            _client: &mbv_core::api::EmbyClient,
        ) -> Result<Vec<mbv_core::api::SessionInfo>, String> {
            Ok(vec![make_session_info(
                "living-room-pc",
                "mbv",
                "10.0.0.5",
                &["DirectTcpPort-9100"],
            )])
        }
        *SESSIONS_LOAD_OVERRIDE.lock().unwrap() = Some(fake_sessions);

        let mut app = make_app_stub();
        app.library_routes
            .insert("music".to_string(), "living-room-pc".to_string());

        let resolved = app.resolve_route_for_library("Music");

        *SESSIONS_LOAD_OVERRIDE.lock().unwrap() = None;
        assert_eq!(
            resolved,
            Some((
                "music".to_string(),
                mbv_core::remote_player::DaemonEndpoint::Tcp(std::net::SocketAddr::from((
                    std::net::Ipv4Addr::new(10, 0, 0, 5),
                    9100
                )))
            ))
        );
    }

    #[test]
    fn resolve_route_for_library_falls_back_to_local_when_device_offline() {
        let _guard = crate::config::TestStateDirGuard::new();
        let _sessions_guard = SESSIONS_LOAD_TEST_LOCK.lock().unwrap();
        fn empty_sessions(
            _client: &mbv_core::api::EmbyClient,
        ) -> Result<Vec<mbv_core::api::SessionInfo>, String> {
            Ok(vec![])
        }
        *SESSIONS_LOAD_OVERRIDE.lock().unwrap() = Some(empty_sessions);

        let mut app = make_app_stub();
        app.library_routes
            .insert("music".to_string(), "living-room-pc".to_string());

        let resolved = app.resolve_route_for_library("Music");

        *SESSIONS_LOAD_OVERRIDE.lock().unwrap() = None;
        assert_eq!(resolved, None);
    }

    #[test]
    fn resolve_route_for_library_no_match_when_library_not_routed() {
        let mut app = make_app_stub();
        assert_eq!(app.resolve_route_for_library("Movies"), None);
    }
```

Check whether a `make_session_info(device_name, client, host, supported_commands)`
test helper already exists near the existing `session_direct_endpoint_*`
tests (`src/app/mod.rs:6476-6503`) -- if the existing tests build
`SessionInfo` by hand with a struct literal instead, use that same struct
literal form (with all its fields) in place of `make_session_info(...)`
above; do not invent a helper that duplicates a pattern already inline.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test resolve_route_for_library`
Expected: FAIL to compile (`SESSIONS_LOAD_OVERRIDE` / `app.library_routes` don't exist yet) or FAIL on assertion (old code still parses `"living-room-pc"` as an endpoint string, which is not valid `tcp://`/`unix://`, producing a warning + `None` instead of a `Tcp` endpoint)

- [ ] **Step 3: Rename the `App` struct fields**

At `src/app/mod.rs:937-940`, replace:

```rust
    /// `Config.daemon_routes` at startup (#223). Endpoints are parsed
    /// lazily via `DaemonEndpoint::parse` at connect time, not eagerly, so
    daemon_routes: std::collections::HashMap<String, String>,
```

(keep whatever the full 3-line doc comment says; only the field name and
its "endpoints are parsed lazily" claim change) with:

```rust
    /// `Config.library_routes` at startup (#239). Values are device names,
    /// resolved lazily against the *live* session list at connect time via
    /// `resolve_device_endpoint`, not eagerly -- so a device that's offline
    /// right now still shows a configured assignment; it just won't
    /// resolve to a connection until it's live again.
    library_routes: std::collections::HashMap<String, String>,
```

At `src/app/mod.rs:1086-1096`, rename `daemon_routes_warned` to
`library_routes_warned` (keep its doc comment, updating "`daemon_routes`"
references in the prose to "`library_routes`").

At `src/app/mod.rs:1163`, rename the corresponding init-struct field the
same way.

- [ ] **Step 4: Update all construction/test-stub sites**

Run `grep -n "daemon_routes" src/app/mod.rs` and rename every remaining
`daemon_routes:` / `daemon_routes_warned:` field-init site (expected at
lines 1657, 1785, 1823, 1899, 1949, 2036, 5768, 5893, 6003) to
`library_routes:` / `library_routes_warned:`. These are plain field-init
renames -- the values assigned (`init.daemon_routes` ->
`init.library_routes`, `client.config.daemon_routes.clone()` ->
`client.config.library_routes.clone()`, `std::collections::HashMap::new()`)
keep their same shape, only the name changes.

- [ ] **Step 5: Add `fetch_sessions_blocking` and `resolve_device_endpoint`**

Immediately after `session_direct_endpoint` (`src/app/mod.rs:2349-2374`),
insert:

```rust
    /// Blocking `GET /Sessions` (unfiltered), factored out only so tests
    /// can override it via `SESSIONS_LOAD_OVERRIDE` -- mirrors
    /// `connect_daemon_route_endpoint`'s `#[cfg(test)]` seam.
    /// `resolve_device_endpoint` is the one caller.
    fn fetch_sessions_blocking(&self) -> Result<Vec<mbv_core::api::SessionInfo>, String> {
        #[cfg(test)]
        if let Some(f) = *SESSIONS_LOAD_OVERRIDE.lock().unwrap() {
            return f(&self.client.lock().unwrap());
        }
        self.client.lock().unwrap().get_sessions_unfiltered()
    }

    /// Resolves a configured `library_routes` device name (#239) to a live
    /// connection, the same way `session_direct_endpoint` resolves a
    /// discovered F3 session: fetches the current (unfiltered) session
    /// list, finds the first mbv session whose `device_name` matches
    /// case-insensitively, and reuses `session_direct_endpoint`'s own
    /// endpoint derivation. Returns `None` if the device isn't currently
    /// live, isn't an mbv session, or the session list fetch itself fails
    /// -- all three collapse to "no route right now", matching #222's
    /// existing local-fallback rule; the caller doesn't distinguish them.
    fn resolve_device_endpoint(
        &self,
        device_name: &str,
    ) -> Option<mbv_core::remote_player::DaemonEndpoint> {
        let sessions = self.fetch_sessions_blocking().ok()?;
        let sess = sessions
            .iter()
            .find(|s| s.device_name.eq_ignore_ascii_case(device_name))?;
        self.session_direct_endpoint(sess)
    }
```

- [ ] **Step 6: Rewrite `resolve_route_for_library`**

Replace the body of `resolve_route_for_library` in
`src/app/library_route.rs:55-99`:

```rust
    pub(super) fn resolve_route_for_library(
        &mut self,
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
                let warn_key = name.to_lowercase();
                let now = Instant::now();
                let should_flash = match self.daemon_routes_warned.get(&warn_key) {
                    Some(last_flashed) => {
                        now.duration_since(*last_flashed) >= DAEMON_ROUTE_WARNING_COOLDOWN
                    }
                    None => true,
                };
                if should_flash {
                    self.daemon_routes_warned.insert(warn_key, now);
                    self.flash_status_high(format!(
                        "daemon_routes entry for library {name:?} is invalid ({e}); using local playback"
                    ));
                }
                None
            }
        }
    }
```

with:

```rust
    /// Resolves the configured library route for a library name (#239):
    /// looks up `library_routes` for a device name, then resolves that
    /// device against the *live* session list via `resolve_device_endpoint`
    /// -- the same mechanism `session_direct_endpoint` already uses for
    /// F3. Returns `(lowercased_library_name, endpoint)` on a live match.
    /// A configured-but-currently-offline device is not an error (no
    /// warning flashed) -- it's the expected, common case of "not routed
    /// right now"; #222's existing fallback (stay local, no hard error)
    /// already covers it via the `None` return.
    pub(super) fn resolve_route_for_library(
        &mut self,
        library_name: &str,
    ) -> Option<(String, mbv_core::remote_player::DaemonEndpoint)> {
        let name = library_name.trim();
        if name.is_empty() {
            return None;
        }
        let device_name = mbv_core::config::resolve_library_route(&self.library_routes, name)?;
        let endpoint = self.resolve_device_endpoint(device_name)?;
        Some((name.to_lowercase(), endpoint))
    }
```

Note this drops the `daemon_routes_warned` flash-on-invalid-endpoint
branch entirely: there is no longer a "malformed string" failure mode to
warn about (a device name is just a string; the only failure mode now is
"not currently live," which is expected and silent, matching the spec's
"existing assignments still display correctly even when offline" rule).
`library_routes_warned`/`DAEMON_ROUTE_WARNING_COOLDOWN` become unused --
remove the field (from Step 3's rename, undo it: delete
`library_routes_warned` from the `App` struct and all its init sites
instead of keeping it) and remove the `DAEMON_ROUTE_WARNING_COOLDOWN`
constant from `src/app/library_route.rs` if `cargo build` reports it as
dead code.

- [ ] **Step 7: Update `src/login.rs:138`**

Change `daemon_routes: base_config.daemon_routes.clone(),` to
`library_routes: base_config.library_routes.clone(),`.

- [ ] **Step 8: Update remaining test call sites**

Run `grep -n "daemon_routes" src/app/mod.rs src/app/actions.rs
crates/mbv-core/src/config.rs` and fix every remaining reference:

- `src/app/mod.rs:6802, 6845, 6873, 6951, 6953, 10186`: change
  `app.daemon_routes.insert("music".to_string(), "tcp://127.0.0.1:9000".to_string())`
  to `app.library_routes.insert("music".to_string(), "living-room-pc".to_string())`
  (and similarly for the `"movies"` entry at line ~6953-6954). Since these
  tests exercise `apply_route_for_playback`'s connect success/failure
  paths (via `DAEMON_ROUTE_CONNECT_OVERRIDE`, which intercepts *after*
  resolution), each of these tests also needs a `SESSIONS_LOAD_OVERRIDE`
  set (per Step 1's pattern) so `resolve_route_for_library` finds a live
  device before the `DAEMON_ROUTE_CONNECT_OVERRIDE` intercepts the
  connect attempt. Add the same `fake_sessions` seam used in Step 1's new
  tests, returning a session with `device_name: "living-room-pc"` (and
  `"movies-pc"` for the two-route test at line ~6951-6954), to each of
  these five tests, guarded by `SESSIONS_LOAD_TEST_LOCK` alongside their
  existing `DAEMON_ROUTE_CONNECT_TEST_LOCK` guard.
  `assert!(app.daemon_routes.is_empty())` at line 10186 (if that's a
  `library_routes`-shaped assertion rather than an unrelated one --
  confirm by reading its surrounding test first) becomes
  `assert!(app.library_routes.is_empty())` with no other changes needed.
- `src/app/actions.rs:7231, 7331, 7357, 7393`: read each surrounding test
  first (`grep -n -B5 -A15 "app.daemon_routes" src/app/actions.rs`) to see
  whether it's testing `resolve_route_for_library` behavior (needs the
  same `SESSIONS_LOAD_OVERRIDE` treatment) or something unrelated (e.g. a
  route-conflict/queue-mixing check that only needs the *presence* of a
  `library_routes` entry keyed by library name, not a working
  connection) -- apply the minimal matching change per site rather than
  a blanket find-replace.

- [ ] **Step 9: Run full test suite**

Run: `cargo test`
Expected: all pass

- [ ] **Step 10: Run clippy and fmt check**

Run: `cargo clippy --all-targets -- -D warnings && cargo fmt --all -- --check`
Expected: clean

- [ ] **Step 11: Commit**

```bash
git add src/app/mod.rs src/app/library_route.rs src/app/actions.rs src/login.rs
git commit -m "app: resolve library routes via live device-name lookup instead of static endpoints"
```

---

### Task 4: F2 Settings UI -- "Library routes" row + device picker

**Files:**
- Modify: `src/app/mod.rs:181-193` (add popup state type + field)
- Modify: `src/app/mod.rs:1289-1315` (`SettingKey` enum + `SETTING_SECTIONS`)
- Modify: `src/app/settings.rs` (label/value functions)
- Modify: `src/app/render/overlays/settings.rs:16-42` (`handle_settings_activate`)
- Create: `src/app/render/overlays/library_routes.rs` (open/close/render, mirrors `multiselect.rs`'s structure)
- Modify: `src/app/render/mod.rs:385-386` (render dispatch)
- Modify: `src/app/input.rs:671-703` (key handling)
- Modify: `src/app/render/overlays/mod.rs` or wherever `mod multiselect;` is declared, to add `mod library_routes;`

**Interfaces:**
- Consumes: `Config.library_routes` (Task 2), `App::client.get_views()` (existing, used identically by `open_multiselect_popup`), `EmbyClient::get_sessions_unfiltered()` (Task 1), `App.client.device_name` (existing pub field on `EmbyClient`).
- Produces: `LibraryRoutePopup` state, `App::open_library_routes_popup()`, `App::close_library_routes_popup()`, `App::render_library_routes_popup(&mut self, f: &mut Frame)`.

- [ ] **Step 1: Add popup state types**

In `src/app/mod.rs`, near the existing `MultiSelectKind`/`MultiSelectPopup`
definitions (`src/app/mod.rs:181-193`), add:

```rust
#[derive(Clone)]
pub(crate) enum LibraryRouteStage {
    /// (library_name_lower, display_name, current_device_or_none)
    PickLibrary {
        items: Vec<(String, String, Option<String>)>,
    },
    /// index 0 is always the synthetic "Local (no route)" entry.
    PickDevice {
        library_lower: String,
        library_display: String,
        devices: Vec<String>,
    },
}

pub(crate) struct LibraryRoutePopup {
    stage: LibraryRouteStage,
    cursor: usize,
}
```

Add a field to the `App` struct (alongside `multiselect_popup:
Option<MultiSelectPopup>` at `src/app/mod.rs:1017`):

```rust
    library_routes_popup: Option<LibraryRoutePopup>,
```

Initialize it to `None` at every site that currently initializes
`multiselect_popup: None` (`src/app/mod.rs:1746` and `5838`, and any other
`App { .. }` literal `cargo build` reports as missing this field).

- [ ] **Step 2: Add `SettingKey::LibraryRoutes`**

In `src/app/mod.rs:1289-1315`, add a new variant to the `SettingKey` enum,
placed right after `FeedViewLibraries`:

```rust
    FeedViewLibraries,
    LibraryRoutes,
```

Find `SETTING_SECTIONS` (right after the enum, `src/app/mod.rs:~1320`) and
add `SettingKey::LibraryRoutes` to whichever section array
`FeedViewLibraries` is currently listed in, immediately after it.

- [ ] **Step 3: Add label/value functions**

In `src/app/settings.rs`, add to the `setting_label` match (after the
`FeedViewLibraries` arm):

```rust
        SettingKey::FeedViewLibraries => "Feed view",
        SettingKey::LibraryRoutes => "Library routes",
```

Add to the `setting_value` match (after the `FeedViewLibraries` arm):

```rust
        SettingKey::FeedViewLibraries => fmt_feed_view_list(&cfg.feed_view_libraries),
        SettingKey::LibraryRoutes => fmt_library_routes(&cfg.library_routes),
```

Add a new formatting function near `fmt_feed_view_list`:

```rust
pub fn fmt_library_routes(routes: &std::collections::HashMap<String, String>) -> String {
    match routes.len() {
        0 => "none".into(),
        1 => {
            let (lib, dev) = routes.iter().next().unwrap();
            format!("{lib} -> {dev}")
        }
        n => format!("{n} routes"),
    }
}
```

- [ ] **Step 4: Create `src/app/render/overlays/library_routes.rs`**

```rust
use super::super::super::palette;
use super::super::super::App;
use super::super::super::{LibraryRoutePopup, LibraryRouteStage};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};
use ratatui::Frame;

const LOCAL_NO_ROUTE: &str = "Local (no route)";

impl App {
    pub(crate) fn open_library_routes_popup(&mut self) {
        let client = self.client.lock().unwrap();
        let all = client.get_views().unwrap_or_default();
        let routes = client.config.library_routes.clone();
        let items: Vec<(String, String, Option<String>)> = all
            .iter()
            .filter(|v| v.collection_type != "playlists")
            .map(|v| {
                let lower = v.name.to_lowercase();
                let assigned = routes.get(&lower).cloned();
                (lower, v.name.clone(), assigned)
            })
            .collect();
        drop(client);
        self.library_routes_popup = Some(LibraryRoutePopup {
            stage: LibraryRouteStage::PickLibrary { items },
            cursor: 0,
        });
    }

    fn enter_device_stage(&mut self, library_lower: String, library_display: String) {
        let sessions = self.fetch_sessions_blocking().unwrap_or_default();
        let local_device_name = self.client.lock().unwrap().device_name.clone();
        let mut devices: Vec<String> = sessions
            .iter()
            .filter(|s| s.client.eq_ignore_ascii_case("mbv"))
            .filter(|s| !s.device_name.eq_ignore_ascii_case(&local_device_name))
            .map(|s| s.device_name.clone())
            .collect();
        devices.sort();
        devices.dedup_by(|a, b| a.eq_ignore_ascii_case(b));

        let current = self
            .client
            .lock()
            .unwrap()
            .config
            .library_routes
            .get(&library_lower)
            .cloned();
        let cursor = current
            .as_ref()
            .and_then(|dev| devices.iter().position(|d| d.eq_ignore_ascii_case(dev)))
            .map(|idx| idx + 1) // +1 for the synthetic "Local (no route)" row at index 0
            .unwrap_or(0);

        if let Some(popup) = &mut self.library_routes_popup {
            popup.stage = LibraryRouteStage::PickDevice {
                library_lower,
                library_display,
                devices,
            };
            popup.cursor = cursor;
        }
    }

    fn commit_device_selection(&mut self) {
        let Some(popup) = &self.library_routes_popup else {
            return;
        };
        let LibraryRouteStage::PickDevice {
            library_lower,
            library_display,
            devices,
        } = popup.stage.clone()
        else {
            return;
        };
        let cursor = popup.cursor;

        {
            let mut c = self.client.lock().unwrap();
            if cursor == 0 {
                c.config.library_routes.remove(&library_lower);
            } else if let Some(device) = devices.get(cursor - 1) {
                c.config
                    .library_routes
                    .insert(library_lower.clone(), device.clone());
            }
        }
        let cfg = self.client.lock().unwrap().config.clone();
        crate::config::save_config_settings(&cfg);

        // Return to the library list, refreshed with the new assignment.
        let all = self.client.lock().unwrap().get_views().unwrap_or_default();
        let routes = cfg.library_routes.clone();
        let items: Vec<(String, String, Option<String>)> = all
            .iter()
            .filter(|v| v.collection_type != "playlists")
            .map(|v| {
                let lower = v.name.to_lowercase();
                let assigned = routes.get(&lower).cloned();
                (lower, v.name.clone(), assigned)
            })
            .collect();
        let restored_cursor = items
            .iter()
            .position(|(lower, _, _)| *lower == library_lower)
            .unwrap_or(0);
        if let Some(popup) = &mut self.library_routes_popup {
            popup.stage = LibraryRouteStage::PickLibrary { items };
            popup.cursor = restored_cursor;
        }
        let _ = library_display; // display name not needed after commit; kept for stage symmetry
    }

    pub(crate) fn handle_library_routes_enter(&mut self) {
        let Some(popup) = &self.library_routes_popup else {
            return;
        };
        match popup.stage.clone() {
            LibraryRouteStage::PickLibrary { items } => {
                if let Some((lower, display, _)) = items.get(popup.cursor) {
                    let lower = lower.clone();
                    let display = display.clone();
                    self.enter_device_stage(lower, display);
                }
            }
            LibraryRouteStage::PickDevice { .. } => {
                self.commit_device_selection();
            }
        }
    }

    pub(crate) fn handle_library_routes_esc(&mut self) {
        let Some(popup) = &self.library_routes_popup else {
            return;
        };
        match &popup.stage {
            LibraryRouteStage::PickLibrary { .. } => {
                self.library_routes_popup = None;
            }
            LibraryRouteStage::PickDevice { .. } => {
                self.open_library_routes_popup();
            }
        }
    }

    pub(crate) fn move_library_routes_cursor(&mut self, delta: i64) {
        let Some(popup) = &mut self.library_routes_popup else {
            return;
        };
        let len = match &popup.stage {
            LibraryRouteStage::PickLibrary { items } => items.len(),
            LibraryRouteStage::PickDevice { devices, .. } => devices.len() + 1,
        };
        if len == 0 {
            return;
        }
        let mut idx = popup.cursor as i64 + delta;
        if idx < 0 {
            idx = 0;
        }
        if idx as usize >= len {
            idx = len as i64 - 1;
        }
        popup.cursor = idx as usize;
    }

    pub(in crate::app::render) fn render_library_routes_popup(&mut self, f: &mut Frame) {
        let Some(ref popup) = self.library_routes_popup else {
            return;
        };
        let (title, lines): (&str, Vec<Line>) = match &popup.stage {
            LibraryRouteStage::PickLibrary { items } => {
                let lines = items
                    .iter()
                    .enumerate()
                    .map(|(i, (_, name, assigned))| {
                        let focused = i == popup.cursor;
                        let arrow = if focused { "▸ " } else { "  " };
                        let name_style = if focused {
                            Style::default().fg(palette::TEXT)
                        } else {
                            Style::default().fg(palette::SUBTLE)
                        };
                        let value = assigned.clone().unwrap_or_else(|| "none".to_string());
                        Line::from(vec![
                            Span::raw(arrow),
                            Span::styled(name.clone(), name_style),
                            Span::raw(" -> "),
                            Span::styled(value, Style::default().fg(palette::FOAM)),
                        ])
                    })
                    .collect();
                (" Library Routes ", lines)
            }
            LibraryRouteStage::PickDevice {
                library_display,
                devices,
                ..
            } => {
                let mut lines = vec![];
                if devices.is_empty() {
                    lines.push(Line::from(Span::styled(
                        "No other mbv devices found right now -- make sure the",
                        Style::default().fg(palette::MUTED),
                    )));
                    lines.push(Line::from(Span::styled(
                        "target is running and connected.",
                        Style::default().fg(palette::MUTED),
                    )));
                }
                let mut rows: Vec<String> = vec![LOCAL_NO_ROUTE.to_string()];
                rows.extend(devices.iter().cloned());
                for (i, name) in rows.iter().enumerate() {
                    let focused = i == popup.cursor;
                    let arrow = if focused { "▸ " } else { "  " };
                    let name_style = if focused {
                        Style::default().fg(palette::TEXT)
                    } else {
                        Style::default().fg(palette::SUBTLE)
                    };
                    lines.push(Line::from(vec![
                        Span::raw(arrow),
                        Span::styled(name.clone(), name_style),
                    ]));
                }
                let _ = library_display;
                (" Pick Device ", lines)
            }
        };

        let max_w = lines.iter().map(|l| l.width()).max().unwrap_or(0);
        let inner_w = ((max_w + 6) as u16).clamp(36, 60);
        let width = inner_w + 2;
        let content_h = lines.len() as u16 + 1;
        let area = f.area();
        let height = (content_h + 2).min(area.height.saturating_sub(2));
        let x = area.x + area.width.saturating_sub(width) / 2;
        let y = area.y + area.height.saturating_sub(height) / 2;
        let rect = Rect {
            x,
            y,
            width,
            height,
        };

        f.render_widget(Clear, rect);
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(palette::IRIS))
            .title(Span::styled(
                title,
                Style::default()
                    .fg(palette::WHITE)
                    .add_modifier(Modifier::BOLD),
            ));
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        let hint = "Enter select  ·  Esc back/close";
        f.render_widget(
            Paragraph::new(Span::styled(hint, Style::default().fg(palette::MUTED))),
            Rect {
                x: inner.x,
                y: inner.y,
                width: inner.width,
                height: 1,
            },
        );
        let list_area = Rect {
            x: inner.x,
            y: inner.y + 1,
            width: inner.width,
            height: inner.height.saturating_sub(1),
        };
        f.render_widget(Paragraph::new(lines), list_area);
    }
}
```

Register the new module: find the `mod multiselect;` declaration
(`grep -rn "mod multiselect" src/app/render/overlays/`) and add
`mod library_routes;` next to it.

- [ ] **Step 5: Wire settings activation**

In `src/app/render/overlays/settings.rs:16-42`
(`handle_settings_activate`), add a new match arm right after
`SettingKey::FeedViewLibraries`:

```rust
            SettingKey::FeedViewLibraries => {
                self.open_multiselect_popup(MultiSelectKind::FeedViewLibraries);
                return;
            }
            SettingKey::LibraryRoutes => {
                self.open_library_routes_popup();
                return;
            }
```

- [ ] **Step 6: Wire render dispatch**

In `src/app/render/mod.rs:385-386`, right after the
`multiselect_popup` render block:

```rust
            if self.multiselect_popup.is_some() {
                self.render_multiselect_popup(f);
            }
            if self.library_routes_popup.is_some() {
                self.render_library_routes_popup(f);
            }
```

- [ ] **Step 7: Wire key handling**

In `src/app/input.rs`, right after the `multiselect_popup`-handling block
(`src/app/input.rs:675-703`), inside `handle_key_settings` and before the
`if self.confirm_logout` check:

```rust
        if self.library_routes_popup.is_some() {
            match key.code {
                KeyCode::Esc => {
                    self.handle_library_routes_esc();
                }
                KeyCode::Enter => {
                    self.handle_library_routes_enter();
                }
                KeyCode::Up => {
                    self.move_library_routes_cursor(-1);
                }
                KeyCode::Down => {
                    self.move_library_routes_cursor(1);
                }
                _ => {}
            }
            return Some(false);
        }
```

- [ ] **Step 8: Write tests**

Add to `src/app/mod.rs`'s test module (or a new `#[cfg(test)] mod tests`
block in the new `library_routes.rs` file, matching whichever convention
`multiselect.rs` uses -- check `grep -n "mod tests" src/app/render/overlays/multiselect.rs`
first):

```rust
    #[test]
    fn open_library_routes_popup_starts_on_pick_library_stage() {
        let mut app = make_app_stub();
        app.open_library_routes_popup();
        let popup = app.library_routes_popup.as_ref().unwrap();
        assert!(matches!(popup.stage, LibraryRouteStage::PickLibrary { .. }));
    }

    #[test]
    fn commit_device_selection_assigns_library_route() {
        let _guard = crate::config::TestStateDirGuard::new();
        let _sessions_guard = SESSIONS_LOAD_TEST_LOCK.lock().unwrap();
        fn fake_sessions(
            _client: &mbv_core::api::EmbyClient,
        ) -> Result<Vec<mbv_core::api::SessionInfo>, String> {
            Ok(vec![make_session_info(
                "living-room-pc",
                "mbv",
                "10.0.0.5",
                &[],
            )])
        }
        *SESSIONS_LOAD_OVERRIDE.lock().unwrap() = Some(fake_sessions);

        let mut app = make_app_stub();
        app.library_routes_popup = Some(LibraryRoutePopup {
            stage: LibraryRouteStage::PickDevice {
                library_lower: "music".to_string(),
                library_display: "Music".to_string(),
                devices: vec!["living-room-pc".to_string()],
            },
            cursor: 1, // index 0 is "Local (no route)"; 1 is the device
        });

        app.handle_library_routes_enter();

        *SESSIONS_LOAD_OVERRIDE.lock().unwrap() = None;
        assert_eq!(
            app.client.lock().unwrap().config.library_routes.get("music"),
            Some(&"living-room-pc".to_string())
        );
    }

    #[test]
    fn commit_device_selection_clears_route_on_local_no_route() {
        let mut app = make_app_stub();
        app.client
            .lock()
            .unwrap()
            .config
            .library_routes
            .insert("music".to_string(), "living-room-pc".to_string());
        app.library_routes_popup = Some(LibraryRoutePopup {
            stage: LibraryRouteStage::PickDevice {
                library_lower: "music".to_string(),
                library_display: "Music".to_string(),
                devices: vec!["living-room-pc".to_string()],
            },
            cursor: 0, // "Local (no route)"
        });

        app.handle_library_routes_enter();

        assert_eq!(
            app.client.lock().unwrap().config.library_routes.get("music"),
            None
        );
    }
```

Use whichever `SessionInfo`-construction helper Task 3 established (either
a `make_session_info` helper, if you added one there, or the same inline
struct literal pattern -- keep it consistent with Task 3's choice rather
than introducing a second convention).

- [ ] **Step 9: Run test to verify it fails, then implement, then verify it passes**

Run: `cargo test library_routes_popup`
Expected: FAIL to compile first (types/methods don't exist yet if Step 4-7
weren't done first in a real TDD ordering -- since this task's steps are
sequenced implementation-first for readability, run this after Step 7 and
confirm PASS)

- [ ] **Step 10: Run full suite, clippy, fmt**

Run: `cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt --all -- --check`
Expected: all clean

- [ ] **Step 11: Commit**

```bash
git add src/app/mod.rs src/app/settings.rs src/app/render/overlays/library_routes.rs src/app/render/overlays/settings.rs src/app/render/overlays/mod.rs src/app/render/mod.rs src/app/input.rs
git commit -m "app: add F2 Library Routes settings row and device picker"
```

---

### Task 5: Update docs

**Files:**
- Modify: `docs/adr/0011-library-scoped-daemon-routing.md` (append addendum, do not rewrite history)
- Modify: `CONTEXT.md:35-36`

**Interfaces:** None (docs only).

- [ ] **Step 1: Append an addendum to ADR 0011**

Read `docs/adr/0011-library-scoped-daemon-routing.md` in full first (it's
short). At the end of the file, append a new section following this
repo's existing convention for post-hoc ADR notes (see how earlier commits
appended "#233-fixed summary" notes to ADR 0011 without rewriting its
original body):

```markdown
## Addendum (#239): config renamed to `[library_routes]`, values are device names

The `[daemon_routes]` config table described above required a raw
`tcp://`/`unix://` connection string per library. #239 replaced it with
`[library_routes]`: library name -> **device name**, resolved against the
live Emby session list at connect time via the same mechanism
`App::session_direct_endpoint` already used for F3's Sessions panel
(`App::resolve_device_endpoint`, added in #239). No raw address is ever
typed or displayed. The `"*"` wildcard is gone; there is no migration path
from the old format -- it's a breaking config change. See
`docs/superpowers/specs/2026-07-18-library-routes-by-device-name-design.md`
for the full design.
```

- [ ] **Step 2: Rewrite the `CONTEXT.md` glossary entry**

Replace `CONTEXT.md:35-36`:

```markdown
**Library route** / **Route table** (`daemon_routes`):
A `[daemon_routes]` config table mapping library name (matched case-insensitively, same convention as `hidden_libraries`/`feed_view_libraries`) to a daemon endpoint string. A play/enqueue action resolved to a routed library swaps the active player to that daemon via `switch_to_library_route`, a sibling to `switch_to_direct_remote` using the same **Suspended local session** mechanism -- tracked by its own `active_route` field, kept independent of the Sessions-panel direct-remote's `connected_session_id`/`direct_remote_label`. `"*"` is a wildcard "route everything" entry. TOML-only for v1; no settings-panel UI. See #223.
```

with:

```markdown
**Library route** / **Route table** (`library_routes`):
A `[library_routes]` config table mapping library name (matched case-insensitively, same convention as `hidden_libraries`/`feed_view_libraries`) to a **device name**, resolved against the live Emby session list at connect time (`App::resolve_device_endpoint`) the same way F3's Sessions panel resolves a session to a connection. A play/enqueue action resolved to a routed library swaps the active player to that device via `switch_to_library_route`, a sibling to `switch_to_direct_remote` using the same **Suspended local session** mechanism -- tracked by its own `active_route` field, kept independent of the Sessions-panel direct-remote's `connected_session_id`/`direct_remote_label`. No wildcard. Editable via config.toml or the F2 Settings "Library routes" row. See #223, #239.
```

- [ ] **Step 3: Commit**

```bash
git add docs/adr/0011-library-scoped-daemon-routing.md CONTEXT.md
git commit -m "docs: update ADR 0011 and CONTEXT.md for library_routes (#239)"
```

---

### Task 6: Final verification

**Files:** None (verification only).

- [ ] **Step 1: Full workspace build**

Run: `cargo build --workspace`
Expected: clean build, no warnings

- [ ] **Step 2: Full test suite**

Run: `cargo test --workspace`
Expected: all tests pass

- [ ] **Step 3: Clippy**

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: no warnings

- [ ] **Step 4: fmt check**

Run: `cargo fmt --all -- --check`
Expected: clean (no diff)

- [ ] **Step 5: Grep for any remaining `daemon_routes` reference**

Run: `grep -rn "daemon_routes" --include="*.rs" --include="*.md" . | grep -v target | grep -v .claude/worktrees`
Expected: no hits outside historical plan/ADR files that intentionally
describe the old #223 design as history (e.g. the original ADR 0011 body
above the new addendum, and `docs/superpowers/plans/2026-07-17-library-scoped-daemon-routing.md`,
which is a historical record and must not be edited).
