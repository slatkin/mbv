## ADDED Requirements

### Requirement: Clients present the full terminal UI
A client of a local daemon SHALL present mbv's complete terminal UI — browsing, search, queue
editing, playback control, and rendering — on its own real terminal. It SHALL NOT be a byte pipe
to another process, and it SHALL perform its own terminal capability detection.

#### Scenario: Client renders on its own terminal
- **WHEN** a client attaches to a local daemon
- **THEN** the client SHALL detect its own terminal's graphics and font capabilities directly
- **THEN** album art and other graphics SHALL render according to that terminal's capabilities

#### Scenario: Terminals with different capabilities
- **WHEN** two clients on terminals with different graphics support are attached at the same time
- **THEN** each SHALL render according to its own terminal's capabilities

### Requirement: Several clients may be attached at once
Any number of clients SHALL be able to attach to a local daemon at the same time. Attaching SHALL
NOT displace an already-attached client.

#### Scenario: A second client attaches
- **WHEN** a client is attached and the user starts mbv in another terminal with stay-alive enabled
- **THEN** the new client SHALL attach to the same local daemon
- **THEN** the already-attached client SHALL remain attached and usable

#### Scenario: Playback state is shared
- **WHEN** one attached client issues a playback command
- **THEN** every attached client SHALL reflect the resulting playback state

### Requirement: Playback continuity across client restarts
Stay-alive SHALL preserve playback across every client closing and a later client opening: what is
playing, the queue, and the playback position SHALL be unaffected by clients coming and going.

#### Scenario: All clients close and one reopens
- **WHEN** every client exits while media is playing, and the user later starts mbv again
- **THEN** playback SHALL have continued uninterrupted throughout
- **THEN** the new client SHALL show the currently playing item, the live queue, and the current position

### Requirement: Session continuity is not provided
mbv SHALL NOT preserve a client's on-screen state across that client exiting. Cursor position,
scroll offsets, open overlays and dialogs, in-flight searches, and the queue undo history SHALL be
reset in a newly started client. This is an explicit non-goal, not a deficiency to be corrected by
reintroducing a terminal-multiplexing layer.

#### Scenario: A client exits with UI state on screen
- **WHEN** a client with an open overlay, an active search, and a scrolled list exits
- **WHEN** the user starts mbv again
- **THEN** the new client SHALL start with no overlay, no active search, and default scroll state
- **THEN** playback SHALL be unaffected

#### Scenario: Persisted state still returns
- **WHEN** a client starts after a previous client exited
- **THEN** per-library browse positions and persisted preference values SHALL be restored as they are in bare mode

### Requirement: A live daemon queue is never overwritten by the saved queue snapshot
When a client attaches to a local daemon that already holds a queue, the client SHALL adopt the
daemon's live queue and SHALL NOT replace it with the queue snapshot saved on disk.

#### Scenario: Attaching to a daemon that is playing
- **WHEN** a client attaches to a local daemon whose queue is non-empty
- **THEN** the client SHALL display the daemon's queue and cursor
- **THEN** the client SHALL NOT overwrite that queue with the contents of the saved queue snapshot

#### Scenario: Attaching to an idle daemon
- **WHEN** a client attaches to a local daemon whose queue is empty and a saved queue snapshot exists
- **THEN** the client SHALL restore the saved queue exactly as bare mode does
- **THEN** the restored queue SHALL become the daemon's queue

### Requirement: Clients persist the state bare mode persists
A client of a same-host local daemon SHALL perform the same session-state persistence a bare-mode
mbv performs, including the auto-reconnect record written at teardown. Persistence SHALL be skipped
only for clients of a genuinely remote daemon.

#### Scenario: Local-daemon client exits
- **WHEN** a client of a same-host local daemon exits with auto-reconnect enabled
- **THEN** the client SHALL write the auto-reconnect record as a bare-mode instance would

#### Scenario: Remote-daemon client exits
- **WHEN** a client of a daemon on another machine exits
- **THEN** the client SHALL NOT write the auto-reconnect record and SHALL NOT clear an existing one

### Requirement: A client indicates that it does not own playback
A client SHALL show an indicator that playback is hosted by a local daemon rather than by this
terminal, so the user can tell that closing the terminal will not stop playback.

#### Scenario: Client is attached to a local daemon
- **WHEN** a client is attached to a local daemon
- **THEN** the UI SHALL show that playback is hosted outside this terminal

#### Scenario: Bare mode
- **WHEN** mbv owns the Player in-process
- **THEN** that indicator SHALL NOT be shown

### Requirement: Quitting a client is not a detach action
In stay-alive mode the client's quit action SHALL exit the client process and SHALL NOT stop
playback. mbv SHALL NOT offer a separate detach action, and SHALL NOT report detach success or
failure.

#### Scenario: User quits a client
- **WHEN** the user quits a client while media is playing
- **THEN** the client SHALL save its state and exit
- **THEN** playback SHALL continue in the local daemon

#### Scenario: Bare mode quit
- **WHEN** the user quits a bare-mode mbv
- **THEN** playback SHALL stop and the process SHALL exit
