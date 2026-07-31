## RENAMED Requirements

- FROM: `### Requirement: Protocol version 4`
- TO: `### Requirement: Ctrl protocol version`

## MODIFIED Requirements

### Requirement: Ctrl protocol version
The ctrl protocol version SHALL be 7. Clients and daemon SHALL negotiate protocol version 7 during
the hello handshake. Any peer offering a different protocol version SHALL be rejected at the
handshake rather than allowed to fail later on an unrecognised message.

Note: this requirement previously stated version 4 while the implementation had already advanced;
the version stated here is the implemented value after this change's bump, and the two are expected
to stay in step from now on.

#### Scenario: v7 client connects to v7 daemon
- **WHEN** a client sends `CtrlCmd::Hello` with protocol version 7
- **WHEN** the daemon supports protocol version 7
- **THEN** the daemon SHALL respond with `CtrlEvent::Hello` with protocol version 7
- **THEN** the connection SHALL proceed with v7 semantics

#### Scenario: Older client connects to v7 daemon
- **WHEN** a client sends `CtrlCmd::Hello` with a protocol version lower than 7
- **WHEN** the daemon requires protocol version 7
- **THEN** the daemon SHALL reject the connection with a protocol version mismatch error

#### Scenario: v7 client connects to an older daemon
- **WHEN** a client sends `CtrlCmd::Hello` with protocol version 7 to a daemon requiring a lower version
- **THEN** the connection SHALL be rejected with a protocol version mismatch error
- **THEN** the client SHALL report the mismatch rather than presenting a UI with no playback backend

## ADDED Requirements

### Requirement: Disconnect reason for deliberate daemon shutdown
The `DisconnectReason` enum SHALL carry a variant meaning "the daemon is shutting down
deliberately". The daemon SHALL broadcast `CtrlEvent::Disconnected` with that reason to every
connected client before closing their connections during an explicit shutdown. Unlike the Emby
authority reason, this reason SHALL indicate that the connection is about to close.

#### Scenario: Daemon shuts down explicitly
- **WHEN** the daemon begins an explicit shutdown with clients connected
- **THEN** the daemon SHALL broadcast `CtrlEvent::Disconnected` with the shutdown reason to all connected clients
- **THEN** the daemon SHALL then close those connections

#### Scenario: Client classifies the disconnect
- **WHEN** a client receives `CtrlEvent::Disconnected` with the shutdown reason
- **THEN** the client SHALL treat the subsequent connection close as expected
- **THEN** the client SHALL NOT synthesise a stopped-playback event as it does for an unexpected close

#### Scenario: Emby authority reason is unchanged
- **WHEN** a client receives `CtrlEvent::Disconnected { reason: TakenOverByEmbyRemote }`
- **THEN** the client SHALL treat it as a notification and SHALL remain connected
