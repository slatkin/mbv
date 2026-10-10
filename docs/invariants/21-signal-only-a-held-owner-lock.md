# Invariant 21 — A held Owner lock is the only authority to signal a PID

## The invariant

1. A PID from a file is signalled only while a live process holds that file's
   `flock`. A PID left in a file by a crashed, killed, or rebooted Owner is
   never signalled.
2. Each Owner role has exactly one PID record. The Local Owner's record is
   the single-instance lock file (`mbv.lock`); it writes no `mbv.pid`. The
   packaged daemon's record is `mbv.pid`, held under an exclusive `flock`
   for the whole run.

The enforcement site is `owner_lock::locked_owner_pid`
(`crates/mbv-daemon/src/owner_lock.rs`): it opens the path read-only (never
creating it), tries an exclusive non-blocking `flock`, and returns `Some(pid)`
only when the lock is contended (`EWOULDBLOCK`, meaning a live holder). A
missing file, an acquirable lock (stale or empty), or any other probe error
yields `None`. `signal_owner` signals only a PID that probe returned.

## Why it matters

Audit issue #915 found that mbv trusted a PID nobody holds a lock on. The
live defect: `mbvd --quit` run from a desktop shell did not set
`MBV_SYSTEM=1`, so it read the Local Owner's PID file
(`~/.local/share/mbv/mbv.pid`, which `start_daemon` wrote for both roles)
and sent SIGTERM to the user's own TUI Owner process — or to whatever process
had since reused a stale PID — while it never reached the systemd `mbvd`.
Nothing in the type system orders "check liveness" before `kill`: calling
`libc::kill` on a file-read PID compiles and passes every functional test.
The property is upheld only by routing every signal path through the
lock probe first.

## How the code upholds it today

- `locked_owner_pid` is the single liveness gate. Both `mbv -q` (via
  `mbv_daemon::owner_lock::signal_owner`) and `mbvd --quit` signal only a
  PID the probe observed under contention.
- `start_daemon` writes the PID file only for `DaemonRole::Packaged`,
  acquires the exclusive lock first, writes the PID after acquiring it, and
  keeps the `PidFileLock` guard alive for the daemon's run; a contended lock
  refuses startup. The shutdown `remove_file` is guarded by the same role
  check. The Local role writes no `mbv.pid`, which removes the collision at
  its source.
- `mbvd --quit` sets `MBV_SYSTEM=1` like the other administrative actions,
  so it resolves the system-instance PID file instead of the Local Owner's,
  and it calls `signal_owner` (`libc::kill`) instead of running `kill` or
  deleting the file itself — the exiting daemon removes its own file.
- `daemon_running()` is `locked_owner_pid(&pid_file()).is_some()`, which
  removes the `/proc` existence test that a reused PID fools.

## Where it still fails

No known violation. One known residual: an old `~/.local/share/mbv/mbv.pid`
left by earlier builds is inert — nothing reads it any more except a
non-system `mbvd --quit`, and that finds no lock holder, so the probe
returns `None` and no signal is sent. No migration removes it.
