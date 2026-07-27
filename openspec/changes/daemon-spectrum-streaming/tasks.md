## 1. Extract CAVA worker to mbv-core

- [ ] 1.1 Move `CavaWorker`, `parse_ascii_frame`, `cava_config`, `spawn_cava`, `create_private_resources`, `cleanup_private_resources`, and related types from `src/app/visualizer.rs` to `crates/mbv-core/src/visualizer.rs`.
- [ ] 1.2 Change visibility from `pub(super)` to `pub` on `CavaWorker` and its methods (`start`, `take_latest_frame`, `stop`).
- [ ] 1.3 Update mbv's `src/app/visualizer.rs` to import `CavaWorker` from `mbv-core` and remove the duplicated code.
- [ ] 1.4 Update mbv's `Cargo.toml` if needed (mbv-core is already a dependency).
- [ ] 1.5 Run tests to verify the extraction did not break existing functionality.

## 2. Add spectrum protocol messages

- [ ] 2.1 Add `CTRL_CAP_SPECTRUM = "spectrum-streaming"` capability constant to `crates/mbv-core/src/ctrl.rs` and include it in `CtrlHello::current()`.
- [ ] 2.2 Add `CtrlCmd::StartSpectrum` and `CtrlCmd::StopSpectrum` variants.
- [ ] 2.3 Add `CtrlEvent::Spectrum { bars: Vec<f32> }` and `CtrlEvent::SpectrumFailed { reason: String }` variants.
- [ ] 2.4 Add wire-stability serialization tests for the new messages (pin JSON shape of `Spectrum { bars: vec![0.0; 64] }`).

## 3. Implement daemon-side spectrum streaming

- [ ] 3.1 Add `DaemonEvent::Spectrum(Vec<f32>)` variant to mbvd's event enum.
- [ ] 3.2 Spawn a dedicated spectrum reader thread on `StartSpectrum`: reads frames from `CavaWorker::take_latest_frame()` and sends `DaemonEvent::Spectrum` to the main loop channel.
- [ ] 3.3 Dispatch `DaemonEvent::Spectrum` in the main loop to `CtrlEvent::Spectrum` for the connected client.
- [ ] 3.4 Auto-stop the `CavaWorker` and join the reader thread when playback stops (player `active` becomes `false`). Do not auto-stop on pause.
- [ ] 3.5 Stop the `CavaWorker` and join the reader thread on `CtrlDisconnected` (client disconnect, crash, session switch).
- [ ] 3.6 Stop the `CavaWorker` and join the reader thread on `StopSpectrum` command.
- [ ] 3.7 Detect `CavaWorker` failure (reader thread exits or `take_latest_frame` returns `Err`) and send `CtrlEvent::SpectrumFailed` to the client.
- [ ] 3.8 Add focused tests for daemon spectrum lifecycle (start, stop, auto-stop on playback stop, disconnect cleanup, failure propagation). Use `#[cfg(test)]` stubs or mock CAVA for CI environments without cava installed.

## 4. Implement client-side spectrum reception

- [ ] 4.1 Add `send_ctrl_cmd(CtrlCmd)` method to `RemotePlayer` for sending non-player commands.
- [ ] 4.2 Update mbv's `sync_visualizer` to branch on connection type: start local `CavaWorker` for standalone playback, send `StartSpectrum` for daemon playback (after checking `spectrum-streaming` capability).
- [ ] 4.3 Handle `CtrlEvent::Spectrum` in the ctrl event path: write incoming frames to `visualizer_frame`.
- [ ] 4.4 Handle `CtrlEvent::SpectrumFailed` by logging the failure, setting `visualizer_failed = true`, and rendering inactive bars.
- [ ] 4.5 Send `StopSpectrum` when the user toggles the visualizer off while connected to a daemon.
- [ ] 4.6 Send `StopSpectrum` on session switch, teardown, and connection-type transition (e.g., daemon → Emby session) in `stop_visualizer_worker()` and related paths.
- [ ] 4.7 Disable the visualizer toggle gracefully when the daemon does not advertise `spectrum-streaming`.
- [ ] 4.8 Add tests for connection-type branching, spectrum frame reception, capability check, and teardown cleanup.

## 5. Verify and document

- [ ] 5.1 Run formatting, Clippy, and all tests in the isolated worktree.
- [ ] 5.2 Manually verify visualizer startup and shutdown for both local and daemon playback paths.
- [ ] 5.3 Verify CAVA cleanup on client disconnect and playback stop.
- [ ] 5.4 Update ADR 0008 (audio visualizer) to document the daemon spectrum streaming path.
- [ ] 5.5 Add `cava` as a documented runtime dependency for mbvd environments.
