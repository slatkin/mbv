# Spec Delta

## ADDED Requirements

### Requirement: A queue replacement reuses a live run

When a queue replacement arrives while a local Playback run exists, the Owner SHALL submit the replacement into that live run when the replacement's headless class matches the running run's class. The run's reported playback state SHALL NOT decide this: a run that is playing, idle, or parked after a failed load is still a live run.

#### Scenario: Replacement while playback is active

- **WHEN** a queue replacement arrives while the live run is playing
- **THEN** the Owner SHALL submit the replacement into the live run
- **AND** the running mpv child SHALL keep serving playback

#### Scenario: Replacement while the run is idle

- **WHEN** a queue replacement arrives while a run exists but reports no active playback
- **THEN** the Owner SHALL submit the replacement into that run instead of ending it and starting a new one

#### Scenario: Replacement into a parked run

- **WHEN** a queue replacement arrives while a run is parked after a failed load, its mpv child still alive
- **THEN** the Owner SHALL submit the replacement into the parked run
- **AND** the parked mpv child SHALL keep serving playback

#### Scenario: Replacement while the run is stopping

- **WHEN** a queue replacement arrives after a stop was requested
- **THEN** the replacement SHALL play exactly once
- **AND** it SHALL play in the live run when the run has not yet applied the stop, cancelling the stop
- **AND** it SHALL play in a new run when the stop has already been applied, without a second playback of the replacement

### Requirement: A queue replacement starts a new run only for a stated reason

The Owner SHALL start a new run for a queue replacement only when no run exists, the replacement's headless class differs from the running run's class, or the live run cannot take the submission. A new run SHALL use the replacement's headless class, and the Owner SHALL report which path it took and, for a new run, why.

#### Scenario: Replacement with no run

- **WHEN** a queue replacement arrives and no run exists
- **THEN** the Owner SHALL start a new run for the replacement

#### Scenario: Headless class differs

- **WHEN** a queue replacement's headless class differs from the running run's class
- **THEN** the Owner SHALL end the existing run and start a new run with the replacement's headless class

#### Scenario: The live run cannot take the submission

- **WHEN** the Owner selects the reuse path but the run's command channel no longer accepts commands
- **THEN** the Owner SHALL start a new run for the replacement
- **AND** the replacement SHALL still play exactly once

#### Scenario: The reuse decision is observable

- **WHEN** the Owner decides between reuse and a new run for a queue replacement
- **THEN** the Owner SHALL report which path it took and, for a new run, why
