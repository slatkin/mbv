# Spec Delta

## MODIFIED Requirements

### Requirement: Stay-alive hosts playback in a local daemon

The Player owner for a TUI SHALL be a local daemon: a user-owned background process on the same
machine, holding no terminal, that binds the user's control socket. This SHALL hold whether
stay-alive is enabled or not; stay-alive decides only the daemon's lifetime. The terminal SHALL NOT
own the Player; it SHALL run as a client of that daemon.

#### Scenario: Stay-alive is requested and no local daemon exists

- **WHEN** mbv starts with stay-alive enabled and nothing is listening on the user's control socket
- **THEN** mbv SHALL start a local daemon that owns the Player
- **THEN** mbv SHALL attach to that daemon as a client and present its normal terminal UI
- **THEN** mbv SHALL NOT create a pseudo-terminal, a relay process, or a byte-pipe client

#### Scenario: Stay-alive is disabled and no local daemon exists

- **WHEN** mbv starts with stay-alive disabled and nothing is listening on the user's control socket
- **THEN** mbv SHALL start a local daemon that owns the Player and admits only this client
- **THEN** mbv SHALL attach to that daemon as a client and present its normal terminal UI

#### Scenario: Stay-alive is requested and a local daemon is already running

- **WHEN** mbv starts with stay-alive enabled and a local daemon is already listening
- **THEN** mbv SHALL NOT start a second daemon
- **THEN** mbv SHALL attach to the running daemon as a client

#### Scenario: Audio window is configured

- **WHEN** `show_audio_window` is enabled and the local daemon plays audio in a desktop session
- **THEN** the local daemon SHALL show the audio window, as a terminal-owned Player did

#### Scenario: The daemon is not yet accepting connections

- **WHEN** mbv has just started a local daemon and the control socket is not yet accepting connections
- **THEN** mbv SHALL retry the connection for a bounded period before reporting failure
- **THEN** mbv SHALL report a diagnostic on the terminal if the daemon never becomes connectable
