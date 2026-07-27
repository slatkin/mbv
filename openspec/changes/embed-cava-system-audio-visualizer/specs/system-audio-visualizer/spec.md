## ADDED Requirements

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
The visualizer SHALL NOT start CAVA for remote players or audio-pipe playback unless a future capability explicitly adds support.

#### Scenario: Audio-pipe playback is active
- **WHEN** playback uses the configured audio pipe
- **THEN** mbv does not start the system-audio visualizer worker

#### Scenario: Remote playback is active
- **WHEN** playback is handled by a remote player or daemon session
- **THEN** mbv does not start the local CAVA visualizer worker
