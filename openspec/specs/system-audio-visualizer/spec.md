# system-audio-visualizer Specification

## Purpose

The embedded visualizer captures the audible default system output through PipeWire and renders a stereo vectorscope without changing playback routing.

## Requirements

### Requirement: Local playback can show a system-audio vectorscope

When enabled during supported local playback, mbv SHALL capture stereo PCM from the current default PipeWire system-output monitor and display it in the existing visualizer area.

#### Scenario: Visualizer starts with local playback

- **WHEN** supported local playback begins with the visualizer enabled
- **THEN** mbv starts PipeWire capture and displays vectorscope points without changing mpv audio output configuration

#### Scenario: Unrelated system audio is present

- **WHEN** another application produces audio on the current default system output
- **THEN** that application's stereo PCM MAY contribute to the vectorscope

### Requirement: Visualizer does not reroute playback

The visualizer SHALL NOT create, modify, or destroy persistent PipeWire nodes, links, sinks, sources, loopbacks, or modules, and SHALL NOT change or restore mpv `ao` or `audio-device` properties.

#### Scenario: Playback routing remains unchanged

- **WHEN** the visualizer is enabled during playback
- **THEN** it leaves persistent PipeWire routing and mpv audio properties unchanged

### Requirement: PipeWire visualizer failure is isolated from playback

If PipeWire is unavailable, the default monitor cannot be captured, the stream disconnects, or sample data cannot be consumed, mbv SHALL log the diagnostic, clear active vectorscope points, and keep playback and normal input handling running.

#### Scenario: Capture fails

- **WHEN** PipeWire capture becomes unavailable or its stream disconnects
- **THEN** mbv logs the diagnostic, clears active vectorscope points, and keeps playback and input handling running

### Requirement: PipeWire capture resources are bounded and cleaned up

mbv SHALL keep captured PCM in a bounded latest-sample buffer, supervise the PipeWire capture thread and stream, and release those resources after normal or failed shutdown.

#### Scenario: Capture outpaces rendering

- **WHEN** PipeWire supplies samples faster than the TUI consumes windows
- **THEN** mbv overwrites old samples and retains only the newest complete stereo samples

#### Scenario: Normal shutdown

- **WHEN** local playback ends or visualization is disabled
- **THEN** mbv disconnects capture, stops its loop, joins its worker, and releases the sample buffer

### Requirement: Unsupported playback paths remain unchanged

The visualizer SHALL NOT start system-output capture for attached Emby Session playback or audio-pipe playback. Playback hosted by a same-host Local daemon SHALL NOT be treated as unsupported. Direct remote Player-owner playback SHALL permit local capture because external local forwarding such as Snapcast can make that playback audible on this machine; when no such forwarding exists, the local system-output monitor is simply silent.

#### Scenario: Attached playback bypasses capture

- **WHEN** playback is hosted by an attached Emby Session or an audio pipe
- **THEN** the visualizer does not start system-output capture
- **AND** same-host Local daemon playback and direct remote Player-owner playback retain their stated capture behavior

### Requirement: Stereo PCM renders as a vectorscope

The visualizer SHALL apply a fixed internal display gain before clamping and mapping left-channel amplitude to horizontal displacement and right-channel amplitude to vertical displacement, clear and rebuild the figure from the newest sample window on each frame, deduplicate terminal cells, and suppress points for a silent window. The gain SHALL not change captured samples or add a user-facing configuration setting.

#### Scenario: A non-silent sample window renders

- **WHEN** the newest captured window contains stereo PCM samples
- **THEN** mbv maps the samples to vectorscope positions using the fixed display gain and renders the newest figure without duplicate terminal cells

### Requirement: Vectorscope glyph is configurable

mbv SHALL persist one valid single-cell Unicode vectorscope glyph and SHALL default it to `●` when the setting is missing, empty, a control character, or does not occupy exactly one terminal cell.

#### Scenario: An invalid glyph setting is loaded

- **WHEN** the persisted glyph is missing, empty, a control character, or not exactly one terminal cell
- **THEN** mbv uses the default `●` glyph

### Requirement: Vectorscope favors fresh frames

During steady capture, mbv SHALL target 60 visual frames per second and SHALL skip stale visualization windows rather than replaying queued frames. On a terminal capable of sustaining the cadence, at least 50 frames per second SHALL use the newest sample window available at render time.

#### Scenario: Capture produces stale windows

- **WHEN** capture supplies more visualization windows than rendering can consume
- **THEN** mbv skips stale windows and renders the newest available window rather than replaying the queue
