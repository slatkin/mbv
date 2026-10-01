## Why

When the `mbv` binary is updated and a terminal relaunches, the new Client attaches
to the already-running local Owner process from the old build and runs silently
against it. Nothing tells the user, and the only remedy is to know to run `mbv -q`
and relaunch. The Owner's hello already carries `app_version`, but the Client logs
it and throws it away (#559, Feature 1).

(#559's Feature 2 — toggle Stay Alive off, then quit, stops the Owner — already
shipped with queue-owner-process: the daemon re-reads `stay_alive` at each lifetime
decision and quit flushes settings first. It is out of scope here.)

## What Changes

- A local Client SHALL refuse to attach to an Owner process whose `app_version`
  differs from its own. The Owner is the same binary, so application version is the
  only comparison; ctrl protocol version is an mbvd (server) concern and is not
  involved. The refusal happens on the Owner's hello, before the Client sends its
  hello: no control credential is transmitted and no client is admitted.
- Before any UI starts, the terminal SHALL prompt: **[R]** stop the Owner and
  relaunch from this binary, or **[Q]** quit. Enter alone, unknown input, EOF, or a
  non-TTY stdin means quit (exit non-zero, naming `mbv -q`). There is no
  continue-with-mismatch option.
- **[R]** sends SIGTERM to the PID in the lock file — exactly what `mbv -q` does, so
  it works across versions and regardless of `stay_alive` — then reuses the existing
  bounded retry-resolve loop to wait for the old Owner to release the lock, spawn a
  fresh Owner from this binary, and attach.
- Ctrl protocol negotiation is untouched: no protocol check is added, removed, or
  reworded, and no `CTRL_PROTOCOL_VERSION` bump is needed.
- Explicit `unix://`/`tcp://` endpoints (packaged `mbvd`) are unaffected.
- `mbv -q`'s signalling is extracted into a helper shared with the restart path;
  its behaviour and messages do not change.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `local-daemon-single-instance`: adds a requirement that a local Client never
  attaches to an Owner process from a different build, and the prompt/restart
  behaviour that follows.

## Impact

- `crates/mbv-remote-player/src/{error.rs,connect.rs,connect/tests.rs}`: new error
  kind and a same-build policy applied to the Local endpoint's server-hello read.
- `src/single_instance.rs`, `src/main.rs`: shared owner-terminate helper; new branch
  and retry helper in `run_local_instance`.
- New `src/owner_restart.rs`: prompt and follow-up decision.
- No ctrl protocol change and no `CTRL_PROTOCOL_VERSION` bump; no config change; no
  new dependency. The `ctrl-protocol` capability is not modified.
