## MODIFIED Requirements

### Requirement: Stay Alive is the sole configuration policy for TUI-exit lifetime

The `stay_alive` configuration setting SHALL be the only setting determining whether this
machine's local daemon automatically outlives a TUI attached to it. No command-line flag
SHALL enable Stay Alive. Explicit lifecycle controls such as `mbv -q`, tray Quit, and
operating-system termination remain independent of this setting. A TUI launched without an
explicit daemon endpoint SHALL always reach playback through this machine's local daemon;
`stay_alive` SHALL decide only that daemon's lifetime and how many TUIs it admits, never whether
it exists.

#### Scenario: Stay Alive enabled

- **WHEN** `stay_alive` is true and the user starts mbv with no local daemon running
- **THEN** a local daemon SHALL own playback and the TUI SHALL attach to it
- **THEN** quitting the TUI SHALL leave the daemon and playback running

#### Scenario: Stay Alive disabled with no daemon running

- **WHEN** `stay_alive` is false and no local daemon is running
- **THEN** mbv SHALL start a local daemon that owns playback and the TUI SHALL attach to it
- **THEN** the TUI process SHALL NOT own a Player
- **THEN** quitting the TUI SHALL stop playback and leave no mbv-owned process running

#### Scenario: Legacy daemon flag is rejected

- **WHEN** the user invokes `mbv -d`
- **THEN** mbv SHALL exit with guidance to enable `stay_alive` in configuration or the
  settings overlay
- **THEN** mbv SHALL NOT silently treat `-d` as an ordinary foreground invocation

### Requirement: Ordinary disconnect is not shutdown

While `stay_alive` is true, a client disconnecting without an accepted coordinated request SHALL
leave the daemon running, including when it is the last connected client. While `stay_alive` is
false, the local daemon SHALL treat the loss of its last connected client as a coordinated
shutdown: it SHALL persist its queue, stop playback, and exit.

#### Scenario: Last client disconnects with Stay Alive on

- **WHEN** the only attached client quits with `stay_alive` true
- **THEN** the daemon and playback SHALL continue running

#### Scenario: Client connection is lost

- **WHEN** a client connection drops without an accepted shutdown request while `stay_alive` is true
- **THEN** the daemon SHALL continue running

#### Scenario: Last client is lost with Stay Alive off

- **WHEN** `stay_alive` is false and the daemon's last connected client is killed, its terminal is closed, or its connection otherwise drops
- **THEN** the daemon SHALL persist its queue, stop playback, and exit
- **THEN** no mbv-owned process SHALL remain running

## ADDED Requirements

### Requirement: The local daemon is independent of Emby setup
The local daemon SHALL start and remain available without a configured, authenticated, or reachable Emby Service, whatever `stay_alive` is set to. Stay Alive policy and single-instance process-role selection SHALL remain independent of Remote Service state.

#### Scenario: Feed-only startup with Stay Alive off
- **WHEN** Stay Alive is disabled and no Remote Service is configured
- **THEN** mbv SHALL start and attach to the local daemon
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


### Requirement: The daemon reads Stay Alive when it decides

The local daemon SHALL read the current `stay_alive` setting each time it makes a lifetime decision:
admitting a client, losing its last client, and evaluating a coordinated shutdown request. It
SHALL NOT act on a value captured when it started.

#### Scenario: Stay Alive turned off after the daemon started with it on

- **WHEN** the local daemon started while `stay_alive` was true
- **WHEN** the user turns Stay Alive off and quits the TUI
- **THEN** the daemon SHALL accept the coordinated shutdown request and exit

#### Scenario: Stay Alive turned on after the daemon started with it off

- **WHEN** the local daemon started while `stay_alive` was false
- **WHEN** the user turns Stay Alive on and quits the TUI
- **THEN** the daemon SHALL keep running and playback SHALL continue

### Requirement: Stay Alive off admits one client

While `stay_alive` is false, the local daemon SHALL admit at most one attached client. A connection
attempt made while a client is already attached SHALL be refused with a reason identifying the
exclusive owner, before any queue or playback state is sent, and the refused connection SHALL NOT
affect the attached client. Clients already attached when Stay Alive is turned off SHALL remain
attached.

#### Scenario: Second terminal with Stay Alive off

- **WHEN** `stay_alive` is false, one TUI is attached, and the user starts mbv in another terminal
- **THEN** the daemon SHALL refuse the new connection with the exclusive-owner reason
- **THEN** the first TUI SHALL remain attached and playback SHALL be unaffected

#### Scenario: Stay Alive turned off with two clients attached

- **WHEN** two TUIs are attached and the user turns Stay Alive off
- **THEN** both TUIs SHALL remain attached
- **THEN** a third connection attempt SHALL be refused with the exclusive-owner reason

## REMOVED Requirements

### Requirement: Local playback ownership is independent of Emby setup
**Reason**: It named Bare mode's in-process Player owner, which no longer exists.
**Migration**: Replaced by "The local daemon is independent of Emby setup", which carries the same guarantees for the local daemon whatever `stay_alive` is set to.
