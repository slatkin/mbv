# Spec Delta

## ADDED Requirements

### Requirement: Pinned Client declaration
The ctrl protocol SHALL carry a client-to-daemon declaration that the Client runs inside a pinwin
panel, carrying the panel's control socket path. The declaration is additive: it SHALL be gated on
the `pinned-panel` capability. It SHALL NOT change
the protocol version, and a Client SHALL NOT send it to a peer that does not advertise the
capability. The daemon SHALL accept the declaration only from an authenticated local Unix ctrl
connection. It SHALL ignore it (log only, no reply) from any other connection. The daemon SHALL
associate the path with the declaring connection and SHALL forget it when that connection closes.
The declaration has no reply, and a repeated declaration on the same connection SHALL replace the
earlier path.

#### Scenario: Local declaration
- **WHEN** a Client on a local Unix ctrl connection to a peer advertising `pinned-panel` declares itself pinned with a socket path
- **THEN** the daemon records that path for that connection

#### Scenario: Connection closes
- **WHEN** a pinned Client's ctrl connection closes
- **THEN** the daemon no longer associates any pinwin socket path with that connection

#### Scenario: TCP declaration ignored
- **WHEN** a declaration arrives over a TCP ctrl connection
- **THEN** the daemon records nothing and sends no reply

#### Scenario: Peer without the capability
- **WHEN** the peer's hello does not advertise `pinned-panel`
- **THEN** the Client does not send the declaration
