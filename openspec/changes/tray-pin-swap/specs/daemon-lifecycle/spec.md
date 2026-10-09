# Spec Delta

## MODIFIED Requirements

### Requirement: Quitting with Stay Alive off stops this machine's local daemon

A TUI whose launch lifecycle is attached to this machine's local daemon SHALL request
coordinated shutdown when it quits and `stay_alive` is false at that moment. The setting
SHALL be read at quit time. A TUI that exits because a Pin swap replaced it SHALL NOT request
coordinated shutdown, whatever `stay_alive` is.

The request SHALL target this machine's local daemon independently of the TUI's current
playback route. It SHALL never be forwarded to a current TCP or explicit Unix target.

#### Scenario: Leftover daemon is cleared on next quit

- **WHEN** a local daemon survives a previous session
- **WHEN** the user starts mbv with `stay_alive` false and attaches to that daemon
- **WHEN** the user quits the TUI
- **THEN** the client SHALL obtain acceptance from that local daemon
- **THEN** no local daemon or stale pid file SHALL remain afterward

#### Scenario: Stay Alive toggled off during the session

- **WHEN** a local-daemon TUI starts with `stay_alive` true
- **WHEN** the user turns Stay Alive off and quits
- **THEN** the client SHALL request coordinated shutdown from the local daemon

#### Scenario: Stay Alive toggled on during the session

- **WHEN** a local-daemon TUI starts or attaches with `stay_alive` false
- **WHEN** the user turns Stay Alive on and quits
- **THEN** the client SHALL disconnect normally and the local daemon SHALL keep running

#### Scenario: TUI is currently routed to a remote daemon

- **WHEN** a TUI launched against this machine's local daemon is currently routed to a TCP
  or explicit Unix daemon
- **WHEN** the TUI quits with `stay_alive` false
- **THEN** the TUI SHALL address the coordinated request to this machine's local daemon
- **THEN** the current remote daemon SHALL receive no shutdown request and SHALL keep running

#### Scenario: Home daemon cannot be reached

- **WHEN** a local-daemon-launched TUI quits with `stay_alive` false
- **WHEN** this machine's local daemon cannot be reached or does not acknowledge within the
  bounded shutdown-request timeout
- **THEN** the TUI SHALL finish exiting without sending the request to any other endpoint
- **THEN** the user SHALL be told that the daemon may still be running and SHALL be given
  `mbv -q` as recovery

#### Scenario: Swapped-out TUI with Stay Alive off

- **WHEN** `stay_alive` is false and a Pin swap replaces the attached TUI
- **THEN** the replaced TUI SHALL exit without requesting coordinated shutdown
- **THEN** the local daemon and playback SHALL keep running with the new TUI attached

### Requirement: Stay Alive off admits one client

While `stay_alive` is false, the local daemon SHALL admit at most one attached client. A connection
attempt made while a client is already attached SHALL be refused with a reason identifying the
exclusive owner, before any queue or playback state is sent, and the refused connection SHALL NOT
affect the attached client. Clients already attached when Stay Alive is turned off SHALL remain
attached. The one exception is the Client the daemon itself started for a Pin swap in progress,
which SHALL be admitted once; any other connection during the swap SHALL still be refused.

#### Scenario: Second terminal with Stay Alive off

- **WHEN** `stay_alive` is false, one TUI is attached, and the user starts mbv in another terminal
- **THEN** the daemon SHALL refuse the new connection with the exclusive-owner reason
- **THEN** the first TUI SHALL remain attached and playback SHALL be unaffected

#### Scenario: Packaged mbvd with Stay Alive off

- **WHEN** `stay_alive` is false and a second client attaches to packaged `mbvd`
- **THEN** `mbvd` SHALL admit it
- **THEN** `mbvd` SHALL keep running when every client has left

#### Scenario: Stay Alive turned off with two clients attached

- **WHEN** two TUIs are attached and the user turns Stay Alive off
- **THEN** both TUIs SHALL remain attached
- **THEN** a third connection attempt SHALL be refused with the exclusive-owner reason

#### Scenario: Pin swap with Stay Alive off

- **WHEN** `stay_alive` is false, one TUI is attached, and the user chooses **Pin** from the Tray
- **THEN** the daemon SHALL admit the pinned Client it started
- **THEN** a terminal mbv started by the user during the swap SHALL be refused with the
  exclusive-owner reason
