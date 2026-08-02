## Why

`App.is_local_daemon` duplicates information already carried by the connected
`DaemonEndpoint`, so every player-target transition must manually keep the boolean in sync.
This fragile representation has already contributed to shipped target-tracking bugs and lets a
new transition silently create contradictory app state.

## What Changes

- Replace the mutable `is_local_daemon` field with the current player's optional daemon endpoint:
  no endpoint for an in-process player, `Local` for this machine's managed daemon, and `Tcp` or
  `Unix` for other daemon connections.
- Derive local-daemon and same-machine player-owner predicates from the stored endpoint instead of
  independently maintained booleans.
- Pass endpoints, rather than projected locality booleans, through remote construction and route
  switching so each successful transition records its source of truth.
- Preserve existing endpoint classification, including the current behavior that `Unix` endpoints
  are not treated as the managed local daemon.
- Restore an in-process player with no endpoint. This intentionally corrects the stale-locality
  state that currently leaves local-daemon UI and queue behavior enabled after returning to bare
  mode; implementation should isolate this correction from the mechanical representation refactor
  for review.
- Leave the immutable launch identity `home_is_local_daemon` unchanged.

## Capabilities

### New Capabilities

- `player-target-locality`: Defines how the app classifies the current player target from its daemon
  endpoint and how that classification changes across construction, route switches, daemon restart,
  and restoration of an in-process player.

### Modified Capabilities

None.

## Impact

- Affected code: app construction, player-route transitions, local-daemon restart, endpoint-derived
  state helpers, locality-dependent UI and queue actions, and their test fixtures.
- Public protocol, configuration, and persistence formats are unchanged.
- `DaemonEndpoint` values are retained in app state and cloned at transition boundaries instead of
  being immediately reduced to booleans.
- Implementation is sequenced after extraction of the shared
  `player_owner_is_on_this_machine()` predicate assumed by issue #423; that prerequisite is not yet
  present in the current tree.
