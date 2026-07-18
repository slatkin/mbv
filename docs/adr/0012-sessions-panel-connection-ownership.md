# Sessions Panel Owns Only Sessions-Panel Connections

F3's Sessions panel owns only playback/control relationships created by selecting a discovered session in F3. Library routes are persistent library playback policy: a route may create a remote daemon transport internally, but that transport is route-owned, not Sessions-panel-owned, so F3's disconnect command must not clear `active_route`, restore local playback, or disconnect routed playback merely because the route currently uses a remote `RemotePlayer`.

## Considered Options

- Treat every remote `RemotePlayer` as F3-disconnectable. Rejected because it lets a Sessions-panel action erase route runtime state while F2/config still say the library route exists.
- Add a full playback-authority enum immediately. Rejected for this change because the immediate policy can be expressed by narrowing F3 action authority without replacing the broader display model.
- Make F3 disconnect target only Sessions-panel connections. Chosen because it matches the user-facing model: F3 lists sessions, so F3 disconnects sessions; route policy is managed by library routing.

## Consequences

F3 `Enter` remains an explicit takeover: selecting a discovered session may tear down an active library route first, then establish a Sessions-panel connection. F3 `d` is asymmetric by design: it disconnects only a Sessions-panel connection that F3 owns. When playback is route-owned and no Sessions-panel connection is active, F3 `d` leaves routing untouched and reports `No session connected`.

Ownership is determined by the user action that created the relationship, not by endpoint or device identity. If the configured route's target device also appears in F3 and the user selects it there, the resulting relationship is Sessions-panel-owned even if it points at the same daemon.
