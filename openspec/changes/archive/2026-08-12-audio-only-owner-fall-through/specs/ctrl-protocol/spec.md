## ADDED Requirements

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
