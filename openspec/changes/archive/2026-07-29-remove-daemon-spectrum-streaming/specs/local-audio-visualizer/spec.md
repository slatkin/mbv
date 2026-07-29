## ADDED Requirements

### Requirement: Visualizer is local-audio only
The system SHALL run CAVA visualization only from the local application's system-audio input during local playback. A daemon-connected client SHALL NOT offer or start a daemon-backed visualizer.

#### Scenario: Local playback enables visualization
- **WHEN** a user enables the visualizer while using the local player
- **THEN** mbv SHALL start its local CAVA worker and render its local spectrum frames

#### Scenario: Daemon connection does not enable visualization
- **WHEN** a user is connected to mbvd
- **THEN** mbv SHALL keep the daemon spectrum visualizer unavailable and SHALL NOT send a spectrum-start command

### Requirement: Daemon does not process spectrum audio
mbvd SHALL NOT start CAVA, Snapclient, or FIFO processing for visualization and SHALL NOT emit spectrum frames over its control connection.

#### Scenario: Daemon handles normal playback
- **WHEN** mbvd starts or plays media
- **THEN** it SHALL not create a visualization CAVA process, dedicated spectrum Snapclient, or spectrum FIFO

#### Scenario: Legacy spectrum command reaches updated daemon
- **WHEN** an older peer sends a removed spectrum control command to updated mbvd
- **THEN** mbvd SHALL keep the connection usable and SHALL NOT start visualization processing

### Requirement: Daemon spectrum setup is not required
mbvd SHALL not require dedicated spectrum Snapclient/FIFO configuration or the `snapclient` executable for normal daemon operation. Unrelated audio-pipe behavior SHALL remain unaffected.

#### Scenario: Daemon starts without Snapclient installed
- **WHEN** mbvd starts on a host without the `snapclient` executable
- **THEN** normal daemon startup and playback SHALL remain available

#### Scenario: Existing audio pipe is configured
- **WHEN** a daemon has unrelated audio-pipe configuration enabled
- **THEN** removing spectrum streaming SHALL not change that audio-pipe configuration's normal behavior
