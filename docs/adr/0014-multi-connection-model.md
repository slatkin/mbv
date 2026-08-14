# Multi-Connection Ctrl Model

**Supersedes ADR 0003** (Exclusive Ctrl-Socket Connection).

> **Amended (2026-08-14):** Protocol version has advanced from v4 (described below) to v7 on `main` via `retire-pty-relay-for-local-daemon-stay-alive` (v6→v7) and `stop-daemon-when-stay-alive-off` added capability `lifecycle-shutdown`. `crates/mbv-core/src/ctrl.rs:21` `CTRL_PROTOCOL_VERSION = 7` on `main`. Additive capabilities since added: `feed-playback`, `unified-queue`, `control-auth`. Open PR #529 tracking issue #523 will reconcile the pre-existing v7-vs-archived-v8 drift and ship v9 removing legacy `auth_token`. Multi-connection semantics unchanged.

## Decision

The daemon accepts **multiple concurrent ctrl-socket connections**. There is no
eviction between ctrl clients. Authority and connection are separate axes:

- **Multiple ctrl clients** may be connected simultaneously. Connecting does not
  evict existing clients.
- **Commands** from any connected client are accepted when authority is `Ctrl`.
  This is MPD-style semantics: last command wins. There is no locking or
  permission hierarchy between ctrl clients.
- **State broadcasts** fan out to all connected clients. Every connected client
  receives the same daemon state.
- **Emby remote authority** is observe-only for ctrl clients. When Emby remote
  takes authority, ctrl clients stay connected and receive broadcasts, but their
  commands are rejected with `CommandRejected`. Authority returns to `Ctrl` on
  the **next ctrl command** (not on connect).
- **Authority goes to `None`** only when the **last** ctrl client disconnects.
  Individual client disconnects do not change authority.
- **Introduced with protocol version 4**: v2 and v3 clients were rejected
  because the no-eviction and authority semantics were incompatible. The
  current protocol number remains defined only by `CTRL_PROTOCOL_VERSION`.

## Context

ADR 0003 established that the daemon accepts at most one ctrl connection, and
connecting *is* the takeover. This caused client-side bugs where stale state
(queue scope, remote queue contents, cached player status) persisted after
connection changes, because the client only tears down what it tracks, not what
the daemon implicitly replaced. The root cause is the single-owner architecture,
not client-side bookkeeping.

The TUI is the only ctrl client, so a coordinated protocol upgrade is trivial.
The daemon's deployment is one central server with one physical audio output, so
multiple simultaneous ctrl clients are a real deployment scenario (e.g., a user
connecting from a laptop and a phone, or reconnecting after a network blip without
the old connection dying first).

## Considered Options

- **Multi-connection, MPD-style (chosen).** Multiple clients connect freely.
  Commands from any client are accepted when authority is `Ctrl`. Last command
  wins. Broadcasts fan out to all. Emby remote authority makes ctrl clients
  observe-only; authority returns on next ctrl command.
- **Multi-connection with a single "driver" chosen on first command (rejected).**
  Rejected in #107 as over-engineering — observers aren't a real deployment need
  and the co-existence window is a correctness hazard. The MPD-style model is
  simpler and matches real daemon usage.
- **Exclusive connection, connect-evicts (rejected, current ADR 0003).**
  Rejected because it causes client-side bugs with stale state. ADR 0003 is
  superseded by this decision.

## Consequences

- The connection registry changes from `Option<CtrlClient>` to
  `Vec<CtrlClient>`. `connect()` appends instead of evicting.
- `Disconnected { reason: TakenOverByCtrlClient }` is eliminated. Ctrl clients
  are never evicted by other ctrl clients.
- `Disconnected { reason: TakenOverByEmbyRemote }` becomes a notification (authority
  change broadcast), not a connection close. No `CtrlOutbound::Close` follows.
- `AuthorityHolder::Ctrl` no longer carries a `CtrlClientId`. Authority is about
  *who can command*, not *which specific client*. All ctrl clients share authority
  equally.
- Connect does **not** override Emby remote authority. The new client receives
  broadcasts but commands are rejected until Emby goes quiet and a ctrl command
  arrives.
- Authority goes to `None` only when the last ctrl client disconnects. If any
  ctrl clients remain, they can still command.
- Method renames: `send_to_driver()` → `broadcast_to_all()`,
  `disconnect_driver()` → `notify_emby_authority()` or removed.
