# Endpoint-Cached Library Route Diagnostics Design

**Issue:** #270

## Goal

Make a normal production `mbv.log` explain every decision in the #256 endpoint-cached library-routing and auto-reconnect lifecycle without changing routing, retry, or fallback behavior.

## Assessed claims

- #256 is authoritative: F2 resolves a friendly live device to a `tcp://host:port` endpoint and persists that endpoint in `[library_routes]`; play and enqueue perform no `/Sessions` lookup and do no endpoint rediscovery.
- `CONTEXT.md` still describes the superseded #239 device-name representation and must be corrected.
- malformed route warnings are currently emitted while `load_config()` runs before `applog::init()`, so they never reach `mbv.log` during normal startup.
- F2 converts `get_views()` and session-fetch errors to empty collections, making network failures indistinguishable from valid empty results.
- config and last-remote-state persistence suppress creation, serialization, write, rename, and removal errors.
- route resolution logs malformed endpoints and ancestor-fetch failure, and connection failure is logged and shown to the user, but ordinary decisions such as empty/missing routes, cache state, bypass, already-active routes, successful connection, and local restoration are incomplete.
- auto-reconnect has silent disabled and missing-state exits; teardown does not explain why it saved, cleared, or skipped state.

## Design

Keep the existing modules and control flow. Add `info` or `warn` logs at the existing lifecycle seams rather than introducing a new event system.

Initialize `mbv.log` before loading configuration so validation warnings are durable. After parsing, emit a sanitized route-table summary containing library names and configured endpoints, plus the `auto_reconnect` setting. Route endpoints are LAN addresses already stored in user-readable configuration; no authentication tokens or media URLs may be logged.

Change config and reconnect-state persistence helpers to return `Result` or an explicit load outcome where callers need to distinguish missing, loaded, and failed state. Errors must include the operation and path. Callers log them and preserve existing user-facing behavior; a diagnostics failure must not make routing or shutdown fatal.

Instrument F2 library/device discovery, endpoint eligibility, route commit/removal, config save, and runtime-copy synchronization. Instrument play/enqueue resolution context, thin-client bypasses, active-library versus ancestor resolution, route-table empty and missing-library outcomes, ancestor-cache hit/miss/expiry, connection attempt/success/failure, already-active no-op, route swap, fallback, and local restoration.

Instrument auto-reconnect startup gates, state load, selected variant, route/session resolution, connection result, and fallback. Instrument teardown’s disabled/remote-instance gates and its decision to save a library route, save a direct session, or clear prior state, including persistence failures.

## Testing

Use TDD for behavior-bearing helper changes and error outcomes. Automated tests should cover config validation after logger startup, state persistence success/failure outcomes, F2 fetch failure handling, and representative route/reconnect decisions without asserting every prose log line.

After automated checks pass, manually run the #256 flow: assign a route through F2, verify `config.toml` contains the endpoint, play from the routed library, quit while routed, restart with `auto_reconnect = true`, and inspect `mbv.log`. The issue is not product-verified until the log alone explains each outcome. If this exposes a routing behavior defect, record the evidence and fix it separately rather than expanding this diagnostics change speculatively.

## Non-goals

- Restoring device-name values or live rediscovery.
- Adding retries or changing fallback-to-local semantics.
- Making diagnostics failures fatal.
- Logging secrets, tokens, or full media URLs.

