## ADDED Requirements

### Requirement: A command with no ctrl wire form is refused fail-closed

The ctrl transport SHALL refuse to encode a Player command that has no wire
representation, returning a refusal to the caller. Refusing such a command SHALL NOT
terminate the sending process or the receiving daemon, SHALL NOT deliver a partial
command, and SHALL NOT mutate queue or playback state.

#### Scenario: Client attempts a command with no wire form

- **WHEN** a Client attempts to transmit a Player command that the ctrl transport cannot encode
- **THEN** the transport SHALL return a refusal to the caller instead of transmitting
- **AND** the sending process SHALL remain alive
- **AND** the refusal SHALL be presentable to the user

#### Scenario: Refusal leaves playback state untouched

- **WHEN** a command is refused for having no wire form
- **THEN** no queue mutation, active-slot change, or playback transition SHALL result from the attempt
- **AND** the daemon SHALL remain connected and serving
