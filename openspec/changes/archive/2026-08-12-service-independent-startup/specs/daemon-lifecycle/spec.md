## ADDED Requirements

### Requirement: Local playback ownership is independent of Emby setup
Bare mode and the Local daemon SHALL start and remain available without a configured, authenticated, or reachable Emby Service. Stay Alive policy and single-instance process-role selection SHALL remain independent of Remote Service state.

#### Scenario: Bare feed-only startup
- **WHEN** Stay Alive is disabled and no Remote Service is configured
- **THEN** mbv SHALL create its in-process Player owner
- **THEN** feed playback SHALL be available

#### Scenario: Stay-alive feed-only startup
- **WHEN** Stay Alive is enabled and no Remote Service is configured
- **THEN** mbv SHALL start or attach to the Local daemon
- **THEN** that daemon SHALL accept playable feed items and preserve playback continuity

#### Scenario: Emby is unavailable during Local daemon startup
- **WHEN** the Local daemon starts with a configured Emby Service that cannot connect
- **THEN** the daemon SHALL remain running and controllable
- **THEN** non-Emby playback SHALL remain available

#### Scenario: Existing Local daemon has no Emby credential
- **WHEN** a client attaches to a running Local daemon using its Control credential
- **THEN** attachment SHALL not require either process to have an Emby credential
