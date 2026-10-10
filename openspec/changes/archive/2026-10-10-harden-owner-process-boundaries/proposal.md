# Proposal

## Why

Audit issue #915 found five defects where mbv trusts something it does not own: a PID nobody
holds a lock on, a `/tmp` path every user shares, a ctrl peer that never speaks or never reads,
an Emby request with no deadline, and a second copy of the state-directory rule. The worst one
is live today: `mbvd --quit` typed in a shell reads the *Local owner's* PID file and sends
SIGTERM to the user's own TUI Owner process (or to whatever process now has a stale PID), while
it never reaches the systemd `mbvd`.

## What Changes

- **Signal only a lock holder.** `mbv -q` and `mbvd --quit` send SIGTERM only when a
  non-blocking `flock` probe shows that a live process holds the owner lock. A PID left in a
  file by a crashed, killed, or rebooted owner is never signalled.
- **One PID record per Owner role.** The packaged daemon holds an exclusive `flock` on
  `mbv.pid` for its whole life. The Local owner stops writing `mbv.pid`: its single-instance
  lock file already records its PID. `mbvd --quit` resolves system-instance paths, as the other
  `mbvd` administrative actions already do, and uses `libc::kill` instead of running `kill`.
- **Private per-user runtime directory.** Without `XDG_RUNTIME_DIR`, the runtime directory is
  `/tmp/mbv-<uid>`, created with mode `0700`. An existing directory is used only when it is a
  real directory (not a symlink), owned by the current uid, and has no group or other
  permissions. Otherwise mbv and mbvd stop at startup with an error. `single_instance` loses
  its own copy of the rule.
- **Bounded ctrl connections.** The daemon gives an accepted ctrl connection a deadline for its
  hello and a write timeout for its whole life. A silent or stalled peer is released, and its
  outbound queue stops growing.
- **Bounded album-artist warm-up request.** The level warm-up Emby request gets the same
  connect and global timeouts as the Emby client, and a cap on its response body. A request
  that does not complete marks the level `Failed`, which can be retried, instead of leaving it
  `Loading` for the rest of the session.
- **One state directory.** `mbv` writes `mbv.log` and its crash line under
  `mbv_config::state_dir()`, the same directory as the rest of its state, and creates that
  directory before it writes the crash line.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `local-daemon-single-instance`: the owner lock lives in a private per-user runtime directory;
  `mbv -q` signals only a held lock.
- `packaged-daemon-service-runtime`: the packaged daemon holds a lock on its PID file, and
  `mbvd --quit` stops only that daemon.
- `ctrl-protocol`: accepted local shutdown no longer names a PID file (the Local owner has
  none).
- `daemon-multi-connection`: silent and stalled ctrl connections are released.
- `stable-music-library-grouping`: a warm-up request that does not complete in time fails and
  can be retried.
- `application-logging`: `mbv`'s log and crash line live in the process's state directory.

## Impact

- `src/single_instance.rs`, `src/main.rs`, `src/local_daemon.rs`, `src/pin.rs`,
  `src/app/dispatch/session/player_event.rs`
- `crates/mbvd/src/main.rs`
- `crates/mbv-daemon/src/{core.rs,run.rs,core_ctrl_spawn.rs}`, and a new owner-lock module in
  `mbv-daemon`
- `crates/mbv-net/src/stream.rs` (timeout forwarding on `SocketStream`)
- `crates/mbv-config/src/paths.rs` (adds the `libc` dependency for `getuid`)
- `src/app/infra/image_fetch/fetch/level_warmup.rs`
- `docs/invariants/`: new entry for the signal-only-a-held-lock rule
- No ctrl protocol version or wire change.
