# Spec Delta

## ADDED Requirements

### Requirement: The hello carries a wire identity

Every ctrl hello SHALL carry a wire identity: a short deterministic digest of the sender's wire shape. The digest covers the wire shapes of the command, event, and hello messages. The ctrl protocol version and documentation text SHALL NOT affect it; the protocol version keeps its own check. Two binaries with the same wire shape SHALL report the same value. The field SHALL be additive: it SHALL NOT change the protocol version, and a peer that predates it SHALL decode an absent value as empty.

#### Scenario: Two builds with the same wire shape

- **WHEN** two binaries are built from different commits that do not change any ctrl message shape
- **THEN** their hellos SHALL report the same wire identity

#### Scenario: A documentation-only change

- **WHEN** a ctrl message type changes only its documentation text
- **THEN** the wire identity SHALL NOT change

#### Scenario: A wire shape change

- **WHEN** a ctrl message gains or loses a variant or field, renames a wire tag, or changes a field type
- **THEN** the wire identity SHALL change

#### Scenario: A peer that predates the field

- **WHEN** a hello without a wire identity is decoded
- **THEN** the decode SHALL succeed and the wire identity SHALL be empty
- **THEN** the protocol version check SHALL decide compatibility as without this requirement, except where the local Owner gate refuses an empty wire identity
