# Tasks

## 1. The transport command

- [x] 1.1 In `crates/mbv-ctrl/src/commands.rs`, add `TransportCommand::TogglePanel` with a doc
      comment that names the effect (hide the pinned panel when it is shown, show it when it is
      hidden) and says the Owner process runs it. Verify: `cargo check -p mbv-ctrl`.
- [x] 1.2 Add the documented no-op arm for the new variant in the two transports that match
      every `TransportCommand`: `RemotePlayer::send_transport`
      (`crates/mbv-remote-player/src/lib.rs`) and the shell's `drain_transport_events`
      (`src/app/shell/run/drains.rs`). Neither transport carries a panel request; the comment says
      so. Verify: `cargo check --workspace` shows no non-exhaustive match.

## 2. The Owner runs the request

- [x] 2.1 In `crates/mbv-daemon/src/core.rs`, add the `TogglePanelHook` type and the
      `toggle_panel` field of `DaemonRuntimeHooks`, beside `swap_command` and `notify`. The hook
      starts the request and returns; it never blocks the event loop. Verify:
      `cargo check -p mbv-daemon`.
- [x] 2.2 In `crates/mbv-daemon/src/event_loop.rs`, handle `TransportCommand::TogglePanel`: read
      the ctrl registry, and run the hook only while a pinned Client is attached. With no pinned
      Client the request does nothing and the Owner logs it. Verify:
      `cargo nextest run -p mbv-daemon toggle`.
- [x] 2.3 In `crates/mbv-daemon/src/run.rs`, carry the hook from `DaemonRuntimeHooks` through
      `DaemonStarted` into the `DaemonLoop`. In `crates/mbvd/src/main.rs`, supply a hook that does
      nothing: mbvd has no desktop session, no Tray and no pinned panel. Verify:
      `cargo check --workspace`.

## 3. The runtime sends the request

- [x] 3.1 In `src/pin.rs`, add the runtime entry point that sends the `pinwin toggle mbv`
      request on a worker thread and reports a failure as a desktop notification, reusing
      `toggle_argv`, `run_request` and `notify`. The request is the one `mbv --toggle` sends.
      Verify: `cargo nextest run -p mbv toggle`.
- [x] 3.2 In `src/local_daemon.rs`, wire the hook to that entry point. Verify:
      `cargo check -p mbv`.

## 4. The Tray click

- [x] 4.1 In `crates/mbv-desktop/src/tray.rs`, implement the `activate` handler of the Tray so
      that a left click on the icon sends `TransportCommand::TogglePanel`. Add the test that
      pins the contract: the click sends the command, and the menu still carries its items.
      Verify: `cargo nextest run -p mbv-desktop tray`.

## 5. Domain docs

- [x] 5.1 In `CONTEXT.md`, update the *Tray* entry to name the icon click: it toggles the pinned
      panel while a pinned Client is attached. Check the *Avoid* lists for a clash with the new
      wording; ask before any rename.

## 6. Integration gates

- [x] 6.1 Run `cargo fmt --all`, then `cargo clippy --workspace --all-targets -- -D warnings`, and
      verify both are clean.
- [x] 6.2 Run `cargo nextest run --workspace` and verify it passes.
- [x] 6.3 Run `make check-code-file-lines` and verify no file is over 800 lines.

## Workflow follow-up

- Sync the delta specs to `openspec/specs/` and archive the change after review.
