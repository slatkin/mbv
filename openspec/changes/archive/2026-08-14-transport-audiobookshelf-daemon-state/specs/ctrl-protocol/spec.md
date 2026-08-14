## ADDED Requirements

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
