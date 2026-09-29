## MODIFIED Requirements

### Requirement: Stay-alive hosts playback in a local daemon
The Player owner for a TUI launched without an explicit daemon endpoint SHALL be a local daemon: a
user-owned background process on the same machine, holding no terminal, that binds the user's
control socket. This SHALL hold whether stay-alive is enabled or not; stay-alive decides only the
daemon's lifetime. The terminal SHALL NOT own the Player; it SHALL run as a client of that daemon.

#### Scenario: Stay-alive is requested and no local daemon exists
- **WHEN** mbv starts with stay-alive enabled and nothing is listening on the user's control socket
- **THEN** mbv SHALL start a local daemon that owns the Player
- **THEN** mbv SHALL attach to that daemon as a client and present its normal terminal UI
- **THEN** mbv SHALL NOT create a pseudo-terminal, a relay process, or a byte-pipe client

#### Scenario: Stay-alive is disabled and no local daemon exists
- **WHEN** mbv starts with stay-alive disabled, without an explicit daemon endpoint, and nothing is listening on the user's control socket
- **THEN** mbv SHALL start a local daemon that owns the Player and admits only this client
- **THEN** mbv SHALL attach to that daemon as a client and present its normal terminal UI

#### Scenario: Stay-alive is requested and a local daemon is already running
- **WHEN** mbv starts with stay-alive enabled and a local daemon is already listening
- **THEN** mbv SHALL NOT start a second daemon
- **THEN** mbv SHALL attach to the running daemon as a client

#### Scenario: The daemon is not yet accepting connections
- **WHEN** mbv has just started a local daemon and the control socket is not yet accepting connections
- **THEN** mbv SHALL retry the connection for a bounded period before reporting failure
- **THEN** mbv SHALL report a diagnostic on the terminal if the daemon never becomes connectable

### Requirement: A client exiting never stops the local daemon
While stay-alive is enabled, the local daemon's lifetime SHALL be independent of its clients.
Closing a client, its terminal, or its SSH connection SHALL NOT stop the daemon or interrupt
playback, regardless of whether that client started the daemon. While stay-alive is disabled, the
daemon's lifetime SHALL end with its client, as defined by the `daemon-lifecycle` capability.

#### Scenario: The client that started the daemon exits
- **WHEN** stay-alive is enabled and the client that started the local daemon exits
- **THEN** the daemon SHALL keep running and playback SHALL continue

#### Scenario: The last client exits
- **WHEN** stay-alive is enabled and the last attached client exits while media is playing
- **THEN** the daemon SHALL keep running and playback SHALL continue

#### Scenario: A client's terminal is destroyed
- **WHEN** stay-alive is enabled and a client's terminal is closed, its SSH session drops, or the client process is killed
- **THEN** the daemon SHALL keep running and playback SHALL continue

#### Scenario: Stay-alive disabled and the client exits
- **WHEN** stay-alive is disabled and the daemon's only client exits for any reason
- **THEN** playback SHALL stop and the daemon SHALL exit

### Requirement: Stopping the local daemon is always explicit
While stay-alive is enabled, the local daemon SHALL stop only in response to an explicit request:
`mbv -q`, the tray's quit action, or a termination signal sent to it directly. No client action
SHALL stop it implicitly. While stay-alive is disabled, quitting the client or losing it SHALL also
stop the daemon.

#### Scenario: Explicit quit
- **WHEN** the user runs `mbv -q` or selects the tray's quit action
- **THEN** the daemon SHALL stop playback, persist its state, and exit

#### Scenario: Quitting a client
- **WHEN** stay-alive is enabled and the user quits a client from within its UI
- **THEN** the client SHALL exit
- **THEN** the daemon SHALL NOT stop and playback SHALL continue

#### Scenario: Quitting the client with stay-alive disabled
- **WHEN** stay-alive is disabled and the user quits the client from within its UI
- **THEN** the daemon SHALL stop playback, persist its state, and exit
- **THEN** the client SHALL exit

## REMOVED Requirements

### Requirement: Bare mode is unchanged when stay-alive is off
**Reason**: There is no in-process Player owner any more. With stay-alive off, playback is hosted by a local daemon that admits one client and ends with it, so there are no longer two queue authorities.
**Migration**: The behaviour users see is unchanged: quitting stops playback, no process is left running, and a second terminal is refused. See the `daemon-lifecycle` requirements "Stay Alive off admits one client" and "Ordinary disconnect is not shutdown". The owner's first start reads the legacy Bare queue file through its existing one-time takeover.
