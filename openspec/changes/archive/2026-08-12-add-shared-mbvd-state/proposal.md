## Why

mbv's queue, library navigation, reconnect target, and selected routing preferences are currently machine-local, so moving between computers loses continuity. A canonical `mbvd` can provide an opt-in, low-management home for this mbv-owned state without making browsing or playback depend on the service.

## What Changes

- Add an opt-in shared-data host role to one canonical `mbvd`, backed by an embedded `redb` database.
- Add an explicit shared-data client endpoint independent of the active playback endpoint and library routes.
- Roam each Emby user's existing queue, library-position, and reconnect documents plus `auto_reconnect` and `library_routes`.
- Keep live playback state, all caches, and all other client settings local.
- Authenticate shared-data connections as one unambiguous Emby user and isolate documents by that user ID.
- Use per-document revisions to reject stale writes from concurrent clients.
- Mirror connected shared state to local files; fall back to those files with a toast when shared data is unavailable, retry in the background, and restore authoritative shared state with a toast after reconnection.
- Provide JSON export for inspection and recovery while retaining JSON as the logical stored representation.

## Capabilities

### New Capabilities

- `shared-mbv-state`: Opt-in hosting, authenticated per-user document storage, client fallback and reconnection, concurrency control, and JSON export for roaming mbv state.

### Modified Capabilities

None.

## Impact

- Adds `redb` as an embedded storage dependency in `mbv-core`.
- Extends daemon bootstrap configuration and client configuration with separate shared-data opt-ins.
- Adds capability-negotiated shared-data protocol messages without changing playback authority or requiring a ctrl protocol version bump.
- Changes state loading and persistence in the TUI to select shared or local storage and maintain a local mirror.
- Tightens token validation for shared-data access so API keys or other tokens without a verified single-user identity cannot access per-user documents.
- Adds user-visible toast notifications and logs for fallback, reconnection, stale-write adoption, and roaming-setting overrides.
