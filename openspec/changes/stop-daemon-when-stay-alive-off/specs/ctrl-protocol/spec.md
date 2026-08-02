## ADDED Requirements

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

## REMOVED Requirements

### Requirement: Protocol version 4

**Reason**: Superseded by "Protocol version 8". The recorded requirement was already stale
because the implementation had moved to version 7; version 8 covers the acknowledged local
shutdown exchange.

**Migration**: Exact-match compatibility remains in force. Stop a surviving older local
daemon with `mbv -q`, then start mbv again.
