## REMOVED Requirements

### Requirement: Bare podcast playback does not add Socket.IO, audiobook, or credential transport
**Reason**: Superseded by the `add-audiobookshelf-progress-refresh` change,
which adds a bounded Audiobookshelf Socket.IO connection to the interactive
bare-mode process for browse/queue progress refresh.
**Migration**: See the `audiobookshelf-progress-refresh` capability for the
Socket.IO connection's scope and lifecycle, and the replacement requirement
below for what bare-mode playback still does not add.

## ADDED Requirements

### Requirement: Bare podcast playback adds no audiobook support or credential transport, and no Socket.IO remote control
Bare-mode Audiobookshelf playback SHALL NOT support audiobook media, transfer
Service credentials between processes, or make Audiobookshelf items playable
by a remote-only ctrl owner. The Audiobookshelf Socket.IO connection added by
the `audiobookshelf-progress-refresh` capability SHALL NOT carry
remote-control commands and SHALL NOT alter the in-process Player owner's own
playback-session lifecycle, which remains driven exclusively by REST.
Daemon owners that have negotiated transport capability MAY carry
Audiobookshelf queue items and acknowledged progress over the capability-gated
ctrl seam established by the transport child (#525); this is not a bare-mode
concern.

#### Scenario: User plays podcasts in bare mode
- **WHEN** the user plays, seeks, pauses, completes, or stops downloaded Audiobookshelf episodes in bare mode
- **THEN** all Audiobookshelf credentials and playback-session lifecycle SHALL remain in the in-process Player owner
- **THEN** the active session's progress SHALL be driven only by REST synchronization, never by a Socket.IO event

#### Scenario: Daemon owner carries Audiobookshelf transport over ctrl
- **WHEN** a daemon owner with installed setup and a capable attached client plays an Audiobookshelf episode
- **THEN** Audiobookshelf credentials and playback-session lifecycle SHALL remain in the daemon owner
- **THEN** queue items and acknowledged progress MAY flow over the capability-gated ctrl seam to capable clients
- **THEN** no Audiobookshelf Socket.IO connection SHALL be opened by the daemon owner
