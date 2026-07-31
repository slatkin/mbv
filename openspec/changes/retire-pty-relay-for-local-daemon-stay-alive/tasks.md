Order matters: the local-daemon path is built and made to work first (groups 1-7), and the relay
is deleted only afterwards (group 8), so there is a working stay-alive path at every commit.

## 1. Ctrl protocol: shutdown disconnect reason

- [x] 1.1 In `crates/mbv-core/src/ctrl.rs`, add a `DaemonShutdown` variant to `DisconnectReason`
      (the enum at the bottom of the file, alongside `TakenOverByEmbyRemote`), with an explicit
      `#[serde(rename = "DaemonShutdown")]` matching the existing variant's style.
- [x] 1.2 In `crates/mbv-core/src/ctrl.rs`, bump `CTRL_PROTOCOL_VERSION` from `6` to `7`. Note it is
      currently `6`, not `4` as `openspec/specs/ctrl-protocol/spec.md` claims — the spec was stale
      and this change's delta corrects it. Do not change `CtrlCompatibility::for_peer`'s
      exact-match-or-reject logic.
- [x] 1.3 Update protocol-version fixtures in `crates/mbv-core/src/remote_player_tests.rs` and any
      handshake/serialization test that hard-codes a version number.
- [x] 1.4 In `crates/mbv-core/src/remote_player.rs`, extend `disconnect_reason_message` and the
      `apply_ctrl_event` `CtrlEvent::Disconnected` match arm for the new variant. Its message is a
      plain "the daemon was stopped" line, not an Emby-authority message.
- [x] 1.5 In `crates/mbv-core/src/remote_player.rs`, in the reader thread's `is_structured_disconnect`
      match (around the "Under multi-connection (v5)" comment), return `true` for `DaemonShutdown`
      so the following close is treated as expected and no synthetic `PlayerEvent::Stopped` is
      emitted. Fix that stale `(v5)` comment while you are in it.
- [x] 1.6 Expose the expected-disconnect distinction to the app layer: `RemotePlayer` must let a
      caller tell "closed after an announced shutdown" from "closed with no warning". Add whatever
      minimal accessor or `PlayerEvent` this needs; `App` consumes it in task 7.2.
- [x] 1.7 In `crates/mbv-core/src/daemon_core.rs`, broadcast
      `CtrlEvent::Disconnected { reason: DaemonShutdown }` to all clients on the explicit-shutdown
      path (the `DaemonEvent::Shutdown` handling reached from `shutdown_signal_rx`), before closing
      connections. Reuse `notify_disconnected_all`.
- [ ] 1.8 Verify: with an `mbvd` running and a client attached, `mbv -q`-equivalent shutdown makes
      the client exit with one line and no error; and an old-version peer is refused at the
      handshake rather than mid-session.

## 2. Local daemon bootstrap

- [x] 2.1 Add a hidden self-spawn subcommand to `src/main.rs` that runs the local daemon in this
      process (name it for the daemon, e.g. `--__local-daemon`; do not reuse `--__relay`). It parses
      its own argv, initialises logging to a daemon log under `state_dir()`, and calls
      `mbv_core::daemon::run_with_options`.
- [x] 2.2 Add a `spawn_detached`-equivalent for the daemon: fork, `setsid()`, ignore `SIGHUP`,
      redirect stdio away from the terminal, and exec `mbv --__local-daemon ...`. Model it on
      `relay::spawn_detached` but drop everything pty-related. Put it in a new small module (e.g.
      `src/local_daemon.rs`); do not add it to `src/relay.rs`, which is being deleted.
- [x] 2.3 Have the daemon acquire the single-instance lock and call `LockGuard::write_pid()` as part
      of its startup, before binding the control socket, so `mbv -q` can find it.
- [x] 2.4 Have the daemon fail fast with a clear message when `EmbyClient::authenticate` fails on the
      cached token (mirror `crates/mbvd/src/main.rs`'s "no cached credentials" behavior), and make
      that reason reach the launching terminal — the spawning client is still on a terminal and must
      report it rather than exiting silently on a connect timeout.
- [ ] 2.5 Verify: run the hidden subcommand by hand, confirm it binds
      `$XDG_RUNTIME_DIR/mbv-ctrl.sock`, holds the lock, writes its PID, survives its parent shell
      closing, and that `mbv --connect-daemon local` attaches to it.

## 3. Single-instance repoint

- [x] 3.1 In `src/single_instance.rs`, change `socket_path()` to return
      `mbv_core::config::control_socket_path()` rather than `runtime_dir().join("mbv-relay.sock")`,
      so the probe and `DaemonEndpoint::Local` agree by construction. Keep this module's own
      `lock_path()` and `runtime_dir()` as they are.
- [x] 3.2 Rename `Resolution::Reattach` to `Resolution::Attach` and update its doc comment: a live
      local daemon exists, attach as a client alongside any others. Update the module header, which
      currently describes relays and pty slaves.
- [x] 3.3 Update the existing `single_instance` tests for the rename; keep both existing cases
      (fresh-when-unlocked, refuse-when-locked-without-socket) — they are the ones that protect the
      trichotomy.
- [x] 3.4 Add one test for the third branch: lock held plus a *bound and listening* socket resolves
      to `Attach`. This is the branch the whole change hinges on and no existing test covers it,
      because binding a real listener was pointless when the socket meant "relay".
- [x] 3.5 Rewrite the `Refuse` message in `src/main.rs` per the spec: name the owning PID, say why
      only one process can own playback, and give both remedies (stop it, or use `mbv -d` for
      several terminals).
- [x] 3.6 In the `mbv -q` branch of `src/main.rs`, replace the "stay-alive relay is starting up but
      not yet attachable" fallback message with a plain retry hint — a local daemon writes its PID
      as part of startup and has no attach gate.

## 4. Startup rewiring in `src/main.rs`

- [x] 4.1 Replace `alive_requested` (`has_flag(&args, "-a") || has_flag(&args, "--alive")`) with
      `has_flag(&args, "-d")`. Remove the `-a`/`--alive` filtering from the argv-forwarding code.
- [x] 4.2 Rewrite `print_usage`: document `-d`, delete the `-a, --alive` entry, and describe `-q` as
      stopping the running Player owner.
- [x] 4.3 Rewrite the `Resolution::Fresh` stay-alive branch: **authenticate first**
      (`authenticate_or_login`), then drop the lock guard, then spawn the daemon, then connect via
      `DaemonEndpoint::Local` and run `run_remote_app(client, remote, player_rx, true)`. This
      inverts today's deliberate skip-authentication behavior — the comment there explaining why
      authentication is skipped is now wrong and must go.
- [x] 4.4 Rewrite the `Resolution::Attach` branch the same way: authenticate, connect to
      `DaemonEndpoint::Local`, run as a client. It must not spawn a daemon and must not take a lock.
- [x] 4.5 Delete the `is_inferior` / `relay::CTRL_FD_ENV` guard and its comment. The recursion it
      defended against cannot occur: the hidden daemon subcommand returns before any stay-alive
      branch is evaluated.
- [ ] 4.6 Verify by hand, in this order: `mbv -d` in terminal A starts playback; terminal B `mbv`
      attaches without disturbing A; closing A leaves playback running; closing B leaves playback
      running; `mbv` in terminal C attaches and shows the live queue; `mbv -q` stops everything.

## 5. Client-side corrections

- [x] 5.1 Retain the local-daemon flag: `App::new_remote` currently consumes its `is_local_daemon`
      parameter during construction without storing it. Store it on `App` (or carry the endpoint on
      `PlayerProxy`) so later code can ask "is my Player owner on this machine?". Tasks 5.2, 5.4,
      5.5 and 6.x all depend on this.
- [x] 5.2 Fix the queue clobber: `src/app/mod.rs` calls `self.restore_queue_state()` unconditionally
      in `run()`, and `restore_queue_state` in `src/app/queue_actions.rs` calls
      `player_tab.set_items(...)` with no guard, so it overwrites a queue that
      `bootstrap_local_daemon_queue` just adopted live from the daemon. Skip the disk restore when
      the app was constructed from a local-daemon bootstrap; the cold-daemon case is already handled
      by `bootstrap.rs` plus `spawn_enrich_queue_state`.
- [x] 5.3 Add a test for 5.2 alongside `src/app/tests_daemon_bootstrap.rs`: a local-daemon app with a
      live adopted queue plus a differing `queue_state.json` on disk must keep the live queue. This
      is a realistic data-loss path that becomes routine once every attach goes through it.
- [x] 5.4 Fix the auto-reconnect teardown gap in `src/app/run_loop_events.rs`: the
      `if self.launched_as_remote { skip }` branch must skip only for a genuinely remote daemon, not
      for a same-host local daemon. Update the long comment above it, which currently justifies the
      skip on the assumption that `new_remote` always means a remote endpoint.
- [x] 5.5 Repoint the alive indicator in `src/app/render/chrome_status.rs` from
      `self.stay_alive_ctrl.is_some()` to the retained local-daemon flag, so it survives the deletion
      of `stay_alive_ctrl` in group 8 rather than silently disappearing.
- [x] 5.6 In `src/app/consume_quit_actions.rs`, delete the stay-alive detach branch of `try_quit` and
      its `stay_alive_on_exit` lookup. Quit is now always a real quit for the client; playback
      continues because the daemon is a separate process.

## 6. Visualizer

- [x] 6.1 In `src/app/visualizer.rs`, replace
      `let is_local = !self.player.is_remote() && self.connected_session_id.is_none();` with a check
      that admits a same-host local daemon: in-process Player *or* local-daemon endpoint, and still
      no attached Emby session. Use the flag retained in task 5.1.
- [x] 6.2 Leave `audio_pipe_enabled` in the gate exactly as it is; it is an independent, unchanged
      reason the visualizer stays off.
- [x] 6.3 Verify by eye: with `mbv -d` playing, the visualizer shows a live spectrum in the client;
      with an explicit remote endpoint it stays off; with the audio pipe enabled it stays off.

## 7. Disconnect handling

- [x] 7.1 Add a blocking modal for unannounced loss of a local daemon, offering
      `[R] Restart and resume`, `[S] Restart, don't resume`, `[Q] Quit`, plus the last known playing
      title and the daemon log path. Follow the existing modal/overlay pattern in `src/app/render`
      and the existing input-blocking convention.
- [x] 7.2 Wire the two branches: on the announced shutdown reason (task 1.6) print one line, restore
      the terminal, and exit; otherwise raise the modal. Only clients of a local daemon get the
      modal — a client of a remote endpoint cannot restart that daemon.
- [x] 7.3 Implement `[R]`: re-run the ensure-daemon-then-attach path from group 2/4 and let the
      normal saved-queue restore happen. Implement `[S]`: same, but suppress the resume so the new
      daemon comes up idle. Implement `[Q]`: restore the terminal and exit.
- [x] 7.4 Add a test for the announced-versus-unannounced branch, at the level where the decision is
      made (`crates/mbv-core/src/remote_player.rs` reader thread, alongside
      `crates/mbv-core/src/daemon_tests.rs`'s existing `DisconnectReason` cases). It protects a
      boundary where getting it backwards means either a spurious crash modal on every clean quit or
      a silent exit on a real crash.
- [ ] 7.5 Verify by hand: `kill -9` the local daemon and confirm the modal appears with correct
      diagnostics, that `[S]` yields a working idle client, and that `mbv -q` produces the clean
      one-line exit instead of the modal.

## 8. Delete the relay

- [x] 8.1 Delete `src/relay.rs` and `src/terminal_client.rs`, and remove their `mod` declarations.
- [x] 8.2 Delete `src/app/stay_alive.rs`, the `stay_alive_ctrl` fields in `src/app/app_struct.rs`
      (both structs) and their initialisers in `src/app/construct.rs`, and the `App::attached` field
      and its handling in `src/app/render_cadence.rs`.
- [x] 8.3 Remove the `parse_relay_args` / `--__relay` branch from `src/main.rs`, and the
      `take_attach_pending()` forced-repaint block in `src/app/mod.rs`'s run loop.
- [x] 8.4 Remove the stay-alive tray block from `App::run()` in `src/app/mod.rs`, and the long
      relay-SIGHUP comment above `install_signal_handlers()` there, which describes a process that no
      longer exists.
- [x] 8.5 Delete the tests that exercised relay detach behavior: the `StayAliveCtrl::for_test` cases
      in `src/app/tests_lifecycle.rs`, `src/app/tests_status_bar.rs`, and `src/app/render/tests.rs`,
      plus the `stay_alive_ctrl: None` initialisers in `src/app/tests.rs`. Delete them rather than
      adapting them; the behavior they cover is gone.
- [x] 8.6 Verify: `cargo build` and `cargo clippy` are clean, and `rg -n 'relay|terminal_client|stay_alive_ctrl' src/ crates/`
      returns nothing but incidental prose.

## 9. Tray moves into the local daemon

- [x] 9.1 In the hidden local-daemon subcommand (task 2.1), supply real `DaemonRuntimeHooks`: the
      `on_player_ready` closure stashes the `DaemonPlayerHandle` in a shared cell, and the
      `on_tray_ready` closure reads it and calls `crate::tray::spawn(shutdown_tx, handle.status,
      handle.command_tx)`. `daemon_run.rs` calls the two hooks in that order, and `tray::spawn`'s
      return type already matches `OnTrayReady`, so no hook signature changes.
- [x] 9.2 Gate the tray on `show_systray_icon`, and log-and-continue when `tray::spawn` returns
      `None`. Emit no terminal warning on either the daemon or the client side.
- [x] 9.3 Confirm the tray's quit action still reaches the same graceful shutdown as `mbv -q`, now
      via the daemon's own `shutdown_signal_tx` rather than a self-`SIGTERM` inside `App`.
- [x] 9.4 Leave `crates/mbvd/src/main.rs`'s `on_tray_ready: Box::new(|_| None)` as-is, and add a
      one-line comment saying the stub is deliberate because `mbvd` runs as a system service with no
      user session.
- [ ] 9.5 Verify: with `mbv -d` running and every client closed, the tray is present, its transport
      controls work, and its quit action stops the daemon.

## 10. Documentation

- [x] 10.1 Add a supersession header to `docs/adr/0005-owned-pty-relay-for-stay-alive.md` in the
      style of `docs/adr/0008-*.md`'s `> **Superseded (date):**` blockquote, pointing at ADR 0015.
      Leave the body intact as historical context.
- [x] 10.2 Amend `docs/adr/0006-single-instance-flock-and-socket-detection.md` in place — do not
      supersede it. Its core decision survives; only the probed socket (`mbv-ctrl.sock`) and the
      meaning of the second branch (attach to a local daemon, non-evicting) change. Its final
      consequence, that there is no implicit local-daemon auto-attach any more, is the one line that
      this change reverses.
- [x] 10.3 Write `docs/adr/0015-local-daemon-for-stay-alive.md` in the house style
      (`## Decision`, `## Considered options`, `## Consequences`). It must record: stay-alive as
      ensure-daemon-then-attach; the rule that a client exiting never stops the daemon; the
      deliberate rejection of session continuity; and the rejected alternatives — keeping the pty
      relay, making everything daemon-mode (leaves a resident mpv after every casual launch and
      contradicts the no-implicit-stop rule), and tmux/abduco/dtach. For the last one, note that ADR
      0005's graphics-passthrough objection **evaporates** here: each client is a full mbv on a real
      terminal doing its own capability detection, with no byte pipe to pass anything through.
- [x] 10.4 Record in ADR 0015 how ADR 0005's crash-isolation rationale is answered rather than
      dropped: libmpv moves out of the terminal-owning process into the daemon, and the client's
      panic hook (installed first thing in `src/main.rs`) restores the terminal exactly as in bare
      mode.
- [x] 10.5 Update any user-facing documentation or README text that mentions `-a`/`--alive`,
      detaching, or reattaching, using the vocabulary in `CONTEXT.md`.

## 11. Final verification

- [ ] 11.1 `cargo fmt --all -- --check` and `cargo clippy` clean.
- [ ] 11.2 Run the test modules touched by this change (`single_instance`, `app::tests_daemon_bootstrap`,
      `mbv_core::daemon_tests`, `mbv_core::remote_player_tests`) rather than the whole suite.
- [ ] 11.3 Walk the manual matrix once: bare mode unchanged (one process, quit stops playback,
      second launch refused with the new message); stay-alive with two clients attached and each
      closed in turn; visualizer live in a client; tray present with no client attached; `mbv -q`
      giving every client a clean exit; `kill -9` of the daemon giving the modal.
