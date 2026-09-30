## ADDED Requirements

### Requirement: Current player locality follows its daemon endpoint

The system SHALL classify the current player as using the managed local daemon only when its
current daemon endpoint is the managed local endpoint. Players reached through TCP or Unix
endpoints SHALL NOT be classified as using the managed local daemon. The terminal UI process SHALL
never be classified as a player.

#### Scenario: Managed local daemon target

- **WHEN** the current player is connected through the managed local daemon endpoint
- **THEN** the system SHALL classify the current player as using the managed local daemon

#### Scenario: TCP daemon target

- **WHEN** the current player is connected through a TCP daemon endpoint
- **THEN** the system SHALL NOT classify the current player as using the managed local daemon

#### Scenario: Unix daemon target

- **WHEN** the current player is connected through a Unix daemon endpoint
- **THEN** the system SHALL NOT classify the current player as using the managed local daemon

### Requirement: Player-owner locality follows the current daemon target

The system SHALL classify the player owner as being on this machine for a player reached through
the managed local daemon endpoint. It SHALL classify the player owner as being elsewhere for
players reached through TCP or Unix daemon endpoints.

#### Scenario: Managed local daemon owner

- **WHEN** the current player is connected through the managed local daemon endpoint
- **THEN** the system SHALL classify the player owner as being on this machine

#### Scenario: Other daemon owner

- **WHEN** the current player is connected through a TCP or Unix daemon endpoint
- **THEN** the system SHALL classify the player owner as being elsewhere

### Requirement: Daemon target transitions update classification

After construction or a successful target transition, the system SHALL classify locality from the
new current player target. Returning from a daemon target to the managed local daemon SHALL
restore managed-local-daemon locality and SHALL NOT retain the previous daemon's locality.

#### Scenario: Switch to another daemon

- **WHEN** a player route or direct-session connection successfully switches to a daemon endpoint
- **THEN** locality-dependent behavior SHALL use that endpoint's classification

#### Scenario: Return to the managed local daemon

- **WHEN** the app leaves a daemon target and returns to its managed local daemon connection
- **THEN** the app SHALL classify the current player as using the managed local daemon and SHALL NOT retain the previous daemon's locality

#### Scenario: Restart managed local daemon

- **WHEN** the app successfully reconnects after restarting the managed local daemon
- **THEN** the app SHALL classify the current player as using the managed local daemon

## REMOVED Requirements

### Requirement: Current player locality follows its endpoint
**Reason**: The in-process player target no longer exists.
**Migration**: Replaced by "Current player locality follows its daemon endpoint".

### Requirement: Player-owner locality follows the current target
**Reason**: The in-process player target no longer exists.
**Migration**: Replaced by "Player-owner locality follows the current daemon target".

### Requirement: Player transitions update target classification
**Reason**: The in-process player target no longer exists.
**Migration**: Replaced by "Daemon target transitions update classification".
