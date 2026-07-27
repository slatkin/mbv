## ADDED Requirements

### Requirement: Protocol version 4
The ctrl protocol version SHALL be 4. Clients and daemon SHALL negotiate protocol version 4 during the hello handshake.

#### Scenario: v4 client connects to v4 daemon
- **WHEN** a client sends `CtrlCmd::Hello` with protocol version 4
- **WHEN** the daemon supports protocol version 4
- **THEN** the daemon SHALL respond with `CtrlEvent::Hello` with protocol version 4
- **THEN** the connection SHALL proceed with v4 semantics

#### Scenario: v3 client connects to v4 daemon
- **WHEN** a client sends `CtrlCmd::Hello` with protocol version 3
- **WHEN** the daemon requires protocol version 4
- **THEN** the daemon SHALL reject the connection with a protocol version mismatch error

#### Scenario: v2 client connects to v4 daemon
- **WHEN** a client sends `CtrlCmd::Hello` with protocol version 2
- **WHEN** the daemon requires protocol version 4
- **THEN** the daemon SHALL reject the connection with a protocol version mismatch error

### Requirement: Authority-on-connect behavior
When a ctrl client connects, the daemon SHALL NOT override authority if it is currently `EmbyRemote`. The new client SHALL receive the initial state snapshot and SHALL receive broadcasts, but its commands SHALL be rejected until authority returns to `Ctrl`.

#### Scenario: Client connects while Emby has authority
- **WHEN** authority is `EmbyRemote`
- **WHEN** a new ctrl client connects and completes the hello handshake
- **THEN** the daemon SHALL send the initial state snapshot to the new client
- **THEN** authority SHALL remain `EmbyRemote`
- **THEN** commands from the new client SHALL be rejected with `CommandRejected { reason: "Emby remote has authority" }`

#### Scenario: Client connects while ctrl has authority
- **WHEN** authority is `Ctrl`
- **WHEN** a new ctrl client connects and completes the hello handshake
- **THEN** authority SHALL remain `Ctrl`
- **THEN** commands from the new client SHALL be accepted

### Requirement: CommandRejected for authority reasons
The daemon SHALL send `CtrlEvent::CommandRejected` to a ctrl client when the client sends a command while Emby remote has authority.

#### Scenario: Ctrl client sends command while Emby has authority
- **WHEN** an Emby remote control session is active (authority is `EmbyRemote`)
- **WHEN** a connected ctrl client sends a player command
- **THEN** the daemon SHALL NOT execute the command
- **THEN** the daemon SHALL send `CtrlEvent::CommandRejected` with reason "Emby remote has authority" to that client
- **THEN** the client SHALL remain connected

### Requirement: Authority return on ctrl command
The daemon SHALL return authority to ctrl when a ctrl client sends a command after Emby remote has gone silent.

#### Scenario: Ctrl command after Emby remote stops
- **WHEN** authority is `EmbyRemote`
- **WHEN** no Emby remote commands have been received for the current session
- **WHEN** a ctrl client sends a player command
- **THEN** the daemon SHALL set authority to `Ctrl`
- **THEN** the daemon SHALL execute the command
- **THEN** all connected ctrl clients SHALL receive the state broadcast

### Requirement: Authority-on-disconnect behavior
When a ctrl client disconnects, the daemon SHALL clear authority to `None` only if it was the last connected ctrl client and authority was `Ctrl`. Individual client disconnects SHALL NOT change authority if other clients remain.

#### Scenario: One client disconnects while others remain
- **WHEN** authority is `Ctrl` and multiple ctrl clients are connected
- **WHEN** one ctrl client disconnects
- **THEN** authority SHALL remain `Ctrl`
- **THEN** remaining clients SHALL continue sending commands

#### Scenario: Last client disconnects
- **WHEN** authority is `Ctrl` and only one ctrl client is connected
- **WHEN** that client disconnects
- **THEN** authority SHALL change to `None`
- **THEN** playback SHALL continue (daemon does not stop)

#### Scenario: Client disconnects while Emby has authority
- **WHEN** authority is `EmbyRemote`
- **WHEN** a ctrl client disconnects
- **THEN** authority SHALL remain `EmbyRemote`

### Requirement: Disconnected event for Emby authority is a notification
The daemon SHALL send `CtrlEvent::Disconnected { reason: TakenOverByEmbyRemote }` to all connected ctrl clients when Emby remote takes authority. This SHALL be a notification only; the daemon SHALL NOT send `CtrlOutbound::Close` and the connection SHALL remain open.

#### Scenario: Emby remote takes authority while ctrl clients are connected
- **WHEN** one or more ctrl clients are connected
- **WHEN** an Emby remote command succeeds and authority changes to `EmbyRemote`
- **THEN** the daemon SHALL broadcast `Disconnected { reason: TakenOverByEmbyRemote }` to all connected ctrl clients
- **THEN** the daemon SHALL NOT send `CtrlOutbound::Close`
- **THEN** all ctrl clients SHALL remain connected
