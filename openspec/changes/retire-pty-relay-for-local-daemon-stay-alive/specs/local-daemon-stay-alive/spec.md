## ADDED Requirements

### Requirement: Stay-alive hosts playback in a local daemon
In stay-alive mode the Player owner SHALL be a local daemon: a user-owned background process on
the same machine, holding no terminal, that binds the user's control socket. The terminal that
requested stay-alive SHALL NOT own the Player; it SHALL run as a client of that daemon.

#### Scenario: Stay-alive is requested and no local daemon exists
- **WHEN** mbv starts with stay-alive enabled and nothing is listening on the user's control socket
- **THEN** mbv SHALL start a local daemon that owns the Player
- **THEN** mbv SHALL attach to that daemon as a client and present its normal terminal UI
- **THEN** mbv SHALL NOT create a pseudo-terminal, a relay process, or a byte-pipe client

#### Scenario: Stay-alive is requested and a local daemon is already running
- **WHEN** mbv starts with stay-alive enabled and a local daemon is already listening
- **THEN** mbv SHALL NOT start a second daemon
- **THEN** mbv SHALL attach to the running daemon as a client

#### Scenario: The daemon is not yet accepting connections
- **WHEN** mbv has just started a local daemon and the control socket is not yet accepting connections
- **THEN** mbv SHALL retry the connection for a bounded period before reporting failure
- **THEN** mbv SHALL report a diagnostic on the terminal if the daemon never becomes connectable

### Requirement: Stay-alive is selected by configuration or the `-d` flag
mbv SHALL enable stay-alive when the `stay_alive` configuration key is true or when the `-d` flag
is present on the command line. The `-d` flag SHALL apply to that invocation only and SHALL have
no effect beyond the configured value when `stay_alive` is already true. mbv SHALL NOT recognise
`-a` or `--alive`.

#### Scenario: Stay-alive requested for one invocation
- **WHEN** the user runs mbv with `-d` and `stay_alive` is false in configuration
- **THEN** that invocation SHALL use stay-alive
- **THEN** the configuration file SHALL NOT be modified

#### Scenario: `-d` with stay-alive already configured
- **WHEN** the user runs mbv with `-d` and `stay_alive` is already true in configuration
- **THEN** mbv SHALL behave exactly as it would without the flag

#### Scenario: Retired flag is used
- **WHEN** the user runs mbv with `-a` or `--alive`
- **THEN** mbv SHALL NOT enable stay-alive as a result of that flag
- **THEN** mbv's usage output SHALL document `-d` and SHALL NOT document `-a` or `--alive`

### Requirement: Authentication precedes daemon start
Because a local daemon holds no terminal and cannot prompt, the terminal requesting stay-alive
SHALL complete authentication before starting the daemon. The daemon SHALL obtain its credentials
from the cached token rather than by prompting.

#### Scenario: Fresh stay-alive start
- **WHEN** mbv starts with stay-alive enabled and no local daemon is running
- **THEN** mbv SHALL authenticate, including any interactive login, before starting the daemon
- **THEN** the daemon SHALL read the cached token rather than prompting for credentials

#### Scenario: Cached credentials are unusable at daemon start
- **WHEN** the local daemon cannot authenticate with the cached token
- **THEN** the daemon SHALL fail to start rather than run without a usable session
- **THEN** the failure and its reason SHALL be reported on the terminal that requested stay-alive
- **THEN** mbv SHALL exit with a non-zero status rather than presenting a UI with no playback backend

### Requirement: A client exiting never stops the local daemon
The local daemon's lifetime SHALL be independent of its clients. Closing a client, its terminal,
or its SSH connection SHALL NOT stop the daemon or interrupt playback, regardless of whether that
client started the daemon.

#### Scenario: The client that started the daemon exits
- **WHEN** the client that started the local daemon exits
- **THEN** the daemon SHALL keep running and playback SHALL continue

#### Scenario: The last client exits
- **WHEN** the last attached client exits while media is playing
- **THEN** the daemon SHALL keep running and playback SHALL continue

#### Scenario: A client's terminal is destroyed
- **WHEN** a client's terminal is closed, its SSH session drops, or the client process is killed
- **THEN** the daemon SHALL keep running and playback SHALL continue

### Requirement: Stopping the local daemon is always explicit
The local daemon SHALL stop only in response to an explicit request: `mbv -q`, the tray's quit
action, or a termination signal sent to it directly. No client action SHALL stop it implicitly.

#### Scenario: Explicit quit
- **WHEN** the user runs `mbv -q` or selects the tray's quit action
- **THEN** the daemon SHALL stop playback, persist its state, and exit

#### Scenario: Quitting a client
- **WHEN** the user quits a client from within its UI
- **THEN** the client SHALL exit
- **THEN** the daemon SHALL NOT stop and playback SHALL continue

### Requirement: Bare mode is unchanged when stay-alive is off
With stay-alive disabled, mbv SHALL run as a single process that owns the Player in-process. mbv
SHALL NOT start a local daemon, SHALL NOT leave any process running after it exits, and SHALL NOT
change its behavior because stay-alive exists.

#### Scenario: Default launch
- **WHEN** the user runs mbv with stay-alive disabled
- **THEN** mbv SHALL own the Player in its own process
- **THEN** no control socket SHALL be bound on mbv's behalf

#### Scenario: Bare mode exits
- **WHEN** a bare-mode mbv exits for any reason
- **THEN** playback SHALL stop and no mbv-owned process SHALL remain running
