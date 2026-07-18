# Route / Sessions Panel Seam — Design

**Status:** Approved by user, ready for implementation planning.

**ADR:** 0012, reserved 2026-07-18.

**Context:** Issue #249 exposed a deeper ownership bug in library routing. Route-owned playback uses the same remote networking machinery as F3 direct session control, but F3's disconnect path currently treats any remote player with a remote queue as disconnectable. That lets F3 `d` clear `active_route` even though F2 and `config.toml` still say the library route exists.

## 1. Decision

F3 controls Sessions-panel connections only. A Sessions-panel connection is created by selecting a discovered session in F3. A library route is persistent playback policy for a library; it may create a remote daemon transport internally, but that transport is route-owned and is not an F3 session.

Therefore F3 `d` disconnects only an active Sessions-panel connection. If playback is currently route-owned and no Sessions-panel connection is active, F3 `d` leaves playback and route state untouched and flashes:

```text
No session connected
```

The status pill can continue to show `route:music`; that is enough context for a user who has configured routes.

## 2. Ownership Rule

Ownership is determined by the user action that created the relationship, not by endpoint identity.

- Selecting a discovered session in F3 creates a Sessions-panel connection.
- Starting playback from a routed library creates route-owned transport.
- If the configured route's target device also appears in F3, the route itself still is not listed as a session.
- If the user selects that same device from F3, the resulting connection is Sessions-panel-owned, even if it points to the same daemon the route would use.

This preserves the existing asymmetry: F3 `Enter` is allowed to supersede an active route because the user explicitly selected a session. F3 `d` cannot supersede a route because it only disconnects the active Sessions-panel connection.

## 3. Implementation Shape

Keep `restore_local_mode` broad. It is still the correct teardown primitive for route-to-local transitions, route failure fallback, and explicit F3 connect takeover.

Narrow the callers instead:

- Stop using display-oriented `RemoteSlotState::DirectRemote` as F3 disconnect authority.
- Derive F3 disconnect eligibility from Sessions-panel-owned state only.
- Leave route-owned playback out of `can_disconnect_remote()` / `disconnect_remote()` unless a real Sessions-panel connection is active.
- If no Sessions-panel connection is active, `disconnect_remote()` should flash `No session connected`.

`RemoteSlotState` may remain useful for rendering, including showing `route:music`, but it must not be the source of truth for what F3 may disconnect.

## 4. Documentation Updates

`CONTEXT.md` should define:

- **Sessions-panel connection**: a playback/control relationship created from F3 by selecting a discovered session.
- **Route-owned transport**: remote daemon transport created as an implementation detail of a library route.
- **Library route**: persistent playback policy, not a session.

ADR 0012 records the big design decision: F3 owns only Sessions-panel connections, not all remote transports.

## 5. Verification

Runtime verification matters more than isolated unit tests for this bug.

Manual repro check:

1. Configure `music` in `[library_routes]`.
2. Start playback from Music and confirm the status pill shows `route:music`.
3. Open F3 and press `d`.
4. Confirm playback remains routed, `active_route` behavior is preserved, and the UI reports `No session connected`.
5. Separately, select a real discovered session from F3, then press `d`, and confirm that Sessions-panel connection disconnects.

Automated coverage should exercise the same policy boundary in app-level behavior: route-owned remote playback is not F3-disconnectable, while F3-created direct remote playback remains disconnectable.

## Acceptance Criteria

- F3 `d` does not clear `active_route` or restore local mode when playback is route-owned.
- F3 `d` reports `No session connected` when no Sessions-panel connection is active.
- F3-created direct remote/session connections remain disconnectable.
- F3 `Enter` may still intentionally tear down an active route before connecting to the selected session.
- Route status rendering remains visible as `route:<library>` during routed playback.
