# ctrl-protocol Specification

## Purpose
TBD - created by archiving change daemon-multi-connection. Update Purpose after archive.
## Requirements
### Requirement: Protocol version 8

The ctrl protocol version SHALL be 8. Clients and daemons SHALL negotiate protocol version
8 during the hello handshake and SHALL reject a peer reporting any other version.

#### Scenario: v8 client connects to v8 daemon

- **WHEN** a client and daemon both report protocol version 8
- **THEN** the connection SHALL proceed with v8 semantics

#### Scenario: v7 client connects to v8 daemon

- **WHEN** a client reports protocol version 7 to a daemon requiring version 8
- **THEN** the daemon SHALL reject the connection with a protocol-version mismatch

#### Scenario: v8 client connects to a leftover v7 local daemon

- **WHEN** an upgraded client uses any Local connection path to a daemon reporting version 7
- **THEN** the client SHALL refuse the connection
- **THEN** the failure message SHALL identify a protocol-version mismatch and name `mbv -q`
  as the way to stop the leftover daemon

### Requirement: Acknowledged local-daemon shutdown request

The ctrl protocol SHALL carry a client-to-daemon lifecycle request for coordinated
shutdown. It SHALL be distinct from the player `Stop` command. The daemon SHALL return a
request-specific acceptance or rejection response; enqueueing the request on the client is
not acknowledgement.

The daemon SHALL accept the request only from an authenticated local Unix ctrl connection.
It SHALL reject the request from a TCP ctrl connection without stopping playback or the
daemon.

#### Scenario: Local request is accepted

- **WHEN** an authenticated client sends the request over the daemon's local Unix ctrl connection
- **WHEN** the daemon durably persists its authoritative queue
- **THEN** the daemon SHALL send `ShutdownAccepted` to the requester
- **THEN** the daemon SHALL begin its existing deliberate-shutdown sequence

#### Scenario: TCP request is rejected

- **WHEN** an authenticated client sends the request over a TCP ctrl connection
- **THEN** the daemon SHALL send `ShutdownRejected` to that client
- **THEN** playback and the daemon SHALL continue running

#### Scenario: Persistence failure rejects shutdown

- **WHEN** a permitted local client requests shutdown
- **WHEN** the daemon cannot durably persist its authoritative queue
- **THEN** the daemon SHALL send `ShutdownRejected` with a diagnostic reason
- **THEN** the daemon SHALL remain running and SHALL keep every client connected

#### Scenario: Accepted request performs deliberate shutdown

- **WHEN** the requester receives `ShutdownAccepted`
- **THEN** every connected client SHALL receive the deliberate-shutdown notification
- **THEN** the daemon SHALL stop playback, remove its pid file, and exit

#### Scenario: Player stop does not stop the daemon

- **WHEN** a connected client sends the player `Stop` command
- **THEN** playback SHALL stop
- **THEN** the daemon SHALL continue running and clients SHALL remain connected

#### Scenario: Local lifecycle request while Emby remote holds authority

- **WHEN** playback authority is `EmbyRemote`
- **WHEN** an authenticated local Unix ctrl client requests shutdown
- **THEN** the request SHALL be evaluated as lifecycle control without first transferring
  playback authority to Ctrl
- **THEN** the request SHALL be accepted if authoritative queue persistence succeeds

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

