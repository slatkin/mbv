# Daemon Control Authority Across Ctrl And Emby Remote

**Updated by ADR 0014** (Multi-Connection Ctrl Model).

## Decision

The daemon models command authority explicitly, separately from playback state
and separately from whether ctrl sockets are currently connected.

The authority holder is one of:

- `None`
- `Ctrl`
- `EmbyRemote`

ADR 0014 governs ctrl-socket semantics: multiple ctrl clients may connect
simultaneously without eviction. Authority is `Ctrl` when any ctrl client is
connected and no Emby remote command has taken authority. ADR 0004 still
governs tray semantics: tray commands stay outside this model and never take
authority.

A successful Emby remote-control websocket command takes authority as
`EmbyRemote`. The daemon broadcasts `Disconnected { reason: TakenOverByEmbyRemote }`
to all connected ctrl clients as a notification, but does **not** close their
sockets. Ctrl clients remain connected and receive state broadcasts, but their
commands are rejected until authority returns to `Ctrl`. Authority returns to
`Ctrl` on the **next ctrl command** (not on connect). Connecting while Emby
has authority does **not** override it.

Rejected or no-op websocket events do not change authority.

## Context

Before #139, ctrl-vs-Emby authority was implied indirectly from "is there a ctrl
client connected?" plus scattered websocket-side calls to evict that client.
That shape had two problems:

- the domain rule was implicit, so reconnect-after-Emby behavior was not stated
  anywhere durable
- websocket handlers encoded takeover behavior directly instead of going through
  one authority transition

The policy settled in #139 was that a ctrl reconnect after Emby remote activity
immediately becomes the driver again. That preserved ADR 0003's broader rule
that connection is authority on the ctrl axis.

ADR 0014 supersedes ADR 0003 and changes the model: connecting no longer
conveys authority. Authority is determined by command flow, not connection
lifecycle. `AuthorityHolder::Ctrl` no longer carries a `CtrlClientId` because
all ctrl clients share authority equally in the multi-connection model.

## Consequences

- Daemon ctrl state stores a `Vec<CtrlClient>` (multiple connections) and the
  current authority holder.
- Successful Emby websocket commands go through one shared "Emby takes
  authority" transition instead of each handler encoding ctrl eviction itself.
- Ctrl clients are never disconnected by Emby remote authority — they observe
  state changes and can resume commanding when Emby goes quiet.
- Authority goes to `None` only when the **last** ctrl client disconnects and
  authority is `Ctrl`. Individual client disconnects do not change authority.
- Authority returns to `Ctrl` on the next ctrl command, not on connect. A
  ctrl client connecting while Emby has authority receives broadcasts but its
  commands are rejected.
- A later ctrl command after Emby remote activity replaces `EmbyRemote` with
  `Ctrl` immediately.
- Tray commands remain non-takeover per ADR 0004 and do not participate in this
  authority enum.
