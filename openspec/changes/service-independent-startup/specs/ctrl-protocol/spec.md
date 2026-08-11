## ADDED Requirements

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

### Requirement: Control authentication migration is capability-gated
Control-credential authentication SHALL be advertised and selected through an additive ctrl capability. A new client SHALL use Control authentication with a capable Local daemon and SHALL preserve legacy Emby-token authentication only when connecting to a peer that does not advertise the capability, including deferred `mbvd` implementations.

#### Scenario: New client connects to capable Local daemon
- **WHEN** the daemon hello advertises Control-credential authentication
- **THEN** the client SHALL respond with its Control credential
- **THEN** it SHALL send no Service credential in the Control-credential field

#### Scenario: New client connects to deferred mbvd
- **WHEN** a daemon hello does not advertise Control-credential authentication
- **WHEN** the client has a Ready Emby Service and a legacy Emby credential
- **THEN** the client MAY use the existing Emby-authenticated handshake for that peer

#### Scenario: Feed-only client reaches a legacy peer
- **WHEN** a daemon hello does not advertise Control-credential authentication
- **WHEN** the client has no Emby credential for the legacy handshake
- **THEN** the client SHALL reject the attachment with a compatibility diagnostic
