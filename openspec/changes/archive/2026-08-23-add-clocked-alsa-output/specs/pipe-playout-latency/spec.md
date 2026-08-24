## ADDED Requirements

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
