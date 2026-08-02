## Why

Turning Stay Alive off has no effect while this machine's local daemon from a previous
run is still alive ([#425](https://github.com/slatkin/mbv/issues/425)). The next TUI
attaches to that daemon, but quitting only disconnects the client, so the daemon and its
playback survive indefinitely unless the user separately runs `mbv -q` or uses tray Quit.

`stay_alive` is currently read only on the fresh-start path. It is not consulted when a
client attaches to an existing daemon or when a TUI exits, and the ctrl protocol has no
local-daemon lifecycle request.

## What Changes

- Add an acknowledged local-daemon shutdown request to ctrl protocol version 8. It is a
  lifecycle operation distinct from the player `Stop` command.
- Accept this request only from the daemon's local Unix control connection. TCP ctrl
  clients cannot terminate a daemon through this verb.
- Before accepting the request, the daemon durably persists its own authoritative queue.
  A persistence failure rejects the request and leaves the daemon running rather than
  silently losing the queue.
- On TUI quit, when `stay_alive` is false and the process launched against this machine's
  local daemon, target that local daemon even if the TUI has since routed playback to a
  different daemon. Never send the request through the mutable current-player connection
  unless that connection is itself the local daemon.
- Wait for an accept/reject response with a bounded timeout. Acceptance proves the
  command reached the daemon and its queue was persisted before the TUI exits.
- Once accepted, shutdown remains unconditional with respect to playback and other
  clients. Every connected client receives the existing deliberate-shutdown announcement.
- **BREAKING** Bump `CTRL_PROTOCOL_VERSION` 7 → 8. Every local connection path reports
  `mbv -q` as the recovery when a leftover v7 daemon blocks a v8 client.
- **BREAKING** Remove `-d` as a Stay Alive override. A legacy `mbv -d` invocation fails
  with guidance to enable `stay_alive`, rather than silently changing behavior.

Explicitly unchanged: `stay_alive` never causes an automatic shutdown request to be sent
to an explicit `unix://…` or `tcp://…` endpoint. Ordinary disconnect still leaves a
daemon running. `mbv -q`, operating-system termination, and tray Quit retain their
existing lifecycle behavior.

## Capabilities

### New Capabilities

- `daemon-lifecycle`: local-daemon start, survival, durable client-requested shutdown,
  and the distinction between an ordinary disconnect and a lifecycle request.

### Modified Capabilities

- `ctrl-protocol`: version 8 adds an acknowledged, local-transport-only shutdown request
  and its rejection behavior.
- `daemon-multi-connection`: deliberate accepted shutdown is the sole lifecycle exception
  to the guarantee that one ctrl client's activity does not disconnect other clients.

## Impact

Protocol and daemon (`crates/mbv-core/`):

- `src/ctrl.rs` — request/response protocol messages and version bump
- `src/daemon_core.rs` — record whether a ctrl connection may request local lifecycle
  operations
- `src/daemon_control.rs` — validate, persist, acknowledge, and dispatch the request
- `src/daemon_run.rs` — expose the authoritative queue snapshot to the request handler;
  reuse the existing `DaemonEvent::Shutdown` sequence
- `src/config_paths.rs` — make queue persistence report failure
- `src/remote_player.rs` / `src/remote_player_connect.rs` — bounded request completion and
  typed protocol-mismatch reporting

Client (`src/`):

- `src/app/run_loop_events.rs` — late `stay_alive` decision and correctly targeted,
  bounded shutdown request
- `src/main.rs` — `-d` migration error, usage/guidance updates, and consistent local
  protocol-mismatch recovery text

Documentation comments referring to live `mbv -d` behavior are updated. Historical
mentions in ADRs remain, while ADR 0015 receives an amendment because its original
decision says a client exit can never stop the local daemon.

No dependency or config-schema changes.
