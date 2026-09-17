## ADDED Requirements

### Requirement: A client-initiated slot jump dispatches by where the Player owner runs

A Client SHALL dispatch a user-initiated jump to a canonical slot according to where the
Player owner runs: when the owner is out-of-process (reached over ctrl, including this
machine's Local daemon), the Client SHALL request the jump from the owner using the
owner-resolved slot request; when the app process is the owner, the Client SHALL resolve
the jump locally. A Client SHALL NOT construct or transmit a command that the ctrl
transport has no wire form for. On completion of a jump requested from an
out-of-process owner, the Client SHALL NOT set its own active slot from the requested
slot; the active slot SHALL continue to follow the owner's queue snapshot.

#### Scenario: Out-of-process owner accepts the on-screen Next-Up accept action

- **WHEN** the mpv Next-Up accept affordance is activated while an out-of-process Player owner holds the Bound queue
- **THEN** the Client SHALL request the jump from that owner
- **AND** the requested slot SHALL become active only through the owner's queue snapshot
- **AND** the Client process SHALL remain running

#### Scenario: App-process owner accepts the on-screen Next-Up accept action

- **WHEN** the mpv Next-Up accept affordance is activated while the app process is the Player owner
- **THEN** the Client SHALL resolve the jump locally and play the requested slot

#### Scenario: Every client-initiated jump uses the same dispatch

- **WHEN** any client-initiated action jumps to an existing canonical slot
- **THEN** it SHALL dispatch through the owner-kind rule rather than constructing a
  local jump command directly
