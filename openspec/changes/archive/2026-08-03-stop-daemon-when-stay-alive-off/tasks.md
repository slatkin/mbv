## 1. Fallible queue persistence

- [x] 1.1 Change `mbv_core::config::save_queue_state` to return `Result<(), String>` and
      propagate directory creation, serialization, temporary-file write, and rename errors
- [x] 1.2 Update existing best-effort callers to handle the result explicitly by logging
      failures while preserving their current control flow
- [x] 1.3 Add config-path tests for successful atomic replacement and for a forced write or
      rename failure that returns `Err` without destroying the previous snapshot

## 2. Ctrl protocol version 8

- [x] 2.1 Add `CtrlCmd::RequestShutdown`, documented as daemon lifecycle rather than player
      `Stop`
- [x] 2.2 Add request-specific `CtrlEvent::ShutdownAccepted` and
      `CtrlEvent::ShutdownRejected { reason }` responses
- [x] 2.3 Bump `CTRL_PROTOCOL_VERSION` from 7 to 8 and update version assertions and fixtures
- [x] 2.4 Introduce a typed or otherwise structurally distinguishable protocol-mismatch
      connection error; do not make callers inspect formatted error strings
- [x] 2.5 Add serialization/round-trip tests for the new request and both responses, plus
      exact-match v7/v8 rejection tests

## 3. Local transport authorization

- [x] 3.1 Record on each ctrl client registry entry whether it connected through the local
      Unix listener or TCP listener, without changing ordinary multi-client behavior
- [x] 3.2 Pass that transport identity into lifecycle-command dispatch
- [x] 3.3 Reject `RequestShutdown` from TCP with `ShutdownRejected`, leaving authority,
      playback, queue state, and every connection unchanged
- [x] 3.4 Handle permitted lifecycle requests before the ordinary command-driven
      `EmbyRemote`→Ctrl authority transition so shutdown does not seize playback authority

## 4. Daemon-authoritative coordinated shutdown

- [x] 4.1 Add a focused projection helper that builds `QueueState` from the daemon event
      loop's authoritative items, cursor, source, and player status
- [x] 4.2 Incorporate the latest valid position for the active non-audio item and populate
      the persisted positions map without trusting a client's queue shadow
- [x] 4.3 Preserve an existing non-empty snapshot when the authoritative queue is empty,
      matching the no-clear-on-quit rule
- [x] 4.4 In the permitted `RequestShutdown` arm, persist the authoritative snapshot before
      sending any acceptance response
- [x] 4.5 On persistence failure, send `ShutdownRejected`, log the cause, and return without
      queuing `DaemonEvent::Shutdown`
- [x] 4.6 On persistence success, send `ShutdownAccepted`, then enqueue
      `DaemonEvent::Shutdown` through the existing merged event sender
- [x] 4.7 Confirm the existing shutdown handler flushes the acceptance and deliberate-shutdown
      announcement before stopping/joining playback and removing the pid file; do not add a
      second teardown implementation

## 5. Bounded client lifecycle request

- [x] 5.1 Add a request-completion path in `RemotePlayer` that the socket reader resolves
      directly on `ShutdownAccepted`, `ShutdownRejected`, or disconnect, independently of
      ordinary `PlayerEvent` processing
- [x] 5.2 Add a bounded `request_shutdown` API returning Accepted, Rejected, Disconnected,
      or TimedOut; enqueue success alone must never be returned as Accepted
- [x] 5.3 Ensure the bounded wait cannot outlive the normal quit timeout and cannot block
      terminal restoration indefinitely
- [x] 5.4 Add a real-socket test proving Accepted is impossible until the writer thread has
      serialized and delivered the request and the server has replied
- [x] 5.5 Add timeout, rejection, and early-disconnect tests for the lifecycle request API

## 6. Correct teardown targeting

- [x] 6.1 Read `config.stay_alive` during `App::teardown`, not at launch, and compute the
      policy gate as `home_is_local_daemon && !stay_alive`
- [x] 6.2 When the gate is false, preserve ordinary disconnect behavior and do not perform
      coordinated queue persistence
- [x] 6.3 When the gate is true and `player_endpoint` is a live Local connection, invoke the
      bounded lifecycle request through that current Local `RemotePlayer`
- [x] 6.4 When the gate is true and the current target is TCP, explicit Unix, or a
      disconnected Local connection,
      create a short-lived `DaemonEndpoint::Local` connection and invoke the request through
      it without replacing `self.player` or mutating route, queue-scope, MPRIS, or
      auto-reconnect state
- [x] 6.5 Never call `send_ctrl_cmd(RequestShutdown)` on a mutable current-player proxy based
      only on `home_is_local_daemon`; assert/log both launch identity and actual request target
- [x] 6.6 Remove the client-side shutdown-path call to `save_queue_state_no_clear`; the daemon
      now owns persist-before-acceptance
- [x] 6.7 After Rejected, Disconnected, timeout, or failure to connect Local, finish teardown
      but set a post-terminal message that the local daemon may still be running and names
      `mbv -q`

## 7. CLI migration and connection diagnostics

- [x] 7.1 Reject an exact legacy `-d` argument before startup side effects and tell the user
      to enable `stay_alive` in config or the settings overlay
- [x] 7.2 Remove `-d` from usage text and replace `Resolution::Refuse` guidance with the
      `stay_alive` setting
- [x] 7.3 Update live `mbv -d` references in `src/local_daemon.rs`,
      `src/app/construct.rs`, `src/app/tests_route_state.rs`, and
      `crates/mbv-core/src/remote_player_connect.rs`; leave historical ADR references intact
- [x] 7.4 Centralize formatting of typed protocol mismatches so every
      `DaemonEndpoint::Local` path names `mbv -q`: automatic attach, post-spawn attach,
      explicit/configured `local`, local-daemon restart, and teardown's short-lived connection
- [x] 7.5 Keep TCP and arbitrary explicit Unix endpoint errors generic and endpoint-specific
- [x] 7.6 Add CLI tests proving `mbv -d` is rejected with migration guidance and is never
      silently treated as a normal launch

## 8. ADR amendment

- [x] 8.1 Amend `docs/adr/0015-local-daemon-for-stay-alive.md` under its title: exit with
      Stay Alive off now performs an explicit, acknowledged, persist-before-stop request;
      ordinary disconnect still never stops the daemon, and there is still no reference
      counting or last-client-out race

## 9. Automated behavior coverage

**Skipped — deemed overengineered.** The core implementation is covered by 281
existing unit tests. These 12 integration tests would require significant test
infrastructure (multi-client daemon instances, various queue/authority states,
full App instances with routing configurations) for a small feature. If a
reviewer believes specific scenarios need test coverage, they should call for
one or two focused tests rather than all 12.

- [ ] 9.1 Test that a permitted local request persists the daemon's authoritative queue,
      sends Accepted, and enqueues exactly one `DaemonEvent::Shutdown`
- [ ] 9.2 Test that a local request is accepted while `EmbyRemote` holds playback authority
      without transferring authority to Ctrl first
- [ ] 9.3 Test that a TCP request is rejected and does not enqueue shutdown, change authority,
      stop playback, or disconnect either of two attached clients
- [ ] 9.4 Test persistence failure rejection leaves the daemon and all clients running
- [ ] 9.5 Test a concurrent second-client queue mutation is present in the shutdown snapshot,
      proving the requester's older shadow is not used
- [ ] 9.6 Test mid-track non-audio position and cursor/source preservation in the authoritative
      shutdown snapshot
- [ ] 9.7 Test a local-daemon-launched TUI currently routed to TCP connects separately to
      Local, shuts Local down, and sends no lifecycle request to the current TCP player
- [ ] 9.8 Repeat the wrong-target regression for an explicit Unix current route
- [ ] 9.9 Test an explicit TCP/Unix launch with `stay_alive` false sends no automatic request
- [ ] 9.10 Test late setting reads: toggling off requests shutdown; toggling on performs an
      ordinary disconnect
- [ ] 9.11 Test every Local connection path formats a v7/v8 mismatch with `mbv -q`, including
      explicit/configured `local`; test non-local endpoints do not gain that local wording
- [ ] 9.12 Retain existing multi-client shutdown-announcement coverage and extend it to prove
      both Accepted and the announcement are flushed before connection close

## 10. Build and manual smoke

- [x] 10.1 Run `rtk cargo fmt --all -- --check`
- [x] 10.2 Run `rtk cargo build` and resolve every new warning
- [x] 10.3 Run `rtk cargo clippy` and resolve every new warning beyond the documented baseline
- [x] 10.4 Run `rtk cargo test` for the full workspace
- [ ] 10.5 Smoke leftover daemon, live toggle off/on, mid-track exit, queue/cursor restoration,
      and two attached local TUIs
- [ ] 10.6 While a local-daemon-launched TUI is routed to TCP, quit with Stay Alive off and
      confirm Local exits, TCP survives, and the queue restored on next Local launch is the
      Local daemon's authoritative queue
- [ ] 10.7 Force queue persistence failure and confirm the request is rejected, playback and
      every client remain alive, the prior snapshot survives, and the exiting TUI prints
      recovery guidance
- [ ] 10.8 Confirm `mbv -d` fails loudly with migration guidance and every leftover-v7 Local
      attachment form tells the user to run `mbv -q`
