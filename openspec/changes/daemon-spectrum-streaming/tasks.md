## 1. Extract CAVA worker to mbv-core

- [x] 1.1 Move `CavaWorker`, `parse_ascii_frame`, `cava_config`, `spawn_cava`, `create_private_resources`, `cleanup_private_resources`, and related types from `src/app/visualizer.rs` to `crates/mbv-core/src/visualizer.rs`.
- [x] 1.2 Change visibility from `pub(super)` to `pub` on `CavaWorker` and its methods (`start`, `take_latest_frame`, `stop`).
- [x] 1.3 Gate the visualizer module with `#[cfg(unix)]` in `crates/mbv-core/src/lib.rs`.
- [x] 1.4 Update mbv's `src/app/visualizer.rs` to import `CavaWorker` from `mbv-core` and remove the duplicated code.
- [x] 1.5 Update mbv's `Cargo.toml` if needed (mbv-core is already a dependency).
- [x] 1.6 Run tests to verify the extraction did not break existing functionality.

## 2. Add spectrum protocol messages

- [x] 2.1 Add `CTRL_CAP_SPECTRUM = "spectrum-streaming"` capability constant to `crates/mbv-core/src/ctrl.rs`.
- [x] 2.2 Add `supports_spectrum: bool` field to `CtrlCompatibility` struct.
- [x] 2.3 Add `CtrlCmd::StartSpectrum` and `CtrlCmd::StopSpectrum` variants.
- [x] 2.4 Add `CtrlEvent::Spectrum { bars: Vec<f32> }` and `CtrlEvent::SpectrumFailed { reason: String }` variants.
- [x] 2.5 Add `PlayerEvent::Spectrum(Vec<f32>)` and `PlayerEvent::SpectrumFailed(String)` variants.
- [x] 2.6 Add wire-stability serialization tests for the new messages (pin JSON shape of `Spectrum { bars: vec![0.0; 64] }`).

## 3. Implement daemon-side spectrum streaming

- [x] 3.1 Add `DaemonEvent::Spectrum(Vec<f32>)` variant to mbvd's event enum.
- [x] 3.2 Define `SpectrumState { worker: Option<CavaWorker>, reader: Option<JoinHandle<()>> }` in daemon main loop with idempotent `stop()` method.
- [x] 3.3 Daemon appends `spectrum-streaming` to `CtrlHello.capabilities` after construction (not via `CtrlHello::current()`, which is shared with client).
- [x] 3.4 Populate `CtrlCompatibility::supports_spectrum` from daemon's hello during `perform_handshake` on client side.
- [x] 3.5 Spawn a dedicated spectrum reader thread on `StartSpectrum`: reads frames from `CavaWorker::take_latest_frame()` and sends `DaemonEvent::Spectrum` to the main loop channel.
- [x] 3.6 Dispatch `DaemonEvent::Spectrum` in the main loop to `CtrlEvent::Spectrum` for the connected client.
- [x] 3.7 Add explicit check in `DaemonEvent::Player(pe)` arm: when `pe` is `PlayerEvent::Stopped` and spectrum is active, call `spectrum_state.stop()`. Do not auto-stop on pause.
- [x] 3.8 Stop the `CavaWorker` and join the reader thread on `CtrlDisconnected` (client disconnect, crash, session switch).
- [x] 3.9 Stop the `CavaWorker` and join the reader thread on `StopSpectrum` command.
- [x] 3.10 Detect `CavaWorker` failure (reader thread exits or `take_latest_frame` returns `Err`) and send `CtrlEvent::SpectrumFailed` to the client.
- [x] 3.11 Add focused tests for daemon spectrum lifecycle (start, stop, auto-stop on playback stop, disconnect cleanup, failure propagation). Use `#[cfg(test)]` stubs or mock CAVA for CI environments without cava installed.

## 4. Implement client-side spectrum reception

- [x] 4.1 Add `send_ctrl_cmd(CtrlCmd) -> bool` method to `PlayerProxy` (returns `false` for local players, forwards to `RemotePlayer::send_ctrl_cmd` for remote players).
- [x] 4.2 Add `supports_spectrum() -> bool` method to `PlayerProxy` that checks `CtrlCompatibility::supports_spectrum`.
- [x] 4.3 Update mbv's `sync_visualizer` to branch on connection type: start local `CavaWorker` for standalone playback, send `StartSpectrum` via `PlayerProxy::send_ctrl_cmd` for daemon playback (after checking `supports_spectrum()`).
- [x] 4.4 Handle `CtrlEvent::Spectrum` in `apply_ctrl_event` by emitting `PlayerEvent::Spectrum(bars)`. Handle `CtrlEvent::SpectrumFailed` by emitting `PlayerEvent::SpectrumFailed(reason)`.
- [x] 4.5 Handle `PlayerEvent::Spectrum(Vec<f32>)` in root crate's event loop: write to `visualizer_frame`. Handle `PlayerEvent::SpectrumFailed(String)` by logging the failure, setting `visualizer_failed = true`, and rendering inactive bars.
- [x] 4.6 Send `StopSpectrum` via `PlayerProxy::send_ctrl_cmd` when the user toggles the visualizer off while connected to a daemon.
- [x] 4.7 Send `StopSpectrum` via `PlayerProxy::send_ctrl_cmd` on session switch, teardown, and connection-type transition (e.g., daemon → Emby session) in `stop_visualizer_worker()` and related paths.
- [x] 4.8 Disable the visualizer toggle gracefully when the daemon does not advertise `spectrum-streaming` (checked via `supports_spectrum()`).
- [x] 4.9 Add tests for connection-type branching, spectrum frame reception, capability check, and teardown cleanup.

## 5. Verify and document

- [x] 5.1 Run formatting, Clippy, and all tests in the isolated worktree.
- [x] 5.2 Manually verify visualizer startup and shutdown for both local and daemon playback paths.
- [x] 5.3 Verify CAVA cleanup on client disconnect and playback stop.
- [x] 5.4 Update ADR 0008 (audio visualizer) to document the daemon spectrum streaming path.
- [x] 5.5 Add `cava` as a documented runtime dependency for mbvd environments.
