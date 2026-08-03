## Context

See proposal.md for motivation. Four existing constraints determine the design:

- `DaemonEvent::Shutdown` already owns the complete shutdown sequence: announce, flush
  writers, stop and join the player, remove the pid file, and exit.
- `App::home_is_local_daemon` is immutable launch identity, while `App::player_endpoint`
  and `self.player` are mutable current-target state. A local-daemon-launched TUI may route
  to TCP or Unix during the session, so those values are intentionally allowed to diverge
  (ADR 0016).
- `RemotePlayer::send_ctrl_cmd` only enqueues to an in-process channel. The socket write is
  performed later by an unjoined writer thread, so enqueue success does not prove delivery.
- The daemon is the sole authoritative queue owner under multiple ctrl connections. A
  client shadow can lag another client's command and stops receiving local state entirely
  while that client is routed elsewhere.

Ctrl compatibility is exact-match. A v8 client cannot attach to a v7 daemon, and this
change does not attempt a cross-version bridge.

## Goals / Non-Goals

**Goals:**

- Stop the correct local daemon when Stay Alive is off, including after a mid-session
  route to another daemon.
- Preserve the authoritative queue durably before accepting client-requested shutdown.
- Give the requester a bounded, observable accept/reject result before its process exits.
- Keep the existing shutdown sequence shared by signals, tray Quit, and accepted requests.
- Prevent TCP ctrl clients from acquiring a new daemon-termination capability.

**Non-Goals:**

- Client reference counting, drain mode, or waiting for playback to finish.
- Making `stay_alive` govern an explicitly configured non-local endpoint.
- Changing the behavior of `mbv -q`, signals, or tray Quit when queue persistence fails;
  only the new coordinated client request has the persist-before-stop contract.
- A graceful protocol bridge between v7 and v8.

## Decisions

### 1. Reuse `DaemonEvent::Shutdown` after request preparation succeeds

The ctrl handler prepares the coordinated shutdown, then sends
`DaemonEvent::Shutdown` through the existing merged event channel. Preparation consists of
transport authorization, authoritative queue persistence, and a request-specific response.
The event handler remains the only implementation of process teardown.

This corrects the earlier wording that the ctrl handler feeds `shutdown_signal_tx`
directly. Signal and tray triggers are adapted into the same `DaemonEvent`, but the ctrl
handler already has the merged-event sender and should use it.

### 2. Separate the home-daemon decision from the connection used to send it

The automatic-shutdown policy remains:

```text
home_is_local_daemon && !stay_alive
```

That launch-time predicate answers whether this process participates in local-daemon
lifecycle. It does not identify the current `PlayerProxy` target.

At teardown:

| Home is local | Current target | Action |
|---|---|---|
| false | any | never request automatic shutdown |
| true | live Local | request through the current Local connection |
| true | TCP/Unix or disconnected Local | open a short-lived `DaemonEndpoint::Local` control connection and request through it |
| true | short-lived Local connection fails | report that the local daemon could not be reached; never forward the request to the current target |

The short-lived connection is deliberately independent of `self.player`; it cannot mutate
route state, queue scope, MPRIS binding, or auto-reconnect persistence. `--connect-daemon
local` also has `home_is_local_daemon == true` and follows the Local row. Explicit
`unix://…` and `tcp://…` launches remain false and never enter this flow.

*Alternative rejected:* gating on `is_local_daemon()`. That avoids wrong-target delivery
but leaves the home daemon alive whenever the TUI happens to be routed elsewhere at quit,
contradicting the launch-lifecycle purpose of `home_is_local_daemon`.

### 3. The daemon persists its authoritative queue before accepting

The quitting client no longer writes its local shadow as the source of truth. The daemon
event loop supplies the request handler with its authoritative `items`, `cursor`, `source`,
and player status. A focused helper projects those values to `QueueState`, including the
active non-audio item's latest valid position and the corresponding positions map.

Queue persistence becomes fallible: `save_queue_state` returns `Result`, uses the existing
temporary-file-and-rename sequence, and reports directory creation, serialization, write,
and rename failures. Existing best-effort callers may log and continue; the coordinated
shutdown handler must not.

If persistence fails, the handler sends `ShutdownRejected` to the requester, logs the
cause, does not enqueue `DaemonEvent::Shutdown`, and leaves every client and playback
running. Data preservation wins over honoring the setting when both cannot be guaranteed.

If the queue is empty, the coordinated snapshot preserves the existing on-disk snapshot,
matching `save_queue_state_no_clear`: quitting is not an explicit Clear Queue action.

*Alternatives rejected:*

- Saving the requesting client's `player_tab`: stale under concurrent clients and after a
  route swap.
- Adding a pre-shutdown client snapshot request: still leaves a race between the snapshot
  and later daemon commands.
- Persisting only after shutdown begins: a failure would be discovered after the daemon
  had already committed to exit.

### 4. Add a bounded request acknowledgement

Version 8 adds request-specific `ShutdownAccepted` and `ShutdownRejected { reason }`
responses. The daemon sends acceptance only after durable queue persistence and immediately
before queuing `DaemonEvent::Shutdown`. The existing shutdown handler then queues the
deliberate-shutdown announcement and flushes every writer, which also flushes the earlier
acceptance response.

`RemotePlayer` exposes a bounded lifecycle method rather than using raw
`send_ctrl_cmd`. Its completion path is wired directly from the reader thread, independent
of the TUI event loop, because teardown has already stopped processing ordinary
`PlayerEvent`s. Outcomes are Accepted, Rejected, Disconnected, or TimedOut.

Acceptance proves the client command was serialized, written, parsed, authorized, and
persisted. A bounded timeout prevents a broken daemon or socket from hanging terminal
restoration. Rejection or timeout leaves the TUI free to exit but produces a post-terminal
message explaining that the local daemon may still be running and naming `mbv -q`.

*Alternative rejected:* fire-and-forget. Channel enqueue success says nothing about
whether the writer thread ran before process exit.

### 5. Restrict the protocol verb to local Unix ctrl connections

The ctrl connection registry records whether a connection arrived through the local Unix
listener or TCP listener. `RequestShutdown` from TCP is rejected without changing
authority, persisting state, or enqueuing shutdown. The normal auth handshake remains
required on the Unix connection as well.

The request is a lifecycle operation, not playback authority. A permitted local request is
honored while `EmbyRemote` holds playback authority and does not first transfer playback
authority to Ctrl. This requires handling lifecycle commands before the existing
command-driven authority transition.

This limits the new termination surface while preserving the intended local TUI workflow.
`mbv -q` and tray Quit remain the administrative controls for other deployment shapes.

### 6. Once accepted, shutdown is unconditional

After authorization and durable persistence succeed, the daemon does not inspect client
count or playback state. Other clients receive `Disconnected { DaemonShutdown }`, writers
are flushed, playback is stopped, and the process exits through the existing handler.

The persistence rejection in Decision 3 is a precondition failure, not client counting or
drain behavior. The lifecycle spec states this distinction explicitly.

### 7. Use a typed protocol mismatch and format it consistently

Connection setup should preserve protocol-version mismatch as a distinguishable error
rather than requiring callers to match text inside `String`. Every connection path whose
endpoint is `DaemonEndpoint::Local` appends the recovery instruction to run `mbv -q`,
including:

- automatic `Resolution::Attach`;
- post-spawn Local attachment;
- explicit/configured endpoint `local`;
- the short-lived teardown connection.

TCP and arbitrary Unix endpoint failures retain their generic endpoint-specific messages.

### 8. Remove `-d` with an explicit migration error

Argument parsing continues recognizing the legacy exact `-d` token solely to reject it
with: enable `stay_alive` in configuration or the settings overlay. Silently ignoring it
would turn an old daemon-launch alias into foreground in-process playback, which is an
unsafe migration for a declared breaking removal.

Usage and `Resolution::Refuse` guidance stop advertising the flag. Historical ADR mentions
remain historical; live comments are updated.

### 9. Amend ADR 0015

ADR 0015's statement that a client exit can never stop the local daemon becomes false. Add
an amendment noting that exit with `stay_alive` off performs an explicit, acknowledged,
persist-before-stop request. The remaining decision still holds: ordinary disconnect does
not stop the daemon, and there is no reference counting or last-client-out race.

## Risks / Trade-offs

- **Local daemon cannot persist its queue** → Reject coordinated shutdown, leave playback
  running, and surface a recovery message; never trade silent data loss for setting
  compliance.
- **Home daemon cannot be reached after a route swap** → Never send through the current
  remote target; time out or report the Local connection failure and name `mbv -q`.
- **Acceptance arrives but the daemon crashes before processing its queued shutdown** → The
  queue is already durable; the next startup's existing stale-process detection handles
  the process failure.
- **A leftover pre-upgrade daemon blocks v8 startup** → Typed mismatch errors make every
  Local connection path name `mbv -q` as the one-time recovery.
- **A second client loses playback after accepted shutdown** → Deliberate behavior; it
  receives the existing structured announcement before disconnect.
- **Legacy `mbv -d` aliases stop working** → Fail loudly with exact migration guidance
  instead of silently changing process ownership.

## Migration Plan

Ship the version bump, protocol messages, daemon handler, and client request helper in the
same binary. Before the upgrade, users may stop a surviving v7 daemon with the old
`mbv -q`; after the upgrade, every v8 Local mismatch message names the same remedy.

Rollback is a straight revert. A surviving v8 daemon seen by a rolled-back v7 binary has
the symmetric exact-version failure and can likewise be stopped with `mbv -q`.
