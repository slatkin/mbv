# Daemon Control Authority Across Ctrl And Emby Remote

## Decision

The daemon models command authority explicitly, separately from playback state
and separately from whether a ctrl socket is currently connected.

The authority holder is one of:

- `None`
- `Ctrl(CtrlClientId)`
- `EmbyRemote`

ADR 0003 still governs ctrl-socket semantics: a ctrl connection is itself a
takeover, so a newly connected ctrl client immediately becomes
`Ctrl(new_id)`. ADR 0004 still governs tray semantics: tray commands stay
outside this model and never take authority.

A successful Emby remote-control websocket command takes authority as
`EmbyRemote`. When a ctrl client is currently connected, the daemon first sends
that client the structured `TakenOverByEmbyRemote` disconnect event, then closes
the socket, then records `EmbyRemote` as the authority holder. If no ctrl client
is connected, the daemon still records `EmbyRemote`.

Rejected or no-op websocket events do not change authority.

## Context

Before #139, ctrl-vs-Emby authority was implied indirectly from "is there a ctrl
client connected?" plus scattered websocket-side calls to evict that client.
That shape had two problems:

- the domain rule was implicit, so reconnect-after-Emby behavior was not stated
  anywhere durable
- websocket handlers encoded takeover behavior directly instead of going through
  one authority transition

The policy settled in #139 is that a ctrl reconnect after Emby remote activity
immediately becomes the driver again. That preserves ADR 0003's broader rule
that connection is authority on the ctrl axis.

## Consequences

- Daemon ctrl state stores both the single ctrl connection and the current
  authority holder.
- Successful Emby websocket commands go through one shared "Emby takes
  authority" transition instead of each handler encoding ctrl eviction itself.
- A daemon may have `EmbyRemote` authority while no ctrl client is connected.
- A later ctrl reconnect replaces `EmbyRemote` with `Ctrl(new_id)` immediately.
- Tray commands remain non-takeover per ADR 0004 and do not participate in this
  authority enum.
