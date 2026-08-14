## MODIFIED Requirements

### Requirement: Protocol version 9

The ctrl protocol version SHALL be 9. Clients and daemons SHALL negotiate protocol version 9 during the hello handshake and SHALL reject a peer reporting any other version before the client sends credential-bearing or command messages. Version 9 SHALL remove the legacy packaged-`mbvd` Emby-token authentication field and behavior. This non-additive version bump is required by the ctrl wire rule because it removes a hello field and changes handshake semantics; the capability rule continues to apply to additive changes.

#### Scenario: v9 client connects to v9 daemon

- **WHEN** a client and daemon both report protocol version 9
- **THEN** the connection SHALL proceed with v9 semantics

#### Scenario: older client connects to v9 daemon

- **WHEN** a client reporting any version other than 9 connects to a daemon requiring version 9
- **THEN** the daemon SHALL reject the protocol mismatch
- **THEN** the client SHALL NOT send an Emby authentication token

#### Scenario: v9 client connects to an older daemon

- **WHEN** a v9 client receives a daemon hello reporting any version other than 9
- **THEN** the client SHALL refuse the connection without sending a Remote Service credential
- **THEN** the failure message SHALL identify a protocol-version mismatch

## ADDED Requirements

### Requirement: Control authentication is role-specific
The Local daemon SHALL authenticate ctrl clients with its stable same-user Control credential. Packaged `mbvd` SHALL perform no application-level ctrl authentication: Unix-socket filesystem permissions and reachability of its configured TCP listener on the trusted LAN SHALL be the access boundaries. Neither role SHALL use, validate, or receive a Remote Service credential as ctrl authentication. This replaces the former capability-gated legacy Emby-token fallback; v9 has no `auth_token` field or legacy authentication path.

#### Scenario: Client presents the Local daemon Control credential
- **WHEN** a client presents the valid Control credential to a Local daemon during the ctrl handshake
- **THEN** the Local daemon SHALL authenticate it independently of all Remote Service states

#### Scenario: Client attaches to packaged mbvd over Unix
- **WHEN** a client reaches packaged `mbvd` through its configured Unix ctrl socket
- **THEN** packaged `mbvd` SHALL accept protocol-compatible ctrl without requesting application credentials
- **THEN** operating-system socket permissions SHALL remain the access boundary

#### Scenario: Client attaches to packaged mbvd over TCP
- **WHEN** a client reaches packaged `mbvd` through its configured TCP listener
- **THEN** packaged `mbvd` SHALL accept protocol-compatible ctrl without requesting application credentials
- **THEN** trusted-LAN reachability SHALL remain the access boundary

#### Scenario: Client has a legacy Service credential configured
- **WHEN** a v9 client connects to a v9 packaged daemon while it has a legacy Emby credential locally
- **THEN** the client SHALL NOT transmit that credential during ctrl admission
- **THEN** the daemon hello and client hello SHALL contain no Service-credential field

### Requirement: Packaged owner-service reconciliation is local-only
The v9 ctrl protocol SHALL carry `CtrlCmd::ApplyServiceSetup { kind, revision }` and the matching `CtrlEvent::ServiceSetupApplied { kind, revision }` or `CtrlEvent::ServiceSetupRejected { kind, revision, reason }`. It SHALL carry no setup values, identity hash, or Service credential. `reason` SHALL be one of `UnsupportedService`, `RevisionMismatch`, `StorageUnavailable`, or `TransitionRejected`.

#### Scenario: Packaged Unix ctrl applies a persisted setup
- **WHEN** a packaged-daemon local Unix client sends `ApplyServiceSetup` for a committed Service revision
- **THEN** the daemon SHALL return an applied or explicitly rejected response for that request

#### Scenario: TCP or Local-daemon client sends owner administration
- **WHEN** a TCP client or Local-daemon client sends `ApplyServiceSetup`
- **THEN** the daemon SHALL reject the request without changing Service runtime state

### Requirement: Transport-scoped lifecycle authority remains enforced
Removing packaged ctrl application authentication SHALL NOT broaden local-only lifecycle privileges. Requests restricted to this machine's Local daemon or a local Unix transport SHALL remain rejected over packaged TCP ctrl.

#### Scenario: TCP client requests daemon shutdown
- **WHEN** a protocol-compatible TCP ctrl client requests coordinated daemon shutdown
- **THEN** the daemon SHALL reject the lifecycle request without stopping playback or the daemon

#### Scenario: Allowed local Unix client requests lifecycle control
- **WHEN** an allowed local Unix ctrl client submits a lifecycle request permitted for that daemon role
- **THEN** the request SHALL be evaluated under its existing transport and role restrictions

### Requirement: Audio-only capability remains additive within a protocol version
Adding or removing the audio-only capability alone SHALL NOT change a ctrl protocol version, and an otherwise compatible peer that does not recognize it SHALL ignore it as specified by the base requirement. This SHALL NOT prohibit a deliberate protocol-version bump required by a non-additive hello-field removal or handshake-framing change.

#### Scenario: v9 daemon advertises audio-only
- **WHEN** otherwise v9-compatible peers negotiate and the daemon is audio-only
- **THEN** the daemon SHALL advertise the audio-only capability
- **THEN** a v9 peer that does not recognize that capability SHALL retain the base capability fallback behavior

## REMOVED Requirements

### Requirement: Control authentication migration is capability-gated
The v9 protocol removes this legacy compatibility fallback. Both v9 peers negotiate their version before either client serializes a hello containing a Service credential, so a capability cannot safely preserve this behavior.
