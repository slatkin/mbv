# clocked-audio-output Specification

## Purpose
Define responsive packaged-daemon playback through a hardware-paced ALSA endpoint without making mbv responsible for the surrounding host audio graph.

## Requirements

### Requirement: Packaged mbvd defaults to clocked ALSA output
Packaged `mbvd` SHALL use clocked ALSA device output when `audio_pipe_enabled` is absent or false. Owner-local configuration SHALL accept an `audio_device` value equal to `alsa` or beginning with `alsa/`; an absent value SHALL resolve to `alsa`. Bare-mode and Local-daemon output defaults SHALL remain unchanged.

#### Scenario: Packaged daemon uses inherited output
- **WHEN** packaged `mbvd` starts a Playback run without an explicit pipe selection or ALSA device
- **THEN** the run uses the default ALSA device
- **THEN** it does not configure mpv as a PCM file writer or create a FIFO for that run

#### Scenario: Packaged daemon uses a selected ALSA endpoint
- **WHEN** packaged `mbvd` starts a Playback run with `audio_pipe_enabled = false` and `audio_device = "alsa/hw:Loopback,0,0"`
- **THEN** that run selects exactly `alsa/hw:Loopback,0,0`

#### Scenario: Another Player owner starts playback
- **WHEN** bare mode or the Local daemon starts a Playback run without an explicit audio-device setting
- **THEN** its existing audio-output selection remains unchanged

### Requirement: ALSA device configuration is validated and daemon-bound
The packaged daemon SHALL reject an empty or non-ALSA `audio_device` value during normal configuration validation. It SHALL load the resolved owner-local value when the daemon starts and apply it to later clocked Playback runs; changing the configuration file SHALL require a daemon restart. If the selected ALSA endpoint cannot be opened, the run SHALL report the output failure and SHALL NOT silently fall back to pipe output or another device.

#### Scenario: Invalid device is configured
- **WHEN** packaged-daemon configuration contains an empty identifier or an identifier outside the `alsa` output
- **THEN** configuration validation rejects it before playback starts

#### Scenario: Device configuration changes on disk
- **WHEN** `audio_device` changes in owner-local configuration while packaged `mbvd` is running
- **THEN** the running daemon and its Playback runs retain the startup value
- **THEN** a restarted daemon loads the new value

#### Scenario: Selected endpoint is unavailable
- **WHEN** a Playback run cannot open its selected ALSA endpoint
- **THEN** the run reports an audio-output failure identifying the configured endpoint without exposing unrelated configuration
- **THEN** it does not open the legacy pipe or an alternate audio device

### Requirement: Clocked output does not inherit pipe control stalls
With ready media and a writable ALSA endpoint, accepted startup, pause, and resume commands SHALL reach the corresponding observed player transition without waiting for a pipe write, pipe-reader availability, the pipe startup guard, or an expected downstream playout delay. These transitions SHALL describe the observable Player boundary and SHALL NOT claim downstream audibility.

#### Scenario: Clocked playback starts
- **WHEN** ready media starts through a writable ALSA endpoint
- **THEN** output startup proceeds without entering a pipe startup phase or downstream buffering estimate

#### Scenario: Clocked playback is paused or resumed
- **WHEN** the daemon accepts Pause or Resume during clocked ALSA playback
- **THEN** the corresponding observed player state transition is not gated by FIFO backpressure or a pipe poll timeout

### Requirement: The external ALSA and distribution graph remains operator-owned
mbv SHALL NOT create, load, expose, configure, or remove ALSA hardware, kernel modules, loopback endpoints, or device mappings. It SHALL NOT discover, configure, restart, or tune Snapserver, Snapclient, or another downstream consumer. Documentation SHALL describe the required external endpoints and SHALL distinguish Player-boundary timing from downstream playout latency.

#### Scenario: ALSA loopback deployment is configured
- **WHEN** an operator selects an ALSA loopback playback endpoint for packaged `mbvd`
- **THEN** mbv uses that endpoint without changing the host's module or device configuration
- **THEN** the operator remains responsible for exposing the paired capture endpoint to the downstream consumer

#### Scenario: Downstream buffering remains configured
- **WHEN** Snapserver or another consumer adds playout buffering after the ALSA endpoint
- **THEN** mbv neither changes that buffer nor reports its completion as confirmed audibility