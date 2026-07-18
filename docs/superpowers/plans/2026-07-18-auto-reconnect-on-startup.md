# Auto-Reconnect on Startup Implementation Plan (#236)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the behavior issue #222 was supposed to deliver but didn't: when `auto_reconnect = true`, mbv reconnects at startup to whatever remote connection (a #223 library route, or a Sessions-panel direct-remote/attached session) was active when it last exited; a failed or impossible reconnect falls back to local playback, never a hard failure.

**Architecture:** Two independent, symmetric mechanisms share one on/off switch and one small persisted-state file, but stay separate concepts in code exactly as `active_route` and `connected_session_id`/`connected_session_state` already are (#222/#223 were separate features; this plan does not merge them). `App::teardown` (the single shared quit-path tail for both the in-app quit key and the SIGHUP/SIGTERM watchdog) persists a tagged `LastRemoteConnection` record — `LibraryRoute { library }` or `DirectSession { device_name }` — describing whichever of the two mutually-exclusive states was active, or removes the file if neither was. `App::new` (the plain local-launch constructor; `App::new_remote`'s explicit `--connect-daemon`/`daemon_client_endpoint` path is untouched per ADR 0010) reads that file back and, if present, resolves and connects using the *same* primitives #222/#223 already built: `resolve_route_for_library` + `try_daemon_route_connect` + `switch_to_library_route` for a library route, or a synchronous `get_sessions()` lookup by device name + the existing `connect_to_session` for a direct/attached session. One shot, no retry, matching ADR 0010 rules 2-3.

**Tech Stack:** Rust (2021 edition), `cargo test` workspace (`mbv` binary crate at repo root, `mbv-core` lib crate), `serde`/`serde_json` for the new persisted-state file, std `mpsc`/`Mutex` test-override statics (existing pattern, e.g. `DAEMON_ROUTE_CONNECT_OVERRIDE`) for the new HTTP test seam.

## Global Constraints

- `App::new_remote` (the `--connect-daemon`/`daemon_client_endpoint` startup path) is untouched — ADR 0010 states that path is unaffected, and this plan does not change that.
- No background retry: a failed or "not found" reconnect attempt at startup falls back to (stays on) local playback and never schedules anything further, exactly like #222's per-play lazy-connect fallback rule.
- `auto_reconnect` defaults to `false`. When `false`, neither the write path (`App::teardown`) nor the read path (`App::new`) touches the persisted-state file at all — zero footprint for users who don't opt in.
- The config key lives in `[general]` (a client-side setting), never under `[daemon.client]`/`[daemon.server]` — this was the whole point of catching #222/#223's framing problem: routing/reconnect preferences belong to the client, not to daemon configuration.
- Never call the daemon/`remote_player.rs` mechanism a "remote session" in code comments, docs, or log messages — that term is reserved for the Sessions-panel (`connected_session_id`) feature (`mem:feedback_remote_session_terminology`).
- Never say "Jellyfin" — Emby only.

---

### Task 1: `auto_reconnect` config schema + docs

**Files:**
- Modify: `crates/mbv-core/src/config.rs` — `Config` struct (add field after `daemon_server_tcp_listen`, line 58), `impl Default for Config` (line ~104, before closing brace), `parse_config` (add parsing near `stay_alive` at line 641, and add to the `Config { ... }` return struct literal near line ~806)
- Modify: `README.md` — add a new `[general]` stanza to the "File-only options" config block (currently ends with `[daemon_routes]`, which this plan does not touch)
- Test: `crates/mbv-core/src/config.rs` (`mod tests`)

**Interfaces:**
- Produces: `Config.auto_reconnect: bool`, consumed by Task 3 (`App::teardown`) and Task 4 (`App::new`) via `self.client.lock().unwrap().config.auto_reconnect`.

- [ ] **Step 1: Write the failing tests**

Add to `crates/mbv-core/src/config.rs`'s `mod tests` block (near the existing `parse_daemon_routes_lowercased_keys` test):

```rust
    #[test]
    fn parse_auto_reconnect_true() {
        let toml = r#"
[server]
url = "http://x"

[general]
auto_reconnect = true
"#;
        let cfg = parse_config(toml).unwrap();
        assert!(cfg.auto_reconnect);
    }

    #[test]
    fn parse_auto_reconnect_defaults_false_when_absent() {
        let toml = r#"
[server]
url = "http://x"
"#;
        let cfg = parse_config(toml).unwrap();
        assert!(!cfg.auto_reconnect);
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p mbv-core parse_auto_reconnect --no-fail-fast`
Expected: FAIL — `no field \`auto_reconnect\` on type \`Config\`` (compile error).

- [ ] **Step 3: Add the field, default, and parsing**

In `crates/mbv-core/src/config.rs`, add to the `Config` struct (right after `pub daemon_server_tcp_listen: String, // ...` at line 58):

```rust
    /// Reconnect at startup to whatever remote connection (a #223 library
    /// route, or a Sessions-panel direct-remote/attached session) was
    /// active when mbv last exited (issue #236 -- #222's original
    /// "auto-reconnect" intent, which #222's own lazy-connect-only design
    /// never actually implemented). Client-side setting: deliberately
    /// under `[general]`, not `[daemon.client]`/`[daemon.server]`, since
    /// this is a routing/reconnect *preference*, not daemon configuration.
    /// Default off; TOML-only for v1, no settings-panel write-back
    /// (matching the `daemon_routes` precedent).
    pub auto_reconnect: bool,
```

In `impl Default for Config` (near line 104, alongside `daemon_server_tcp_listen: String::new(),`):

```rust
            auto_reconnect: false,
```

In `parse_config`, near the `stay_alive` parsing (line 641):

```rust
    let auto_reconnect = general
        .and_then(|m| m.get("auto_reconnect"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
```

And add `auto_reconnect,` to the `Config { ... }` struct literal `parse_config` returns (near `daemon_server_tcp_listen,` at line ~806).

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p mbv-core parse_auto_reconnect --no-fail-fast`
Expected: PASS (2 tests).

- [ ] **Step 5: Update README**

In `README.md`, in the "### File-only options (`~/.config/mbv/config.toml`)" fenced block, add a new `[general]` stanza. Insert it as the first stanza in the block (before `[server]`), since it's the most general-purpose one:

```toml
[general]
# Reconnect at startup to whatever remote connection (a routed library, or
# a Sessions-panel direct-remote/attached session) was active when mbv
# last exited. Off by default. A failed or impossible reconnect (e.g. the
# other device is offline) falls back to local playback instead of
# blocking startup or erroring.
auto_reconnect = false

[server]
# Override the server URL. Rarely needed — the login screen sets and persists
# this after your first successful login.
url = "http://emby.local:8096"
```

(Leave the rest of the existing fenced block — `[mpv]` through `[daemon_routes]` — unchanged; only the new `[general]` stanza and the `[server]` header immediately after it are touched.)

- [ ] **Step 6: Commit**

```bash
git add crates/mbv-core/src/config.rs README.md
git commit -m "config: add auto_reconnect (#236)"
```

---

### Task 2: `LastRemoteConnection` persistence primitives

**Files:**
- Modify: `crates/mbv-core/src/config.rs` — add near the existing `QueueState`/`save_queue_state`/`load_queue_state` (lines 291-359), which this mirrors
- Test: `crates/mbv-core/src/config.rs` (`mod tests`)

**Interfaces:**
- Produces: `pub enum LastRemoteConnection { LibraryRoute { library: String }, DirectSession { device_name: String } }` (`Debug, Clone, PartialEq, Eq, Serialize, Deserialize`), `pub fn save_last_remote_connection(conn: Option<&LastRemoteConnection>)`, `pub fn load_last_remote_connection() -> Option<LastRemoteConnection>`. Consumed by Task 3 (`save_last_remote_connection`) and Task 4 (`load_last_remote_connection`, matching on the two variants).

- [ ] **Step 1: Write the failing tests**

Add to `crates/mbv-core/src/config.rs`'s `mod tests` block:

```rust
    #[test]
    fn save_and_load_last_remote_connection_round_trips_library_route() {
        let _guard = TestStateDirGuard::new();
        let conn = LastRemoteConnection::LibraryRoute {
            library: "music".to_string(),
        };

        save_last_remote_connection(Some(&conn));

        assert_eq!(load_last_remote_connection(), Some(conn));
    }

    #[test]
    fn save_and_load_last_remote_connection_round_trips_direct_session() {
        let _guard = TestStateDirGuard::new();
        let conn = LastRemoteConnection::DirectSession {
            device_name: "living-room-mbv".to_string(),
        };

        save_last_remote_connection(Some(&conn));

        assert_eq!(load_last_remote_connection(), Some(conn));
    }

    #[test]
    fn save_last_remote_connection_none_clears_a_previously_saved_record() {
        let _guard = TestStateDirGuard::new();
        save_last_remote_connection(Some(&LastRemoteConnection::LibraryRoute {
            library: "music".to_string(),
        }));

        save_last_remote_connection(None);

        assert_eq!(load_last_remote_connection(), None);
    }

    #[test]
    fn load_last_remote_connection_returns_none_when_no_file_exists() {
        let _guard = TestStateDirGuard::new();
        assert_eq!(load_last_remote_connection(), None);
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p mbv-core last_remote_connection --no-fail-fast`
Expected: FAIL — `cannot find type \`LastRemoteConnection\`` (compile error).

- [ ] **Step 3: Implement**

Add to `crates/mbv-core/src/config.rs`, directly after `clear_queue_state` (end of the `QueueState` block, before `save_library_position_state`):

```rust
/// Which remote connection (if any) was active when mbv last exited
/// (issue #236). `App::teardown` writes this; `App::new` reads it back at
/// the next launch when `Config.auto_reconnect` is true. The two
/// variants mirror `App`'s own separate `active_route` (#223 library
/// routing) and `connected_session_id`/`connected_session_state`
/// (Sessions-panel direct-remote/attached) fields -- #222 and #223 were
/// distinct features and stay distinct here, even though both are
/// restored under the same on/off switch.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind")]
pub enum LastRemoteConnection {
    /// A #223 library route, keyed by the library name that was resolved
    /// active (`App.active_route`). Re-resolved fresh against current
    /// `daemon_routes` at startup, not replayed verbatim -- if the config
    /// changed since the last exit, the new config wins.
    LibraryRoute { library: String },
    /// A Sessions-panel direct-remote or attached session, keyed by the
    /// other device's name (`SessionInfo.device_name`), not its session id
    /// -- Emby session ids are ephemeral per-connection and would not
    /// still identify the same device at the next launch.
    DirectSession { device_name: String },
}

fn last_remote_connection_path() -> PathBuf {
    state_dir().join("last_remote_connection.json")
}

/// Persists (or, given `None`, clears) the connection active at exit.
/// Called from `App::teardown` only when `auto_reconnect` is
/// enabled -- when the feature is off, this file is never written or
/// read, by design (Task 1's `Global Constraints`).
pub fn save_last_remote_connection(conn: Option<&LastRemoteConnection>) {
    let path = last_remote_connection_path();
    let Some(conn) = conn else {
        let _ = std::fs::remove_file(&path);
        return;
    };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(json) = serde_json::to_string(conn) {
        let tmp = path.with_extension("json.tmp");
        if std::fs::write(&tmp, &json).is_ok() {
            let _ = std::fs::rename(&tmp, &path);
        }
    }
}

pub fn load_last_remote_connection() -> Option<LastRemoteConnection> {
    let text = std::fs::read_to_string(last_remote_connection_path()).ok()?;
    match serde_json::from_str(&text) {
        Ok(conn) => Some(conn),
        Err(e) => {
            log::warn!(target: "auto_reconnect", "last_remote_connection.json failed to parse, not reconnecting: {e}");
            None
        }
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p mbv-core last_remote_connection --no-fail-fast`
Expected: PASS (4 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/mbv-core/src/config.rs
git commit -m "core: add LastRemoteConnection persistence for auto-reconnect (#236)"
```

---

### Task 3: Persist the active connection at quit

**Files:**
- Modify: `src/app/mod.rs` — `App::teardown` (lines 3412-3484 currently; this task edits only its opening lines, before the existing `let quit_requested = ...` line)
- Test: `src/app/mod.rs` (`mod tests`, near `teardown_fast_when_player_thread_is_not_hung`)

**Interfaces:**
- Consumes: `mbv_core::config::{LastRemoteConnection, save_last_remote_connection}` (Task 2), `App.active_route: Option<String>` (existing), `App.connected_session_state: Option<mbv_core::api::SessionInfo>` (existing, has `.device_name`), `App.client` (existing `Arc<Mutex<EmbyClient>>`, `.config.auto_reconnect`).
- Produces: no new public interface — this task only adds a side effect at the top of the existing private `teardown` method.

- [ ] **Step 1: Write the failing tests**

Add to `src/app/mod.rs`'s `mod tests` block, directly after `teardown_fast_when_player_thread_is_not_hung`:

```rust
    #[test]
    fn teardown_persists_active_library_route_when_auto_reconnect_enabled() {
        let _guard = crate::config::TestStateDirGuard::new();
        let mut app = make_app_stub();
        app.client.lock().unwrap().config.auto_reconnect = true;
        app.active_route = Some("music".to_string());

        app.teardown(Duration::from_secs(1));

        assert_eq!(
            crate::config::load_last_remote_connection(),
            Some(crate::config::LastRemoteConnection::LibraryRoute {
                library: "music".to_string()
            })
        );
    }

    #[test]
    fn teardown_persists_connected_session_when_auto_reconnect_enabled() {
        let _guard = crate::config::TestStateDirGuard::new();
        let mut app = make_app_stub();
        app.client.lock().unwrap().config.auto_reconnect = true;
        let sess = make_session("living-room-mbv", "mbv");
        app.connected_session_id = Some(sess.id.clone());
        app.connected_session_state = Some(sess);

        app.teardown(Duration::from_secs(1));

        assert_eq!(
            crate::config::load_last_remote_connection(),
            Some(crate::config::LastRemoteConnection::DirectSession {
                device_name: "living-room-mbv".to_string()
            })
        );
    }

    #[test]
    fn teardown_clears_persisted_connection_when_exiting_local() {
        let _guard = crate::config::TestStateDirGuard::new();
        crate::config::save_last_remote_connection(Some(
            &crate::config::LastRemoteConnection::LibraryRoute {
                library: "music".to_string(),
            },
        ));
        let mut app = make_app_stub();
        app.client.lock().unwrap().config.auto_reconnect = true;

        app.teardown(Duration::from_secs(1));

        assert_eq!(crate::config::load_last_remote_connection(), None);
    }

    #[test]
    fn teardown_never_touches_persisted_state_when_auto_reconnect_disabled() {
        let _guard = crate::config::TestStateDirGuard::new();
        crate::config::save_last_remote_connection(Some(
            &crate::config::LastRemoteConnection::LibraryRoute {
                library: "music".to_string(),
            },
        ));
        let mut app = make_app_stub();
        assert!(!app.client.lock().unwrap().config.auto_reconnect);
        app.active_route = None;

        app.teardown(Duration::from_secs(1));

        // Feature is off: the file from before this test's own `app` even
        // existed must be left exactly as it was, not cleared just because
        // `active_route` is currently `None`.
        assert_eq!(
            crate::config::load_last_remote_connection(),
            Some(crate::config::LastRemoteConnection::LibraryRoute {
                library: "music".to_string()
            })
        );
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test teardown_persists_active_library_route teardown_persists_connected_session teardown_clears_persisted_connection teardown_never_touches_persisted_state --no-fail-fast`
Expected: FAIL — assertion failures (persisted state is `None` / unchanged, since `teardown` doesn't write anything yet).

- [ ] **Step 3: Implement**

In `src/app/mod.rs`, at the top of `fn teardown(&mut self, quit_timeout: Duration) {` (line 3412), before the existing `let quit_requested = QUIT_REQUESTED.load(Ordering::Relaxed);` line, insert:

```rust
        // #236: persist whichever remote connection (if any) is active
        // right now, before anything below or in the caller's cleanup
        // path clears `active_route`/`connected_session_state` -- so the
        // next launch's `App::new` can restore it. Mutually exclusive by
        // construction (library routing and Sessions-panel direct-remote
        // are two independent ways to end up thin-client; #223's
        // `restore_local_mode` and `connect_to_session` never let both be
        // set at once). Gated on `auto_reconnect` so the file is
        // never written (or read) at all when the feature is off.
        if self.client.lock().unwrap().config.auto_reconnect {
            let last = if let Some(library) = self.active_route.clone() {
                Some(mbv_core::config::LastRemoteConnection::LibraryRoute { library })
            } else {
                self.connected_session_state.as_ref().map(|sess| {
                    mbv_core::config::LastRemoteConnection::DirectSession {
                        device_name: sess.device_name.clone(),
                    }
                })
            };
            mbv_core::config::save_last_remote_connection(last.as_ref());
        }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test teardown_persists_active_library_route teardown_persists_connected_session teardown_clears_persisted_connection teardown_never_touches_persisted_state --no-fail-fast`
Expected: PASS (4 tests).

- [ ] **Step 5: Commit**

```bash
git add src/app/mod.rs
git commit -m "app: persist active remote connection on quit for auto-reconnect (#236)"
```

---

### Task 4: Attempt reconnect at startup

**Files:**
- Modify: `src/app/mod.rs` — add a new test-override seam next to `DIRECT_CONNECT_OVERRIDE`/`DAEMON_ROUTE_CONNECT_OVERRIDE` (lines 26-53), a new private `fetch_sessions_blocking` method and a new private `try_auto_reconnect` method (near `try_daemon_route_connect`/`connect_daemon_route_endpoint`, lines ~2380-2480), and one call site in `App::new` (line ~1917-1918)
- Test: `src/app/mod.rs` (`mod tests`)

**Interfaces:**
- Consumes: `mbv_core::config::load_last_remote_connection` (Task 2), `App::resolve_route_for_library` (existing, `src/app/library_route.rs`), `App::try_daemon_route_connect` / `App::switch_to_library_route` (existing), `App::connect_to_session` (existing), `EmbyClient::get_sessions` (existing, `crates/mbv-core/src/api.rs`).
- Produces: `App::try_auto_reconnect(&mut self)` (private, called once from `App::new`), `App::fetch_sessions_blocking(&self) -> Result<Vec<mbv_core::api::SessionInfo>, String>` (private, with a `#[cfg(test)]` override seam so tests never make a real HTTP call).

- [ ] **Step 1: Write the failing tests**

Add to `src/app/mod.rs`'s `mod tests` block, near the `try_daemon_route_connect_*` tests:

```rust
    #[test]
    fn try_auto_reconnect_restores_a_persisted_library_route() {
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

        crate::config::save_last_remote_connection(Some(
            &crate::config::LastRemoteConnection::LibraryRoute {
                library: "music".to_string(),
            },
        ));
        let mut app = make_app_stub();
        app.client.lock().unwrap().config.auto_reconnect = true;
        app.daemon_routes
            .insert("music".to_string(), "tcp://127.0.0.1:9000".to_string());

        app.try_auto_reconnect();

        *DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = None;
        assert_eq!(app.active_route.as_deref(), Some("music"));
        assert!(app.player.is_remote());
    }

    #[test]
    fn try_auto_reconnect_falls_back_to_local_when_route_no_longer_configured() {
        let _guard = crate::config::TestStateDirGuard::new();
        crate::config::save_last_remote_connection(Some(
            &crate::config::LastRemoteConnection::LibraryRoute {
                library: "music".to_string(),
            },
        ));
        let mut app = make_app_stub();
        app.client.lock().unwrap().config.auto_reconnect = true;
        // No `daemon_routes` entry for "music" this time -- config changed
        // since the last exit.

        app.try_auto_reconnect();

        assert!(app.active_route.is_none());
        assert!(!app.player.is_remote());
    }

    #[test]
    fn try_auto_reconnect_restores_a_persisted_direct_session() {
        let _guard = crate::config::TestStateDirGuard::new();
        let _sessions_guard = SESSIONS_LOAD_TEST_LOCK.lock().unwrap();
        fn sessions_with_living_room(
            _client: &mbv_core::api::EmbyClient,
        ) -> Result<Vec<mbv_core::api::SessionInfo>, String> {
            Ok(vec![make_session("living-room-mbv", "mbv")])
        }
        *SESSIONS_LOAD_OVERRIDE.lock().unwrap() = Some(sessions_with_living_room);

        crate::config::save_last_remote_connection(Some(
            &crate::config::LastRemoteConnection::DirectSession {
                device_name: "living-room-mbv".to_string(),
            },
        ));
        let mut app = make_app_stub();
        app.client.lock().unwrap().config.auto_reconnect = true;

        app.try_auto_reconnect();

        *SESSIONS_LOAD_OVERRIDE.lock().unwrap() = None;
        assert_eq!(app.connected_session_id.as_deref(), Some("sess-1"));
    }

    #[test]
    fn try_auto_reconnect_falls_back_to_local_when_device_not_found() {
        let _guard = crate::config::TestStateDirGuard::new();
        let _sessions_guard = SESSIONS_LOAD_TEST_LOCK.lock().unwrap();
        fn sessions_without_living_room(
            _client: &mbv_core::api::EmbyClient,
        ) -> Result<Vec<mbv_core::api::SessionInfo>, String> {
            Ok(vec![])
        }
        *SESSIONS_LOAD_OVERRIDE.lock().unwrap() = Some(sessions_without_living_room);

        crate::config::save_last_remote_connection(Some(
            &crate::config::LastRemoteConnection::DirectSession {
                device_name: "living-room-mbv".to_string(),
            },
        ));
        let mut app = make_app_stub();
        app.client.lock().unwrap().config.auto_reconnect = true;

        app.try_auto_reconnect();

        *SESSIONS_LOAD_OVERRIDE.lock().unwrap() = None;
        assert!(app.connected_session_id.is_none());
        assert!(!app.player.is_remote());
    }

    #[test]
    fn try_auto_reconnect_is_a_no_op_when_disabled() {
        let _guard = crate::config::TestStateDirGuard::new();
        crate::config::save_last_remote_connection(Some(
            &crate::config::LastRemoteConnection::LibraryRoute {
                library: "music".to_string(),
            },
        ));
        let mut app = make_app_stub();
        assert!(!app.client.lock().unwrap().config.auto_reconnect);
        app.daemon_routes
            .insert("music".to_string(), "tcp://127.0.0.1:9000".to_string());

        app.try_auto_reconnect();

        assert!(app.active_route.is_none());
        assert!(!app.player.is_remote());
    }

    #[test]
    fn try_auto_reconnect_is_a_no_op_when_nothing_was_persisted() {
        let _guard = crate::config::TestStateDirGuard::new();
        let mut app = make_app_stub();
        app.client.lock().unwrap().config.auto_reconnect = true;

        app.try_auto_reconnect();

        assert!(app.active_route.is_none());
        assert!(!app.player.is_remote());
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test try_auto_reconnect --no-fail-fast`
Expected: FAIL — `cannot find function \`try_auto_reconnect\`` / `cannot find value \`SESSIONS_LOAD_OVERRIDE\`` (compile errors).

- [ ] **Step 3: Add the sessions-fetch test seam**

In `src/app/mod.rs`, near the existing `DAEMON_ROUTE_CONNECT_OVERRIDE`/`DAEMON_ROUTE_CONNECT_TEST_LOCK` statics (lines 51-53), add:

```rust
#[cfg(test)]
type SessionsLoadFn = fn(&mbv_core::api::EmbyClient) -> Result<Vec<mbv_core::api::SessionInfo>, String>;
#[cfg(test)]
static SESSIONS_LOAD_OVERRIDE: Mutex<Option<SessionsLoadFn>> = Mutex::new(None);
#[cfg(test)]
static SESSIONS_LOAD_TEST_LOCK: Mutex<()> = Mutex::new(());
```

- [ ] **Step 4: Implement `fetch_sessions_blocking` and `try_auto_reconnect`**

In `src/app/mod.rs`, add these two methods to `impl App` directly after `try_daemon_route_connect` (after line 2480, before `fn switch_to_direct_remote`):

```rust
    /// Blocking `GET /Sessions`, factored out only so tests can override it
    /// (mirrors `connect_daemon_route_endpoint`'s `#[cfg(test)]` seam) --
    /// `try_auto_reconnect`'s `DirectSession` case is the one caller.
    fn fetch_sessions_blocking(&self) -> Result<Vec<mbv_core::api::SessionInfo>, String> {
        #[cfg(test)]
        if let Some(f) = *SESSIONS_LOAD_OVERRIDE.lock().unwrap() {
            return f(&self.client.lock().unwrap());
        }
        self.client.lock().unwrap().get_sessions()
    }

    /// Restores the remote connection active when mbv last exited (issue
    /// #236 -- #222's original "auto-reconnect" intent). Called once from
    /// `App::new` (never `App::new_remote`, whose `--connect-daemon`
    /// startup path is a separate, unaffected mechanism per ADR 0010). A
    /// no-op unless `auto_reconnect` is enabled and
    /// `load_last_remote_connection` has a record. One shot, no retry: a
    /// failed connect, a route no longer present in `daemon_routes`, or a
    /// device not found in the current session list all fall back to (stay
    /// on) local playback, exactly like #222's per-play lazy-connect
    /// fallback rule -- never a hard failure at startup.
    fn try_auto_reconnect(&mut self) {
        if !self.client.lock().unwrap().config.auto_reconnect {
            return;
        }
        let Some(last) = mbv_core::config::load_last_remote_connection() else {
            return;
        };
        match last {
            mbv_core::config::LastRemoteConnection::LibraryRoute { library } => {
                let Some((name, endpoint)) = self.resolve_route_for_library(&library) else {
                    log::info!(
                        target: "auto_reconnect",
                        "persisted library route {library:?} no longer resolves; staying local"
                    );
                    return;
                };
                match self.try_daemon_route_connect(&endpoint, &name) {
                    Ok((remote, remote_rx)) => {
                        self.switch_to_library_route(&name, remote, remote_rx)
                    }
                    Err(message) => self.flash_status_high(message),
                }
            }
            mbv_core::config::LastRemoteConnection::DirectSession { device_name } => {
                let sessions = match self.fetch_sessions_blocking() {
                    Ok(sessions) => sessions,
                    Err(e) => {
                        log::warn!(target: "auto_reconnect", "failed to list sessions: {e}");
                        self.flash_status_high(format!(
                            "\u{26a0} Auto-reconnect couldn't list sessions ({e}), using local playback"
                        ));
                        return;
                    }
                };
                match sessions
                    .into_iter()
                    .find(|s| s.device_name.eq_ignore_ascii_case(&device_name))
                {
                    Some(sess) => self.connect_to_session(&sess),
                    None => {
                        log::info!(
                            target: "auto_reconnect",
                            "device {device_name:?} not found in current sessions; staying local"
                        );
                        self.flash_status_high(format!(
                            "\u{26a0} {device_name} not found, using local playback"
                        ));
                    }
                }
            }
        }
    }
```

- [ ] **Step 5: Wire the call site into `App::new`**

In `src/app/mod.rs`, in `impl App::new` (line ~1917), change:

```rust
        app.mpris = Some(mpris_handle);
        app
    }
```

to:

```rust
        app.mpris = Some(mpris_handle);
        app.try_auto_reconnect();
        app
    }
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test try_auto_reconnect --no-fail-fast`
Expected: PASS (6 tests).

- [ ] **Step 7: Run the full workspace test suite**

Run: `cargo test --workspace`
Expected: all tests pass (no regressions in existing `library_route`, `switch_to_library_route`, `connect_to_session`, or `teardown` tests).

- [ ] **Step 8: Run clippy**

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: clean.

- [ ] **Step 9: Commit**

```bash
git add src/app/mod.rs
git commit -m "app: attempt startup reconnect to last remote connection (#236)"
```

---

### Task 5: Correct the docs that describe the old (wrong) lazy-only behavior

**Files:**
- Modify: `docs/adr/0010-lazy-daemon-route-connect-lifecycle.md`
- Modify: `CONTEXT.md` (line 145)

**Interfaces:** None — documentation only.

- [ ] **Step 1: Rewrite ADR 0010's rule 1 and its stale "no startup call site" framing**

In `docs/adr/0010-lazy-daemon-route-connect-lifecycle.md`, replace rule 1 under "## Decision":

Old:
```
1. **Lazy connect.** mbv never attempts a daemon-route connection at
   startup. The first play/enqueue action that resolves to a configured
   route is what triggers the first connect attempt.
```

New:
```
1. **Lazy connect, with an opt-in startup exception.** By default, mbv
   never attempts a daemon-route connection at startup — the first
   play/enqueue action that resolves to a configured route is what
   triggers the first connect attempt. When `auto_reconnect` is
   enabled (issue #236), mbv additionally makes one attempt at startup to
   restore whichever remote connection (library route or Sessions-panel
   direct-remote/attached session) was active when it last exited. This
   was #222's original intent — its initial design mistakenly ruled out
   any startup connection entirely, which #236 corrected. See
   `App::try_auto_reconnect` (`src/app/mod.rs`).
```

Then, near the end of the "## Context" section, replace the paragraph that begins "Implementation: `App::try_daemon_route_connect` / `App::connect_daemon_route_endpoint`..." — specifically its closing sentence about `#[allow(dead_code)]` and "No production call site exists yet" — is already stale (#223 added the call site; #236 added a second one in `App::new`). Append a short correction paragraph after the existing "## Consequences" section:

```
## Correction (#236)

This ADR's original rule 1 stated flatly that mbv "never attempts a
daemon connection at startup, regardless of config" and that
`try_daemon_route_connect` "must not be invoked from `App::new`,
`App::new_remote`, or `App::build`" (from this ADR's originating plan,
`docs/superpowers/plans/2026-07-17-daemon-connect-lifecycle.md`). That
was a misreading of issue #222's own title ("auto-reconnect to remote
client") against its body: #222 was supposed to deliver reconnect-at-
startup, not rule it out. Issue #236 corrected this: `App::new` now calls
`App::try_auto_reconnect` once, gated on the new `auto_reconnect`
config flag (default off), restoring whichever connection was active at
last exit. `App::new_remote` (the separate, pre-existing
`--connect-daemon`/`daemon_client_endpoint` path) remains untouched, as
rule 1 always intended.
```

- [ ] **Step 2: Fix `CONTEXT.md` line 145**

In `CONTEXT.md`, the paragraph beginning "The connect-timing rule for the daemon-route lifecycle..." (line 145) currently ends: "mbv never attempts a daemon connection at startup for a route." Replace that sentence with:

```
By default, mbv never attempts a daemon connection at startup for a route; when `auto_reconnect` is enabled (#236), it additionally makes one startup attempt to restore whichever remote connection was active at last exit, via `App::try_auto_reconnect` (`src/app/mod.rs`) — see ADR 0010's "Correction (#236)" section.
```

- [ ] **Step 3: Commit**

```bash
git add docs/adr/0010-lazy-daemon-route-connect-lifecycle.md CONTEXT.md
git commit -m "docs: correct ADR 0010/CONTEXT.md's lazy-only framing now that #236 adds startup reconnect"
```

---

## Self-Review Notes

- **Spec coverage:** #236's acceptance criteria are covered: `auto_reconnect` config (Task 1), startup reconnect for both mechanisms with fallback (Task 4), default-off/no-touch-when-disabled (Tasks 1, 3, 4), no hard failure on a failed attempt (Task 4), ADR corrections (Task 5).
- **Scope boundary respected:** `App::new_remote`/`--connect-daemon` is never modified; #222 and #223's `active_route`/`connected_session_id` fields stay separate types in `LastRemoteConnection`, matching the user's direction that the two features stay distinct even while sharing the on/off switch and restore trigger.
- **Type consistency checked:** `LastRemoteConnection` (Task 2) is matched identically in Task 3 (write) and Task 4 (read); `fetch_sessions_blocking`'s return type matches `EmbyClient::get_sessions`'s existing `Result<Vec<SessionInfo>, String>`.
