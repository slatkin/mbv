# Library Route Endpoint Cache Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `[library_routes]` store a resolved `tcp://host:port` endpoint directly instead of a device name, so route resolution on the play/enqueue path is a pure config read with zero `/Sessions` network calls.

**Architecture:** `library_routes: HashMap<String, String>` keeps its exact shape (library name -> string); only the string's meaning changes, from device name to endpoint. `mbv_core::config::resolve_library_route` parses that string via the existing `DaemonEndpoint::parse`/`Display` (already used by the pre-#239 raw-endpoint config format) and requires it to be `Tcp(_)` — library routing is a remote-only feature (#239 addendum), so anything else (including a stale pre-#256 device-name string) is a malformed entry: logged and skipped, never routed. `App::resolve_device_endpoint` and its live-session lookup are deleted outright — nothing calls it once `resolve_route_for_library` no longer needs live resolution. The F2 "Library Routes" settings dialog is the *only* place a device name is ever seen: its device-picker still fetches the live session list (unavoidable — that's how you choose a *new* target), but purely for display and for preselecting the currently-assigned entry by comparing *resolved endpoints*, never by name. Nothing is persisted from that comparison.

**Tech Stack:** Rust, `mbv-core` library crate + `mbv` binary crate, existing `toml` parsing in `crates/mbv-core/src/config.rs`, existing `DaemonEndpoint` in `crates/mbv-core/src/remote_player.rs`.

## Global Constraints

- No device name is ever persisted to `config.toml` for a library route — only `tcp://host:port`.
- No automatic rediscovery/self-heal on a stale or failing cached endpoint. A connect failure falls back to local playback exactly like any other daemon-connect failure today (#222's existing rule) — never a special "try discovery" path.
- Library routing remains TCP-only / remote-only (`#239` addendum: "#222 and #223 are remote-connection features only") — a config value that parses to `DaemonEndpoint::Local` or `DaemonEndpoint::Unix` is treated as malformed, not as a valid same-host route.
- `cargo build --workspace`, `cargo clippy --workspace --all-targets` (zero warnings — delete unused code, never `#[allow(unused)]`), and `cargo test --workspace` must all pass before any commit in this plan (`mem:task_completion`).
- No `Co-Authored-By` trailers in commit messages; ask before pushing.

---

### Task 1: `mbv-core` + `App` — resolve_library_route becomes a pure, validated endpoint read

**This task lands as a single atomic commit spanning both crates.** `mbv-core` and the `mbv` binary are members of the same Cargo workspace, and `mbv` depends on `mbv-core` via a local path dependency (not a version boundary) — so the moment `resolve_library_route`'s return type changes from `Option<&str>` to `Option<DaemonEndpoint>`, `src/app/library_route.rs` (which calls it) stops compiling until it's updated in the same commit. Splitting this across two commits would leave the workspace in a broken intermediate state that violates this plan's own Global Constraint ("`cargo build --workspace` ... must pass before any commit"). Steps 1-6 below touch `mbv-core` only (including an interim crate-scoped sanity check in Step 6, cheap and useful for isolating a failure to this half of the change before the app-layer edits pile on top — it is NOT a substitute for Step 12's workspace-wide verification, which still re-runs everything); Steps 7-11 touch the `mbv` binary; Steps 12-13 verify and commit the whole thing together.

**Files:**
- Modify: `crates/mbv-core/src/config.rs:46-54` (field doc comment), `crates/mbv-core/src/config.rs:133-142` (`resolve_library_route`), `crates/mbv-core/src/config.rs:1359-1434` (tests)
- Modify: `src/app/library_route.rs:46-67` (`resolve_route_for_library`), plus its test module (`src/app/library_route.rs:210-618`)
- Modify: `src/app/mod.rs:2455-2473` (delete `resolve_device_endpoint`), `src/app/mod.rs:55-59` and `src/app/mod.rs:996-1000` (doc comments)

**Interfaces:**
- Consumes: `crate::remote_player::DaemonEndpoint::parse(&str) -> Result<DaemonEndpoint, String>` and its `Display` impl (both already exist, unchanged, in `crates/mbv-core/src/remote_player.rs`).
- Produces: `pub fn resolve_library_route(routes: &HashMap<String, String>, library_name: &str) -> Option<crate::remote_player::DaemonEndpoint>` — new return type (was `Option<&str>`). Also produces/keeps: `App::resolve_route_for_library(&mut self, library_name: &str) -> Option<(String, DaemonEndpoint)>` — same signature as before, so `route_for_active_library_view`, `route_for_item_via_ancestors`, `resolve_route_for_play`, `resolve_route_for_enqueue_folder`, `apply_route_for_playback`, and `try_auto_reconnect` need **no changes** — they only ever consumed this method's `(String, DaemonEndpoint)` output, never the resolution mechanism behind it. Task 3 (the F2 picker) consumes both of these.

- [ ] **Step 1: Update the `library_routes` field doc comment**

In `crates/mbv-core/src/config.rs`, find:

```rust
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

Replace with:

```rust
    /// Library name (lowercased) -> resolved `tcp://host:port` daemon
    /// endpoint, from `[library_routes]` (#256, replacing #239's
    /// device-name values). Playback/enqueue resolved to one of these
    /// libraries connects straight to this stored endpoint -- no
    /// `/Sessions` lookup on the play/enqueue path at all. No device name
    /// is stored; a device's friendly name is used only transiently by
    /// the F2 "Library Routes" picker to let the user *pick* a device,
    /// then immediately resolved to an endpoint before being written here.
    /// A value that isn't a valid `tcp://` endpoint (including a stale
    /// pre-#256 device-name string) is malformed: logged and skipped by
    /// `resolve_library_route`, never routed. No `"*"` wildcard. Editable
    /// via the F2 Settings "Library routes" row, and hand-editable in
    /// `config.toml`.
    pub library_routes: std::collections::HashMap<String, String>,
```

- [ ] **Step 2: Write the failing tests for the new `resolve_library_route`**

In `crates/mbv-core/src/config.rs`, replace the existing `resolve_library_route_has_no_wildcard_fallback` test (and the two tests that call `resolve_library_route`/read `library_routes` with device-name values) with endpoint-based versions. Find:

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
```

Replace with:

```rust
    #[test]
    fn parse_library_routes_lowercased_keys() {
        let toml = r#"
[server]
url = "http://host"
[library_routes]
Music = "tcp://192.168.0.104:47788"
"#;
        let cfg = parse_config(toml).unwrap();
        assert_eq!(
            cfg.library_routes.get("music").map(String::as_str),
            Some("tcp://192.168.0.104:47788")
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
"*" = "tcp://192.168.0.104:47788"
"#;
        let cfg = parse_config(toml).unwrap();
        assert_eq!(
            cfg.library_routes.get("*").map(String::as_str),
            Some("tcp://192.168.0.104:47788")
        );
        assert_eq!(resolve_library_route(&cfg.library_routes, "movies"), None);
    }
```

Now find:

```rust
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

Replace with:

```rust
    #[test]
    fn resolve_library_route_has_no_wildcard_fallback() {
        let mut routes = std::collections::HashMap::new();
        routes.insert(
            "music".to_string(),
            "tcp://192.168.0.104:47788".to_string(),
        );
        assert_eq!(
            resolve_library_route(&routes, "Music"),
            Some(crate::remote_player::DaemonEndpoint::Tcp(
                "192.168.0.104:47788".parse().unwrap()
            ))
        );
        assert_eq!(resolve_library_route(&routes, "movies"), None);
    }

    #[test]
    fn resolve_library_route_rejects_a_bare_device_name_as_malformed() {
        // A stale pre-#256 config entry (device name, no scheme) must
        // NOT silently resolve -- DaemonEndpoint::parse would otherwise
        // accept it as a bogus Unix(PathBuf) socket path. Library routing
        // is tcp://-only (#239 addendum), so anything that doesn't parse
        // to Tcp(_) is treated as malformed: logged and skipped.
        let mut routes = std::collections::HashMap::new();
        routes.insert("music".to_string(), "living-room-pc".to_string());
        assert_eq!(resolve_library_route(&routes, "music"), None);
    }

    #[test]
    fn resolve_library_route_rejects_unix_and_local_endpoints() {
        // Library routing is remote-only -- a unix:// or bare "local"
        // value is well-formed as a DaemonEndpoint but not a valid
        // library route, so it must still resolve to None.
        let mut routes = std::collections::HashMap::new();
        routes.insert(
            "music".to_string(),
            "unix:///run/mbvd.sock".to_string(),
        );
        routes.insert("movies".to_string(), "local".to_string());
        assert_eq!(resolve_library_route(&routes, "music"), None);
        assert_eq!(resolve_library_route(&routes, "movies"), None);
    }
```

- [ ] **Step 3: Run the tests to confirm they fail to compile / fail**

Run: `cargo test -p mbv-core resolve_library_route -- --nocapture`
Expected: compile error (`resolve_library_route` still returns `Option<&str>`, doesn't match the new assertions) — confirms the old implementation is in place and the new tests exercise the change we're about to make.

- [ ] **Step 4: Implement the new `resolve_library_route`**

In `crates/mbv-core/src/config.rs`, find:

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

Replace with:

```rust
/// Resolves the configured endpoint for a library name (#256). Matches
/// case-insensitively (the query is lowercased before lookup; `routes`'
/// keys are already lowercased by `parse_config`). No wildcard fallback --
/// returns `None` if the library has no route, and the caller stays local.
///
/// Parses the stored string via `DaemonEndpoint::parse` and requires it to
/// be `Tcp(_)` -- library routing is a remote-only feature (#239 addendum:
/// "#222 and #223 are remote-connection features only"), so anything else
/// is malformed: a bare pre-#256 device-name string (which `parse` would
/// otherwise silently accept as a bogus `Unix(PathBuf)` socket path), a
/// `unix://` value, or a bare `local`/empty value are all logged and
/// skipped rather than routed. This is a pure, synchronous, no-network
/// lookup -- the entire point of #256 is that route resolution on the
/// play/enqueue path never touches `/Sessions` again.
pub fn resolve_library_route(
    routes: &std::collections::HashMap<String, String>,
    library_name: &str,
) -> Option<crate::remote_player::DaemonEndpoint> {
    let raw = routes.get(&library_name.to_lowercase())?;
    match crate::remote_player::DaemonEndpoint::parse(raw) {
        Ok(endpoint @ crate::remote_player::DaemonEndpoint::Tcp(_)) => Some(endpoint),
        Ok(other) => {
            log::warn!(
                target: "library_route",
                "library_routes entry {raw:?} parsed as {other:?}, but library routing is tcp://-only; skipping"
            );
            None
        }
        Err(e) => {
            log::warn!(
                target: "library_route",
                "library_routes entry {raw:?} is not a valid tcp:// endpoint: {e}; skipping"
            );
            None
        }
    }
}
```

- [ ] **Step 5: Run the tests to confirm they pass**

Run: `cargo test -p mbv-core resolve_library_route -- --nocapture` and `cargo test -p mbv-core parse_library_routes`
Expected: all `PASS`.

- [ ] **Step 6: Run the full `mbv-core` test suite and clippy (interim, crate-scoped)**

Run: `cargo test -p mbv-core && cargo clippy -p mbv-core --all-targets`
Expected: all tests pass, zero clippy warnings.

Do not commit here, and do not run `cargo build --workspace` or anything scoped to `-p mbv` yet — `mbv-core` itself builds and tests fine in isolation (it has no dependency on the `mbv` binary), but the `mbv` binary crate does not compile against this new `resolve_library_route` signature until Step 10 lands. Continue straight to Step 7.

- [ ] **Step 7: Write the failing test for the simplified resolver**

In `src/app/library_route.rs`, inside `mod tests`, replace the existing `resolve_route_for_library_matches_case_insensitively` test. Find:

```rust
    #[test]
    fn resolve_route_for_library_matches_case_insensitively() {
        let _guard = crate::config::TestStateDirGuard::new();
        let _sessions_guard = SESSIONS_LOAD_TEST_LOCK.lock().unwrap();
        fn fake_sessions(
            _client: &mbv_core::api::EmbyClient,
        ) -> Result<Vec<mbv_core::api::SessionInfo>, String> {
            let mut sess = make_session("living-room-pc", "mbv");
            sess.host = "127.0.0.1".into();
            sess.supported_commands = vec![mbv_core::api::mbv_direct_tcp_port_command(9000)];
            Ok(vec![sess])
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
                mbv_core::remote_player::DaemonEndpoint::Tcp("127.0.0.1:9000".parse().unwrap())
            ))
        );
    }
```

Replace with:

```rust
    #[test]
    fn resolve_route_for_library_matches_case_insensitively() {
        // #256: resolution is now a pure config read -- no live session
        // lookup, no SESSIONS_LOAD_OVERRIDE seam needed at all.
        let mut app = make_app_stub();
        app.library_routes.insert(
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
    fn resolve_route_for_library_returns_none_for_a_malformed_endpoint() {
        // #256: a config value that isn't a valid tcp:// endpoint (stale
        // pre-#256 device name, unix://, etc.) resolves to None rather
        // than being routed or panicking.
        let mut app = make_app_stub();
        app.library_routes
            .insert("music".to_string(), "living-room-pc".to_string());

        assert_eq!(app.resolve_route_for_library("Music"), None);
    }
```

Now delete the two tests that exercised live device-name resolution, which no longer applies now that resolution never touches the session list. Find and remove entirely:

```rust
    #[test]
    fn resolve_route_for_library_resolves_via_live_device_name() {
        let _guard = crate::config::TestStateDirGuard::new();
        let _sessions_guard = SESSIONS_LOAD_TEST_LOCK.lock().unwrap();
        fn fake_sessions(
            _client: &mbv_core::api::EmbyClient,
        ) -> Result<Vec<mbv_core::api::SessionInfo>, String> {
            let mut sess = make_session("living-room-pc", "mbv");
            sess.host = "10.0.0.5".into();
            sess.supported_commands = vec![mbv_core::api::mbv_direct_tcp_port_command(9100)];
            Ok(vec![sess])
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
    fn resolve_route_for_library_skips_same_device_non_mbv_sessions() {
        let _guard = crate::config::TestStateDirGuard::new();
        let _sessions_guard = SESSIONS_LOAD_TEST_LOCK.lock().unwrap();
        fn fake_sessions(
            _client: &mbv_core::api::EmbyClient,
        ) -> Result<Vec<mbv_core::api::SessionInfo>, String> {
            let mut browser = make_session("music.local", "Firefox");
            browser.host = "10.0.0.104".into();

            let mut mbv = make_session("music.local", "mbv");
            mbv.host = "10.0.0.104".into();
            mbv.supported_commands = vec![mbv_core::api::mbv_direct_tcp_port_command(9100)];
            Ok(vec![browser, mbv])
        }
        *SESSIONS_LOAD_OVERRIDE.lock().unwrap() = Some(fake_sessions);

        let mut app = make_app_stub();
        app.library_routes
            .insert("music".to_string(), "music.local".to_string());

        let resolved = app.resolve_route_for_library("Music");

        *SESSIONS_LOAD_OVERRIDE.lock().unwrap() = None;
        assert_eq!(
            resolved,
            Some((
                "music".to_string(),
                mbv_core::remote_player::DaemonEndpoint::Tcp(std::net::SocketAddr::from((
                    std::net::Ipv4Addr::new(10, 0, 0, 104),
                    9100
                )))
            ))
        );
    }
```

And find/replace `resolve_route_for_library_falls_back_to_local_when_device_offline` (the "offline device" concept no longer applies -- there's no liveness check left to fall back from):

```rust
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
```

Delete it entirely (superseded by `resolve_route_for_library_returns_none_for_a_malformed_endpoint` above and by `resolve_route_for_library_returns_none_when_unconfigured`, which already covers "no entry at all").

- [ ] **Step 8: Update the other tests in this file that used `SESSIONS_LOAD_OVERRIDE` only to satisfy the old device-name resolution**

These tests (`route_for_active_library_view_uses_nav_state_no_network`, `resolve_route_for_play_does_not_panic_from_the_queue_tab`, `resolve_route_for_play_from_queue_resolves_item_when_no_route_is_active`, `resolve_route_for_enqueue_folder_matches_a_library_root_folder_by_its_own_name`) each set up a `fake_sessions`/`SESSIONS_LOAD_OVERRIDE` purely so the old `resolve_device_endpoint` could resolve `"living-room-pc"` to a live endpoint. That's gone now, so simplify each: drop the `_sessions_guard`/`fake_sessions`/`SESSIONS_LOAD_OVERRIDE` setup and teardown lines, and change the inserted route value from `"living-room-pc"` to `"tcp://127.0.0.1:9000"` directly.

For example, find:

```rust
    #[test]
    fn route_for_active_library_view_uses_nav_state_no_network() {
        // "No network" here means no `get_ancestors` round-trip -- the
        // active library is already known from nav state. Resolving the
        // routed device against the live session list is still needed
        // (#239), hence the SESSIONS_LOAD_OVERRIDE seam.
        let _guard = crate::config::TestStateDirGuard::new();
        let _sessions_guard = SESSIONS_LOAD_TEST_LOCK.lock().unwrap();
        fn fake_sessions(
            _client: &mbv_core::api::EmbyClient,
        ) -> Result<Vec<mbv_core::api::SessionInfo>, String> {
            let mut sess = make_session("living-room-pc", "mbv");
            sess.host = "127.0.0.1".into();
            sess.supported_commands = vec![mbv_core::api::mbv_direct_tcp_port_command(9000)];
            Ok(vec![sess])
        }
        *SESSIONS_LOAD_OVERRIDE.lock().unwrap() = Some(fake_sessions);

        let mut app = make_app_stub();
        app.library_routes
            .insert("music".to_string(), "living-room-pc".to_string());
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

        *SESSIONS_LOAD_OVERRIDE.lock().unwrap() = None;
        assert_eq!(resolved.map(|(name, _)| name), Some("music".to_string()));
    }
```

Replace with:

```rust
    #[test]
    fn route_for_active_library_view_uses_nav_state_no_network() {
        // The active library is already known from nav state, and (#256)
        // resolving its routed endpoint is now a pure config read too --
        // this is unconditionally no-network, not just "no get_ancestors
        // round-trip".
        let mut app = make_app_stub();
        app.library_routes.insert(
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
```

Apply the same shape of simplification (drop `_sessions_guard`/`fake_sessions`/`SESSIONS_LOAD_OVERRIDE` set/reset, change `"living-room-pc"` to `"tcp://127.0.0.1:9000"`) to `resolve_route_for_play_does_not_panic_from_the_queue_tab`, `resolve_route_for_play_from_queue_resolves_item_when_no_route_is_active`, and `resolve_route_for_enqueue_folder_matches_a_library_root_folder_by_its_own_name`.

The remaining tests in this file (`route_for_active_library_view_none_for_unrouted_library`, the four `route_for_item_via_ancestors_*` cache tests, `resolve_route_for_library_returns_none_when_unconfigured`, `resolve_route_for_enqueue_folder_falls_back_to_ancestor_lookup_for_a_non_root_folder`) don't exercise live resolution at all — leave them as-is (their `"living-room-pc"` string literals are inert placeholders never parsed as an endpoint in those code paths, since they fail or short-circuit before reaching `resolve_route_for_library`).

- [ ] **Step 9: Run the test module to confirm the new/edited tests fail to compile**

Run: `cargo test -p mbv --lib library_route::tests -- --nocapture`
Expected: compile error — `resolve_route_for_library` still calls the now-removed-by-the-next-step `resolve_device_endpoint`, and (since Step 4 already changed `mbv_core::config::resolve_library_route`'s return type) passes a `DaemonEndpoint` where `resolve_device_endpoint` expects a `&str`. Confirms Step 10 is required before this compiles.

- [ ] **Step 10: Implement the simplified `resolve_route_for_library`**

In `src/app/library_route.rs`, find:

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

Replace with:

```rust
    /// Resolves the configured library route for a library name (#256):
    /// a pure, synchronous `library_routes` config read -- no `/Sessions`
    /// call, ever, on this path. Returns `(lowercased_library_name,
    /// endpoint)` when the config has a well-formed `tcp://` entry for
    /// this library. A missing or malformed entry is not an error (no
    /// warning flashed, beyond `resolve_library_route`'s own log line for
    /// the malformed case) -- it's the expected, common case of "not
    /// routed"; #222's existing fallback (stay local, no hard error)
    /// already covers it via the `None` return.
    pub(super) fn resolve_route_for_library(
        &mut self,
        library_name: &str,
    ) -> Option<(String, mbv_core::remote_player::DaemonEndpoint)> {
        let name = library_name.trim();
        if name.is_empty() {
            return None;
        }
        let endpoint = mbv_core::config::resolve_library_route(&self.library_routes, name)?;
        Some((name.to_lowercase(), endpoint))
    }
```

- [ ] **Step 11: Delete the now-dead `resolve_device_endpoint` and update its doc references**

In `src/app/mod.rs`, find and delete entirely:

```rust
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
        sessions
            .iter()
            .filter(|s| s.device_name.eq_ignore_ascii_case(device_name))
            .find_map(|s| self.session_direct_endpoint(s))
    }
```

Then find (a few lines above `fetch_sessions_blocking`'s own doc comment):

```rust
    /// Blocking `GET /Sessions` (unfiltered), factored out only so tests
    /// can override it via `SESSIONS_LOAD_OVERRIDE` -- mirrors
    /// `connect_daemon_route_endpoint`'s `#[cfg(test)]` seam. Callers:
    /// `resolve_device_endpoint` (#239's library-route resolution) and
    /// `try_auto_reconnect`'s `DirectSession` case (#236).
```

Replace with:

```rust
    /// Blocking `GET /Sessions` (unfiltered), factored out only so tests
    /// can override it via `SESSIONS_LOAD_OVERRIDE` -- mirrors
    /// `connect_daemon_route_endpoint`'s `#[cfg(test)]` seam. Callers:
    /// `try_auto_reconnect`'s `DirectSession` case (#236) and the F2
    /// "Library Routes" device picker (`enter_device_stage`, #256) --
    /// library-route *resolution* itself no longer calls this (#256).
```

Then find the `SESSIONS_LOAD_OVERRIDE` type doc comment:

```rust
// Test seam for live-session-list lookups, mirroring
// DAEMON_ROUTE_CONNECT_OVERRIDE/_TEST_LOCK above: lets tests inject a fake
// session list without a real network call. Shared by
// `try_auto_reconnect`'s `DirectSession` lookup (#236) and
// `resolve_device_endpoint`'s library-route lookup (#239).
```

Replace with:

```rust
// Test seam for live-session-list lookups, mirroring
// DAEMON_ROUTE_CONNECT_OVERRIDE/_TEST_LOCK above: lets tests inject a fake
// session list without a real network call. Shared by
// `try_auto_reconnect`'s `DirectSession` lookup (#236) and the F2
// "Library Routes" device picker (`enter_device_stage`, #256).
```

Then find the `library_routes` field doc comment on `App` itself:

```rust
    /// `Config.library_routes` at startup (#239). Values are device names,
    /// resolved lazily against the *live* session list at connect time via
    /// `resolve_device_endpoint`, not eagerly -- so a device that's offline
    /// right now still shows a configured assignment; it just won't
    /// resolve to a connection until it's live again.
    library_routes: std::collections::HashMap<String, String>,
```

Replace with:

```rust
    /// `Config.library_routes` at startup (#256). Values are resolved
    /// `tcp://host:port` endpoints, read directly with no live-session
    /// lookup -- see `mbv_core::config::resolve_library_route`.
    library_routes: std::collections::HashMap<String, String>,
```

- [ ] **Step 12: Run the full workspace build, test suite, and clippy**

This is the atomic-commit boundary for this task — verify at the workspace level, not scoped to one crate, since Steps 1-6 (mbv-core) and Steps 7-11 (the `mbv` binary) only compose into a working build together.

Run: `cargo build --workspace`
Expected: succeeds with zero errors.

Run: `cargo test --workspace`
Expected: all tests pass, including every test touched in Steps 2 and 7-8.

Run: `cargo clippy --workspace --all-targets`
Expected: zero warnings (confirms `resolve_device_endpoint`'s removal left nothing else dangling — if clippy reports anything else unused, delete it rather than suppressing, per `mem:conventions`).

- [ ] **Step 13: Commit**

```bash
git add crates/mbv-core/src/config.rs src/app/library_route.rs src/app/mod.rs
git commit -m "core+app: resolve_library_route reads a cached tcp:// endpoint, drop live lookup (#256)"
```

---

### Task 2: Pre-existing clippy cleanup (unrelated to #256)

**Not part of #256's design** — inserted by explicit user decision during Task 1's review. Task 1's diff was clean, but `cargo clippy --workspace --all-targets` on the resulting HEAD showed 6 pre-existing warnings in files no task in this plan otherwise touches, confirmed (via `git diff` against Task 1's base commit) to already exist before this plan started. The plan's Global Constraint says `cargo clippy --workspace --all-targets` must show zero warnings before any commit "in this plan" — read literally, that requires this cleanup too. The user was asked whether to scope the constraint to touched files instead, and chose to fix these now rather than defer them. This task is pure mechanical cleanup with no design content; each fix below is clippy's own suggested rewrite, verified against the surrounding code.

**Files:**
- Modify: `src/app/input.rs:4362-4374` (test-only complex type)
- Modify: `src/app/render/power/home.rs:605-610, 649, 656` (enum variant size)
- Modify: `src/app/render/power/queue.rs:271-273` (obfuscated if/else)
- Modify: `src/app/render/mod.rs:936-947` (explicit counter loop)
- Modify: `src/mpris.rs:186-205` (complex return type)

**Interfaces:** None — every fix here is internal (private types, a test helper, a closure body). No other task consumes anything from this one.

- [ ] **Step 1: Factor the complex parameter type in `input.rs`'s test helper into a named type**

In `src/app/input.rs`, find:

```rust
    impl RecursiveFetchServer {
        fn start(
            responses: Vec<(
                &'static str,
                Result<Vec<(&'static str, &'static str, i64)>, &'static str>,
            )>,
        ) -> Self {
```

Replace with:

```rust
    type RecursiveFetchResponses = Vec<(
        &'static str,
        Result<Vec<(&'static str, &'static str, i64)>, &'static str>,
    )>;

    impl RecursiveFetchServer {
        fn start(responses: RecursiveFetchResponses) -> Self {
```

- [ ] **Step 2: Box the large variant of `DisplayRow` in `render/power/home.rs`**

`DisplayRow::Item` carries a full `mbv_core::api::MediaItem`, making it far larger than the other data-free variants (`Pills`, `Empty`, `Blank`) — clippy's `large_enum_variant` lint. Box the payload.

Find:

```rust
        enum DisplayRow {
            Pills,
            Empty,
            Item(usize, mbv_core::api::MediaItem),
            Blank,
        }
```

Replace with:

```rust
        enum DisplayRow {
            Pills,
            Empty,
            Item(usize, Box<mbv_core::api::MediaItem>),
            Blank,
        }
```

Then update the two construction sites (the four call sites that only pattern-match — `flat_idx, _`, or match through a `&DisplayRow` reference and only touch `flat_idx`/read fields via auto-deref — need no changes; verified by reading the full match arm at line ~720, which only reads `item.is_folder`/`item.runtime_ticks`/`item.display_name()`, all of which work identically through `&Box<MediaItem>` via auto-deref).

Find:

```rust
            for (idx, item) in continue_items.into_iter().enumerate() {
                rows.push(DisplayRow::Item(idx, item));
            }
```

Replace with:

```rust
            for (idx, item) in continue_items.into_iter().enumerate() {
                rows.push(DisplayRow::Item(idx, Box::new(item)));
            }
```

Find:

```rust
            for (idx, item) in section.items.iter().cloned().enumerate() {
                rows.push(DisplayRow::Item(section.flat_start + idx, item));
            }
```

Replace with:

```rust
            for (idx, item) in section.items.iter().cloned().enumerate() {
                rows.push(DisplayRow::Item(section.flat_start + idx, Box::new(item)));
            }
```

- [ ] **Step 3: Replace the obfuscated `then_some(..).unwrap_or(..)` chains in `render/power/queue.rs`**

Find:

```rust
                    let metadata_w = dur_visible.then_some(dur.width()).unwrap_or(0)
                        + pct_visible.then_some(pct_str.width()).unwrap_or(0)
                        + metadata_gap;
```

Replace with:

```rust
                    let metadata_w = (if dur_visible { dur.width() } else { 0 })
                        + (if pct_visible { pct_str.width() } else { 0 })
                        + metadata_gap;
```

- [ ] **Step 4: Replace the manual loop counter in `render/mod.rs`'s `joined_width` closure with `.enumerate()`**

Find:

```rust
        let joined_width = |widths: &[u16]| -> u16 {
            let mut total = 0u16;
            let mut count = 0u16;
            for width in widths.iter().copied().filter(|w| *w > 0) {
                total = total.saturating_add(width);
                if count > 0 {
                    total = total.saturating_add(1);
                }
                count += 1;
            }
            total
        };
```

Replace with:

```rust
        let joined_width = |widths: &[u16]| -> u16 {
            let mut total = 0u16;
            for (count, width) in widths.iter().copied().filter(|w| *w > 0).enumerate() {
                total = total.saturating_add(width);
                if count > 0 {
                    total = total.saturating_add(1);
                }
            }
            total
        };
```

This preserves identical behavior: `enumerate()`'s index is 0 on the first non-zero-width entry (same as the old `count` before its first increment), so no separator is added before the first entry and one is added before each subsequent one, exactly as before.

- [ ] **Step 5: Factor the complex return type of `status_and_sender` in `mpris.rs` into a named type**

Find:

```rust
    fn status_and_sender(
        &self,
    ) -> (
        Arc<Mutex<PlayerStatus>>,
        Arc<dyn Fn(PlayerCommand) + Send + Sync>,
    ) {
        let source = self.source.lock().unwrap();
        (source.status.clone(), source.send.clone())
    }
```

Replace with:

```rust
    fn status_and_sender(&self) -> StatusAndSender {
        let source = self.source.lock().unwrap();
        (source.status.clone(), source.send.clone())
    }
```

And immediately above the `impl MediaPlayer2Player` block containing this method, find:

```rust
impl MediaPlayer2Player {
    /// Clones the current `status`/`send` pair out from behind `self.source`'s
```

Replace with:

```rust
type StatusAndSender = (
    Arc<Mutex<PlayerStatus>>,
    Arc<dyn Fn(PlayerCommand) + Send + Sync>,
);

impl MediaPlayer2Player {
    /// Clones the current `status`/`send` pair out from behind `self.source`'s
```

- [ ] **Step 6: Run the full workspace build, test, and clippy**

Run: `cargo build --workspace`
Expected: succeeds with zero errors.

Run: `cargo test --workspace`
Expected: all tests pass (these are style-only changes — no behavior change, so no test should need editing).

Run: `cargo clippy --workspace --all-targets`
Expected: zero warnings, workspace-wide.

- [ ] **Step 7: Commit**

```bash
git add src/app/input.rs src/app/render/power/home.rs src/app/render/power/queue.rs src/app/render/mod.rs src/mpris.rs
git commit -m "chore: fix pre-existing clippy warnings across app/mpris (unrelated to #256)"
```

---

### Task 3: F2 "Library Routes" picker — resolve to an endpoint at commit time, preselect by endpoint not name

**Files:**
- Modify: `src/app/mod.rs:207-219` (`LibraryRouteStage::PickDevice.devices` type)
- Modify: `src/app/render/overlays/library_routes.rs` (`enter_device_stage`, `commit_device_selection`, `render_library_routes_popup`, and its test module)

**Interfaces:**
- Consumes: `App::fetch_sessions_blocking(&self) -> Result<Vec<SessionInfo>, String>` and `App::session_direct_endpoint(&self, &SessionInfo) -> Option<DaemonEndpoint>` (both unchanged, from `src/app/mod.rs`), plus `mbv_core::remote_player::DaemonEndpoint::parse`/`Display` from Task 1.
- Produces: `LibraryRouteStage::PickDevice { devices: Vec<(String, Option<mbv_core::remote_player::DaemonEndpoint>)>, .. }` — device display name paired with its live-resolved endpoint, where `None` means "visible in the live session list but not currently routable" (shown greyed out with a reason, not omitted — see Step 4's rationale). No other task depends on this type, but it must stay internally consistent within this file.

- [ ] **Step 1: Write the failing test for endpoint-based commit**

In `src/app/render/overlays/library_routes.rs`, inside `mod tests`, find:

```rust
    #[test]
    fn commit_device_selection_assigns_library_route() {
        let _guard = crate::config::TestStateDirGuard::new();
        let _sessions_guard = SESSIONS_LOAD_TEST_LOCK.lock().unwrap();
        fn fake_sessions(
            _client: &mbv_core::api::EmbyClient,
        ) -> Result<Vec<mbv_core::api::SessionInfo>, String> {
            Ok(vec![make_session("living-room-pc", "mbv")])
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
            app.client
                .lock()
                .unwrap()
                .config
                .library_routes
                .get("music"),
            Some(&"living-room-pc".to_string())
        );
        assert_eq!(
            app.library_routes.get("music"),
            Some(&"living-room-pc".to_string())
        );
    }
```

Replace with:

```rust
    #[test]
    fn commit_device_selection_assigns_library_route_as_an_endpoint() {
        // #256: the config value committed here must be the device's
        // resolved endpoint, never its name.
        let mut app = make_app_stub();
        let endpoint = mbv_core::remote_player::DaemonEndpoint::Tcp(
            "127.0.0.1:9000".parse().unwrap(),
        );
        app.library_routes_popup = Some(LibraryRoutePopup {
            stage: LibraryRouteStage::PickDevice {
                library_lower: "music".to_string(),
                library_display: "Music".to_string(),
                devices: vec![("living-room-pc".to_string(), Some(endpoint.clone()))],
            },
            cursor: 1, // index 0 is "Local (no route)"; 1 is the device
        });

        app.handle_library_routes_enter();

        assert_eq!(
            app.client
                .lock()
                .unwrap()
                .config
                .library_routes
                .get("music"),
            Some(&endpoint.to_string())
        );
        assert_eq!(
            app.library_routes.get("music"),
            Some(&endpoint.to_string())
        );
    }
```

Now find:

```rust
    #[test]
    fn commit_device_selection_clears_route_on_local_no_route() {
        let mut app = make_app_stub();
        app.client
            .lock()
            .unwrap()
            .config
            .library_routes
            .insert("music".to_string(), "living-room-pc".to_string());
        app.library_routes
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
            app.client
                .lock()
                .unwrap()
                .config
                .library_routes
                .get("music"),
            None
        );
        assert_eq!(app.library_routes.get("music"), None);
    }
```

Replace with:

```rust
    #[test]
    fn commit_device_selection_clears_route_on_local_no_route() {
        let mut app = make_app_stub();
        app.client
            .lock()
            .unwrap()
            .config
            .library_routes
            .insert("music".to_string(), "tcp://127.0.0.1:9000".to_string());
        app.library_routes
            .insert("music".to_string(), "tcp://127.0.0.1:9000".to_string());
        app.library_routes_popup = Some(LibraryRoutePopup {
            stage: LibraryRouteStage::PickDevice {
                library_lower: "music".to_string(),
                library_display: "Music".to_string(),
                devices: vec![(
                    "living-room-pc".to_string(),
                    Some(mbv_core::remote_player::DaemonEndpoint::Tcp(
                        "127.0.0.1:9000".parse().unwrap(),
                    )),
                )],
            },
            cursor: 0, // "Local (no route)"
        });

        app.handle_library_routes_enter();

        assert_eq!(
            app.client
                .lock()
                .unwrap()
                .config
                .library_routes
                .get("music"),
            None
        );
        assert_eq!(app.library_routes.get("music"), None);
    }
```

Add a new test for endpoint-based preselection:

```rust
    #[test]
    fn enter_device_stage_preselects_by_resolved_endpoint_not_name() {
        // #256: preselecting which picker row matches the current
        // assignment must compare resolved endpoints, not device names --
        // endpoints are the stable identifier here (a hostname is more
        // likely to change than the address it currently resolves to).
        let _guard = crate::config::TestStateDirGuard::new();
        let _sessions_guard = SESSIONS_LOAD_TEST_LOCK.lock().unwrap();
        fn fake_sessions(
            _client: &mbv_core::api::EmbyClient,
        ) -> Result<Vec<mbv_core::api::SessionInfo>, String> {
            let mut sess = make_session("living-room-pc", "mbv");
            sess.host = "127.0.0.1".into();
            sess.supported_commands = vec![mbv_core::api::mbv_direct_tcp_port_command(9000)];
            Ok(vec![sess])
        }
        *SESSIONS_LOAD_OVERRIDE.lock().unwrap() = Some(fake_sessions);

        let mut app = make_app_stub();
        app.client
            .lock()
            .unwrap()
            .config
            .library_routes
            .insert("music".to_string(), "tcp://127.0.0.1:9000".to_string());
        app.open_library_routes_popup();
        app.handle_library_routes_enter(); // PickLibrary -> PickDevice for "music"

        *SESSIONS_LOAD_OVERRIDE.lock().unwrap() = None;
        let popup = app.library_routes_popup.as_ref().unwrap();
        assert_eq!(popup.cursor, 1); // 0 = "Local (no route)", 1 = the matched device
    }
```

Add two more new tests, covering a live mbv session that can't yield a resolvable endpoint (no advertised direct-connect port): it must still appear in the picker (not be silently dropped), and attempting to commit it must be a no-op that explains why rather than writing garbage to config:

```rust
    #[test]
    fn enter_device_stage_lists_an_unresolvable_device_instead_of_omitting_it() {
        // #256: a live "mbv" session that session_direct_endpoint can't
        // resolve to an endpoint (here: no advertised direct-connect port)
        // must still show up in the picker, paired with `None` -- silently
        // omitting it would leave a device visible in F3's Sessions panel
        // with no explanation for why it doesn't appear here.
        let _guard = crate::config::TestStateDirGuard::new();
        let _sessions_guard = SESSIONS_LOAD_TEST_LOCK.lock().unwrap();
        fn fake_sessions(
            _client: &mbv_core::api::EmbyClient,
        ) -> Result<Vec<mbv_core::api::SessionInfo>, String> {
            // No supported_commands entry -> parse_mbv_direct_tcp_port
            // finds nothing -> session_direct_endpoint returns None.
            Ok(vec![make_session("no-port-device", "mbv")])
        }
        *SESSIONS_LOAD_OVERRIDE.lock().unwrap() = Some(fake_sessions);

        let mut app = make_app_stub();
        app.open_library_routes_popup();
        app.handle_library_routes_enter(); // PickLibrary -> PickDevice

        *SESSIONS_LOAD_OVERRIDE.lock().unwrap() = None;
        let popup = app.library_routes_popup.as_ref().unwrap();
        let LibraryRouteStage::PickDevice { devices, .. } = &popup.stage else {
            panic!("expected PickDevice stage");
        };
        assert_eq!(
            devices,
            &vec![("no-port-device".to_string(), None)]
        );
    }

    #[test]
    fn commit_device_selection_flashes_and_does_not_commit_for_an_unroutable_device() {
        // #256: selecting a greyed-out (None-endpoint) row must not write
        // anything to config -- there is nothing meaningful to write --
        // and must tell the user why, rather than silently doing nothing.
        let mut app = make_app_stub();
        app.library_routes_popup = Some(LibraryRoutePopup {
            stage: LibraryRouteStage::PickDevice {
                library_lower: "music".to_string(),
                library_display: "Music".to_string(),
                devices: vec![("no-port-device".to_string(), None)],
            },
            cursor: 1, // index 0 is "Local (no route)"; 1 is the device
        });

        app.handle_library_routes_enter();

        assert_eq!(app.client.lock().unwrap().config.library_routes.get("music"), None);
        assert_eq!(app.library_routes.get("music"), None);
        assert!(app.status.contains("no-port-device"));
        assert!(app.status.contains("not currently routable"));
        // Still on the PickDevice stage -- a no-op, not silently
        // reverting to the library list either.
        assert!(matches!(
            app.library_routes_popup.as_ref().unwrap().stage,
            LibraryRouteStage::PickDevice { .. }
        ));
    }
```

- [ ] **Step 2: Run the tests to confirm they fail to compile**

Run: `cargo test -p mbv --lib render::overlays::library_routes::tests -- --nocapture`
Expected: compile error — `LibraryRouteStage::PickDevice.devices` is still `Vec<String>`, doesn't accept tuple literals.

- [ ] **Step 3: Change the `devices` field type**

In `src/app/mod.rs`, find:

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
```

Replace with:

```rust
#[derive(Clone)]
pub(crate) enum LibraryRouteStage {
    /// (library_name_lower, display_name, current_device_or_none)
    PickLibrary {
        items: Vec<(String, String, Option<String>)>,
    },
    /// index 0 is always the synthetic "Local (no route)" entry.
    /// Each entry pairs a device's display name (UX only -- #256 never
    /// persists it) with its live-resolved endpoint (what actually gets
    /// written to config on commit). `None` means the device is visible
    /// in the live session list but session_direct_endpoint couldn't
    /// resolve it to a connectable address (e.g. no advertised
    /// direct-connect port, or an unparseable host) -- shown greyed out
    /// with a reason rather than silently omitted, and not committable.
    PickDevice {
        library_lower: String,
        library_display: String,
        devices: Vec<(String, Option<mbv_core::remote_player::DaemonEndpoint>)>,
    },
}
```

- [ ] **Step 4: Update `enter_device_stage` to resolve each candidate's endpoint and preselect by endpoint**

In `src/app/render/overlays/library_routes.rs`, find:

```rust
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
```

Replace with:

```rust
    fn enter_device_stage(&mut self, library_lower: String, library_display: String) {
        let sessions = self.fetch_sessions_blocking().unwrap_or_default();
        let local_device_name = self.client.lock().unwrap().device_name.clone();
        let mut devices: Vec<(String, Option<mbv_core::remote_player::DaemonEndpoint>)> =
            sessions
                .iter()
                .filter(|s| s.client.eq_ignore_ascii_case("mbv"))
                .filter(|s| !s.device_name.eq_ignore_ascii_case(&local_device_name))
                .map(|s| {
                    // A live mbv session that doesn't yield a resolvable
                    // endpoint (e.g. no advertised direct-connect port, or
                    // an unparseable host) is kept in the list, paired
                    // with None, rather than dropped (#256): omitting it
                    // entirely would leave a device the user can see live
                    // in F3's Sessions panel silently missing here with no
                    // way to tell why. `render_library_routes_popup`
                    // renders a `None` entry greyed out with a reason, and
                    // `commit_device_selection` refuses to commit it.
                    (s.device_name.clone(), self.session_direct_endpoint(s))
                })
                .collect();
        devices.sort_by(|a, b| a.0.cmp(&b.0));
        devices.dedup_by(|a, b| a.0.eq_ignore_ascii_case(&b.0));

        // Preselect by resolved endpoint, not by name (#256): a hostname
        // is more likely to change than the address it currently resolves
        // to, and this comparison is free -- `devices` above already paid
        // for the live session fetch this stage needs regardless, to let
        // the user pick a *new* device.
        let current_endpoint = self
            .client
            .lock()
            .unwrap()
            .config
            .library_routes
            .get(&library_lower)
            .and_then(|raw| mbv_core::remote_player::DaemonEndpoint::parse(raw).ok());
        let cursor = current_endpoint
            .and_then(|current| {
                devices
                    .iter()
                    .position(|(_, ep)| ep.as_ref() == Some(&current))
            })
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
```

- [ ] **Step 5: Update `commit_device_selection` to store the endpoint**

In `src/app/render/overlays/library_routes.rs`, find:

```rust
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
```

Replace with:

```rust
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

        if cursor > 0 {
            if let Some((name, None)) = devices.get(cursor - 1) {
                // #256: a device shown in this picker without a
                // resolvable endpoint (greyed out, see enter_device_stage)
                // can't be committed -- there is nothing meaningful to
                // write to config for it. Flash the reason and stay on
                // this stage rather than silently doing nothing.
                self.flash_status(format!(
                    "{name} isn't currently routable (no resolvable direct-connect endpoint)"
                ));
                return;
            }
        }

        {
            let mut c = self.client.lock().unwrap();
            if cursor == 0 {
                c.config.library_routes.remove(&library_lower);
            } else if let Some((_, Some(endpoint))) = devices.get(cursor - 1) {
                // #256: persist the resolved endpoint, never the device
                // name -- the name was only ever needed to let the user
                // pick a device in this dialog.
                c.config
                    .library_routes
                    .insert(library_lower.clone(), endpoint.to_string());
            }
        }
```

- [ ] **Step 6: Update the render function to grey out an unroutable device with its reason**

In `src/app/render/overlays/library_routes.rs`, find:

```rust
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
```

Replace with:

```rust
                // (label, routable) -- a device without a resolvable
                // endpoint (#256) is shown greyed out with its reason
                // appended, rather than omitted, so a device visible in
                // F3 but not currently pickable here isn't a silent
                // mystery. It stays visible via arrow-key navigation but
                // `commit_device_selection` refuses to commit it.
                let mut rows: Vec<(String, bool)> = vec![(LOCAL_NO_ROUTE.to_string(), true)];
                rows.extend(devices.iter().map(|(name, endpoint)| {
                    if endpoint.is_some() {
                        (name.clone(), true)
                    } else {
                        (format!("{name} (not currently routable)"), false)
                    }
                }));
                for (i, (label, routable)) in rows.iter().enumerate() {
                    let focused = i == popup.cursor;
                    let arrow = if focused { "▸ " } else { "  " };
                    let name_style = if !routable {
                        Style::default().fg(palette::MUTED)
                    } else if focused {
                        Style::default().fg(palette::TEXT)
                    } else {
                        Style::default().fg(palette::SUBTLE)
                    };
                    lines.push(Line::from(vec![
                        Span::raw(arrow),
                        Span::styled(label.clone(), name_style),
                    ]));
                }
```

- [ ] **Step 7: Run the tests to confirm they pass**

Run: `cargo test -p mbv --lib render::overlays::library_routes::tests -- --nocapture`
Expected: all `PASS`.

- [ ] **Step 8: Run the full app test suite and clippy**

Run: `cargo test -p mbv --lib && cargo clippy -p mbv --all-targets`
Expected: all tests pass, zero clippy warnings.

- [ ] **Step 9: Commit**

```bash
git add src/app/mod.rs src/app/render/overlays/library_routes.rs
git commit -m "app: F2 library-route picker stores the resolved endpoint, preselects by endpoint (#256)"
```

---

### Task 4: Docs + final full-workspace verification

**Files:**
- Modify: `docs/adr/0011-library-scoped-daemon-routing.md` (new addendum)

**Interfaces:**
- Consumes: nothing code-level; this task is documentation + a whole-workspace check that the prior two tasks compose correctly.
- Produces: nothing consumed by later tasks (this is the last task in the plan).

- [ ] **Step 1: Add the ADR addendum**

In `docs/adr/0011-library-scoped-daemon-routing.md`, after the existing `## Addendum (#239): config renamed to \`[library_routes]\`, values are device names` section, append:

```markdown

## Addendum (#256): `[library_routes]` values become resolved endpoints, no rediscovery

#239's device-name resolution paid a blocking `GET /Sessions` call on every
routed play/enqueue attempt -- extra synchronous work #223's original
raw-endpoint config didn't have, and slower than the F3 Sessions-panel path,
which starts from an already-discovered `SessionInfo`. #256 replaces the
stored device name with the endpoint it resolves to: `[library_routes]`
values become `tcp://host:port` strings again (parsed via the same
`DaemonEndpoint::parse`/`Display` #223's original config used), and
`resolve_route_for_library` becomes a pure, synchronous config read with no
network call on the play/enqueue path at all.

This is a deliberate, explicit trade-off, not an oversight:

- **No device name is persisted anywhere.** The F2 "Library Routes" picker
  still fetches the live session list to let the user *choose* a device (an
  unavoidable, already-paid cost of that one screen), but the name is used
  only for that screen's display and for preselecting the currently-assigned
  row -- by comparing each candidate's *resolved endpoint* against the
  stored endpoint, not by name, since an endpoint is a more stable
  identifier than a hostname. Nothing from that comparison is written back
  to config.
- **No automatic rediscovery/self-heal.** If a routed library's cached
  endpoint stops working (the target device's address changed), that's an
  ordinary daemon-connect failure: #222's existing fallback applies (fall
  back to local playback, log a warning, never a hard error) exactly like
  any other failed connect. There is no special "try re-resolving by name"
  path. This is a LAN, single-user tool -- a device's address is expected
  to be stable; if it isn't, the fix is reassigning the route via F2 (which
  re-resolves live, same as first assignment), not an automatic background
  mechanism.
- **Library routing stays tcp://-only / remote-only**, per the #239
  addendum above -- `resolve_library_route` requires the parsed value to be
  `DaemonEndpoint::Tcp(_)`; a `unix://`/`local` value or a stale pre-#256
  device-name string (which `DaemonEndpoint::parse` would otherwise accept
  as a bogus `Unix(PathBuf)`) is treated as malformed and logged, never
  routed.
- **Breaking change, no migration** -- consistent with #239's own
  precedent: a pre-#256 device-name config entry simply stops resolving
  (logged as malformed) until the user reassigns it via F2. Same
  single-user-repo, no-config-compat-guarantee reasoning as #239.
- **F2 picker: a live device without a resolvable endpoint is shown, not
  hidden.** Storing an endpoint requires resolving one at pick-time (there
  is nothing meaningful to write otherwise), so a live "mbv" session that
  `session_direct_endpoint` can't resolve (no advertised direct-connect
  port, or an unparseable host) can no longer be *committed* the way it
  could pre-#256 (which stored the name and deferred resolution to connect
  time). Rather than silently dropping such a device from the picker list
  -- which would leave a device visible in F3's Sessions panel mysteriously
  absent from F2 with no explanation -- it's shown as a greyed-out row
  suffixed `(not currently routable)`; selecting it flashes that reason
  and commits nothing.

See `docs/superpowers/plans/2026-07-18-library-route-endpoint-cache.md` for
the implementation plan. Note this supersedes §4 ("Connect-time resolution
+ error handling") of
`docs/superpowers/specs/2026-07-18-library-routes-by-device-name-design.md`
-- that section's "one extra 'list sessions' API call" framing described
the #239 behavior this addendum replaces.
```

- [ ] **Step 2: Full workspace build, lint, and test**

Run: `cargo build --workspace`
Expected: succeeds with zero errors.

Run: `cargo clippy --workspace --all-targets`
Expected: zero warnings.

Run: `cargo test --workspace`
Expected: all tests pass.

- [ ] **Step 3: Commit**

```bash
git add docs/adr/0011-library-scoped-daemon-routing.md
git commit -m "docs: add #256 addendum to ADR 0011 for endpoint-cached library routes"
```

---

## Self-Review

**Spec coverage** (against the final, user-confirmed scope — not #256's original text, which the "no fallback" decision deliberately narrows):
- `[library_routes]` stores a resolved endpoint, not a device name -> Task 1 (parsing/validation + resolution), Task 3 (where it's written).
- Route playback tries the stored endpoint with no `/Sessions` call -> Task 1.
- No device name persisted anywhere; F2 still shows friendly names, sourced live, never stored -> Task 3.
- Malformed/legacy entries logged and skipped, not routed -> Task 1.
- Logs distinguish "used the stored endpoint" from "malformed, skipped" -> Task 1's `resolve_library_route` log lines (the "discovery fallback attempted/succeeded" log criterion from #256's original text is explicitly dropped, per the ADR addendum in Task 4).
- Existing `apply_route_for_playback`/`switch_to_library_route`/`try_auto_reconnect` behavior unaffected -> confirmed unchanged in Task 1 (they only consume `resolve_route_for_library`'s output type, which is preserved).
- Workspace never sits in a broken intermediate state at a commit boundary -> Task 1 lands as one atomic commit spanning `mbv-core` and the `mbv` binary specifically because of this (see Task 1's header note); Task 3 and Task 4 are each single-crate/docs-only changes with no equivalent cross-crate coupling.
- A live device visible in F3 but unresolvable to an endpoint must not silently vanish from the F2 picker -> Task 3 Steps 1/3/4/5/6 (shown greyed out with a reason, not committable), documented in Task 4's ADR addendum.
- Pre-existing, unrelated clippy debt surfaced during Task 1's review -> Task 2 (inserted by explicit user decision, not part of #256's own design; see Task 2's header note).

**Placeholder scan:** no TBD/TODO, every step has complete before/after code and exact commands.

**Type consistency:** `resolve_library_route` returns `Option<DaemonEndpoint>` consistently from Task 1 through Task 3; `App::resolve_route_for_library` keeps its `Option<(String, DaemonEndpoint)>` signature throughout; `LibraryRouteStage::PickDevice.devices` is `Vec<(String, Option<DaemonEndpoint>)>` consistently in Task 3's Steps 1/3/4/5/6 (the two new Step 1 tests, the field declaration, `enter_device_stage`, `commit_device_selection`, and the render function all agree on this shape and on `None` meaning "not currently routable").
