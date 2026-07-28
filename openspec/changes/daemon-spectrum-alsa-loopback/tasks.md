## 1. Add daemon spectrum configuration and prerequisite detection

- [x] 1.1 Add configuration values for the spectrum Snapserver host/port, stable Snapclient host ID (default: `puffin-balls`), instance number (default: `2`), and FIFO output path (default: `/tmp/mbv-spectrum.fifo`), following existing parse/default/save patterns.
- [x] 1.2 Implement a side-effect-free prerequisite probe for audio-pipe mode, `cava`, and `snapclient` executables, with actionable diagnostics for each missing prerequisite.
- [x] 1.3 Advertise `spectrum-streaming` only when the prerequisite probe succeeds, and repeat the probe before handling `StartSpectrum`.
- [x] 1.4 Ensure `mbvd.service` contains no PulseAudio, `PULSE_SERVER`, or user `XDG_RUNTIME_DIR` workaround.

## 2. Support explicit CAVA input backends

- [x] 2.1 Separate CAVA input configuration from its existing raw spectrum-output configuration so callers can select the current local system input or a daemon FIFO input.
- [x] 2.2 Generate daemon CAVA configuration with `method = fifo` and the configured FIFO source path while leaving local mbv visualization behavior unchanged.
- [x] 2.3 Capture bounded CAVA stderr and add a startup grace/readiness check so immediate device or configuration failures return an actionable error instead of reporting a successful start.
- [x] 2.4 Update affected existing visualizer tests for backend-specific configuration and readiness behavior.

## 3. Supervise the dedicated spectrum Snapclient

- [x] 3.1 Build the dedicated Snapclient command from configuration, including server host/port, distinct instance, stable host ID, file output mode (`-o file`), FIFO output path, foreground operation, and captured diagnostics.
- [x] 3.2 Add a child guard and bounded startup handling for Snapclient that always terminates and reaps the process on failure or drop.
- [x] 3.3 Refactor `SpectrumState` to own Snapclient, CAVA, the spectrum frame reader, stop signaling, and private resources as one idempotently stoppable session.
- [x] 3.4 Define and implement startup ordering that ensures the FIFO is created before CAVA attempts to read it, and roll back both children when either side fails readiness.
- [x] 3.5 Preserve existing stop triggers for `StopSpectrum`, playback stop, controlling-client disconnect, and daemon shutdown while proving that none of them touches the primary Snapclient or mpv/Snapserver pipe.
- [x] 3.6 Update affected existing daemon lifecycle tests to cover partial startup cleanup, child failure attribution, and idempotent stop behavior without requiring live audio devices in CI.

## 4. Prevent repeated remote startup after failure

- [x] 4.1 Make the remote visualizer start branch honor `visualizer_failed` before sending `StartSpectrum`.
- [x] 4.2 Define the existing explicit toggle and new-playback transitions that clear the failure latch, and ensure periodic synchronization alone cannot clear it.
- [x] 4.3 Update affected client behavior tests to verify one `SpectrumFailed` event produces no automatic `StartSpectrum` retry loop.

## 5. Document and verify the deployment

- [x] 5.1 Update ADR 0008 to replace the Pulse/system-bus daemon capture claim with the dedicated post-Snapcast FIFO topology and its latency rationale.
- [x] 5.2 Document configuring the dedicated visualizer Snapclient identity, verifying it appears in Snapserver, and assigning it to the audible Snapcast stream via Snapserver's web UI or configuration.
- [x] 5.3 Run formatting, compile checks, Clippy, the existing test suite, and static diagnostics for all changed Rust files.
- [x] 5.4 On `music.local`, verify that the dedicated visualizer Snapclient appears with its stable identity, receives the same stream as the primary client, and produces active CAVA frames.
- [ ] 5.5 During target-host playback, compare bar timing with audible output and record residual post-playout lag; confirm there is no Snapcast-buffer-sized visual lead.
- [x] 5.6 Verify toggle-off, playback stop, client disconnect, CAVA failure, Snapclient failure, and daemon shutdown all clean up spectrum resources without interrupting primary playback.
