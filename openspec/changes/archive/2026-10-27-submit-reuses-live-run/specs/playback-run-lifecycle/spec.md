# Spec Delta

## ADDED Requirements

### Requirement: A queue replacement reuses a live run

When a queue replacement arrives while a local Playback run exists, the Owner SHALL submit the replacement into that live run when the replacement's headless class matches the running run's class, no stop was requested for that run, and the run's loop has not exited. The run's reported playback state SHALL NOT decide this: a run that is playing, idle, or parked after a failed load is still a live run.

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

### Requirement: A queue replacement starts a new run only for a stated reason

The Owner SHALL start a new run for a queue replacement only when no run exists, the replacement's headless class differs from the running run's class, a stop was requested for the running run, the running run's loop has exited, or the live run cannot take the submission. A new run SHALL use the replacement's headless class. A replacement the Owner submitted SHALL play once. The Owner SHALL report which path it took and, for a new run, why.

#### Scenario: Replacement with no run

- **WHEN** a queue replacement arrives and no run exists
- **THEN** the Owner SHALL start a new run for the replacement

#### Scenario: Headless class differs

- **WHEN** a queue replacement's headless class differs from the running run's class
- **THEN** the Owner SHALL end the existing run and start a new run with the replacement's headless class

#### Scenario: Replacement after a stop was requested

- **WHEN** a queue replacement arrives after a stop was requested for the running run
- **THEN** the Owner SHALL NOT submit the replacement into that run
- **AND** the Owner SHALL start a new run in which the replacement plays once

#### Scenario: The run's loop has exited

- **WHEN** a queue replacement arrives after the running run's loop exited and before its thread ended
- **THEN** the Owner SHALL start a new run in which the replacement plays once

#### Scenario: The live run cannot take the submission

- **WHEN** the Owner selects the reuse path but the run's command channel no longer accepts commands
- **THEN** the Owner SHALL start a new run in which the replacement plays once

#### Scenario: The send lands as the loop exits

- **WHEN** a submission is accepted by a run's command channel in the same loop pass in which the run exits
- **THEN** that replacement MAY NOT play, and the user repeats the action
- **AND** this residual window is the only way an accepted replacement is lost

#### Scenario: The reuse decision is observable

- **WHEN** the Owner decides between reuse and a new run for a queue replacement
- **THEN** the Owner SHALL report which path it took and, for a new run, why (`no_run`, `headless_mismatch`, `stop_requested`, `run_exited`, or `send_failed`)
