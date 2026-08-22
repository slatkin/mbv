## MODIFIED Requirements

### Requirement: Unsupported playback paths remain unchanged

The visualizer SHALL NOT start system-output capture for attached Emby Session playback, playback on an attached cast target, or audio-pipe playback. Playback hosted by a same-host Local daemon SHALL NOT be treated as unsupported. Direct remote Player-owner playback SHALL permit local capture because external local forwarding such as Snapcast can make that playback audible on this machine; when no such forwarding exists, the local system-output monitor is simply silent.

#### Scenario: Attached playback bypasses capture

- **WHEN** playback is hosted by an attached Emby Session or an audio pipe
- **THEN** the visualizer does not start system-output capture
- **AND** same-host Local daemon playback and direct remote Player-owner playback retain their stated capture behavior

#### Scenario: Attached cast target bypasses capture

- **WHEN** a cast target is attached
- **THEN** the visualizer does not start system-output capture
- **AND** any capture already running is stopped and its resources released
