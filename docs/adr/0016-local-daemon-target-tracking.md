# Tracking the current daemon target separately from launch mode

> **Amended (PR #426, `3359c81`).** The decision below stands: current target
> and launch mode remain two separate pieces of state. What changed is how the
> first one is represented — the mutable `is_local_daemon` boolean became
> `player_endpoint: Option<DaemonEndpoint>`, with `is_local_daemon()` derived
> from it. This ADR has been updated in place to describe the current shape;
> the manual-sync hazard described under Consequences is the thing that
> refactor removed.

## Decision

`App` tracks a thin client's relationship to its own Local daemon with two
separate pieces of state, not one, because "is my Player owner this machine's
Local daemon *right now*" and "did this process *launch* as a Local-daemon
thin client" are different questions that diverge the moment a stay-alive
client makes its first Sessions-panel or library-route connection elsewhere.

- **`player_endpoint`** (`Option<DaemonEndpoint>`, mutable): where the Player
  currently lives. `None` is an in-process player (bare mode),
  `Some(DaemonEndpoint::Local)` is this machine's own Local daemon, and
  `Some(Tcp | Unix)` is a different daemon. Set at construction and reassigned
  by every place the target can change: `switch_to_direct_remote`,
  `switch_to_library_route`, `restore_local_mode`, and
  `daemon_restart::restart_local_daemon`.

  Two predicates read it, and call sites should use them rather than matching
  on the field directly:
  - `is_local_daemon()` — the target is *specifically* this machine's Local
    daemon.
  - `player_owner_is_on_this_machine()` — the Player owner is any process on
    this machine, in-process or Local daemon. Note this is not
    `!player.is_remote()`: a stay-alive thin client is `is_remote() == true`
    from the moment it launches, despite never having left home base.

- **`home_is_local_daemon`** (`bool`, immutable): true only if the process was
  originally launched as a Local-daemon thin client. Fixed at construction,
  never updated again.

Use `player_endpoint`'s predicates for anything that must reflect where the
Player is pointed *right now*: the visualizer's system-audio capture gate
(`visualizer.rs`), whether a daemon-lost modal can offer a restart
(`player_event.rs`), the alive/heartbeat status icon (`chrome_status.rs`), and
— the proximate cause of this ADR — whether a Sessions-panel connect should
attempt a direct upgrade rather than falling back to a read-only Session watch
(`connect_to_session`, `session_connect.rs`).

Use `home_is_local_daemon` for anything that needs to know how to get back
home, or whether this process's connection lifecycle is subject to
local-daemon rules at all: `restore_local_mode`'s choice between "reconnect to
the Local daemon" and "nothing to restore, a genuinely local Player was never
suspended," and `teardown`'s auto-reconnect persistence gate, which must not
skip saving state just because a mid-session remote swap repointed
`player_endpoint`, and must never run at all for a genuinely-remote
`--connect-daemon` launch (a separate mechanism, ADR 0010), which never sets
`home_is_local_daemon` in the first place.

## Considered options

- **A single mutable flag (the design prior to this one, rejected).** Before
  the current-target state was retained on `App` at all, it existed only as a
  constructor argument, used once and discarded. Adding it back as one mutable
  field would have made "current target" and "launch mode" the same question
  by construction — correct only until the first mid-session swap, at which
  point any consumer that actually needed the launch-time answer (chiefly
  `teardown`'s persistence gate) would start acting on stale current-target
  state instead.

- **Keeping the current target as a `bool` (the original form of this ADR,
  since replaced).** Every producer of that boolean already held the
  `DaemonEndpoint` it had just connected to and projected it down with
  `endpoint.is_local()`, so the bool carried no information of its own while
  adding a value that had to be manually reassigned at every transition.
  Storing the endpoint and deriving the predicates removes that duty: a new
  route-switch path cannot forget to update a derived value. PR #426 made this
  change; issue #423 has the full rationale.

## Consequences

This split is easy to get right at each individual call site and easy to get
wrong across the codebase as a whole, because the two are in agreement for the
overwhelming majority of a stay-alive session's lifetime — they only disagree
during the (comparatively rare) window where a client has swapped away from
its Local daemon to a genuinely different target. Two incidents came from
exactly this seam:

- The teardown/auto-reconnect fix (`2c6f9d3`, "restore auto-reconnect for
  local-daemon stay-alive clients") wired `try_auto_reconnect` into the
  Local-daemon launch path and gated `teardown`'s persistence on
  `home_is_local_daemon` rather than the current-target state — using the
  latter there would have skipped saving state for any session that had
  swapped remote mid-session, silently discarding a real reconnect target.
- `connect_to_session`'s direct-upgrade guard checked `!player.is_remote()`
  alone. Since a Local-daemon thin client is `is_remote() == true` from the
  moment it launches — despite never having left home base — this
  unconditionally skipped the direct-upgrade attempt for every stay-alive
  client, regardless of the `stay_alive` config value, and silently fell back
  to session-watch-only.

Both were failures to keep a manually-maintained boolean in step with reality.
Deriving the predicates from `player_endpoint` (see Considered options) closes
that class of bug: there is no longer a value that can drift, only a target
that is recorded when it changes. The two-question split itself remains, and
still has to be reasoned about.

Future call sites should ask: does this decision depend on where the Player is
pointed right now, or on what this process's connection lifecycle rules are
for its whole life? The former wants `is_local_daemon()` or
`player_owner_is_on_this_machine()`; the latter wants `home_is_local_daemon`.
When unsure, `visualizer.rs`'s capture gate is the clearest existing example
of the first kind of decision, and `teardown`'s persistence gate the clearest
example of the second.
