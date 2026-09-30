## ADDED Requirements

### Requirement: The tray belongs to a stay-alive local daemon
The tray SHALL be owned by the local daemon, started through the daemon runtime's tray-ready hook
as the daemon comes up, and only when stay-alive is enabled at that moment. No client SHALL start a
tray. A tray that was started SHALL therefore exist for the whole life of the daemon, independent
of whether any client is attached.

#### Scenario: Daemon starts
- **WHEN** a local daemon starts with stay-alive enabled, the tray icon enabled, and a desktop session available
- **THEN** the daemon SHALL start the tray as part of its own startup

#### Scenario: All clients exit
- **WHEN** every client exits while the local daemon keeps playing
- **THEN** the tray SHALL remain present and usable

#### Scenario: A client is running
- **WHEN** a client is attached to a local daemon
- **THEN** the client SHALL NOT start a tray of its own

#### Scenario: Stay-alive disabled
- **WHEN** a local daemon starts while stay-alive is disabled
- **THEN** no tray SHALL be started

## REMOVED Requirements

### Requirement: The tray belongs to the local daemon
**Reason**: Its "Bare mode" scenario described a TUI that owns the Player. The tray now depends on whether stay-alive is on when the daemon starts.
**Migration**: Replaced by "The tray belongs to a stay-alive local daemon".
