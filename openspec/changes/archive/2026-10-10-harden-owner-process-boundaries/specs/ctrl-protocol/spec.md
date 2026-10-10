# Spec Delta

## MODIFIED Requirements

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
- **THEN** the daemon SHALL stop playback and exit, releasing its owner lock

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
