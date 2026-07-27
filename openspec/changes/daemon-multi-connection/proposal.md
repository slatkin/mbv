## Why

mbvd currently enforces an exclusive connection model: only one ctrl client can be connected at a time, and connecting a new client evicts the incumbent. This causes client-side bugs where stale state (queue scope, remote queue contents, cached player status) persists after connection changes. The root cause is the daemon's single-owner architecture, not client-side bookkeeping.

## What Changes

- **BREAKING**: Protocol version bump (v3 → v4) with new connection semantics. **v2 and v3 clients rejected.**
- Multiple ctrl clients can connect simultaneously without eviction
- Commands from any connected client are accepted (MPD-style: last command wins)
- State broadcasts fan out to all connected clients
- Emby remote authority no longer disconnects ctrl clients — they stay connected and observe state changes, but their commands are rejected while Emby has authority
- Connecting while Emby has authority does NOT override it; authority returns on next ctrl command
- Authority goes to `None` only when the last ctrl client disconnects
- `Disconnected { reason: TakenOverByCtrlClient }` eliminated entirely
- `Disconnected { reason: TakenOverByEmbyRemote }` becomes a notification, not a connection close
- Supersedes ADR 0003 (exclusive ctrl connection) and updates ADR 0007 (control authority)

## Capabilities

### New Capabilities
- `daemon-multi-connection`: Core support for multiple concurrent ctrl client connections with MPD-style command acceptance and broadcast fan-out

### Modified Capabilities
- `ctrl-protocol`: Protocol version bump to v4 with new multi-connection semantics (no eviction, authority-based command rejection)

## Impact

- **Protocol**: Breaking change for v2 and v3 clients. Existing clients will not understand the new authority-based `CommandRejected` reasons or the absence of eviction behavior.
- **Daemon**: Connection registry (`CtrlClients`) changes from `Option<CtrlClient>` to `Vec<CtrlClient>`. `connect()` appends instead of evicting. `broadcast()` fans out to all. `disconnect_driver()` and `disconnect()` become dead code or are repurposed. `send_to_driver()` renamed to `broadcast_to_all()`. Spectrum state tracking deferred to phase 2.
- **TUI client**: Significant rework. Must update to protocol v4. Full state machine rework needed: ~80 references to `connected_session_id`/`direct_remote_connected` across 20+ files. `restore_local_mode()` path must not tear down on authority-change notifications. `RemoteDisconnected` handling must distinguish connection-close from authority-change.
- **ADRs**: New ADR to supersede ADR 0003 and update ADR 0007.
- **Cava visualization**: Currently single-recipient. Phase 1: stop-on-any-disconnect. Phase 2: subscription-based fan-out (subscribe/unsubscribe commands).
