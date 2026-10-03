# local-daemon-stay-alive Specification

## Purpose
TBD - created by archiving change retire-pty-relay-for-local-daemon-stay-alive. Update Purpose after archive.

## Requirements

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

#### Scenario: Audio window is configured
- **WHEN** `show_audio_window` is enabled and the local daemon plays audio in a desktop session
- **THEN** the local daemon SHALL show the audio window, as a terminal-owned Player did

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

### Requirement: Audiobookshelf playback and progress synchronization continue across client exits
When a daemon owner has an Audiobookshelf episode active, playback and periodic progress synchronization SHALL continue uninterrupted after every attached client exits. Session finalization, bounded retry, and queue advancement SHALL proceed from the daemon owner without requiring a client to be present, consistent with how non-Audiobookshelf media remains active under the existing stay-alive lifecycle.

#### Scenario: Last client exits while Audiobookshelf episode is active
- **WHEN** the last attached client exits while the daemon owner is playing an Audiobookshelf episode
- **THEN** playback SHALL continue and periodic synchronization SHALL proceed on its normal interval from the daemon owner
- **THEN** no Audiobookshelf finalization or queue mutation SHALL be triggered by client exit alone

#### Scenario: A later client attaches while daemon is playing an Audiobookshelf episode
- **WHEN** a later capable client attaches to a daemon owner that is playing an Audiobookshelf episode
- **THEN** the client SHALL receive the live canonical queue, active slot, status, and last-broadcast acknowledged Audiobookshelf progress via the existing attach snapshot
- **THEN** the daemon owner's playback and synchronization authority SHALL not be transferred to the attaching client
