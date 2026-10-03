# Spec Delta

## MODIFIED Requirements

### Requirement: Stopping the local daemon is always explicit
While stay-alive is enabled, the local daemon SHALL stop only in response to an explicit request:
`mbv -q`, the tray's quit action, or a termination signal sent to it directly. No client action
SHALL stop it implicitly. While stay-alive is disabled, quitting the client or losing it SHALL also
stop the daemon. When the daemon stops, it SHALL stop playback and allow the playing item's final
stop report to complete, within a bounded time, before the process exits. It SHALL persist its
state after playback has stopped.

#### Scenario: Explicit quit
- **WHEN** the user runs `mbv -q` or selects the tray's quit action
- **THEN** the daemon SHALL stop playback, persist its state, and exit

#### Scenario: Quit while an Emby video plays
- **WHEN** the daemon is stopped while an Emby video is playing at 31%
- **THEN** the stop report carrying that position SHALL be sent to Emby before the process exits, unless the shutdown time bound expires first
- **THEN** the next play of that item, on any machine, SHALL resume at that position

#### Scenario: Quitting a client
- **WHEN** stay-alive is enabled and the user quits a client from within its UI
- **THEN** the client SHALL exit
- **THEN** the daemon SHALL NOT stop and playback SHALL continue

#### Scenario: Quitting the client with stay-alive disabled
- **WHEN** stay-alive is disabled and the user quits the client from within its UI
- **THEN** the daemon SHALL stop playback, persist its state, and exit
- **THEN** the client SHALL exit
