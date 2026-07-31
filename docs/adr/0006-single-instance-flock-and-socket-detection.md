# Single-instance via flock + socket connectability

## Decision

mbv enforces single-instance (Discord-style, always on and independent of
stay-alive) and detects an alive stay-alive session using an **advisory
`flock`** as the stale-proof authority, plus **socket connectability** to
disambiguate the two "a lock is held" cases.

- The **Player-owning app** holds an advisory `flock` at
  `$XDG_RUNTIME_DIR/mbv.lock`. The kernel **auto-releases it on any process
  death** (clean exit, panic, `kill -9`), so a held lock *always* means a live
  app — there is no stale-lock case to reason about.
- Startup does a **non-blocking `flock`**:
  - acquired → no session exists → **start fresh** (keep the lock);
  - would-block → a live app exists → connect to the `mbv-ctrl.sock` socket to
    decide:
    - **connect succeeds** → an alive local daemon → **attach** as a client,
      non-evicting;
    - **connect refused / socket absent** → a bare foreground TUI owns the lock
      (no local daemon) → **refuse** with an informative message.
- Detection **never trusts socket-file existence** — only a successful connect
  counts, so a stale socket left by a `kill -9`'d local daemon can't produce a
  false "alive".
- The app writes its **PID into the lock file** after acquiring it; `mbv -q`
  reads it and `SIGTERM`s the app for a graceful, non-interactive shutdown.
- **Thin clients** (explicit `--connect-daemon` / configured endpoint to an
  `mbvd`) own no Player and take **no flock**; stay-alive does not apply to
  them.

## Considered options

- **flock + socket connectability (chosen).** Stale-proof by construction;
  cleanly separates reattach from refuse.
- **pidfile-based liveness (rejected).** A pidfile survives `kill -9`, so it
  needs stale detection / PID reuse handling the flock gets for free.
- **Socket-file-existence checks (rejected).** A leftover socket file causes a
  false "alive" and a failed reattach.

## Consequences

- This supersedes the old `mbv -d` / `--daemon-inner` self-spawn and
  `daemon_mode_on_exit`. **Update ([ADR 0015](0015-local-daemon-for-stay-alive.md)):**
  this ADR originally said there was no implicit local-daemon auto-attach.
  That is now reversed. The would-block branch above (`Resolution::Attach`)
  *is* an implicit auto-attach: any launch that finds the lock held and the
  socket connectable attaches to whatever local daemon is already running,
  regardless of which invocation spawned it. The only thing that stays
  explicit is spawning a daemon that isn't there yet, which stay-alive (`-d`)
  now does as its own bootstrap step, ahead of this same flock probe.
