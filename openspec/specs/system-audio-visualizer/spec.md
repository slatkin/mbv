# system-audio-visualizer Specification

## Purpose
TBD - created by archiving change embed-cava-system-audio-visualizer. Update Purpose after archive.
## Requirements
### Requirement: Local playback can show a system-audio visualizer
When the embedded visualizer is enabled for supported local playback, mbv SHALL start CAVA using its normal default system-audio input and SHALL display the resulting spectrum in the embedded visualizer area.

#### Scenario: Visualizer starts with local playback
- **WHEN** supported local playback begins with the visualizer enabled
- **THEN** mbv starts CAVA, consumes its spectrum frames, and displays active bars without changing the mpv audio output configuration

#### Scenario: Unrelated system audio is present
- **WHEN** another application produces audio on the system default audio path
- **THEN** that audio MAY contribute to the displayed spectrum

### Requirement: Visualizer startup does not reroute playback
The visualizer SHALL NOT create, modify, or destroy PulseAudio/PipeWire sinks, sources, links, loopbacks, or modules, and SHALL NOT change or restore mpv `ao` or `audio-device` properties.

#### Scenario: Visualizer is enabled
- **WHEN** mbv starts the visualizer
- **THEN** mpv continues using the exact audio output configuration it had before visualization

#### Scenario: Visualizer is disabled
- **WHEN** mbv stops the visualizer
- **THEN** mbv stops CAVA and leaves the audio graph and mpv output configuration unchanged

### Requirement: CAVA failure is isolated from playback
If CAVA cannot start, cannot open its default input, exits unexpectedly, or emits invalid frames, mbv SHALL log the diagnostic, clear the visualizer state, and SHALL keep playback and normal input handling running.

#### Scenario: CAVA is unavailable
- **WHEN** the `cava` executable cannot be started
- **THEN** mbv keeps playback running and renders inactive visualizer bars

#### Scenario: CAVA emits invalid output
- **WHEN** CAVA emits an incomplete or malformed spectrum frame
- **THEN** mbv rejects that frame, clears active bars, and continues the player session

### Requirement: CAVA resources are bounded and cleaned up
mbv SHALL use a private bounded transport for CAVA spectrum frames, SHALL supervise the CAVA child, and SHALL remove its private temporary resources after normal or failed shutdown.

#### Scenario: Normal shutdown
- **WHEN** local playback ends or visualization is disabled
- **THEN** mbv terminates and reaps CAVA, stops its frame reader, and removes the private transport

#### Scenario: Application exits unexpectedly
- **WHEN** mbv exits while CAVA is running
- **THEN** CAVA does not remain as an unmanaged child and private transport resources are eventually reclaimable

### Requirement: Unsupported playback paths remain unchanged
The visualizer SHALL NOT start CAVA for playback that is not audible on this machine, or for
audio-pipe playback, unless a future capability explicitly adds support. Playback hosted by a
same-host local daemon SHALL NOT be treated as unsupported: it is audible on this machine and is
covered by the supported-playback requirement below.

#### Scenario: Audio-pipe playback is active
- **WHEN** playback uses the configured audio pipe
- **THEN** mbv does not start the system-audio visualizer worker

#### Scenario: Remote playback is active
- **WHEN** playback is handled by a daemon on another machine, or by an attached Emby session on another device
- **THEN** mbv does not start the local CAVA visualizer worker

#### Scenario: Audio-pipe playback through a local daemon
- **WHEN** playback is hosted by a same-host local daemon and the audio pipe is enabled
- **THEN** mbv does not start the system-audio visualizer worker

### Requirement: Same-host local-daemon playback supports the visualizer
CAVA captures the machine's default system audio and has no connection to the playing process, so
audio played by a same-host local daemon is as capturable as in-process playback. A client of a
same-host local daemon SHALL be able to run the system-audio visualizer under the same conditions
as bare-mode playback. mbv SHALL decide this from whether the daemon endpoint is on this machine,
not from whether the Player is in-process.

#### Scenario: Client of a local daemon enables the visualizer
- **WHEN** a client attached to a same-host local daemon has the visualizer enabled and playback is active
- **THEN** mbv starts CAVA with its normal default system-audio input and displays the spectrum
- **THEN** mbv does not alter the daemon's audio output configuration

#### Scenario: Several clients show the visualizer
- **WHEN** more than one client of the same local daemon has the visualizer enabled
- **THEN** each client runs its own CAVA worker against system audio and displays a spectrum

#### Scenario: Client of a remote daemon enables the visualizer
- **WHEN** a client attached to a daemon on another machine has the visualizer enabled
- **THEN** mbv does not start the local CAVA visualizer worker

