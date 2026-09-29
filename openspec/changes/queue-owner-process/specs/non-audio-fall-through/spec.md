## MODIFIED Requirements

### Requirement: Local Player preparation precedes ending the attachment

Before ending the attachment, the client SHALL prepare a local Player: this machine's local
daemon, reached through the client's existing connection to it when it holds one, or started or
attached the way a launch without an explicit endpoint would when it does not. The terminal UI
process SHALL NOT construct a Player of its own. It SHALL end the attachment only after that
preparation succeeds. When preparation fails, including when the local daemon refuses the
connection as an exclusive owner, the client SHALL keep the attachment unchanged, report the
failure, and play nothing locally. The client's media-key target SHALL follow the Player that owns
transport after the fall-through.

#### Scenario: No local Player exists
- **WHEN** a client launched straight onto a daemon confirms a fall-through and holds no connection to this machine's local daemon
- **THEN** the client SHALL start or attach this machine's local daemon before ending the attachment
- **THEN** the client SHALL play the selection on that local daemon

#### Scenario: Preparation fails
- **WHEN** the client cannot prepare a local Player
- **THEN** the client SHALL keep the attachment unchanged
- **THEN** the client SHALL report the failure
- **THEN** the client SHALL NOT play the selection locally
