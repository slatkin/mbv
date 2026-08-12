# daemon-lifecycle Specification

## Purpose

Defines when this machine's local daemon starts, survives a TUI, and stops, including the
durability and targeting guarantees for coordinated shutdown.
## Requirements
### Requirement: Stay Alive is the sole configuration policy for TUI-exit lifetime

The `stay_alive` configuration setting SHALL be the only setting determining whether this
machine's local daemon automatically outlives a TUI attached to it. No command-line flag
SHALL enable Stay Alive. Explicit lifecycle controls such as `mbv -q`, tray Quit, and
operating-system termination remain independent of this setting.

#### Scenario: Stay Alive enabled

- **WHEN** `stay_alive` is true and the user starts mbv with no local daemon running
- **THEN** a local daemon SHALL own playback and the TUI SHALL attach to it
- **THEN** quitting the TUI SHALL leave the daemon and playback running

#### Scenario: Stay Alive disabled with no daemon running

- **WHEN** `stay_alive` is false and no local daemon is running
- **THEN** mbv SHALL own playback in-process and SHALL NOT start a daemon

#### Scenario: Legacy daemon flag is rejected

- **WHEN** the user invokes `mbv -d`
- **THEN** mbv SHALL exit with guidance to enable `stay_alive` in configuration or the
  settings overlay
- **THEN** mbv SHALL NOT silently treat `-d` as an ordinary foreground invocation

### Requirement: Quitting with Stay Alive off stops this machine's local daemon

A TUI whose launch lifecycle is attached to this machine's local daemon SHALL request
coordinated shutdown when it quits and `stay_alive` is false at that moment. The setting
SHALL be read at quit time.

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

### Requirement: Accepted shutdown is unconditional

After a permitted request's queue persistence succeeds and the daemon accepts the request,
the daemon SHALL shut down regardless of attached client count or playback state. It SHALL
NOT defer for the last client or the end of the current track.

#### Scenario: Daemon is playing when request is accepted

- **WHEN** the daemon is mid-track and accepts a coordinated shutdown request
- **THEN** the daemon SHALL stop playback and exit

#### Scenario: Other clients are attached

- **WHEN** multiple TUIs are attached and one obtains shutdown acceptance
- **THEN** the daemon SHALL announce deliberate shutdown to every attached client
- **THEN** every other client SHALL exit cleanly rather than report an unannounced loss

#### Scenario: Persistence failure occurs before acceptance

- **WHEN** queue persistence fails while evaluating a coordinated request
- **THEN** the daemon SHALL reject rather than accept the request
- **THEN** the unconditional-shutdown rule SHALL NOT begin and playback SHALL continue

### Requirement: Coordinated shutdown durably preserves the authoritative queue

Before accepting coordinated shutdown, the daemon SHALL persist its authoritative queue,
cursor, source, and current non-audio playback positions to disk. It SHALL use daemon-owned
state rather than a requesting client's shadow. An empty queue at quit SHALL NOT erase an
older non-empty snapshot; only an explicit Clear Queue action may do that.

#### Scenario: Concurrent client changed the queue

- **WHEN** another client changes the queue before the daemon evaluates the shutdown request
- **THEN** the persisted snapshot SHALL contain the daemon's resulting authoritative queue
  and cursor rather than the requester's older shadow

#### Scenario: Requester is routed away from the local daemon

- **WHEN** the requesting TUI has not been receiving local-daemon queue broadcasts because
  it is currently routed elsewhere
- **THEN** coordinated shutdown SHALL still persist the local daemon's current queue

#### Scenario: Mid-track position is preserved

- **WHEN** the local daemon is playing a non-audio item when it evaluates the request
- **THEN** the persisted snapshot SHALL include the latest valid playback position before
  the daemon accepts and stops

#### Scenario: Durable write fails

- **WHEN** directory creation, serialization, temporary-file write, or atomic rename fails
- **THEN** the daemon SHALL reject shutdown and leave the previous snapshot intact

### Requirement: Ordinary disconnect is not shutdown

A client disconnecting without an accepted coordinated request SHALL leave the daemon
running, including when it is the last connected client.

#### Scenario: Last client disconnects with Stay Alive on

- **WHEN** the only attached client quits with `stay_alive` true
- **THEN** the daemon and playback SHALL continue running

#### Scenario: Client connection is lost

- **WHEN** a client connection drops without an accepted shutdown request
- **THEN** the daemon SHALL continue running

### Requirement: Explicit non-local clients never request automatic shutdown

A client launched against an explicit `unix://` or `tcp://` endpoint SHALL NOT derive an
automatic shutdown request from the local `stay_alive` value.

#### Scenario: Explicit remote client quits

- **WHEN** a client launched against `unix://…` or `tcp://…` quits with `stay_alive` false
- **THEN** it SHALL send no automatic shutdown request
- **THEN** that daemon SHALL keep running

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

