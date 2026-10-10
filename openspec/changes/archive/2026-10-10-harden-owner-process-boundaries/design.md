# Design

## Context

See proposal.md, "Why". The facts below shape the approach:

- The Local owner and packaged `mbvd` both start through
  `mbv_daemon::run_with_options` → `start_daemon` (`crates/mbv-daemon/src/run.rs:196`). That
  function writes `pid_file()` = `data_dir_system_or_local()/mbv.pid` for **both** roles. For the
  Local role the path is `~/.local/share/mbv/mbv.pid`.
- `contrib/mbvd.service` runs mbvd as `User=root` with `Environment=MBV_SYSTEM=1`, so the packaged
  daemon writes `/var/lib/mbv/mbv.pid`. The mbvd actions `--connect emby`, `--connect abs` and
  `--disconnect abs` set `MBV_SYSTEM=1` themselves (`crates/mbvd/src/main.rs:294,346,405`).
  `--quit` does not, so from a desktop shell it reads the Local owner's PID file.
- The Local owner already holds an exclusive `flock` on `single_instance::lock_path()` for its
  whole life and writes its PID there (`src/single_instance.rs`, ADR 0006). `terminate_owner`
  reads that PID without checking the lock.
- `SocketStream` (`crates/mbv-net/src/stream.rs`) is an enum over `UnixStream` and `TcpStream`.
  It has no timeout methods. In `CtrlClientSession::run` (`core_ctrl_spawn.rs`) the same
  `BufReader` reads the hello and then every later command line.
- A failed level warm-up already becomes an empty `AlbumArtistLevelFetched`. The handler
  (`src/app/dispatch/library/event.rs:199`) removes it from `level_artist_warmups_in_flight`
  and marks it `LevelFillState::Failed`, which can be retried. The only thing missing is a
  deadline that makes the request return.
- The Emby client uses a 5 s connect timeout and a 30 s global timeout
  (`crates/mbv-emby/src/client_auth.rs:39-40`).

## Goals / Non-Goals

**Goals:**
- Never send a signal unless a live process holds the lock that the PID belongs to.
- Keep exactly one PID record for each Owner role.
- Check the runtime directory once, at the process boundary. The path getters stay infallible.

**Non-Goals:**
- No change to the ctrl wire protocol or its version. The issue's "Related" items (protocol
  version 11 vs 10 mismatch, `eprintln!` in signal handlers, FIFO mode, cache eviction) are out
  of scope.
- No bounded `mpsc` channel for ctrl outbound queues. The write timeout bounds how long an
  unbounded queue can grow.
- No hard bound (`run_with_hard_bound`) around the warm-up request. ureq's global timeout
  matches what every other Emby request has.

## Decisions

### D1. A shared owner-lock module in `mbv-daemon`

New module `crates/mbv-daemon/src/owner_lock.rs` (exported from `mbv-daemon`). Both binaries
already depend on `mbv-daemon`, and the crate already has `nix` (`fs`) and `libc`. `core.rs` is
767 lines, so the code goes in a new file, and `pid_file()` moves there with it.

```
locked_owner_pid(path) -> Option<u32>
    open path read-only (never create)            -- missing file  -> None
    flock(LockExclusiveNonblock)
        Ok        -> drop lock, None               -- nobody holds it: stale or empty
        EWOULDBLOCK -> read PID from file, Some(pid)
        other err -> None

signal_owner(path) -> Result<u32, SignalOwnerError>
    pid = locked_owner_pid(path) ok_or NoOwner
    libc::kill(pid, SIGTERM) -> Ok(pid) | Err(Signal { pid, io::Error })
```

`locked_owner_pid` is the testable part. Its tests hold a real `flock` on a temp file and never
send a signal. `signal_owner` is a three-line wrapper with no test of its own.

`SignalOwnerError` is a hand-written domain error (`Display` + `Error`, no `thiserror`). It
replaces `single_instance::TerminateOwnerError`, keeping the same two variants and messages so
the user-visible `mbv -q` text does not change.

*Alternatives:* (a) Check `/proc/<pid>/exe` or `comm` before the kill. That is Linux-only, can
race, and is wrong after an in-place upgrade. Rejected, because the lock is already the
liveness truth for the Local owner (ADR 0006). (b) Have `mbvd --quit` send a ctrl request over
the socket. It needs the control credential and a new admission path for a CLI, which is too
much for a fix. Rejected.

### D2. The packaged daemon holds the lock; the Local owner writes no `mbv.pid`

`start_daemon` writes the PID file only when `role == DaemonRole::Packaged`. It then holds a
`PidFileLock` guard (a `Flock<File>` with the PID written after the lock is acquired, as in
`single_instance::LockGuard::write_pid`) for the life of the run. The existing `remove_file`
on shutdown (`run.rs:652`) stays, guarded by the same role check. If the lock cannot be taken,
the daemon refuses to start. mbvd's `daemon_running()` becomes
`owner_lock::locked_owner_pid(&pid_file()).is_some()`, which removes the `/proc` existence test
that a reused PID fools.

For the Local role, `mbv.pid` was a second record of what the single-instance lock file already
holds. Removing it removes the collision at its source, so the guard is needed only for
Packaged. An old `~/.local/share/mbv/mbv.pid` left by earlier builds is harmless: nothing reads
it any more except a non-system `mbvd --quit`, and that finds no lock holder.

### D3. `mbvd --quit` uses system-instance paths and `signal_owner`

`--quit` sets `MBV_SYSTEM=1` like the other administrative actions (same `SAFETY` comment). It
then calls `owner_lock::signal_owner(&pid_file())`. It no longer runs `kill` and no longer
deletes the PID file itself, because the exiting daemon does that. The flock probe opens the
file read-only. A non-root user can therefore still probe `/var/lib/mbv/mbv.pid` (mode 0644)
and gets a `Signal { EPERM }` error naming the PID, which is the right message.

### D4. Per-user runtime fallback, checked once at startup

`paths::runtime_dir()` stays infallible and pure:
`/run/mbv` (system) → `$XDG_RUNTIME_DIR` → `/tmp/mbv-<uid>` (uid from `libc::getuid()`, adding
the `libc` dependency to `mbv-config`). A new public `ensure_runtime_dir() -> Result<(),
RuntimeDirError>` does nothing when `XDG_RUNTIME_DIR` is set or the instance is a system
instance. Otherwise it calls `create_dir` with mode 0700 (an `AlreadyExists` error is fine) and
then `check_private_dir(path, uid)`:

```
symlink_metadata(path)         -- lstat: never follow a planted symlink
  is_symlink         -> Err(Symlink)
  !is_dir            -> Err(NotADirectory)
  uid() != uid       -> Err(ForeignOwner)
  mode & 0o077 != 0  -> Err(Permissive)
```

`check_private_dir` takes `uid` as a parameter, so a unit test can cover the foreign-owner case
by passing `getuid() + 1`, without root. `RuntimeDirError` names the path and the reason.

Call sites: the start of `main` in `src/main.rs` (before the `-q` branch and before
single-instance resolution), `run_local_daemon_main` in `src/local_daemon.rs`, and the mbvd
`Serve` path. Each prints the error and exits non-zero. Clients check too: a client that
connects to a socket in a directory another user controls would send its control credential
to an impostor.

`src/single_instance.rs::runtime_dir()` is deleted. `lock_path()` becomes
`mbv_config::owner_lock_path()`, a new sibling of `control_socket_path()` in `paths.rs`, so
there is one rule.

*Alternative:* return `Result` from every path getter. That forces edits at every socket and
lock call site for a check that only has to pass once. Rejected.

### D5. Ctrl socket timeouts: a hello deadline, then a permanent write timeout

`SocketStream` gains `set_read_timeout` and `set_write_timeout`, forwarding to the inner stream.
In `spawn_ctrl_client`:

```
stream.set_write_timeout(Some(CTRL_WRITE_TIMEOUT = 30s))   // shared socket option: also covers writer clone
stream.set_read_timeout(Some(CTRL_HELLO_DEADLINE = 10s))
session.run:
  reader = BufReader::new(stream)
  reader.read_line(&mut hello)       -- timeout/EOF/error -> return
  reader.get_ref().set_read_timeout(None)   -- admitted clients may be idle forever
  for line in reader.lines() { ... }        -- unchanged
```

The reader timeout must be cleared after the hello. Commands arrive only on user action, so a
permanent read timeout would disconnect every idle TUI. That is the trap in the issue's
"two lines" fix.

The write timeout is a socket option, so it applies to the writer thread's `try_clone`d handle
as well. When `writeln!` times out, the writer breaks out of its loop and calls `shutdown()`.
The reader then sees EOF and sends `CtrlDisconnected`, and `retain_clients` drops the dead
sender on the next broadcast. All of that cleanup path exists today.

30 s rather than the issue's 5 s: the first queue-state snapshot to a client on a slow TCP link
can be large, and a healthy client's reader thread drains continuously. 10 s for the hello is
generous for any real client, which sends its hello right after connecting.

### D6. Warm-up request bounds match the Emby client

`level_warmup.rs` uses `native_tls_agent(Emby, Some(5s), Some(30s))` and reads the body as
`into_body().into_reader().take(64 MiB)` → `serde_json::from_reader`. 64 MiB is far above any
real reply (about 300 bytes per item with these `Fields`, times `Limit=100000`, is about 30 MB)
and still bounds memory. A truncated or timed-out body fails to parse, which becomes an empty
result, then `Failed`, which can be retried.

### D7. One state directory

Delete `src/main.rs::state_dir()`. Replace the `crate::state_dir()` callers (`main.rs`
`crash_log_path` and the applog file path, `local_daemon.rs:236`, `player_event.rs:524`) with
`mbv_config::state_dir()`. `write_crash_log` (and the `pin.rs:358` opener) call
`create_dir_all` on the parent before `OpenOptions`, ignoring that error as the open already
does.

## Risks / Trade-offs

- [A legitimately slow peer is dropped by the 30 s write timeout] → It reconnects through the
  existing reconnect path. A peer that has not read for 30 s is not usable.
- [Users with an existing world-readable `/tmp/mbv-<uid>`] → mbv refuses to start with a message
  naming the directory. Nothing created it before this change, so only a deliberate or hostile
  directory hits this.
- [`/tmp/mbv-<uid>` is cleaned by tmpfiles while mbv runs] → This already happens to the current
  `/tmp/mbv-ctrl.sock`. No change.
- [No automated test for the ctrl timeouts] → A test would need a real wait for a socket
  timeout, and AGENTS.md forbids that. The behavior is two kernel socket options on paths whose
  cleanup is already covered. Verified manually (see tasks).
- [Old `mbv.pid` in the user data dir] → Inert, as D2 explains. No migration.

## Migration Plan

No data migration. A running old-build Local owner keeps writing its `mbv.pid`, which a new
`mbvd --quit` no longer reads (system paths). Rollback is a plain revert.
