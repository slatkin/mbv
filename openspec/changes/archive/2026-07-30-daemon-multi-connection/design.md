## Context

mbvd currently enforces an exclusive connection model (ADR 0003): exactly one ctrl client at a time, where connection and driving authority are the same concept. Connecting is the takeover. This causes client-side bugs where stale state persists after connection changes, because the client only tears down what it tracks, not what the daemon implicitly replaced.

The daemon accepts connections over Unix socket and TCP, speaking a line-delimited JSON protocol (currently v3). The connection registry is `Option<CtrlClient>` — one or zero connections. Authority is a separate axis (ADR 0007): `None | Ctrl(id) | EmbyRemote`.

**This change supersedes ADR 0003** (exclusive ctrl connection) and **updates ADR 0007** (control authority). A new ADR will document the multi-connection model.

## Goals / Non-Goals

**Goals:**
- Multiple ctrl clients can connect simultaneously without eviction
- Commands from any connected client are accepted (MPD-style: last command wins)
- State broadcasts fan out to all connected clients
- Emby remote authority coexists with connected ctrl clients (observe-only mode)
- Protocol version bump to v4 with clear multi-connection semantics

**Non-Goals:**
- Client awareness of other connected clients (over-engineering)
- Cava visualization fan-out (deferred to phase 2 with subscription model)
- Permission levels or locking between ctrl clients
- Connection management by the daemon (clients manage their own lifecycle)

## Decisions

### 1. Connection registry: `Vec<CtrlClient>` over `HashMap<CtrlClientId, CtrlClient>`

**Decision**: Use a `Vec<CtrlClient>` to store multiple connections.

**Rationale**: The daemon doesn't need to look up clients by ID for targeted sends in the multi-connection model. Broadcasts iterate the whole collection. Removal on disconnect is a linear scan, but the expected client count is small (< 10). A HashMap adds complexity without benefit.

**Alternatives considered**: HashMap for O(1) lookup — rejected because we never need targeted sends by ID in the new model.

### 2. Authority model: `AuthorityHolder` stays a single enum

**Decision**: Keep `AuthorityHolder` as `None | Ctrl | EmbyRemote`. The `Ctrl` variant no longer carries a `CtrlClientId` since all ctrl clients share authority equally.

**Rationale**: Authority is about *who can command*, not *which specific client*. In the multi-connection model, any ctrl client can command when authority is `Ctrl`. The ID in the current `Ctrl(id)` variant only existed for eviction targeting, which is gone.

**Alternatives considered**: Per-client authority tracking — rejected as unnecessary complexity for the MPD-style model.

### 3. Authority-on-connect policy

**Decision**: When a ctrl client connects and authority is `EmbyRemote`, authority remains `EmbyRemote`. The new client receives broadcasts but its commands are rejected until authority returns to `Ctrl`.

**Rationale**: Connecting is no longer the takeover. Authority is determined by command flow, not connection lifecycle.

**Alternatives considered**: Connect sets authority to `Ctrl` unconditionally (current behavior) — rejected because it contradicts the spec and defeats the Emby observe-only model.

### 4. Authority-on-disconnect policy

**Decision**: Authority goes to `None` only when the last ctrl client disconnects. Individual client disconnects remove the client from the vec but do not change authority.

**Rationale**: Simple and correct. If any ctrl clients remain, they can still command. If none remain, no one is driving.

**Alternatives considered**: Authority clears on any disconnect — rejected because remaining clients should still be able to command.

### 5. Emby remote authority: observe-only for ctrl clients

**Decision**: When Emby remote takes authority, ctrl clients stay connected and receive broadcasts, but their commands are rejected with `CommandRejected { reason: "Emby remote has authority" }`. Authority returns to `Ctrl` on the next ctrl command.

The daemon SHALL broadcast `Disconnected { reason: TakenOverByEmbyRemote }` as a notification to all ctrl clients, but SHALL NOT send `CtrlOutbound::Close`. The writer thread continues running.

**Rationale**: Emby clients don't support the new multi-connection world, so we respect their takeover semantics. But ctrl clients shouldn't be disconnected — they can observe state changes and resume commanding when Emby goes quiet. This is the simplest middle ground.

**Alternatives considered**: Disconnect all ctrl clients on Emby takeover (current behavior scaled up) — rejected because it defeats the purpose of multi-connection. Timeout-based authority return — rejected as unnecessary complexity.

### 6. Protocol version: bump to v4

**Decision**: Increment protocol version from 3 to 4. v4 clients understand multi-connection semantics; v3 clients are incompatible.

**Rationale**: The behavioral changes are significant enough that old clients would be confused: they won't expect `CommandRejected` for authority reasons, and they may assume they're the sole driver if not evicted. A clean version break is simpler than backward compatibility.

**Alternatives considered**: Backward-compatible v3 with feature flags — rejected because the TUI is the only ctrl client, so a coordinated update is trivial.

### 7. `Disconnected` event semantics change

**Decision**: `Disconnected { reason: TakenOverByCtrlClient }` is eliminated. `Disconnected { reason: TakenOverByEmbyRemote }` becomes a notification (authority change), not a connection close. No `CtrlOutbound::Close` follows.

**Rationale**: In the multi-connection model, ctrl clients are never evicted by other ctrl clients. Emby remote takeover is an authority change, not a connection termination. The daemon is not a connection manager.

### 8. Spectrum state tracking (phase 1: stop-on-any-disconnect)

**Decision**: In phase 1, spectrum state stops on any client disconnect (preserves current behavior). This is acknowledged as imperfect; phase 2 will track which client started spectrum.

**Rationale**: `daemon_run.rs:377-382` already has a comment calling this out. Phase 1 keeps it simple; phase 2 adds per-client spectrum tracking when subscriptions are added.

**Alternatives considered**: Per-client spectrum tracking in phase 1 — rejected as premature optimization given the small expected client count.

### 9. Method renames

**Decision**: Rename `CtrlClients::send_to_driver()` → `broadcast_to_all()`. Rename `disconnect_driver()` → `notify_emby_authority()` (or remove if unused). Remove `disconnect()` if dead code.

**Rationale**: The old names reflect the single-driver model. New names reflect the multi-connection reality.

## Risks / Trade-offs

- **[Risk] ADR 0003/0007 contradictions** → Mitigation: write a new ADR (0012) that explicitly supersedes 0003 and updates 0007. Prerequisite for implementation.
- **[Risk] TUI changes underestimated** → Mitigation: ~80 references to `connected_session_id`/`direct_remote_connected` across 20+ files. `restore_local_mode()` path tears down on `RemoteDisconnected`. Expanded TUI tasks added to cover the full state machine rework.
- **[Risk] Existing tests break en masse** → Mitigation: tests in `daemon_tests.rs` and `daemon_tests_connection.rs` assert eviction behavior and single-connection invariants. Task 8.5 expanded to cover full test rewrite.
- **[Risk] Broadcast fan-out performance with many clients** → Mitigation: expected client count is small (< 10). If this becomes an issue, broadcasts can be batched or rate-limited.
- **[Risk] Stale connections consuming resources** → Mitigation: clients manage their own lifecycle. The daemon removes connections on send failure (broken pipe) or explicit disconnect. No keepalive or timeout needed.
- **[Risk] Cava visualization not fanning out in phase 1** → Mitigation: defer to phase 2 with subscription model. Phase 1 can send cava data to the first client, or not at all.
- **[Risk] Race: connect during EmbyRemote authority** → Mitigation: design decision 3 specifies connect does NOT override EmbyRemote authority. New client receives broadcasts but commands rejected until Emby goes quiet.
