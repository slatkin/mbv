# Spec Delta

## Purpose

Defines when a local Playback run ends because nothing can play, which queue slot a load failure belongs to, and how the Player owner learns, exactly one time, that a run ended.

## ADDED Requirements

### Requirement: A run that cannot start ends with an error

When an entry fails to load before playback began in the current submission, the Player owner SHALL end the run and report one stop with an error. Playback begins at the first restart of playback after the submission. A new submission into a running mpv child starts the count again.

#### Scenario: The first entry fails to load

- **WHEN** a submission starts and its first entry fails to load before any playback began
- **THEN** the run SHALL end and report one stop with an error

#### Scenario: A new submission into a run that played before

- **WHEN** a run played an earlier submission, and the first entry of a new submission fails to load before playback of the new submission began
- **THEN** the run SHALL end and report one stop with an error
- **AND** the run SHALL NOT continue to the next entry of the new submission

### Requirement: A failure after playback began ends the run only when nothing follows

When an entry fails to load after playback began, the run SHALL continue to the next entry if mpv holds one after the failed entry. When mpv holds no entry after the failed one, the run SHALL end and report one stop with an error. An active-file projection holds only the active entry, so a failure there SHALL always end the run.

#### Scenario: A middle entry fails

- **WHEN** playback began and a queue entry that has a next entry fails to load
- **THEN** the run SHALL stay active and SHALL play the next entry
- **AND** the run SHALL report the next entry as the active slot

#### Scenario: The last entry fails

- **WHEN** playback began and the last queue entry fails to load
- **THEN** the run SHALL end and report one stop with an error

#### Scenario: An active-file entry fails

- **WHEN** playback began in an active-file projection, and the active entry fails to load while the canonical queue holds more slots
- **THEN** the run SHALL end and report one stop with an error

### Requirement: A failed jump target is judged from the target

When the target of a jump fails to load, the Player owner SHALL decide from the target slot, not from the slot played before the jump. The decision SHALL follow the rules above, and a stop SHALL name the target slot. The failed target SHALL leave no jump in flight. Later moves of mpv to another entry SHALL then be followed, and no later entry SHALL get the resume position of the failed target. The active-file projection SHALL hold no jump in flight.

#### Scenario: The jump target is the last entry and fails

- **WHEN** the user jumps to the last queue entry and that entry fails to load
- **THEN** the run SHALL end and report one stop with an error that names the target slot

#### Scenario: The jump target has a next entry and fails

- **WHEN** playback began, the user jumps to an entry that has a next entry, and the target fails to load
- **THEN** the run SHALL stay active and SHALL report the entry after the target as the active slot

#### Scenario: A failed target with a resume position

- **WHEN** the user jumps to an entry with a stored resume position, the target fails to load, and mpv starts the entry after it
- **THEN** the entry after the target SHALL start from its own start position, not from the resume position of the failed target

#### Scenario: A jump from idle fails

- **WHEN** the run is idle, the user jumps to an entry, and that entry fails to load before playback restarts
- **THEN** the decision SHALL use the slot after the target, not the slot after the entry played before

#### Scenario: A jump armed before the active-file projection

- **WHEN** a jump is armed in the playlist projection and the run then enters the active-file projection
- **THEN** the active-file projection SHALL hold no jump in flight

### Requirement: An active-file selection that fails ends the run

When the Player owner cannot select or install the active entry of an active-file projection, after a jump, a replacement, or a promotion to the projection, the run SHALL report one stop with an error that names the slot it tried to select. The run SHALL NOT stay marked active with nothing loaded.

#### Scenario: An active-file jump fails

- **WHEN** the user jumps to a slot in an active-file projection and the selection of that slot fails
- **THEN** the run SHALL report one stop with an error that names that slot

#### Scenario: A promotion to the active-file projection fails

- **WHEN** a lifecycle-backed item enters a playlist run and the install of the active-file projection fails
- **THEN** the run SHALL report one stop with an error

### Requirement: Each run submission ends with exactly one stop

For each run submission, the Player owner SHALL report exactly one stop when that submission's run ends, whatever the end path: end of queue, error, quit, mpv shutdown, or a missing completed slot. A second end path for the same submission SHALL report nothing. A later submission SHALL be able to end again.

#### Scenario: Quit after a failed replacement

- **WHEN** a replacement fails and reports a stop with an error, and the user then quits the run
- **THEN** the quit SHALL NOT report a second stop for the same run identity

#### Scenario: A fatal error after mpv moved by itself

- **WHEN** mpv moves to another entry by itself, the outgoing item is reported stopped to the Service, and the adopted entry then fails with no entry after it
- **THEN** the run SHALL report one stop with an error

#### Scenario: A new submission after an ended run

- **WHEN** a run ended with a stop, and a new submission starts in the same mpv child and then ends
- **THEN** that submission SHALL report its own stop, under a new run identity when it refreshes the identity and under the reused identity when it does not

#### Scenario: A replacement after an ended run

- **WHEN** a run ended with a stop, and a replacement submits into the same mpv child and then ends
- **THEN** that replacement SHALL report its own stop under the run identity its submission carries
