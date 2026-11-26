# Spec Delta

## ADDED Requirements

### Requirement: The output switch is additively negotiated

A Player owner that hosts a remote playback output SHALL advertise an additive output-switch capability in its ctrl hello and SHALL accept the output switch command. The command and the capability SHALL be additive within protocol version 10 and SHALL NOT bump the protocol version. A Client SHALL NOT send the command to an owner that does not advertise the capability.

#### Scenario: Owner that hosts a remote output

- **WHEN** the Owner can host a Cast receiver or an Emby Session as its output
- **THEN** its hello advertises the output-switch capability

#### Scenario: Owner that hosts no remote output

- **WHEN** the Player owner is packaged `mbvd`, which has local playback as its only output
- **THEN** its hello does not advertise the output-switch capability

#### Scenario: Client meets an Owner without the capability

- **WHEN** the connected Owner does not advertise the output-switch capability
- **THEN** the Client SHALL NOT send the output switch
- **AND** the Client keeps the prompt whose second answer cancels

#### Scenario: The command carries the same target as a selection

- **WHEN** a Client sends the output switch
- **THEN** it names its target with the same selection shape that the output selection command uses
