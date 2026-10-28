# Spec Delta

## ADDED Requirements

### Requirement: A whole-queue replacement can carry a start position

A Player owner that advertises the additive start-position capability SHALL accept an optional start position on a whole-queue replacement that names a start slot, and SHALL start that slot at the given position. The field SHALL be additive within protocol version 10 and SHALL NOT bump the protocol version. A peer that does not advertise the capability SHALL NOT receive the field.

#### Scenario: Owner with the capability

- **WHEN** a Client sends a whole-queue replacement with start slot 2 and a start position of 95 seconds to an owner that advertises the capability
- **THEN** the owner SHALL start slot 2 at 95 seconds

#### Scenario: Replacement without a start position

- **WHEN** a Client sends a whole-queue replacement without a start position
- **THEN** the owner SHALL start the start slot under its per-medium resume source, as before

#### Scenario: Owner without the capability

- **WHEN** the connected owner does not advertise the capability
- **THEN** the Client SHALL NOT send a start position to it
