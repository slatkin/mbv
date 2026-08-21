## RENAMED Requirements

- FROM: `### Requirement: CAVA resources are bounded and cleaned up`
  TO: `### Requirement: PipeWire capture resources are bounded and cleaned up`

## MODIFIED Requirements

### Requirement: Local playback can show a system-audio visualizer
When the embedded visualizer is enabled for supported local playback, mbv SHALL capture stereo PCM from the current default PipeWire system-output monitor and SHALL display it as a stereo vectorscope in the existing visualizer area.

#### Scenario: Visualizer starts with local playback
- **WHEN** supported local playback begins with the visualizer enabled
- **THEN** mbv starts PipeWire system-output capture and displays vectorscope points without changing the mpv audio output configuration

#### Scenario: Unrelated system audio is present
- **WHEN** another application produces audio on the current default system output
- **THEN** that application's stereo PCM MAY contribute to the displayed vectorscope

### Requirement: Visualizer startup does not reroute playback
The visualizer SHALL NOT create, modify, or destroy PipeWire nodes, links, sinks, sources, loopbacks, or modules, and SHALL NOT change or restore mpv `ao` or `audio-device` properties.

#### Scenario: Visualizer is enabled
- **WHEN** mbv starts the visualizer
- **THEN** mpv continues using the exact audio output configuration it had before visualization

#### Scenario: Visualizer is disabled
- **WHEN** mbv stops the visualizer
- **THEN** mbv disconnects its capture stream and leaves the audio graph and mpv output configuration unchanged

### Requirement: PipeWire capture resources are bounded and cleaned up
mbv SHALL keep captured PCM in a bounded latest-sample buffer, SHALL supervise the PipeWire capture thread and stream, and SHALL release those resources after normal or failed shutdown.

#### Scenario: Capture outpaces rendering
- **WHEN** PipeWire supplies samples faster than the TUI consumes visualization windows
- **THEN** mbv overwrites the oldest buffered samples and retains the newest complete stereo samples without unbounded growth

#### Scenario: Normal shutdown
- **WHEN** local playback ends or visualization is disabled
- **THEN** mbv disconnects the capture stream, stops its capture loop, joins its thread, and releases its sample buffer

#### Scenario: Application exits unexpectedly
- **WHEN** mbv exits while visualization is active
- **THEN** the capture stream and worker do not remain active after the mbv process terminates

### Requirement: Unsupported playback paths remain unchanged
The visualizer SHALL NOT start system-output capture for playback that is not audible on this machine, or for audio-pipe playback, unless a future capability explicitly adds support. Playback hosted by a same-host Local daemon SHALL NOT be treated as unsupported: it is audible on this machine and is covered by the supported-playback requirement below.

#### Scenario: Audio-pipe playback is active
- **WHEN** playback uses the configured audio pipe
- **THEN** mbv does not start the system-audio visualizer worker

#### Scenario: Remote playback is active
- **WHEN** playback is handled by a daemon on another machine, or by an attached Emby session on another device
- **THEN** mbv does not start local PipeWire capture

#### Scenario: Audio-pipe playback through a local daemon
- **WHEN** playback is hosted by a same-host Local daemon and the audio pipe is enabled
- **THEN** mbv does not start the system-audio visualizer worker

### Requirement: Same-host Local-daemon playback supports the visualizer
PipeWire captures the machine's default system output and has no connection to the playing process, so audio played by a same-host Local daemon is as capturable as in-process playback. A client of a same-host Local daemon SHALL be able to run the system-audio visualizer under the same conditions as bare-mode playback. mbv SHALL decide this from whether the daemon endpoint is on this machine, not from whether the Player is in-process.

#### Scenario: Client of a Local daemon enables the visualizer
- **WHEN** a client attached to a same-host Local daemon has the visualizer enabled and playback is active
- **THEN** mbv captures the default PipeWire system output and displays the stereo vectorscope
- **THEN** mbv does not alter the daemon's audio output configuration

#### Scenario: Several clients show the visualizer
- **WHEN** more than one client of the same Local daemon has the visualizer enabled
- **THEN** each client runs its own PipeWire capture stream and displays a vectorscope

#### Scenario: Client of a remote daemon enables the visualizer
- **WHEN** a client attached to a daemon on another machine has the visualizer enabled
- **THEN** mbv does not start local PipeWire capture

## ADDED Requirements

### Requirement: PipeWire visualizer failure is isolated from playback
If PipeWire is unavailable, the default system-output monitor cannot be captured, the stream disconnects, or the negotiated samples cannot be consumed, mbv SHALL log the diagnostic, clear active vectorscope points, and SHALL keep playback and normal input handling running.

#### Scenario: PipeWire capture is unavailable
- **WHEN** the visualizer cannot connect to PipeWire or capture the default system output
- **THEN** mbv keeps playback running and renders an inactive visualizer

#### Scenario: Capture stream fails during playback
- **WHEN** the active PipeWire capture stream disconnects or supplies unusable sample data
- **THEN** mbv clears active vectorscope points and continues the player session

### Requirement: Stereo PCM renders as a vectorscope
The visualizer SHALL map each newest complete stereo sample pair into the existing panel with left-channel amplitude controlling horizontal displacement and right-channel amplitude controlling vertical displacement. The figure SHALL be cleared and rebuilt from the newest sample window on each visual frame.

#### Scenario: Stereo signal is captured
- **WHEN** the captured left and right channels contain non-silent samples
- **THEN** mbv draws vectorscope points at positions determined by the paired channel amplitudes

#### Scenario: Captured output is silent
- **WHEN** the newest complete sample window contains silence
- **THEN** the visualizer area contains no active vectorscope points

### Requirement: Vectorscope glyph is configurable
mbv SHALL persist one valid single-cell Unicode vectorscope glyph and SHALL default it to `●` when no valid value is configured.

#### Scenario: Configured glyph is valid
- **WHEN** a valid single-cell Unicode glyph is configured
- **THEN** every vectorscope point uses that glyph

#### Scenario: Configured glyph is absent or invalid
- **WHEN** the glyph setting is missing, empty, a control character, or does not occupy exactly one terminal cell
- **THEN** mbv uses `●` without preventing startup or playback

### Requirement: Vectorscope favors fresh frames
During steady capture, mbv SHALL target 60 visual frames per second and SHALL avoid queueing stale visualization frames. On a terminal capable of sustaining that cadence, at least 50 visual frames per second SHALL contain the newest sample window available at render time.

#### Scenario: Capture and terminal remain responsive
- **WHEN** supported local playback produces continuous audio and the terminal can sustain the target cadence
- **THEN** measurement at the terminal render boundary observes at least 50 fresh vectorscope frames per second

#### Scenario: Rendering temporarily stalls
- **WHEN** another TUI operation delays a visualizer render
- **THEN** the next vectorscope frame uses the newest complete sample window rather than replaying queued stale windows

## REMOVED Requirements

### Requirement: CAVA failure is isolated from playback
**Reason**: The visualizer no longer starts or consumes output from Cava; PipeWire capture has its own replacement failure contract.

**Migration**: Install and run PipeWire. Visualizer failure remains non-fatal to playback under the new `PipeWire visualizer failure is isolated from playback` requirement.
