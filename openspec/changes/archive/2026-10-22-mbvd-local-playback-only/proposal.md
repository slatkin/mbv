# Proposal

## Why

Issue #946. The packaged `mbvd` is a headless server daemon, so a Cast receiver on the user's LAN or an Emby remote-control Session is not its job — a receiver is often unreachable from the machine that runs `mbvd`. Today `mbvd` stays local-only only by convention: `crates/mbvd/src/main.rs` passes `cast_connector: None` and `emby_session_output: false` into `DaemonRuntimeHooks`, and the ctrl capability advertisement and the selection guards derive from those two fields. Nothing in the type boundary stops a future `mbvd` change from supplying a Cast connector or flipping the flag, and the spec requirement "The packaged owner has only local playback" does not say how the exclusion is enforced. The two runtime-hook fields are the last Owner features that a packaged owner can express but must never use.

## What Changes

- Replace the two remote-output fields on `DaemonRuntimeHooks` (`cast_connector: Option<CastConnector>` and `emby_session_output: bool`) with a typed owner-output capability that the packaged daemon role cannot construct with a remote variant. The compiler, not a convention comment, holds the boundary. The exact shape is a design decision.
- Keep the derived behavior that already works and pin it with tests: a packaged `mbvd` hello advertises neither `playback-output` nor `emby-session-output`; a Client connected to `mbvd` offers no Cast or Emby-remote output selection; a remote selection request that still reaches `mbvd` is rejected with `CommandRejected` and a clear reason, and local playback stays the output.
- No ctrl wire change: the capabilities `playback-output` and `emby-session-output` and the `SelectPlaybackOutput` command exist; this change only narrows which owner role may advertise and accept them.
- Strengthen the spec requirement so it states the type-and-capability enforcement, not only the observable outcome.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `owner-playback-output`: the requirement "The packaged owner has only local playback" gains the enforcement clause — a packaged owner's runtime cannot express a remote output (Cast or Emby Session) at the type boundary, its hello does not advertise the remote-output capabilities, and a rejected remote selection keeps local playback with a stated reason.

## Impact

- `crates/mbv-daemon`: `DaemonRuntimeHooks` (`run.rs`), `DaemonStartupContext` / `DaemonRole`, the hello capability derivation (`core_ctrl_spawn.rs`), and the two selection guards (`cast/connect.rs`, `emby_session/connect.rs`).
- `crates/mbvd/src/main.rs` and `src/local_daemon.rs`: the two constructors of the runtime hooks.
- Tests: the daemon hello/selection test families (`crates/mbv-daemon/src/tests/`), the ctrl wire tests, and the shell-layer output-selection tests in `src/app/tests/`.
- No `mbv-ctrl` protocol change; no config schema change; no TUI paint change.
