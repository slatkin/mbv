## Context

See `proposal.md` for motivation and `specs/player-target-locality/spec.md` for the behavior
contract. `App` currently stores a mutable `is_local_daemon` boolean separately from the
`PlayerProxy`. Constructors and target-switch paths project a `DaemonEndpoint` to this boolean,
while restoration and restart paths assign literals. The app also stores
`home_is_local_daemon`, an immutable launch-time fact with different semantics.

The desired equivalence between the player representation and endpoint state is
`player.is_remote() == player_endpoint.is_some()` for active and disconnected proxies alike. A
`Unix` endpoint remains non-local because `DaemonEndpoint::is_local()` recognizes only `Local`.

Issue #423 assumes a preceding predicate-unification change has moved both copies of
`!player.is_remote() || is_local_daemon` behind `player_owner_is_on_this_machine()`. That helper is
not in the current tree, so implementation must satisfy that prerequisite before replacing its
inputs.

## Goals / Non-Goals

**Goals:**

- Make the endpoint the single source of truth for current player locality.
- Make target transitions establish the player proxy and endpoint as one coherent state change.
- Preserve all existing locality classifications except the documented stale boolean after an
  in-process player is restored.
- Assert the representation invariant across each transition family.

**Non-Goals:**

- Changing `home_is_local_daemon` or deriving launch identity from the current target.
- Reclassifying Unix endpoints as local.
- Removing `PlayerProxy::is_remote()` or making `PlayerProxy` own endpoint metadata.
- Changing protocol, configuration, persistence, queue semantics, or route selection beyond the
  stale-locality correction required by the spec.
- Correcting the startup-only `maybe_restore_queue_state` comment or reachability assumptions.

## Decisions

### Store `Option<DaemonEndpoint>` on `App`

Replace `is_local_daemon: bool` with `player_endpoint: Option<DaemonEndpoint>`. `None` denotes an
in-process player; `Some(endpoint)` denotes a remote `PlayerProxy` connected to or originating from
that endpoint. Keep `home_is_local_daemon` as the immutable launch snapshot.

This representation preserves enough information to derive every existing predicate and makes a
forgotten transition visible through invariant tests. Keeping only the boolean was rejected because
it retains the synchronization problem. Moving endpoint ownership into `PlayerProxy` was rejected
as a larger cross-crate abstraction change.

### Derive both locality predicates in the existing state-helper module

Add `is_local_daemon()` and update the prerequisite
`player_owner_is_on_this_machine()` helper alongside `remote_slot_state()` in
`remote_slot_state.rs`:

```rust
fn is_local_daemon(&self) -> bool {
    matches!(self.player_endpoint, Some(DaemonEndpoint::Local))
}

fn player_owner_is_on_this_machine(&self) -> bool {
    !matches!(
        self.player_endpoint,
        Some(DaemonEndpoint::Tcp(_) | DaemonEndpoint::Unix(_))
    )
}
```

This intentionally treats `Unix` as non-local, matching `DaemonEndpoint::is_local()` and existing
behavior rather than physical socket location. Deriving same-machine ownership from
`PlayerProxy::is_remote()` plus a separate flag was rejected because it would preserve two sources
of truth.

### Thread complete endpoints through constructors and switches

`run_remote_app` and `App::new_remote` take a `DaemonEndpoint`; construction derives launch-only
decisions with `endpoint.is_local()` and stores `Some(endpoint)`. Bare construction stores `None`.
Test and rendering helpers that previously passed `false` use a fixed TCP endpoint so their
classification stays unchanged.

Direct-remote and library-route switch functions take `&DaemonEndpoint` and clone it into app state
after connection succeeds. Their callers already hold the endpoint, so no endpoint reconstruction
is needed. Local-daemon restoration and restart store `Some(DaemonEndpoint::Local)` only after a
successful connection.

Passing the endpoint by reference at switch boundaries avoids moving values callers still need;
storing the clone is acceptable because endpoints are small and transitions are infrequent.

### Clear endpoint state when an in-process player is restored

When `restore_local_mode` reinstates `suspended_local`, set `player_endpoint = None`. This fixes the
known stale state where a bare-mode app can restore its own player while still presenting
local-daemon-only status and queue behavior. Keep this edit and its regression assertion in a
separately labelled implementation task so review can distinguish it from mechanical field
replacement.

If reconnecting a local-daemon baseline fails, retain the endpoint belonging to the disconnected
remote proxy rather than assigning `Local` or `None`: no new target was established, and retaining
`Some(previous_endpoint)` preserves the representation invariant without falsely claiming a
successful local-daemon reconnect. Existing disconnect/error handling remains responsible for the
unavailable proxy. This is intentionally different from the suspended-local success branch, where
an actual in-process player has been restored.

### Verify the representation invariant at transition boundaries

Add focused assertions that `player.is_remote() == player_endpoint.is_some()` after bare and remote
construction, both route-switch methods, restoration of a suspended in-process player, restoration
to the local-daemon baseline, failed baseline reconnection, and local-daemon restart. Existing
lifecycle, auto-reconnect, route-state, session-connect, and daemon-bootstrap coverage remains the
behavioral regression suite.

## Risks / Trade-offs

- [A transition updates the proxy but not the endpoint, or vice versa] -> Centralize derived reads
  and assert the invariant after every transition family.
- [The refactor accidentally treats Unix sockets as local] -> Specify and directly test the current
  Unix classification.
- [Current-target state is confused with launch identity] -> Leave `home_is_local_daemon` unchanged
  and keep launch-sensitive reads on that field.
- [The stale-state correction is hidden in a broad mechanical diff] -> Implement and test the
  suspended-local endpoint clear as a distinct task suitable for a separate commit.
- [A failed local-daemon restoration is represented as an in-process player] -> Preserve the
  previous endpoint while its disconnected remote proxy remains installed.

## Migration Plan

No persisted data or wire migration is required. Implement the predicate-unification prerequisite,
then land the representation changes and stale-state correction in reviewable steps. Rollback is a
code revert; no user data needs conversion.
