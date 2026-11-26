# Spec Delta

## ADDED Requirements

### Requirement: Emby person image fields are additively negotiated

The ctrl hello SHALL advertise the additive `emby-person-images` capability without changing `CTRL_PROTOCOL_VERSION`. The capability SHALL mean that the peer can decode and re-emit the optional person id and primary image tag in unified queue item payloads. Missing fields SHALL decode as empty and SHALL mean that no person photo is known.

#### Scenario: Capable peers round-trip person image fields

- **WHEN** a queue item payload carrying a person id and primary image tag passes between peers that advertise the capability
- **THEN** the receiver decodes both values and re-emits them unchanged

#### Scenario: A payload predates person image fields

- **WHEN** a queue item payload without the person id or primary image tag is decoded
- **THEN** the decode succeeds and both values are empty
