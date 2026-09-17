## ADDED Requirements

### Requirement: A client-initiated slot jump dispatches by Player owner locality

A Client SHALL dispatch a user-initiated jump to a canonical slot according to the
current Player owner: when the owner is remote, the Client SHALL request the jump from
the owner using the owner-resolved slot request; when the Client itself is the owner, the
Client SHALL resolve the jump locally. A Client SHALL NOT construct or transmit a
command that the ctrl transport has no wire form for. On completion of a remote jump
request, the Client SHALL NOT set its own active slot from the requested slot; the active
slot SHALL continue to follow the owner's queue snapshot. When the owner refuses the
request, the Client SHALL present the refusal and remain usable.

#### Scenario: Remote owner accepts the on-screen Next-Up accept action

- **WHEN** the mpv Next-Up accept affordance is activated while a remote Player owner holds the Bound queue
- **THEN** the Client SHALL request the jump from that owner
- **AND** the requested slot SHALL become active only through the owner's queue snapshot
- **AND** the Client process SHALL remain running

#### Scenario: Local owner accepts the on-screen Next-Up accept action

- **WHEN** the mpv Next-Up accept affordance is activated while this Client is the Player owner
- **THEN** the Client SHALL resolve the jump locally and play the requested slot

#### Scenario: Remote owner refuses the jump request

- **WHEN** the owner refuses a requested slot jump
- **THEN** the Client SHALL present the refusal
- **AND** the Client SHALL remain connected and usable
- **AND** the Client's active slot SHALL reflect the owner's last snapshot rather than the requested slot

#### Scenario: Every client-initiated jump uses the same dispatch

- **WHEN** any client-initiated action jumps to an existing canonical slot
- **THEN** it SHALL dispatch through the owner-locality rule rather than constructing a
  local jump command directly
