# daemon-spectrum-streaming Specification

## Purpose
TBD - created by archiving change daemon-spectrum-streaming. Update Purpose after archive.
## Requirements
### Requirement: CAVA worker extraction
The CAVA worker (`CavaWorker`) SHALL be located in `crates/mbv-core/src/visualizer.rs` and usable by both mbv and mbvd.

#### Scenario: mbv uses extracted CavaWorker
- **WHEN** mbv starts a local visualizer for standalone playback
- **THEN** mbv imports and uses `CavaWorker` from `mbv-core::visualizer`

#### Scenario: mbvd uses extracted CavaWorker
- **WHEN** mbvd receives a `StartSpectrum` command from a connected client
- **THEN** mbvd imports and uses `CavaWorker` from `mbv-core::visualizer`

### Requirement: Spectrum capability advertisement
The daemon SHALL advertise `spectrum-streaming` in `CtrlHello.capabilities` to signal spectrum support. The capability is always advertised; if `cava` is not installed, the daemon relies on `SpectrumFailed` as the fallback.

#### Scenario: Daemon advertises spectrum capability
- **WHEN** mbvd starts
- **THEN** mbvd includes `spectrum-streaming` in its `CtrlHello.capabilities`

#### Scenario: Client checks capability before enabling
- **WHEN** the user toggles the visualizer on while connected to a daemon
- **THEN** mbv checks the daemon's advertised capabilities for `spectrum-streaming` before sending `StartSpectrum`. If the capability is absent, the visualizer toggle is disabled gracefully.

### Requirement: StartSpectrum command
The client SHALL send `CtrlCmd::StartSpectrum` to request the daemon to start CAVA and stream spectrum frames.

#### Scenario: Client requests spectrum streaming
- **WHEN** the user toggles the visualizer on while mbv is connected to mbvd and the daemon advertises `spectrum-streaming`
- **THEN** mbv sends `CtrlCmd::StartSpectrum` to the daemon

### Requirement: StopSpectrum command
The client SHALL send `CtrlCmd::StopSpectrum` to request the daemon to stop CAVA.

#### Scenario: Client stops spectrum streaming on toggle off
- **WHEN** the user toggles the visualizer off while mbv is connected to mbvd
- **THEN** mbv sends `CtrlCmd::StopSpectrum` to the daemon

#### Scenario: Client stops spectrum on session switch
- **WHEN** mbv switches from a daemon connection to a different session type (local, Emby session, different daemon) while the visualizer is active
- **THEN** mbv sends `CtrlCmd::StopSpectrum` to the previous daemon before tearing down the connection

#### Scenario: Client stops spectrum on teardown
- **WHEN** mbv shuts down or disconnects while the visualizer is active
- **THEN** mbv sends `CtrlCmd::StopSpectrum` to the daemon as part of teardown

### Requirement: Spectrum event
The daemon SHALL send `CtrlEvent::Spectrum { bars: Vec<f32> }` to stream normalized spectrum frames to the connected client.

#### Scenario: Daemon streams spectrum frames
- **WHEN** the daemon's spectrum reader thread produces a spectrum frame from the CAVA worker
- **THEN** the daemon sends `CtrlEvent::Spectrum` with the normalized bar values (64 values, 0.0–1.0)

### Requirement: SpectrumFailed event
The daemon SHALL send `CtrlEvent::SpectrumFailed { reason: String }` when CAVA fails to start or stops unexpectedly.

#### Scenario: CAVA fails to start on daemon
- **WHEN** the daemon receives `StartSpectrum` but CAVA is unavailable or fails to start
- **THEN** the daemon sends `CtrlEvent::SpectrumFailed` with a human-readable reason

#### Scenario: CAVA crashes during streaming
- **WHEN** the CAVA worker stops unexpectedly during active spectrum streaming
- **THEN** the daemon sends `CtrlEvent::SpectrumFailed` with the failure reason

### Requirement: Auto-stop on full playback stop
The daemon SHALL automatically stop the CAVA worker when audio playback fully stops (player `active` becomes `false`). The daemon SHALL NOT auto-stop CAVA on pause.

#### Scenario: Playback stops while spectrum is active
- **WHEN** the player status `active` becomes `false` (Stopped event) while CAVA is running
- **THEN** the daemon stops the CAVA worker and joins the spectrum reader thread without requiring a `StopSpectrum` command from the client

#### Scenario: Playback paused while spectrum is active
- **WHEN** the player is paused while CAVA is running
- **THEN** the daemon keeps CAVA running (audio remains on the system bus; bars show silence)

### Requirement: Stop CAVA on client disconnect
The daemon SHALL stop the CAVA worker and join the spectrum reader thread when the ctrl client disconnects.

#### Scenario: Client disconnects while spectrum is active
- **WHEN** the ctrl client disconnects (crash, network loss, session switch) while CAVA is running
- **THEN** the daemon stops the CAVA worker and joins the spectrum reader thread as part of the `CtrlDisconnected` handler

### Requirement: Source-agnostic rendering
The visualizer renderer SHALL accept spectrum frames from either a local CAVA worker or a remote daemon without code changes.

#### Scenario: Local playback renders spectrum
- **WHEN** mbv is in standalone mode with local playback
- **THEN** the visualizer renders frames from the local CavaWorker

#### Scenario: Daemon playback renders spectrum
- **WHEN** mbv is connected to mbvd with spectrum streaming active
- **THEN** the visualizer renders frames received via `CtrlEvent::Spectrum`, written into the same `visualizer_frame` field used by the local path

### Requirement: Spectrum reader thread
The daemon SHALL use a dedicated spectrum reader thread to decouple spectrum frame production from the main event loop poll timeout.

#### Scenario: Spectrum frame rate is independent of main loop
- **WHEN** the daemon is streaming spectrum frames
- **THEN** the spectrum reader thread reads from the CAVA worker at the worker's native rate (~6fps given the 100ms poll interval), independent of the main event loop's 250ms poll timeout

