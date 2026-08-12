## MODIFIED Requirements

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

## ADDED Requirements

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
