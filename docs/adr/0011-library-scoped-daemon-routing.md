# Library-Scoped Daemon Routing

## Decision

Daemon routing can be decided per library via a `[daemon_routes]` config
table (library name, case-insensitive -> daemon endpoint string, plus an
optional `"*"` wildcard). Resolving a play/enqueue action to a routed
library swaps the active player to that daemon; other libraries keep
playing locally, unaffected.

Route resolution is queue-level, not per-track: a queue's route is decided
once from the item(s) that started it and held for that queue's lifetime.
Enqueuing an item whose resolved route differs from the queue's current
route is rejected with a toast, not auto-swapped or auto-cleared. Starting
a new queue (replace) re-evaluates the route from scratch.

Route resolution order:
1. Nav-scoped views (Library tab, Power View, Album/Artist drill-down,
   in-library search) resolve the active library directly from
   navigation state -- no network call.
2. Cross-library aggregate views (Home tab: Continue Watching/Next Up,
   Favorites) resolve via `EmbyClient::get_ancestors`, walking the
   item's ancestor chain to its owning `CollectionFolder`. A *successful*
   lookup (whether it finds an owning library or confirms there isn't
   one) is cached per item id for the session; a *failed* lookup
   (transient error) is never cached, so it retries on the item's next
   play/enqueue attempt instead of being stuck at "no route" until the
   process restarts. `do_enqueue_folder` additionally checks whether the
   enqueued item is itself a library root before falling back to
   ancestor lookup, since a library root has no `CollectionFolder`
   ancestor above it for the lookup to find.
3. The Queue tab (no library context of its own) keeps whatever route is
   already active rather than re-resolving or clearing it.
4. No match in any case means local playback.

Connecting to a routed library's daemon **is** takeover of that daemon's
driving-client slot (ADR 0003/0007) -- an accepted, explicit consequence,
not a hidden side effect. This matters if multiple devices route to the
same music-only daemon. The connect attempt itself, including this
consequence, is delegated to `App::try_daemon_route_connect`
(ADR 0010, #222) rather than re-implemented here.

Library routing (`active_route`) is tracked independently of the
Sessions-panel direct-remote concept (`connected_session_id` /
`direct_remote_label`); they are two separate ways to end up thin-client
and must never be conflated in `App` state, even though both reuse the
same suspend/restore machinery (`SuspendedLocalSession`,
`switch_to_direct_remote`/`switch_to_library_route`,
`restore_local_mode`). Two specific conflation hazards were identified and
closed: (1) a play/enqueue action while already thin-client for a reason
*other* than library routing (Sessions-panel direct-remote, local-daemon
mode) must not let library routing swap the player out from under that
other connection; (2) a Sessions-panel direct-remote upgrade while already
on a library route must tear the library route down cleanly first --
`connect_to_session` runs `restore_local_mode` at its own top whenever
`active_route` is set, before its existing direct-upgrade attempt (itself
gated on the player not already being remote) can run. This was chosen
over a bare inline `active_route = None` write, discovered during
implementation to be dead code: `switch_to_direct_remote`'s already-remote
branch is only reachable from `connect_to_session` when the player is
*not* already remote at the point that check runs, so clearing the field
without restoring the player first would never fire in the scenario it
was meant to fix.

## Context

ADR 0010 (#222) established the connection lifecycle this depends on:
fully lazy connect (never at startup), fallback to local playback with a
status-bar warning on a failed connect via `App::try_daemon_route_connect`,
no background retry, no connection parking (disconnect cleanly on
swap-away; reconnect fresh next time). This issue (#223) reuses that
lifecycle -- calling `try_daemon_route_connect` directly, on the same
`DAEMON_ROUTE_CONNECT_OVERRIDE` test seam ADR 0010 introduced -- for
per-library routing rather than only the wildcard "route everything" case
#222 introduced.

## Consequences

- `Config` gains a `daemon_routes: HashMap<String, String>` field,
  parsed like `hidden_libraries`/`feed_view_libraries` (lowercased keys),
  but with no settings-panel write-back for v1 -- TOML-only, matching the
  `hidden_libraries` value-editing precedent without exposing this table
  for in-app editing.
- `App` gains `active_route: Option<String>` and a per-item
  `library_route_cache` for ancestor-lookup memoization (successes only).
- `restore_local_mode` is the single shared "go back to local" tail for
  both the Sessions-panel and library-route thin-client paths; it clears
  `active_route` in addition to its existing resets, and `connect_to_session`
  calls it proactively to tear down a library route before attempting its
  own direct-upgrade path.
- A malformed `daemon_routes` endpoint value is logged and skipped
  (treated as no route for that library) rather than failing startup or
  blocking other routes.
- Mid-queue per-track route swapping and connection parking/reuse across
  route switches remain explicitly out of scope, per #223.
- ADR 0010's "disconnects cleanly... reconnects fresh" rule (rule 4, which
  this routing feature depends on) was previously aspirational due to a
  `RemotePlayer` socket/thread leak on route swaps (#233 was identified but
  filed separately). (#233 is now fixed: `RemotePlayer::disconnect()` shuts
  down the shared socket both before a route-to-route (or remote-to-remote)
  swap replaces the old connection, and in `restore_local_mode`'s
  return-to-local path, so ADR 0010's framing is now accurate rather than
  aspirational.)
