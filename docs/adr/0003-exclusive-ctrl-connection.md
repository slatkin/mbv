# Exclusive Ctrl-Socket Connection

## Decision

A daemon accepts **at most one ctrl-socket connection at a time**, and that
connection *is* the driving authority — connection and driver are a single
concept. **Connecting is the takeover**: a newly connecting TUI causes the daemon
to evict the incumbent connection (explicit takeover-disconnect event → the old
TUI turns its remote/session indicator off and toasts the user → the daemon closes
that socket) *before* the newcomer becomes authoritative. There is no live
"pending" or "observer" connection coexisting with the driver.

Settled originally in #107's grilling: the daemon's real deployment is one central
server with one physical audio output, so multiple simultaneous ctrl clients (as
observers or otherwise) are not a domain concept. The expected case is one person
moving between machines, where the new client should win and the stale one should
be told, not silently kept alive.

## Considered options

- **Exclusive connection, connect-evicts (chosen).** One connection; connecting is
  intent to drive and displaces the incumbent with an explicit, user-visible notice.
- **Multi-client with a single "driver" chosen on first successful command
  (rejected).** Lets clients co-connect and observe; the driver is whichever last
  exerted will. Rejected in #107 — observers aren't a real deployment need and the
  co-existence window is a correctness hazard (see Consequences / #119).
- **Connection rejection when a client is already attached (rejected).** Refusing
  the newcomer strands the person who just switched machines with no way in; eviction
  of the stale side is the better failure mode.

## Consequences

- The multi-client `ctrl_clients: Vec<_>` in `daemon.rs` and `add_pending`
  (which pushes without evicting; `take_over` fires only on a *successful command*,
  never on connect) are implementation debt to migrate: connect must evict. Target
  shape is a single `Option<CtrlClient>` replacing the `Vec<CtrlClient>` + separate
  `driver: Option<CtrlClientId>` — the connection *is* the driver, so one field says
  it all and the "two live connections" state becomes unrepresentable rather than
  merely disallowed by discipline.
- Disconnect must not disturb the daemon's queue or playback (the **Daemon
  contract** in `CONTEXT.md`). Current `CtrlDisconnected` handling already only
  removes the client from the registry and leaves the player running, so this is
  preserved behavior, not a change.
- Scope boundary: this covers ctrl-socket connection exclusivity only. The
  ctrl-vs-Emby-remote-websocket *authority* axis (a WS command evicts the ctrl TUI,
  but the Emby remote is not itself a ctrl connection) is a separate follow-up.
- This dissolves #119 ("AdoptQueue rejection leaves RemotePlayer state permanently
  diverged"). That bug required two clients co-connected to a cold daemon so a losing
  adopter could linger with optimistic state. Under exclusive connection the loser is
  evicted at connect, so the co-pending losing-adopter cannot exist.
- A residual `AdoptQueue` rejection is still reachable via the Emby remote-control
  websocket warming the daemon between a sole client's baseline read and its adopt
  command. This stays a defensive guard; recovery is trivial because there is only
  ever one connection to reconcile.

## Why this ADR exists

The #107 decision lived only as a `CONTEXT.md` glossary entry. A same-session doc
edit (commit `267efc2`, one minute after the eviction clause) inserted a "pending
connection … does not evict … merely by connecting" sentence that silently reversed
it, and the implementation never migrated off the multi-client `Vec`. That drift is
the root cause of #119. Recording the decision as an ADR gives the eviction model a
durable home that a future glossary "clarification" cannot quietly undo.
