# Spec Delta

## Purpose

Keeps mpv in a child process of the Player owner, so a fault in mpv ends one playback run and not the Owner process, and memory that mpv holds returns to the operating system when the run ends.

## ADDED Requirements

### Requirement: mpv SHALL run in a child process of the Player owner

The Player owner SHALL run mpv as a child process and control it over mpv's IPC socket. The Player owner SHALL NOT load the mpv library into its own address space.

#### Scenario: Playback run starts

- **WHEN** the Player owner starts a playback run
- **THEN** one `mpv` child process SHALL serve that run, and the run SHALL control it through the IPC socket

#### Scenario: Playback run ends

- **WHEN** a playback run ends, for any reason
- **THEN** its `mpv` child process SHALL exit, and the Player owner SHALL NOT leave it running

#### Scenario: Owner process exits

- **WHEN** the Owner process exits while a playback run is active
- **THEN** its `mpv` child process SHALL NOT outlive the Owner process

### Requirement: An mpv fault SHALL end only the run that it belongs to

When the `mpv` child process exits or crashes during a run, and the Player owner did not request the exit, the Player owner SHALL end that run with an error. The Owner process, the canonical queue, and every connected Client SHALL stay up.

#### Scenario: mpv crashes during playback

- **WHEN** the `mpv` child process crashes while a run plays an item
- **THEN** the run SHALL stop and report an error, with the position that the Player owner last observed
- **AND** the Owner process and its Clients SHALL keep running

#### Scenario: The next run after a crash

- **WHEN** a run ended because its `mpv` child process crashed, and the user starts playback again
- **THEN** the Player owner SHALL start a new `mpv` child process and play normally

#### Scenario: Stop requested by the Player owner

- **WHEN** the Player owner asks mpv to quit and the child process exits
- **THEN** the run SHALL end as a normal stop, not as an mpv fault

### Requirement: A missing mpv program SHALL be reported as a start error

When the `mpv` program cannot be started, the Player owner SHALL fail the run that needs it with an error that names the missing program. It SHALL NOT stop the Owner process.

#### Scenario: mpv is not installed

- **WHEN** playback starts on a host with no `mpv` program on the search path
- **THEN** the run SHALL fail with an error that says the `mpv` program was not found
- **AND** the Owner process SHALL keep running

### Requirement: Playback behavior SHALL NOT depend on the order of property-change events

The Player owner SHALL give the same playback outcome whether an observed property change arrives before or after a nearby non-property event.

#### Scenario: Property change lands on either side of a reconfiguration

- **WHEN** mpv delivers a property change before an event in one run and after it in another run, with the same media and commands
- **THEN** the playback state, the queue position, and the reports that the Player owner sends SHALL be the same in both runs

### Requirement: Shutdown SHALL be detected when mpv delivers no shutdown event

mpv does not send a shutdown event on its IPC socket when it quits. The Player owner SHALL treat the close of the IPC socket and the exit of the child process as shutdown, including a quit during a pending load.

#### Scenario: Quit while a load is pending

- **WHEN** the Player owner asks mpv to quit while a file load is pending, and mpv closes the socket with no end-file event
- **THEN** the run SHALL end as a normal stop and report the last known position
