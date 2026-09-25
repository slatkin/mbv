# Spec Delta

## RENAMED Requirements

- FROM: `### Requirement: Protocol version 9`
- TO: `### Requirement: Protocol version 11`

## MODIFIED Requirements

### Requirement: Protocol version 11

The ctrl protocol version SHALL be 11. Clients and daemons SHALL negotiate protocol version 11 during the hello handshake and SHALL reject a peer reporting any other version before the client sends credential-bearing or command messages. Version 11 SHALL change `PlayerEvent::Stopped` and `PlayerEvent::TrackCompleted` to carry `run_identity` as a single generation value instead of a `(request_id, generation)` pair. This non-additive version bump is required by the ctrl wire rule because it changes the wire shape of an existing event field; the capability rule continues to apply to additive changes.

#### Scenario: v11 client connects to v11 daemon

- **WHEN** a client and daemon both report protocol version 11
- **THEN** the connection SHALL proceed with v11 semantics

#### Scenario: older client connects to a v11 daemon

- **WHEN** a client reporting any version other than 11 connects to a daemon requiring version 11
- **THEN** the daemon SHALL reject the protocol mismatch

#### Scenario: v11 client connects to an older daemon

- **WHEN** a v11 client receives a daemon hello reporting any version other than 11
- **THEN** the client SHALL refuse the connection without sending a Remote Service credential
- **THEN** the failure message SHALL identify a protocol-version mismatch
