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
