# local-daemon-tray Specification

## Purpose
TBD - created by archiving change retire-pty-relay-for-local-daemon-stay-alive. Update Purpose after archive.
## Requirements
### Requirement: The tray belongs to the local daemon
The tray SHALL be owned by the local daemon, started through the daemon runtime's tray-ready hook
as the daemon comes up. No client SHALL start a tray. The tray SHALL therefore exist for the whole
life of the daemon, independent of whether any client is attached.

#### Scenario: Daemon starts
- **WHEN** a local daemon starts with the tray icon enabled and a desktop session available
- **THEN** the daemon SHALL start the tray as part of its own startup

#### Scenario: All clients exit
- **WHEN** every client exits while the local daemon keeps playing
- **THEN** the tray SHALL remain present and usable

#### Scenario: A client is running
- **WHEN** a client is attached to a local daemon
- **THEN** the client SHALL NOT start a tray of its own

#### Scenario: Bare mode
- **WHEN** mbv runs in bare mode
- **THEN** no tray SHALL be started

### Requirement: The tray controls the daemon's playback
The tray SHALL show the daemon's current playback state and SHALL offer transport controls and a
quit action that act on the daemon's Player.

#### Scenario: Transport control from the tray
- **WHEN** the user selects a transport action from the tray while media is playing
- **THEN** the daemon's Player SHALL perform that action
- **THEN** every attached client SHALL reflect the resulting state

#### Scenario: Quit from the tray
- **WHEN** the user selects quit from the tray
- **THEN** the daemon SHALL perform the same graceful shutdown as `mbv -q`, persisting its state
- **THEN** attached clients SHALL be notified of the deliberate shutdown

### Requirement: The packaged system daemon has no tray
`mbvd` SHALL NOT start a tray. It runs as a system service without a user desktop session, so its
tray-ready hook SHALL remain a no-op.

#### Scenario: mbvd starts
- **WHEN** `mbvd` starts as a system service
- **THEN** it SHALL NOT attempt to register a tray icon
- **THEN** its playback behavior SHALL be unaffected

### Requirement: A missing tray is not an error
When no tray can be shown — no desktop session, no status-icon host, or the tray icon disabled in
configuration — the local daemon SHALL continue running normally without it. mbv SHALL NOT warn the
user on the terminal, on either the daemon or the client side.

#### Scenario: Headless host
- **WHEN** a local daemon starts over SSH or on a bare terminal with no desktop session
- **THEN** the daemon SHALL run and play normally with no tray
- **THEN** no warning about the missing tray SHALL be printed to any terminal
- **THEN** the condition SHALL be recorded in the daemon's log only

#### Scenario: Tray icon disabled in configuration
- **WHEN** the tray icon is disabled in configuration
- **THEN** the local daemon SHALL NOT attempt to start a tray and SHALL run normally

#### Scenario: Stopping a daemon with no tray
- **WHEN** the user needs to stop a local daemon that has no tray and no attached client
- **THEN** `mbv -q` SHALL stop it

