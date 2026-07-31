## Context

Today `mbv -a` (or config `stay_alive = true`) takes the path described in ADR 0005. The
launching process calls `relay::spawn_detached`, which re-execs `mbv --__relay <sock> -- <argv>`.
That relay opens a pty, binds `$XDG_RUNTIME_DIR/mbv-relay.sock`, and on first attach forks an
inferior — a complete `mbv` running `App::run()` on the pty slave, owning the Player. The
launcher then degrades itself into `terminal_client::run_terminal_client`, a raw byte pipe
between the real terminal and the relay socket. Attach is single-slot: `relay.rs` keeps one
`current_data`/`current_winsz` client, so a newcomer evicts the incumbent.

Separately, mbv already has a daemon. `crates/mbv-core/src/daemon_run.rs` runs a headless
Player behind a ctrl socket; `DaemonEndpoint::Local` resolves to
`crates/mbv-core/src/config_types_paths.rs::control_socket_path()`
(`$XDG_RUNTIME_DIR/mbv-ctrl.sock` for a user instance, `/run/mbv/` only when
`is_system_instance()`), and `DaemonEndpoint::connect_stream` already retries on connect for
exactly this endpoint. `App::new_remote(.., is_local_daemon: true)` already exists and is
already documented to "behave exactly like a plain local session — one unified queue, normal
queue-state persistence — the only difference is that the daemon owns mpv." The
`daemon-multi-connection` capability already specifies non-evicting multi-client attach at the
protocol level.

So the substrate for stay-alive already exists and is better than the relay. What is missing is
the bootstrap (nothing currently spawns a *local* daemon — ADR 0006 explicitly retired the old
`mbv -d` self-spawn), plus a handful of paths that assume "remote" means "not on this machine".

Constraints:

- The daemon has no terminal, so it cannot run interactive login.
- Packaged `mbvd` runs as root under systemd with no user session; it must not grow a tray.
- `mbv` and `mbvd` ship from one workspace and are versioned together.

## Goals / Non-Goals

**Goals:**

- Delete the pty relay and the byte-pipe terminal client; make stay-alive a thin bootstrap over
  the existing local daemon and its existing client path.
- Allow any number of terminals to be attached at once, without eviction.
- Guarantee that a client exiting never stops playback in stay-alive mode.
- Keep the system-audio visualizer working for clients of a same-host local daemon.
- Give the tray a home that outlives every terminal.
- Give a client something useful to do when its daemon dies.

**Non-Goals:**

- **Session continuity.** On-screen state — cursor, scroll, open overlays, in-flight search, the
  queue undo stack — is not preserved across a client closing. This is a deliberate rejection,
  not an unfinished corner: the whole point of the relay was to preserve a terminal's byte
  stream, and preserving it is what forced single-slot attach, pty ownership, and eviction. Do
  not re-introduce a relay to recover scroll position.
- Changing bare mode. With `stay_alive` off, nothing about mbv's topology changes.
- Changing packaged `mbvd`, other than the shared protocol version and the shared
  `DisconnectReason` enum.
- Multi-user or cross-machine stay-alive. One local daemon per user, on one machine.
- Changing `audio_pipe_enabled` (Snapcast) visualizer behavior, which remains an orthogonal
  reason the visualizer stays off.

## Decisions

### 1. Stay-alive is "ensure the local daemon exists, then attach"

There is one local daemon per user. Stay-alive is not a new topology; it is a bootstrap. The
launching `mbv` self-spawns a hidden local-daemon subcommand (the same self-exec pattern the
relay used, minus the pty), the daemon binds `$XDG_RUNTIME_DIR/mbv-ctrl.sock`, and the launcher
then runs its normal TUI as a client of `DaemonEndpoint::Local`. The connect-retry already in
`connect_stream` covers the spawn-then-connect window without new coordination.

*Alternative rejected — keep the relay.* It costs ~900 lines of pty/winsize/signal code to
preserve a property (byte-level session continuity) we are explicitly dropping, and it forces
eviction on attach.

*Alternative rejected — make everything daemon-mode.* Uniform, but it leaves a resident mpv
process after every casual `mbv` launch, and it contradicts decision 3 below: since a client
exiting must never stop the daemon, "always daemon" means "always leaves something running."

### 2. A client exiting never stops the local daemon

This is the rule that dissolves lifetime ownership. It does not matter whether this invocation
spawned the daemon, an earlier one did, or a user ran it by hand. Stopping is only ever
explicit: `mbv -q` or tray Quit. Consequences: no reference counting, no "last client out turns
off the lights" race, no special-casing the spawning client.

### 3. Bare mode is unchanged and remains the default

Users expect a normal application by default and should not be dropped into a client/server
model they never asked for. Bare mode therefore stays single-instance — and that restriction is
now honest rather than arbitrary: two bare instances genuinely contend for the audio device and
the Emby playback session. `mbv -d` is the first-class escape hatch for wanting several windows.

### 4. The flock survives, repointed

`src/single_instance.rs` keeps the flock at `$XDG_RUNTIME_DIR/mbv.lock` and the
Fresh/Attach/Refuse trichotomy. Only the probed socket changes, from `mbv-relay.sock` to
`mbv-ctrl.sock` (`control_socket_path()`, so the probe and the client agree on one path by
construction). ADR 0006's core reasoning is untouched: the flock is the stale-proof authority
because the kernel releases it on any process death, and connectability — never file existence —
disambiguates the two lock-held cases.

The lock means exactly one thing: *who owns the Player*. In bare mode that is the app; in
stay-alive mode that is the local daemon. **Clients take no lock.** That is precisely what
permits N of them, and it is the same rule ADR 0006 already applies to `--connect-daemon`
clients.

The Refuse branch gains a real remedy. Today it says "only one mbv instance may run at a time.
Close it, or use `mbv -q`." It should now also say that running `mbv -d` gives multiple windows,
because that is the actual answer to what the user was trying to do.

`mbv -q` is unchanged in mechanism: whoever holds the lock writes its PID into the lock file,
and `-q` SIGTERMs it. The `read_pid` fallback that probes the socket for a "relay starting up,
not yet attachable" state is no longer meaningful — a local daemon binds its socket and writes
its PID as part of normal startup, with no attach gate — so that message should be replaced with
a plain retry hint rather than relay vocabulary.

### 5. Authentication ordering inverts

Today `src/main.rs` deliberately *skips* authentication on the stay-alive launcher path,
discarding the constructed `EmbyClient`, because the inferior authenticates interactively on the
pty. A detached daemon has no terminal and cannot prompt. So the ordering inverts: **the
launching `mbv` authenticates first (`authenticate_or_login`), then spawns the daemon**, which
picks up the cached token via `token_cache_path()` exactly as `mbvd` already does.

The failure case needs an answer, because the daemon cannot prompt: if the cached token is
missing, invalid, or expired when the daemon starts, it must fail fast and report the reason
rather than sit there unusable. `crates/mbvd/src/main.rs` already models the shape of this
(`"mbvd: no cached credentials; run mbv interactively first"`), but for the self-spawned daemon
the launching client is right there and can surface it. Since the client just authenticated
successfully moments earlier, this should be rare; treat it as a startup error the client
reports on the terminal it still owns, not as a background condition to be logged and forgotten.

### 6. Disconnect handling: deliberate versus unannounced

Clients do **not** exit on connection loss. Two cases, distinguished by whether the daemon said
goodbye:

- **Deliberate** (`mbv -q`, tray Quit): the daemon broadcasts `CtrlEvent::Disconnected { reason:
  <new shutdown variant> }` to every client before closing. Clients print one line and exit
  cleanly. The plumbing for this already exists: `remote_player.rs` computes
  `is_structured_disconnect` from an exhaustive match on `DisconnectReason` precisely so a new
  variant can mark a disconnect as expected; today the only variant returns `false`, and the new
  one returns `true`.
- **Unannounced** (socket dies with no such event): the client raises a modal that blocks input
  and offers `[R] Restart and resume`, `[S] Restart, don't resume`, `[Q] Quit`, plus diagnostics
  (last item title, daemon log path). `[S]` exists specifically to escape a crash loop where
  resuming the offending item re-triggers the crash. Without it the only recovery is editing
  `queue_state.json` by hand.

The restart race needs no new coordination: if several clients hit Restart at once, the existing
flock arbitrates. One acquires it and spawns the daemon; the others find the socket connectable
and attach.

### 7. Protocol version bump — and a spec/code drift found while planning

Bumping the ctrl protocol version is the right call even though the new `DisconnectReason`
variant is additive: with serde's externally-tagged enums, an old peer hits a deserialization
error on the unknown variant *mid-session*, at the worst possible moment (shutdown). Bumping
makes an old `mbvd` and a new `mbv` refuse each other cleanly at the hello handshake instead.
`CtrlCompatibility::for_peer` already implements exact-match-or-reject, so this is a one-constant
change plus fixtures.

**Drift found:** `crates/mbv-core/src/ctrl.rs` has `CTRL_PROTOCOL_VERSION: u32 = 6`, but
`openspec/specs/ctrl-protocol/spec.md` still says "Protocol version 4". The archived
`reliable-daemon-playback-intents` change bumped the constant (task 1.2) without updating the
spec, and a further bump followed. The delta in this change therefore does two things: it
corrects the spec to reflect reality and it applies this change's bump, landing at **version 7**.
A comment in `remote_player.rs` that says "Under multi-connection (v5)" is stale for the same
reason and should be corrected while touching that file.

### 8. Tray moves into the local daemon

`crates/mbv-core/src/daemon_run.rs:87` already calls `(hooks.on_tray_ready)(shutdown_signal_tx)`
and `crates/mbvd/src/main.rs` already passes `Box::new(|_| None)`. The working tray is
`src/tray.rs::spawn(shutdown_tx, status, cmd_tx) -> Option<Box<dyn Send>>`, whose return type
already matches `OnTrayReady`. So the move is: lift the tray out of `App::run()` and supply a
real `on_tray_ready` for the self-spawned local daemon only.

One sequencing detail matters. `on_tray_ready` receives only the shutdown sender; the tray also
needs `PlayerStatus` and the player command channel, which arrive through
`on_player_ready(DaemonPlayerHandle { status, command_tx })`. `daemon_run.rs` calls
`on_player_ready` first and `on_tray_ready` immediately after, so the local daemon's two closures
share a cell: `on_player_ready` stashes the handle, `on_tray_ready` reads it and passes it to
`tray::spawn`. No hook signature change is needed. `DaemonPlayerHandle.command_tx` is already
`Arc<Mutex<Option<Sender<PlayerCommand>>>>`, the exact type `tray::spawn` wants.

**Packaged `mbvd` keeps the no-op stub.** It runs as root under systemd with no user session and
no D-Bus session bus; a tray there would be broken by construction. This is also what ADR 0004
already asserts — the tray belongs to the daemon process — so this change fulfils that ADR rather
than reinterpreting it.

Headless local daemons (SSH, bare TTY, `show_systray_icon = false`) simply get no tray. **No
warning is emitted.** The daemon logs it and moves on; `mbv -q` is the documented remedy. Warning
would mean warning on every SSH launch about a thing the user cannot fix from there.

### 9. Visualizer gate must stop conflating "daemon" with "remote"

`src/app/visualizer.rs:18` reads:

```rust
let is_local = !self.player.is_remote() && self.connected_session_id.is_none();
```

`PlayerProxy::is_remote()` is true for *every* daemon connection, local or not. But CAVA captures
**system audio** — `crates/mbv-core/src/visualizer.rs` starts it with `method = pulse` reading the
default sink monitor, with no connection to mpv whatsoever. A same-host local daemon's playout is
therefore exactly as capturable as in-process playback. The gate is simply wrong for this case.

The fix uses a signal that already exists: `DaemonEndpoint::is_local()`, computed at connect time
and passed into `App::new_remote(.., is_local_daemon)` at `src/app/construct.rs`. That flag is
consumed during construction and **not retained** on `App`, so it must now be stored (on `App`,
or on `PlayerProxy` alongside the endpoint) for the gate to read. The gate becomes "playback is
audible on this machine", satisfied by in-process playback or by a same-host local daemon, and
still excluding genuinely remote daemons and attached Emby sessions.

`audio_pipe_enabled` stays an independent, unchanged reason the visualizer is off.

### 10. `restore_queue_state()` clobber (must be fixed here)

`src/app/mod.rs:273` calls `self.restore_queue_state()` unconditionally at startup, and
`src/app/queue_actions.rs::restore_queue_state` has no guard: it reads `queue_state.json` and
calls `self.player_tab.set_items(...)`, overwriting whatever is there. Meanwhile
`src/app/bootstrap.rs::bootstrap_local_daemon_queue` may have just adopted a **live** queue from
the daemon into `player_tab`.

Today this is rare, because reaching a local daemon requires an explicit endpoint. Once
stay-alive is the local daemon, *every* attach hits it — the live queue of whatever is playing
would be replaced by a disk snapshot on each new terminal. The fix is to skip the disk restore
when the app was constructed from a live local-daemon bootstrap that already supplied a queue,
while keeping the cold-daemon path (empty daemon queue, saved state adopted) working — that path
already routes through `bootstrap` and `spawn_enrich_queue_state`, so it does not need
`restore_queue_state` either.

### 11. `launched_as_remote` teardown gap

`src/app/run_loop_events.rs:198-229` skips writing the auto-reconnect record whenever
`launched_as_remote` is true, and `App::new_remote` sets that flag for every client. The reason
given in the comment is sound for a *network* daemon: `new_remote` instances never populate
`active_route`/`connected_session_state`, so running the block would compute `None` and wipe a
real record saved by an `App::new` session. But under stay-alive, `App::new` sessions become
rare, so nothing refreshes that record any more.

The narrow fix is to make the skip conditional on the endpoint being genuinely remote rather than
on `launched_as_remote` alone — the same `is_local_daemon` flag decision 9 already requires us to
retain. A same-host local-daemon client is, per the existing `new_remote` doc comment, meant to
behave exactly like a local session, so it should participate in the same persistence.

### 12. Playback commands stay optimistic-free (accepted as-is)

`src/app/playback_target_local.rs` flashes "Pause requested" / "Next requested" and waits for the
daemon round-trip, where a bare app toggles instantly. Over a same-host Unix socket the latency
is negligible; the flash text is the only real difference and it is arguably honest. **Decision:
accept the existing path unchanged.** Revisit as a follow-up only if it demonstrably feels
laggy in use. Special-casing local daemons here would add a second command path for a
sub-millisecond difference.

### 13. Crash isolation is preserved by different means

ADR 0005 credited the dumb-pipe terminal client with crash isolation: the terminal-owning process
carried none of mbv's crash-prone machinery. That argument must be answered, not dropped.

Under this design the isolation is better, not worse. The crash-prone component — libmpv — moves
*out* of the terminal-owning process into the daemon, which is the whole point. And a TUI panic
in the client is handled the same way it is in bare mode today: `src/main.rs` installs a global
panic hook before anything else, which writes the crash log and restores the terminal. The
terminal-owning process is no longer trivially small, but it no longer hosts the component most
likely to take it down, and a client dying now costs the user nothing because the daemon keeps
playing.

### 14. `q`, detach, and the alive indicator

Today `q` under stay-alive is a *detach*: `src/app/consume_quit_actions.rs::try_quit` sends
`DETACH` over the relay control channel and keeps the run loop going. With the relay gone there
is nothing to detach from — the client is a whole process on its own terminal. So **`q` simply
exits the client, and in stay-alive mode never stops playback.** The user-visible behavior is
the same ("close the window, music keeps playing"); the mechanism collapses from a control
message to a process exit. The detach-failure flash and the `attached` bookkeeping
(`App::attached`, `take_attach_pending`, the forced repaint at `src/app/mod.rs:517`, and
`render_cadence`'s handling) all become dead and go with the relay.

The status-bar alive indicator at `src/app/render/chrome_status.rs:267` is gated on
`stay_alive_ctrl.is_some()` and would silently disappear when that field is deleted. It should
be **repointed, not dropped**: it is still true and still useful to show that this terminal is
attached to a local daemon rather than owning playback. Drive it from the retained
`is_local_daemon` flag.

### 15. Naming and CLI

- Config key stays `stay_alive`. It still reads true — playback stays alive — and renaming costs
  a config migration for no behavioral gain.
- **`-d`** becomes the flag: ad-hoc, turns stay-alive on for this invocation, no-op when
  `stay_alive = true` is already configured. The name is currently free (ADR 0006 retired the old
  `-d` local-daemon flag) and it is the mnemonic users of the old flag already have.
- **`-a` / `--alive` is retired with no alias.** Keeping an alias would preserve a name for a
  mechanism that no longer exists.
- `print_usage` in `src/main.rs` must be rewritten for both flags, and its `--connect-daemon`
  description ("instead of owning a local Player") is worth aligning with the new vocabulary.

## Risks / Trade-offs

- **Existing stay-alive users lose on-screen continuity.** → Unavoidable and intended; it is the
  cost of deleting the pty. Mitigated by what already persists: per-library browse position
  (`src/app/library_position_state.rs`), `prefs.json` scalars, and `queue_state.json`. Must be
  called out as **BREAKING** in release notes, not buried.
- **`queue_state.json` may be stale after a crash.** → It is written on quit/teardown
  (`run_loop_events.rs:256`) *and* on queue mutations and some player events
  (`player_event.rs:179,286`, `context_menu_actions.rs`, `shuffle_folder_actions.rs`,
  `artist_header_actions.rs`), so it is not purely a shutdown artifact — but there is still no
  periodic snapshot, so "Restart and resume" after an unannounced daemon death can restore a
  queue a little behind reality. Accept for now, document in the modal's diagnostics, and treat
  periodic snapshotting as a follow-up rather than scope for this change.
- **Protocol lockstep.** → A user with a packaged `mbvd` from an older build and a newer `mbv`
  gets a clean handshake refusal instead of a working connection. That is the intended trade and
  it is why the bump exists; both binaries ship from one workspace.
- **Two topologies to reason about.** → Bare and stay-alive genuinely diverge (in-process Player
  versus daemon-backed), so bugs can be mode-specific. Mitigated by the fact that the
  daemon-backed path is not new code: it is the already-shipping `App::new_remote` +
  `is_local_daemon` path, which this change simply makes reachable without an explicit endpoint.
- **A daemon can outlive the user's awareness of it.** → By design ("a client exiting never stops
  the daemon"), and the tray is the affordance that makes it visible. On headless hosts there is
  no tray and no warning, so a user can leave a daemon running after SSH-ing out. `mbv -q` is the
  remedy; this is documented behavior, deliberately not a nag.
- **Deleting `stay_alive_ctrl` touches more than the relay files.** → Its readers include
  `consume_quit_actions.rs`, `render/chrome_status.rs`, `render_cadence.rs`, `app_struct.rs`,
  `app/mod.rs`, and four test modules (`tests_lifecycle.rs`, `tests_status_bar.rs`,
  `render/tests.rs`, `app/tests.rs`). Those tests exercise detach behavior that ceases to exist
  and should be deleted with it, not adapted.
