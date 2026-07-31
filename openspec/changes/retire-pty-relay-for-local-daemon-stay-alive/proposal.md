## Why

Stay-alive is currently built on an owned pty relay (ADR 0005): a detached process that
opens a pty, forks a full mbv on the slave, and reduces every terminal to a raw byte pipe.
That machinery is ~900 lines of pty, winsize, signal, and single-slot attach code whose only
job is to keep one process alive behind a terminal — and it enforces one attached terminal at
a time by eviction.

mbv already has the thing that does this properly. A daemon owning the Player and serving
`$XDG_RUNTIME_DIR/mbv-ctrl.sock` exists, clients already connect to it over
`DaemonEndpoint::Local` with connect-retry, and the `daemon-multi-connection` capability
already specifies non-evicting multi-client attach. Stay-alive is not a separate concept: it
is "make sure the local daemon exists, then attach to it as a client." Retiring the relay
deletes the pty layer, makes several terminals usable at once, and puts the tray where ADR
0004 already said it belongs — in the process that owns playback.

## What Changes

- Stay-alive becomes **ensure the local daemon exists, then attach**. When `stay_alive` is on,
  `mbv` authenticates, spawns a local daemon if nothing is listening on
  `$XDG_RUNTIME_DIR/mbv-ctrl.sock`, then runs its own full TUI as a client of that daemon.
- **A client exiting never stops the local daemon.** Stopping it is only ever explicit:
  `mbv -q` or tray Quit. It does not matter which invocation spawned it.
- **Bare mode is unchanged and stays the default.** With `stay_alive` off, `mbv` is one process
  owning an in-process Player, exactly as today. The two topologies coexist deliberately.
- **BREAKING**: `-a` / `--alive` is retired with no alias. `-d` becomes the ad-hoc flag that
  turns stay-alive on for one invocation. The config key stays `stay_alive`.
- **BREAKING**: stay-alive guarantees **playback continuity only**. Cursor, scroll, open
  overlays, in-flight search and the queue undo stack do not survive a client closing. Users
  of today's relay-backed stay-alive lose that on-screen continuity. Per-library browse
  position, `prefs.json` scalars, and `queue_state.json` continue to persist as they do now.
- **BREAKING**: the ctrl protocol version is bumped so a mismatched `mbv` and `mbvd` refuse
  each other at the hello handshake instead of failing obscurely mid-session on an unknown
  `DisconnectReason`. `mbv` and `mbvd` must be upgraded together.
- Clients no longer exit silently when the socket dies. A deliberate shutdown is announced by
  a new `DisconnectReason` and clients exit cleanly; an unannounced socket death raises a
  blocking modal offering restart-and-resume, restart-without-resume, or quit.
- The tray moves out of `App::run()` into the local daemon via the existing
  `DaemonRuntimeHooks::on_tray_ready` hook. Packaged `mbvd` keeps its no-op stub.
- The system-audio visualizer keeps working for clients of a same-host local daemon. CAVA reads
  the default sink monitor and has no connection to mpv, so a local daemon's playout is exactly
  as capturable as in-process playback.
- The single-instance flock survives, repointed from `mbv-relay.sock` to `mbv-ctrl.sock`.
  Clients take no lock, which is what allows several of them.
- `src/relay.rs`, `src/terminal_client.rs`, the `--__relay` hidden subcommand, and
  `src/app/stay_alive.rs` are deleted.

## Capabilities

### New Capabilities
- `local-daemon-stay-alive`: stay-alive self-spawns a hidden local-daemon subcommand that owns
  the Player; ensure-exists-then-attach semantics; authenticate-before-spawn ordering; a client
  exiting never stops the daemon.
- `local-daemon-thin-client`: terminals run the full TUI as clients over `DaemonEndpoint::Local`;
  playback continuity without session continuity; exactly what resets and what persists.
- `local-daemon-single-instance`: the flock repointed to `mbv-ctrl.sock`; the Fresh / Attach /
  Refuse trichotomy; clients take no lock; the Refuse message carries a real remedy; `mbv -q`.
- `local-daemon-tray`: the tray is owned by the local daemon through `on_tray_ready`; packaged
  `mbvd` keeps the stub; headless behavior is documented, not warned about.
- `daemon-disconnect-handling`: deliberate-shutdown broadcast versus unannounced crash; the
  three modal options; restart arbitration through the existing flock.

### Modified Capabilities
- `ctrl-protocol`: the protocol-version requirement is bumped and gains a shutdown
  `DisconnectReason` variant that clients must treat as an expected, connection-closing event.
- `system-audio-visualizer`: `Unsupported playback paths remain unchanged` is narrowed so a
  same-host local-daemon session is a supported visualizer path rather than being lumped in
  with genuinely remote playback.

## Impact

Deleted: `src/relay.rs` (580 lines), `src/terminal_client.rs` (322 lines),
`src/app/stay_alive.rs` (82 lines), the `--__relay` parsing in `src/main.rs`, and the
relay-attach plumbing that hangs off `stay_alive_ctrl` (`src/app/consume_quit_actions.rs`,
`src/app/mod.rs`, `src/app/render_cadence.rs`, `src/app/render/chrome_status.rs`, and their
tests).

Repointed or amended: `src/single_instance.rs`, `src/main.rs` startup ordering,
`src/tray.rs` wiring, `crates/mbvd/src/main.rs` (stub retained, documented),
`crates/mbv-core/src/ctrl.rs`, `crates/mbv-core/src/remote_player.rs`,
`crates/mbv-core/src/daemon_core.rs`, `src/app/visualizer.rs`, `src/app/construct.rs`,
`src/app/queue_actions.rs`, `src/app/run_loop_events.rs`.

Documentation: ADR 0005 is superseded, ADR 0006 is amended in place, and a new ADR 0015
records the local-daemon stay-alive decision.

Dependencies: `mbv` and `mbvd` ship from the same workspace and are versioned together, so the
protocol bump costs a lockstep upgrade and nothing more. No new external dependencies.
