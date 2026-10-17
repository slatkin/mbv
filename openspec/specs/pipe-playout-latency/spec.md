# pipe-playout-latency Specification

## Purpose
TBD - created by archiving change surface-pipe-playout-latency. Update Purpose after archive.
## Requirements
### Requirement: Pipe startup phases are reported honestly
For direct-daemon pipe output, the daemon SHALL report request-correlated startup phases derived from mbv-owned intent and player transitions. The system MUST distinguish observed output startup from estimated downstream buffering and MUST NOT describe either as confirmed audibility.

#### Scenario: Target is being resolved
- **WHEN** an accepted pipe-output Play is resolving its target
- **THEN** the client can present the Resolving phase

#### Scenario: Player output starts
- **WHEN** a concrete player event indicates output startup for the current Play generation
- **THEN** the daemon reports OutputStarted without claiming downstream audibility

### Requirement: Downstream playout delay is optional and generic
Pipe-output configuration SHALL accept an optional nonnegative expected downstream playout delay. The value SHALL be used only for status estimation and the pipe startup guard and SHALL NOT configure or query a downstream system.

#### Scenario: Delay is configured
- **WHEN** output starts with a configured downstream playout delay
- **THEN** the daemon enters OutputBuffering with an approximate remaining duration

#### Scenario: Delay is not configured
- **WHEN** output starts without a configured delay
- **THEN** the daemon settles at the observable output boundary and presents no numeric audibility estimate

#### Scenario: Invalid delay is configured
- **WHEN** the configured delay is negative or cannot be parsed
- **THEN** normal configuration validation rejects it

### Requirement: Estimated buffering extends same-target startup guarding
While a configured OutputBuffering deadline is active, an equivalent same-target Play SHALL be coalesced. Different-target Play and Stop SHALL retain their supersession behavior. After the deadline settles, same-target Play SHALL restart normally.

#### Scenario: Equivalent Play during estimated buffering
- **WHEN** the current target is in OutputBuffering and the same target is requested again
- **THEN** the request is coalesced and does not restart playback

#### Scenario: Different target during estimated buffering
- **WHEN** Play B targets a different item while Play A is buffering
- **THEN** B supersedes A according to the playback-intent policy

#### Scenario: Stop during estimated buffering
- **WHEN** Stop is invoked while Play is buffering
- **THEN** Stop invalidates the Play and its buffering deadline

#### Scenario: Same target after estimated buffering
- **WHEN** the buffering deadline has settled and Play is invoked for the current target
- **THEN** the daemon accepts it as a deliberate restart

### Requirement: Buffering deadlines are generation-safe
Every buffering deadline SHALL be bound to its control connection and Play generation. A deadline from superseded, stopped, or disconnected work MUST NOT settle or modify a newer request.

#### Scenario: Old deadline expires after target replacement
- **WHEN** Play A is superseded by Play B and A's deadline later expires
- **THEN** the expiration has no effect on B or authoritative playback state

### Requirement: Pipe latency presentation is route-specific
Startup phase and buffering presentation SHALL appear only for a TUI directly controlling pipe-output `mbvd`. Local playback, attached Emby sessions, and non-pipe daemon output SHALL retain existing presentation.

#### Scenario: Direct pipe-output route is active
- **WHEN** the TUI has a pending Play against pipe-output `mbvd`
- **THEN** it presents the current phase and any configured approximate remaining delay

#### Scenario: Another playback route is active
- **WHEN** playback is local, attached to Emby, or non-pipe daemon output
- **THEN** pipe phases and delay estimates are not presented

### Requirement: Phase timing is diagnosable
The daemon SHALL log each pipe startup phase transition and terminal outcome with request correlation and elapsed monotonic time. These logs SHALL require no downstream service connection.

#### Scenario: Pipe Play settles
- **WHEN** a pipe-output Play progresses to settlement
- **THEN** logs identify its request and generation and provide elapsed timing for each phase

#### Scenario: Pipe Play is superseded
- **WHEN** a pipe-output Play is superseded during startup
- **THEN** its timing sequence ends with the Superseded outcome

### Requirement: mbv does not manage the downstream consumer
The pipe-latency capability MUST NOT discover, query, configure, restart, or tune Snapserver or any other pipe consumer. Documentation SHALL describe the delay as a user-owned estimate and downstream tuning as out of scope.

#### Scenario: Pipe latency feature is enabled
- **WHEN** phase reporting or a downstream delay estimate is enabled
- **THEN** mbv performs no downstream API calls or configuration writes

### Requirement: Pipe output requires explicit legacy selection
The packaged daemon SHALL use pipe output only when `audio_pipe_enabled` is explicitly true. In that mode it SHALL retain the existing pipe path, PCM format, startup phases, startup guard, downstream playout-delay estimate, and diagnostics. When pipe output is not selected, pipe-only configuration SHALL NOT alter audio-device output or activate pipe-specific presentation.

#### Scenario: Legacy pipe output is selected
- **WHEN** packaged `mbvd` starts a Playback run with `audio_pipe_enabled = true`
- **THEN** it uses the configured pipe output and retains the existing pipe-latency behavior
- **THEN** any configured `audio_device` value does not alter that run

#### Scenario: Legacy pipe output is not selected
- **WHEN** packaged `mbvd` starts a Playback run with `audio_pipe_enabled` absent or false
- **THEN** it does not create or open the configured pipe
- **THEN** pipe path, format, startup guard, and expected playout-delay values do not affect that run

#### Scenario: Clocked output fails
- **WHEN** the selected ALSA endpoint cannot be opened while pipe output is disabled
- **THEN** the Playback run fails without enabling the legacy pipe as a fallback

