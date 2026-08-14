# ctrl-protocol Specification

## Purpose
TBD - created by archiving change daemon-multi-connection. Update Purpose after archive.
## Requirements
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

### Requirement: Local daemon control authentication is Service-independent
The Local daemon SHALL authenticate ctrl clients with a stable mbv-owned Control credential scoped to that Player owner. It SHALL NOT use, validate, or receive an Emby or Audiobookshelf Service credential as its control credential.

#### Scenario: Client presents the Local daemon Control credential
- **WHEN** a client presents the valid Control credential during the ctrl handshake
- **THEN** the Local daemon SHALL authenticate the client independently of all Remote Service states

#### Scenario: Client presents a Service credential as control authentication
- **WHEN** a client presents an Emby or Audiobookshelf credential where the Local daemon requires its Control credential
- **THEN** the Local daemon SHALL reject the connection
- **THEN** it SHALL NOT attempt to validate that credential with a Remote Service

#### Scenario: Feed-only client attaches
- **WHEN** a client has no configured Remote Service but presents the valid Local daemon Control credential
- **THEN** the Local daemon SHALL accept the ctrl connection

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
The v9 ctrl protocol SHALL carry `CtrlCmd::ApplyServiceSetup { kind, revision }` and the matching `CtrlEvent::ServiceSetupApplied { kind, revision }` or `CtrlEvent::ServiceSetupRejected { kind, revision, reason }` for `ServiceKind::Emby` and `ServiceKind::Audiobookshelf`. It SHALL carry no setup values, identity hash, or Service credential. `reason` SHALL be one of `UnsupportedService`, `RevisionMismatch`, `StorageUnavailable`, or `TransitionRejected`.

#### Scenario: Packaged Unix ctrl applies a persisted setup
- **WHEN** a packaged-daemon local Unix client sends `ApplyServiceSetup` for a committed Emby or Audiobookshelf Service revision
- **THEN** the daemon SHALL return an applied or explicitly rejected response for that request

#### Scenario: TCP client sends owner administration to a packaged daemon
- **WHEN** a TCP client sends `ApplyServiceSetup` to a packaged daemon
- **THEN** the daemon SHALL reject the request without changing Service runtime state

#### Scenario: Cross-owner client sends owner administration
- **WHEN** a client attached to one owner sends `ApplyServiceSetup` to a different owner process it is not the local client of
- **THEN** the receiving owner SHALL reject the request without rereading Service storage or changing runtime state

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

### Requirement: Audio-only capability advertisement

A daemon that cannot play non-audio items SHALL advertise an audio-only
capability in its hello handshake. The capability SHALL be additive: the
protocol version SHALL NOT change, and a peer that does not recognise the
capability SHALL ignore it and behave as it does without it.

#### Scenario: Audio-only daemon greets a client

- **WHEN** a daemon running audio-only accepts a ctrl connection
- **THEN** its hello SHALL include the audio-only capability

#### Scenario: Daemon that can play video greets a client

- **WHEN** a daemon that can play non-audio items accepts a ctrl connection
- **THEN** its hello SHALL NOT include the audio-only capability

#### Scenario: Client that does not know the capability

- **WHEN** a client that does not recognise the audio-only capability connects
  to a daemon advertising it
- **THEN** the connection SHALL proceed
- **THEN** the client SHALL submit playback as it does today
- **THEN** the daemon SHALL admit that submission on its normal terms

### Requirement: Capability is the primary routing signal, rejection the backstop

A client that recognises the audio-only capability SHALL use it to decide where
to send eligible explicit playback before submitting. Relationship eligibility
is a client concern independent of the handshake. The existing structured
rejection SHALL remain available for cases the client did not anticipate, and
SHALL NOT be the mechanism a client relies on to discover that a daemon is
audio-only.

#### Scenario: Client routes before submitting

- **WHEN** a client holding a connection to a daemon advertising audio-only
  decides where to send a non-audio selection
- **THEN** it SHALL decide from the advertised capability
- **THEN** it SHALL NOT submit the selection in order to learn the answer from a
  rejection

### Requirement: Audiobookshelf daemon state is additively negotiated
The ctrl hello SHALL advertise additive capabilities for Audiobookshelf queue transport and provider-qualified progress events without changing `CTRL_PROTOCOL_VERSION`. Capability advertisement SHALL indicate binary protocol support and SHALL NOT reveal owner setup or readiness.

#### Scenario: Capable daemon greets a client
- **WHEN** a daemon binary supports Audiobookshelf queue and progress wire behavior
- **THEN** its hello SHALL advertise those capabilities independently of installed Service state

#### Scenario: Older peer connects
- **WHEN** a peer does not recognize the Audiobookshelf capabilities
- **THEN** the connection and every previously supported ctrl behavior SHALL continue compatibly

### Requirement: Acknowledged Audiobookshelf progress has a provider-qualified event
The ctrl protocol SHALL define an event containing Audiobookshelf podcast identity, acknowledged position and completion state, and setup generation. The event SHALL contain no API key, Authorization header, resolved URL, or playback `sessionId`.

#### Scenario: Owner has acknowledged progress to publish
- **WHEN** later daemon playback code emits acknowledged Audiobookshelf progress
- **THEN** the protocol event SHALL preserve provider-qualified identity and generation without exposing lifecycle secrets

### Requirement: Progress events are gated per connection
An Audiobookshelf progress event SHALL be sent only to connected peers that negotiated its capability. Peers without the capability SHALL receive no substitute unknown event and SHALL retain their existing state stream.

#### Scenario: Mixed-version clients are attached
- **WHEN** one connected client supports Audiobookshelf progress and another does not
- **THEN** only the capable client SHALL receive the provider-qualified progress event
- **THEN** both clients SHALL continue receiving every state event they otherwise support

### Requirement: Progress transport remains dormant before playback activation
This change SHALL provide the event shape and capability-gated daemon/client plumbing without generating Audiobookshelf progress from daemon playback or reconciling client browse state.

#### Scenario: Transport change is deployed alone
- **WHEN** no later daemon playback child has been applied
- **THEN** no daemon-owned Audiobookshelf lifecycle SHALL emit progress or become playable

### Requirement: Same-user Local daemon reconciles owner service setup on client signal
The v9 ctrl protocol SHALL let an attached same-user client signal a Local daemon to reread its own owner-local Service setup by sending `CtrlCmd::ApplyServiceSetup { kind, revision }` over the Local daemon's own control socket. The Local daemon SHALL respond with `CtrlEvent::ServiceSetupApplied { kind, revision }` or `CtrlEvent::ServiceSetupRejected { kind, revision, reason }`. The request and response SHALL carry no setup values, identity hash, or Service credential.

#### Scenario: Local daemon applies a client's setup signal
- **WHEN** an attached same-user client sends `ApplyServiceSetup` to its Local daemon for a Service revision matching the daemon's persisted setup
- **THEN** the Local daemon SHALL reread its own storage and return `ServiceSetupApplied`

#### Scenario: Local daemon rejects a mismatched or unsupported signal
- **WHEN** an attached same-user client sends `ApplyServiceSetup` for a revision other than the persisted one, or for a Service the Local daemon does not hold owner context for
- **THEN** the Local daemon SHALL return `ServiceSetupRejected` with `RevisionMismatch` or `UnsupportedService`
- **THEN** it SHALL keep its runtime unchanged

#### Scenario: Credentials never cross the signal
- **WHEN** a client signals a Local daemon to reread owner Service setup
- **THEN** no Service credential, Authorization value, or resolved setup value SHALL appear in the request or response

