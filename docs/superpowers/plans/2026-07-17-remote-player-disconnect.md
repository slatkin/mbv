# RemotePlayer Real Disconnect Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix issue #233 -- `RemotePlayer` route-to-route (and direct-remote-to-direct-remote) swaps leak a reader thread and a half-open socket, because the only teardown today is an implicit `Drop`, which closes one fd duplicate but leaves the reader thread blocked forever on its own separate duplicate of the same socket.

**Architecture:** Give `RemotePlayer` a real `disconnect()` method that calls `.shutdown(Shutdown::Both)` on a kept clone of the underlying control socket. Socket shutdown operates on the shared underlying kernel socket, not per-fd, so it unblocks the reader thread's blocking `read()` (inside `reader.lines()`) immediately regardless of which fd duplicate performs the call. Thread this through `PlayerProxy::disconnect_remote()` (a thin dispatch, no-op for `Local`), then call it from the two `App` call sites that swap one live remote connection directly for another without going through `restore_local_mode` first: `switch_to_direct_remote`'s and `switch_to_library_route`'s already-remote branches.

**Tech Stack:** Rust workspace (`crates/mbv-core` shared lib, `src/` `mbv` TUI binary), `cargo test`, plain `#[test]` functions, no mocking framework. Socket-level tests use a real `std::net::TcpListener` loopback daemon stand-in, mirroring the existing pattern in `crates/mbv-core/src/remote_player.rs`'s `connect_endpoint_propagates_active_remote_playback_status` test.

## Global Constraints

- This is a pre-existing bug (predates #223), not something to attribute to #223 -- but #223's library-route flips are what turns it from an occasional leak into a per-route-flip one, per issue #233's own framing. No behavior change to routing/connection *decisions* is in scope here, only teardown of the socket a `RemotePlayer` no longer needs.
- `RemotePlayer` is `#[derive(Clone)]` and calling code (`mpris_remote = remote.clone()`, MPRIS rebind closures) legitimately holds multiple clones alive concurrently. `disconnect()` must be safe to call on any clone and must not panic or double-shutdown if called more than once (idempotent).
- Do not change `RemotePlayer::join()` (documented no-op -- daemon keeps running when the TUI exits; that's a different, intentional design decision unrelated to this bug) and do not add retry/reconnect logic -- out of scope per the issue.
- Match this codebase's convention: no mocking framework: exercise real socket behavior via a loopback `TcpListener` thread standing in for a daemon (see `crates/mbv-core/src/remote_player.rs`'s existing tests for the pattern to copy).
- `cargo build --workspace` must stay warning-free; `cargo clippy --workspace --all-targets` must introduce no new warnings; `cargo fmt --check` must pass.

---

### Task 1: `ControlStream::shutdown()`

**Files:**
- Modify: `crates/mbv-core/src/remote_player.rs:150-157` (`impl ControlStream`, next to the existing `try_clone`)
- Test: `crates/mbv-core/src/remote_player.rs` `mod tests` (same file, near the other `ControlStream`/socket-level tests around line 859)

**Interfaces:**
- Produces: `ControlStream::shutdown(&self) -> std::io::Result<()>` -- Task 2's `RemotePlayer::disconnect()` calls this on its kept clone.

- [ ] **Step 1: Write the failing test**

Add to `crates/mbv-core/src/remote_player.rs`'s `mod tests` (near the top-level socket helpers, e.g. right before `connect_endpoint_propagates_active_remote_playback_status`):

```rust
    #[test]
    fn control_stream_shutdown_unblocks_a_concurrent_blocking_read() {
        // #233: shutdown() must affect the *shared underlying socket*, not
        // just the fd this particular ControlStream clone holds -- that's
        // the whole point of using shutdown() instead of Drop. Prove it by
        // shutting down one clone and confirming a DIFFERENT clone's
        // blocking read unblocks (returns Ok(0), i.e. EOF) as a result.
        use std::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let accept_thread = std::thread::spawn(move || listener.accept().unwrap().0);

        let client_stream = ControlStream::Tcp(TcpStream::connect(addr).unwrap());
        let _server_stream = accept_thread.join().unwrap();

        let reader_clone = client_stream.try_clone().unwrap();
        let read_thread = std::thread::spawn(move || {
            let mut reader_clone = reader_clone;
            let mut buf = [0u8; 8];
            reader_clone.read(&mut buf)
        });

        // Give the read thread a moment to actually block in read() before
        // we shut down the OTHER clone.
        std::thread::sleep(Duration::from_millis(50));
        client_stream.shutdown().unwrap();

        let result = read_thread
            .join()
            .expect("read thread must exit, not hang, once the socket is shut down");
        assert_eq!(
            result.unwrap(),
            0,
            "a shut-down socket must unblock a concurrent read with EOF (Ok(0))"
        );
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p mbv-core control_stream_shutdown_unblocks_a_concurrent_blocking_read`
Expected: FAIL with a compile error (`no method named 'shutdown' found for enum 'ControlStream'`).

- [ ] **Step 3: Implement `ControlStream::shutdown`**

In `crates/mbv-core/src/remote_player.rs`, in `impl ControlStream` (immediately after the existing `try_clone` method, around line 156):

```rust
impl ControlStream {
    fn try_clone(&self) -> io::Result<Self> {
        match self {
            Self::Unix(stream) => stream.try_clone().map(Self::Unix),
            Self::Tcp(stream) => stream.try_clone().map(Self::Tcp),
        }
    }

    /// Shuts down the underlying socket for both reads and writes (#233).
    /// Unlike dropping a `ControlStream` clone -- which only closes *that*
    /// clone's fd duplicate -- `shutdown` acts on the shared underlying
    /// socket in the kernel, so it unblocks a concurrent blocking `read()`
    /// on any other clone of the same connection immediately.
    fn shutdown(&self) -> io::Result<()> {
        match self {
            Self::Unix(stream) => stream.shutdown(std::net::Shutdown::Both),
            Self::Tcp(stream) => stream.shutdown(std::net::Shutdown::Both),
        }
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p mbv-core control_stream_shutdown_unblocks_a_concurrent_blocking_read`
Expected: PASS

- [ ] **Step 5: Run the full remote_player test module to check for regressions**

Run: `cargo test -p mbv-core remote_player::tests`
Expected: all existing tests still PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/mbv-core/src/remote_player.rs
git commit -m "remote_player: add ControlStream::shutdown for real socket teardown"
```

---

### Task 2: `RemotePlayer::disconnect()` actually shuts down the connection

**Files:**
- Modify: `crates/mbv-core/src/remote_player.rs:33-42` (`struct RemotePlayer`), `:329-448` (`connect_endpoint`), `:576-601` (`stub_with_command_rx`)
- Test: `crates/mbv-core/src/remote_player.rs` `mod tests`

**Interfaces:**
- Consumes: `ControlStream::shutdown` (Task 1).
- Produces: `RemotePlayer::disconnect(&self)` -- Task 3's `PlayerProxy::disconnect_remote()` calls this.

- [ ] **Step 1: Write the failing test**

Add to `crates/mbv-core/src/remote_player.rs`'s `mod tests`, near `connect_endpoint_propagates_active_remote_playback_status`:

```rust
    #[test]
    fn disconnect_causes_the_reader_thread_to_observe_the_shutdown_and_exit() {
        // #233: the only pre-existing teardown was an implicit Drop of the
        // writer thread's fd duplicate, which never affected the reader
        // thread's *separate* duplicate of the same socket -- so the
        // reader thread's blocking `read()` inside `reader.lines()` never
        // unblocked, leaking the thread forever. `disconnect()` must fix
        // this: after calling it, the reader thread must observe EOF/an
        // error on its own read and exit, which is exactly what flips
        // `is_disconnected()` to true (see the reader thread's loop-exit
        // code in `connect_endpoint`).
        use std::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        let daemon = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut writer = stream.try_clone().unwrap();
            let mut reader = BufReader::new(stream);

            let hello = serde_json::to_string(&CtrlEvent::Hello(CtrlHello::current())).unwrap();
            writeln!(writer, "{hello}").unwrap();
            let mut client_hello = String::new();
            reader.read_line(&mut client_hello).unwrap();

            let initial_state = serde_json::to_string(&CtrlEvent::State(CtrlState {
                status: PlayerStatus::default(),
                items: Vec::new(),
                cursor: 0,
                source: crate::config::QueueSource::Unknown,
            }))
            .unwrap();
            writeln!(writer, "{initial_state}").unwrap();

            // Keep the daemon-side handle open well past the point the
            // client calls disconnect(), so the test can distinguish "the
            // client's shutdown() itself caused the reader to exit" from
            // "the daemon happened to hang up around the same time."
            std::thread::sleep(Duration::from_secs(2));
        });

        let (remote, _event_rx) =
            RemotePlayer::connect_endpoint(&DaemonEndpoint::Tcp(addr), "token").unwrap();
        assert!(!remote.is_disconnected());

        remote.disconnect();

        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while !remote.is_disconnected() && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(
            remote.is_disconnected(),
            "reader thread must observe the shutdown and exit, flipping is_disconnected()"
        );

        drop(daemon); // let the daemon thread's sleep finish in the background
    }

    #[test]
    fn disconnect_is_idempotent() {
        // A second call must not panic (Task 2's Option::take() makes the
        // stored stream handle single-use).
        let (remote, _event_rx) = RemotePlayer::stub(Vec::new(), 0);
        remote.disconnect();
        remote.disconnect();
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p mbv-core disconnect_causes_the_reader_thread_to_observe_the_shutdown_and_exit disconnect_is_idempotent`
Expected: FAIL with a compile error (`no method named 'disconnect' found for struct 'RemotePlayer'`).

- [ ] **Step 3: Add the `control_stream` field and thread it through construction**

In `crates/mbv-core/src/remote_player.rs`, update the struct (around line 33):

```rust
#[derive(Clone)]
pub struct RemotePlayer {
    pub status: Arc<Mutex<PlayerStatus>>,
    pub subtitle_prefs: Arc<Mutex<crate::player::SubtitlePrefs>>,
    pub items: Arc<Mutex<Vec<MediaItem>>>,
    pub queue_source: Arc<Mutex<crate::config::QueueSource>>,
    cmd_tx: mpsc::Sender<CtrlCmd>,
    disconnected: Arc<AtomicBool>,
    ctrl_compatibility: CtrlCompatibility,
    /// A kept clone of the control socket, used only by `disconnect()`
    /// (#233) to shut the connection down on demand rather than relying
    /// on `Drop` -- which only closes this clone's own fd duplicate, not
    /// the reader/writer threads' separate duplicates of the same
    /// underlying socket. `Arc<Mutex<..>>` so every `RemotePlayer` clone
    /// shares one handle and `disconnect()` is safe to call from any of
    /// them; `Option` so a second call is a no-op instead of a double
    /// shutdown.
    control_stream: Arc<Mutex<Option<ControlStream>>>,
}
```

In `connect_endpoint`, immediately after `let stream = endpoint.connect_stream()?;` (around line 333), take the extra clone `disconnect()` will need, before `stream` gets moved into the writer thread later in the function:

```rust
        let stream = endpoint.connect_stream()?;
        log::info!(target: "remote", "connected to daemon endpoint {endpoint}");

        // Kept aside for `disconnect()` (#233) -- taken before `stream` is
        // moved into the writer thread below.
        let disconnect_stream = stream.try_clone().map_err(|e| e.to_string())?;
```

Then add `control_stream: Arc::new(Mutex::new(Some(disconnect_stream))),` to the final `RemotePlayer { ... }` struct literal at the end of `connect_endpoint` (around line 437-445), alongside the existing fields:

```rust
        Ok((
            RemotePlayer {
                status,
                subtitle_prefs,
                items,
                queue_source,
                cmd_tx,
                disconnected,
                ctrl_compatibility,
                control_stream: Arc::new(Mutex::new(Some(disconnect_stream))),
            },
            event_rx,
        ))
```

In `stub_with_command_rx` (around line 588-597), add the field so the test-only constructor still compiles, wired to `None` since there's no real socket:

```rust
        (
            RemotePlayer {
                status,
                subtitle_prefs,
                items,
                queue_source,
                cmd_tx,
                disconnected,
                ctrl_compatibility: CtrlCompatibility::current(),
                control_stream: Arc::new(Mutex::new(None)),
            },
            event_rx,
            cmd_rx,
        )
```

- [ ] **Step 4: Add `RemotePlayer::disconnect()`**

In `impl RemotePlayer`, right after `pub fn join(&self)` (around line 551-553):

```rust
    pub fn join(&self) {
        // No thread to join; daemon keeps running when TUI exits.
    }

    /// Actively tears down the control-socket connection (#233): shuts
    /// down the shared underlying socket so the reader thread's blocking
    /// `read()` (inside `reader.lines()` in `connect_endpoint`) observes
    /// EOF/an error and exits, instead of leaking forever the way it did
    /// when the only teardown was an implicit `Drop` of one fd duplicate.
    /// Idempotent: the stored handle is taken out on first use, so a
    /// second call is a no-op rather than a double `shutdown()`.
    pub fn disconnect(&self) {
        if let Some(stream) = self.control_stream.lock().unwrap().take() {
            if let Err(e) = stream.shutdown() {
                log::warn!(target: "remote", "control-socket shutdown failed: {e}");
            }
        }
    }
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p mbv-core disconnect_causes_the_reader_thread_to_observe_the_shutdown_and_exit disconnect_is_idempotent`
Expected: PASS (2 passed)

- [ ] **Step 6: Run the full remote_player test module to check for regressions**

Run: `cargo test -p mbv-core remote_player::tests`
Expected: all existing tests still PASS. In particular check `connect_endpoint_propagates_active_remote_playback_status` and the v2/v3 peer tests, which construct `RemotePlayer` via `connect_endpoint` and are the ones most likely to break from the field addition.

- [ ] **Step 7: Commit**

```bash
git add crates/mbv-core/src/remote_player.rs
git commit -m "remote_player: give RemotePlayer a real disconnect() that shuts down the socket"
```

---

### Task 3: `PlayerProxy::disconnect_remote()`

**Files:**
- Modify: `crates/mbv-core/src/player.rs:3044-3054` (`PlayerProxyInner`/`PlayerProxy`), near the existing `join`/`stop` dispatch methods around line 3182-3220
- Test: `crates/mbv-core/src/player.rs` `mod tests`

**Interfaces:**
- Consumes: `RemotePlayer::disconnect` (Task 2).
- Produces: `PlayerProxy::disconnect_remote(&self)` -- Task 4's `App` call sites call this.

- [ ] **Step 1: Write the failing test**

Add to `crates/mbv-core/src/player.rs`'s `mod tests`, near the other `PlayerProxy` dispatch tests (search for `fn is_remote` usage in tests for the right neighborhood):

```rust
    #[test]
    fn disconnect_remote_is_a_no_op_for_a_local_player() {
        let status = Arc::new(Mutex::new(PlayerStatus::default()));
        let proxy = PlayerProxy::stub(status);
        assert!(!proxy.is_remote());

        proxy.disconnect_remote(); // must not panic
    }

    #[test]
    fn disconnect_remote_disconnects_a_remote_player() {
        let (remote, _event_rx) = crate::remote_player::RemotePlayer::stub(Vec::new(), 0);
        let proxy = PlayerProxy {
            always_play_next: false,
            status: remote.status.clone(),
            subtitle_prefs: remote.subtitle_prefs.clone(),
            inner: PlayerProxyInner::Remote(remote),
        };
        assert!(proxy.is_remote());

        proxy.disconnect_remote(); // must not panic; a stub has no real
                                    // socket, so this only exercises the
                                    // dispatch, not the shutdown itself
                                    // (that's covered by Task 2's tests).
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p mbv-core disconnect_remote_is_a_no_op_for_a_local_player disconnect_remote_disconnects_a_remote_player`
Expected: FAIL with a compile error (`no method named 'disconnect_remote' found for struct 'PlayerProxy'`).

- [ ] **Step 3: Implement the dispatch method**

In `crates/mbv-core/src/player.rs`, in `impl PlayerProxy`, right after `pub fn join(&self)` (the method matching `PlayerProxyInner::Remote(r) => r.join()` around line 3199):

```rust
    /// Tears down the underlying connection if this proxy is currently
    /// remote (#233): a no-op for `Local` (there's no socket to close).
    /// Call this on the *old* `PlayerProxy` before overwriting it with a
    /// freshly connected one -- a remote-to-remote swap that skips this
    /// leaks the old connection's reader thread (see
    /// `RemotePlayer::disconnect`'s doc comment for why `Drop` alone isn't
    /// enough).
    pub fn disconnect_remote(&self) {
        if let PlayerProxyInner::Remote(r) = &self.inner {
            r.disconnect();
        }
    }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p mbv-core disconnect_remote_is_a_no_op_for_a_local_player disconnect_remote_disconnects_a_remote_player`
Expected: PASS (2 passed)

- [ ] **Step 5: Run the full player test module to check for regressions**

Run: `cargo test -p mbv-core player::tests`
Expected: all existing tests still PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/mbv-core/src/player.rs
git commit -m "player: add PlayerProxy::disconnect_remote dispatch"
```

---

### Task 4: Wire the two already-remote swap branches in `App`

**Files:**
- Modify: `src/app/mod.rs:2485-2518` (`switch_to_direct_remote`), `:2565-2596` (`switch_to_library_route`)
- Test: `src/app/mod.rs` `pub(crate) mod tests`

**Interfaces:**
- Consumes: `PlayerProxy::disconnect_remote` (Task 3).
- Produces: no new public interface -- this task closes the actual leak at its two call sites.

- [ ] **Step 1: Write the failing tests**

Add to `src/app/mod.rs`'s `pub(crate) mod tests`, near the existing `switch_to_library_route_*`/`switch_to_direct_remote_*` tests:

```rust
    #[test]
    fn switch_to_library_route_disconnects_the_previous_remote_on_a_route_to_route_swap() {
        // #233 regression guard: swapping from one active library route
        // straight to another (the already-remote branch) must tear down
        // the OLD RemotePlayer's connection before replacing it, not just
        // let it leak via Drop. Uses two real TCP loopback "daemons" (not
        // RemotePlayer::stub, which has no real socket to observe) so the
        // first daemon's accepted connection can observe its client side
        // actually closing.
        use mbv_core::remote_player::{ControlStream, DaemonEndpoint, RemotePlayer};
        use std::io::Read as _;
        use std::net::TcpListener;

        let listener_a = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr_a = listener_a.local_addr().unwrap();
        let daemon_a = std::thread::spawn(move || {
            let (stream, _) = listener_a.accept().unwrap();
            crate::app::tests::run_stub_daemon_handshake(stream)
        });

        let listener_b = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr_b = listener_b.local_addr().unwrap();
        let daemon_b = std::thread::spawn(move || {
            let (stream, _) = listener_b.accept().unwrap();
            crate::app::tests::run_stub_daemon_handshake(stream)
        });

        let mut app = make_app_stub();
        let (remote_a, remote_a_rx) =
            RemotePlayer::connect_endpoint(&DaemonEndpoint::Tcp(addr_a), "token").unwrap();
        app.switch_to_library_route("music", remote_a, remote_a_rx);
        assert!(!app.player.is_remote_disconnected());

        let (remote_b, remote_b_rx) =
            RemotePlayer::connect_endpoint(&DaemonEndpoint::Tcp(addr_b), "token").unwrap();
        app.switch_to_library_route("movies", remote_b, remote_b_rx);

        // The OLD (music) connection's daemon-side accept handle should
        // see its client hang up shortly after the swap -- proof the
        // reader thread actually exited instead of leaking.
        let mut daemon_a_stream = daemon_a.join().unwrap();
        daemon_a_stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut buf = [0u8; 8];
        let n = daemon_a_stream.read(&mut buf).unwrap_or(usize::MAX);
        assert_eq!(
            n, 0,
            "old library route's client socket must be shut down after the swap"
        );

        drop(daemon_b);
        let _ = addr_b; // silence unused warning if daemon_b's thread hasn't been joined
    }
```

This test needs a small shared test helper. Add it to `src/app/mod.rs`'s `pub(crate) mod tests` (a free function, not a method, next to `make_session`):

```rust
    /// Minimal daemon-side protocol handshake for tests that need a real
    /// TCP socket `RemotePlayer::connect_endpoint` can connect to (#233):
    /// sends the protocol hello, drains the client's hello line, then
    /// sends an empty initial state. Returns the accepted `TcpStream` so
    /// the caller can observe what happens to it afterward (e.g. that the
    /// client shuts it down).
    pub(crate) fn run_stub_daemon_handshake(stream: std::net::TcpStream) -> std::net::TcpStream {
        use std::io::{BufRead, BufReader, Write};
        let mut writer = stream.try_clone().unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());

        let hello = serde_json::to_string(&mbv_core::ctrl::CtrlEvent::Hello(
            mbv_core::ctrl::CtrlHello::current(),
        ))
        .unwrap();
        writeln!(writer, "{hello}").unwrap();

        let mut client_hello = String::new();
        reader.read_line(&mut client_hello).unwrap();

        let initial_state = serde_json::to_string(&mbv_core::ctrl::CtrlEvent::State(
            mbv_core::ctrl::CtrlState {
                status: mbv_core::player::PlayerStatus::default(),
                items: Vec::new(),
                cursor: 0,
                source: crate::config::QueueSource::Unknown,
            },
        ))
        .unwrap();
        writeln!(writer, "{initial_state}").unwrap();

        stream
    }
```

Also add the `switch_to_direct_remote` counterpart:

```rust
    #[test]
    fn switch_to_direct_remote_disconnects_the_previous_remote_on_a_remote_to_remote_swap() {
        // Same #233 regression, but for the Sessions-panel direct-remote
        // path's already-remote branch (a second "Direct Remote" upgrade
        // while already on one).
        use mbv_core::remote_player::{DaemonEndpoint, RemotePlayer};
        use std::io::Read as _;
        use std::net::TcpListener;

        let listener_a = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr_a = listener_a.local_addr().unwrap();
        let daemon_a = std::thread::spawn(move || {
            let (stream, _) = listener_a.accept().unwrap();
            crate::app::tests::run_stub_daemon_handshake(stream)
        });

        let listener_b = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr_b = listener_b.local_addr().unwrap();
        let daemon_b = std::thread::spawn(move || {
            let (stream, _) = listener_b.accept().unwrap();
            crate::app::tests::run_stub_daemon_handshake(stream)
        });

        let mut app = make_app_stub();
        let sess_a = make_session("daemon-a", "mbv");
        let (remote_a, remote_a_rx) =
            RemotePlayer::connect_endpoint(&DaemonEndpoint::Tcp(addr_a), "token").unwrap();
        app.switch_to_direct_remote(&sess_a, remote_a, remote_a_rx);

        let sess_b = make_session("daemon-b", "mbv");
        let (remote_b, remote_b_rx) =
            RemotePlayer::connect_endpoint(&DaemonEndpoint::Tcp(addr_b), "token").unwrap();
        app.switch_to_direct_remote(&sess_b, remote_b, remote_b_rx);

        let mut daemon_a_stream = daemon_a.join().unwrap();
        daemon_a_stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut buf = [0u8; 8];
        let n = daemon_a_stream.read(&mut buf).unwrap_or(usize::MAX);
        assert_eq!(
            n, 0,
            "old direct-remote client socket must be shut down after the swap"
        );

        drop(daemon_b);
        let _ = addr_b;
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test switch_to_library_route_disconnects_the_previous_remote_on_a_route_to_route_swap switch_to_direct_remote_disconnects_the_previous_remote_on_a_remote_to_remote_swap`
Expected: FAIL -- the read from `daemon_a_stream` returns something other than `0` within the timeout (or the assertion on `n == 0` fails), because nothing shuts the old connection down yet.

- [ ] **Step 3: Wire the fix into both already-remote branches**

In `src/app/mod.rs`, in `switch_to_direct_remote` (around line 2515-2518), change:

```rust
        } else {
            self.player = PlayerProxy::remote(remote, always_play_next);
            self.player_rx = remote_rx;
        }
```

to:

```rust
        } else {
            // #233: tear down the previous remote connection's socket
            // before dropping the old PlayerProxy, so its reader thread
            // observes the shutdown and exits instead of leaking.
            self.player.disconnect_remote();
            self.player = PlayerProxy::remote(remote, always_play_next);
            self.player_rx = remote_rx;
        }
```

And in `switch_to_library_route` (around line 2593-2596), the identical change:

```rust
        } else {
            // #233: tear down the previous remote connection's socket
            // before dropping the old PlayerProxy, so its reader thread
            // observes the shutdown and exits instead of leaking.
            self.player.disconnect_remote();
            self.player = PlayerProxy::remote(remote, always_play_next);
            self.player_rx = remote_rx;
        }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test switch_to_library_route_disconnects_the_previous_remote_on_a_route_to_route_swap switch_to_direct_remote_disconnects_the_previous_remote_on_a_remote_to_remote_swap`
Expected: PASS (2 passed)

- [ ] **Step 5: Run the full workspace test suite to check for regressions**

Run: `cargo test --workspace`
Expected: all tests PASS, including every existing `switch_to_library_route_*`/`switch_to_direct_remote_*`/`connect_to_session_*`/`apply_route_for_playback_*` test from #223, none of which exercise the already-remote branch with a *real* socket today, so none should observe any behavior change from this fix.

- [ ] **Step 6: Run build, clippy, and fmt checks**

Run: `cargo build --workspace` -- expect clean, no warnings.
Run: `cargo clippy --workspace --all-targets` -- expect no new warnings introduced by this branch.
Run: `cargo fmt --check` -- expect no diff.

- [ ] **Step 7: Commit**

```bash
git add src/app/mod.rs
git commit -m "app: disconnect the previous remote before a remote-to-remote player swap (#233)"
```

---

### Task 5: Update ADR framing and close out the docs

**Files:**
- Modify: `docs/adr/0011-library-scoped-daemon-routing.md` (the "Consequences" section already references this as deferred), `docs/adr/0010-lazy-daemon-route-connect-lifecycle.md` (the "disconnects cleanly... reconnects fresh" framing issue #233's acceptance criteria calls out)
- Modify: `CONTEXT.md` if a glossary entry references the old leaky behavior (check for one during this task; none is currently known to reference it explicitly)

**Interfaces:** None -- documentation only, no code interfaces.

- [ ] **Step 1: Check for stale references**

Run: `grep -rn "reader thread\|half-open socket\|233" docs/adr/ CONTEXT.md`

Expected: hits in `docs/adr/0011-library-scoped-daemon-routing.md`'s existing note about the leak being filed as #233, and possibly a passing mention in `docs/adr/0010-lazy-daemon-route-connect-lifecycle.md`'s "disconnects cleanly... reconnects fresh" line.

- [ ] **Step 2: Add a short note to ADR 0011's Consequences section**

Find the existing bullet in `docs/adr/0011-library-scoped-daemon-routing.md` that reads (approximately) "A pre-existing `RemotePlayer` socket/thread-leak on route-to-route swaps was identified but filed separately as #233" and append, in the same bullet or immediately after it:

```markdown
  (#233 is now fixed: `RemotePlayer::disconnect()` shuts down the shared
  socket before a route-to-route swap replaces the old connection, so
  ADR 0010's "disconnects cleanly... reconnects fresh" framing is now
  accurate rather than aspirational.)
```

- [ ] **Step 3: Commit**

```bash
git add docs/adr/0011-library-scoped-daemon-routing.md
git commit -m "docs: note #233's disconnect fix against ADR 0011's deferred-leak callout"
```

---

## Self-Review Notes

- **Spec coverage:** All four of issue #233's acceptance criteria are covered: (1) `RemotePlayer::disconnect()` exists and shuts down the socket (Task 2), (2) reader/writer thread exit is verified via `is_disconnected()` flipping true in a real-socket test (Task 2) and, at the `App` level, via the daemon-side observing the client socket close (Task 4), (3) both already-remote branches (`switch_to_library_route`, `switch_to_direct_remote`) call the new teardown before reassigning `self.player` (Task 4), (4) ADR framing updated to drop the "aspirational" caveat (Task 5).
- **No placeholders:** every step has complete, concrete code -- no "add appropriate handling" language.
- **Type consistency:** `ControlStream::shutdown(&self) -> io::Result<()>` (Task 1) is what `RemotePlayer::disconnect` (Task 2) calls; `RemotePlayer::disconnect(&self)` (Task 2) is what `PlayerProxy::disconnect_remote(&self)` (Task 3) calls; `PlayerProxy::disconnect_remote(&self)` (Task 3) is what both `App` call sites (Task 4) call. Names match end to end.
