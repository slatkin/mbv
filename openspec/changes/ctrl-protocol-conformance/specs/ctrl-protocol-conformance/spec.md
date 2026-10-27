# Spec Delta

## Purpose

Pins the Client-to-Player-owner wire protocol to committed, machine-checkable
artifacts — a versioned schema and a golden fixture corpus — so wire drift
between a Client and a separately packaged `mbvd` fails CI instead of surfacing
silently at runtime.

## ADDED Requirements

### Requirement: A versioned protocol directory pins each wire contract

The repository SHALL carry a public protocol directory `protocol/ctrl/v<N>/`
whose `<N>` is the `CTRL_PROTOCOL_VERSION` the directory pins. The directory
SHALL hold machine-checkable schema documents, a golden fixture corpus, and
documentation of the update and bump rules.

#### Scenario: Current version has its directory

- **WHEN** the current ctrl protocol version is 10
- **THEN** `protocol/ctrl/v10/` SHALL exist with schema documents, fixtures,
  and the rules document

#### Scenario: A protocol bump opens a new directory

- **WHEN** a breaking wire change bumps `CTRL_PROTOCOL_VERSION`
- **THEN** a new versioned directory SHALL be opened for the new version
- **THEN** the prior directory SHALL remain unchanged as the record of its
  version

### Requirement: Committed schema documents the wire roots

Each wire root — client-to-owner commands, owner-to-client events, and the
hello handshake — SHALL have a committed JSON Schema document generated from
the wire types in code. The schema SHALL carry the protocol version it
describes, and CI SHALL fail when regenerating the schema disagrees with the
committed document or with `CTRL_PROTOCOL_VERSION`.

#### Scenario: Wire type changes without regeneration

- **WHEN** a wire-reachable type changes in code
- **WHEN** the committed schema is not regenerated
- **THEN** the schema freshness check SHALL fail

#### Scenario: Schema is regenerated and committed

- **WHEN** the schema is regenerated after a wire change
- **THEN** the freshness check SHALL pass
- **THEN** the schema's declared version SHALL equal the code's
  `CTRL_PROTOCOL_VERSION`

### Requirement: The fixture corpus covers every wire shape

The corpus SHALL hold one golden fixture per command variant, per event
variant including every player-event variant, per negotiated handshake shape,
per queue item kind inside queue slot and snapshot shapes, and per rejection
and disconnect reason. A wire shape with no fixture, or a fixture with no wire
shape, SHALL fail the conformance check.

#### Scenario: New variant has no fixture yet

- **WHEN** a variant is added to a wire enum
- **WHEN** no fixture is added for it
- **THEN** the conformance check SHALL fail

#### Scenario: Removed variant leaves an orphaned fixture

- **WHEN** a wire variant is removed but its fixture remains
- **THEN** the conformance check SHALL fail

#### Scenario: Handshake coverage includes capability negotiation

- **WHEN** the corpus is enumerated
- **THEN** it SHALL include the Local control-credential hello, the
  credential-free packaged-daemon hello, the service-setup-admin hello, the
  pin-swap hello, the owner-action hello, and an audio-only daemon hello

### Requirement: Conformance runs in both directions

For every fixture, CI SHALL verify that encoding the canonical message yields
the fixture content and that decoding the fixture yields the canonical
message. The owner decode path used by packaged `mbvd` SHALL be exercised
against the command corpus.

#### Scenario: Encode drift is caught

- **WHEN** a wire change alters what the encoder emits for a fixture's shape
- **THEN** the conformance check SHALL fail until the fixture is regenerated

#### Scenario: Decode drift is caught

- **WHEN** a wire change makes a fixture no longer decode, or silently drops
  fixture fields on decode
- **THEN** the conformance check SHALL fail until the fixture is regenerated

#### Scenario: Packaged-owner decode of the command corpus

- **WHEN** the owner-side conformance check runs
- **THEN** every command fixture SHALL decode through the same path the
  packaged daemon uses for inbound ctrl lines

### Requirement: Fixture and schema updates are deliberate acts

Corpus and schema updates SHALL be produced only through the explicit
regeneration path and reviewed as part of the wire change. An additive change
(new variant, defaulted field, new capability) SHALL update fixtures inside
the current version directory with no version bump. A breaking change SHALL
bump `CTRL_PROTOCOL_VERSION`, open a new versioned directory, and SHALL
require explicit user approval — never a ride-along.

#### Scenario: Additive change in the current directory

- **WHEN** an additive wire change lands
- **THEN** its fixtures and schema SHALL be regenerated inside the current
  version directory
- **THEN** `CTRL_PROTOCOL_VERSION` SHALL NOT change

#### Scenario: Breaking change requires approval

- **WHEN** a wire change renames or removes a field or variant, changes a
  meaning, unit, or nullability, or changes handshake or framing
- **THEN** the change SHALL bump `CTRL_PROTOCOL_VERSION` and open a new
  versioned directory
- **THEN** it SHALL proceed only with explicit user approval
