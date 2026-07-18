# Lazy Daemon Route Connect: Fallback, No Retry, No Parking

## Decision

A daemon-route connect attempt (the general "route everything" wildcard case
in #222, and the per-library case #223 builds on top of it) follows four
rules, distinct from the existing startup-only `--connect-daemon` /
`daemon_client_endpoint` thin-client path (`main.rs`'s `explicit_daemon_endpoint`
branch), which is unaffected by this ADR:

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
2. **Fallback, not hard-fail.** A failed connect attempt falls back to (or
   stays on) local playback. It never hard-fails or exits the process. The
   raw failure is always logged (`log::warn!`, `target: "daemon_route"`).
   `App::try_daemon_route_connect` returns a fully-formatted, ready-to-
   display status-bar warning as its `Err` payload rather than flashing it
   directly -- the caller decides how to surface it (a direct
   `App::flash_status_high`, or threaded through a state-teardown path),
   since the right *mechanism* depends on caller-side state this primitive
   deliberately knows nothing about (e.g. #223's `active_route`). Every
   call site is still required to surface it somehow: `Result`'s `Err` arm
   must be handled, so the warning can't be silently dropped by omission.
3. **No background retry.** A failed attempt schedules nothing further. The
   next attempt happens only on the next natural trigger — the next
   play/enqueue action that resolves to that route (in practice, for the
   wildcard case, the next mbv restart).
4. **No connection parking.** When mbv swaps away from a daemon route back
   to local, it disconnects cleanly — the `RemotePlayer` is dropped, not
   parked the way `SuspendedLocalSession` parks a local `Player` during a
   direct-remote takeover. The next time that route is needed, it
   reconnects fresh.

Connecting to a daemon route **is** taking that daemon's driving-client
authority (ADR 0003, ADR 0007) — an accepted, explicit consequence, not a
hidden side effect. This matters most for rule 4: a daemon route connection
is not something mbv should hold open "just in case," because doing so
continues to occupy that daemon's single ctrl-connection slot (ADR 0003)
even while the route sits idle.

## Context

Issue #222 replaced mbv's only pre-existing daemon-connect failure mode —
`explicit_daemon_endpoint`'s hard error + `std::process::exit(1)` — with a
resilient model for a *new*, separate mechanism: routing ordinary play
actions to a configured daemon rather than the local `Player`. That new
mechanism needed its own connect-timing and failure-handling rules, because
copying the explicit-endpoint path's hard-fail-at-startup behavior would be
wrong for a mechanism that, per #223, may have many possible routes (one per
library) rather than one fixed endpoint decided once at launch.

The four rules above were settled in the design that produced #222 before
this ADR was written; #222's issue body treats them as already-decided
scope, not open questions. This ADR exists to give them a durable home
outside a single issue body, mirroring why ADR 0003 exists (see that ADR's
"Why this ADR exists" section) — so a future doc edit cannot quietly narrow
or reverse them without leaving a trace.

Implementation: `App::try_daemon_route_connect` /
`App::connect_daemon_route_endpoint` in `src/app/mod.rs` (issue #222). Two
production call sites now exist: #223's `apply_route_for_playback` (the
per-library swap, a sibling to `switch_to_direct_remote` /
`restore_local_mode`, wiring the actual play/enqueue trigger), and #236's
`try_auto_reconnect` (restoring the last remote connection at startup).
Both methods carried a scoped `#[allow(dead_code)]` until #223's call site
landed (see the doc comments on each in `src/app/mod.rs`) -- this repo's
"fix all compile warnings, never `#[allow(unused)]`" convention
(`mem:conventions`) was deliberately overridden in that narrow,
self-documenting case, not silently worked around.

## Consequences

- `App::try_daemon_route_connect` is the one place connect/fallback/
  no-retry logic and warning-message formatting live; #223's per-library
  swap function must call it and surface its `Err(message)` rather than
  re-implementing connect/fallback logic or re-deriving the warning
  wording inline. The primitive does not flash the warning itself --
  #223's swap function owns *where* it gets displayed (see rule 2 above),
  but not *what* it says.
- A daemon-route `RemotePlayer` is never stored anywhere that outlives the
  swap that created it (no new `SuspendedLocalSession`-style parking
  struct for daemon routes). #223's swap-back path should simply let the
  `RemotePlayer` drop.
- Multiple devices routing to the same music-only `mbvd` (the scenario
  #223's design doc calls out) will see ordinary ADR 0003 eviction
  behavior — the newest connecting client takes over, the previous one is
  evicted with the existing structured disconnect event. This ADR does not
  change that; it only makes explicit that *initiating* a route connect is
  choosing to trigger it.
- The existing bounded intra-attempt retry loop for `DaemonEndpoint::Local`
  in `DaemonEndpoint::connect_stream` (`crates/mbv-core/src/remote_player.rs`,
  `LOCAL_DAEMON_CONNECT_RETRY_TIMEOUT`/`LOCAL_DAEMON_CONNECT_RETRY_INTERVAL`)
  is unrelated to rule 3 above and is unaffected: it waits out a
  same-machine daemon's own startup race *within* one connect attempt, not
  a retry *after* a whole attempt already failed.

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
