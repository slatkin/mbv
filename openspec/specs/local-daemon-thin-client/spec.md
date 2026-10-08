# local-daemon-thin-client Specification

## Purpose
TBD - created by archiving change retire-pty-relay-for-local-daemon-stay-alive. Update Purpose after archive.

## Requirements

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
While stay-alive is enabled, any number of clients SHALL be able to attach to a local daemon at the
same time, and attaching SHALL NOT displace an already-attached client. While stay-alive is
disabled, the daemon admits one client, as defined by the `daemon-lifecycle` capability.

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
mbv SHALL NOT reconstruct a Client's complete on-screen session across that Client exiting. Scroll offsets, open overlays and dialogs, in-flight searches, multi-selection, nested Workspace selections, Queue selection, and the queue undo history SHALL reset in a newly started Client.

A newly started Client SHALL restore only the bounded TUI launch-state snapshot: the selected tab, that tab's selected main Selector pill and selected library item, and Panel focus, with the current-content fallbacks defined by the `tui-launch-state` capability. This bounded launch location is not full Session continuity and SHALL NOT reintroduce a terminal-multiplexing layer.

#### Scenario: A client exits with UI state on screen
- **WHEN** a Client with an open overlay, an active search, multi-selection, a scrolled list, and a selected Queue item exits
- **WHEN** the user starts mbv again
- **THEN** the new Client SHALL start with no overlay, no active search, no multi-selection, default scroll state, and no restored Queue selection
- **THEN** playback SHALL be unaffected

#### Scenario: Persisted state still returns
- **WHEN** a Client exits normally with a selected tab, main Selector pill, library item, and Panel focus
- **WHEN** the user starts mbv again
- **THEN** the new Client SHALL restore that bounded launch location
- **THEN** missing identities SHALL use the current-content fallbacks defined by the `tui-launch-state` capability

### Requirement: A live daemon queue is never overwritten by the saved queue snapshot

The local daemon SHALL hold the authoritative queue and queue source, persist them on accepted queue-changing operations and on graceful shutdown, and reload them at startup. A Client attaching to the local daemon SHALL adopt that owner's live queue and SHALL NOT replace it with a saved Client snapshot. An empty queue SHALL persist as empty: a persisted empty queue SHALL NOT be replaced by an older saved snapshot. No Client SHALL persist, restore, or seed a queue: queue persistence belongs only to the Player owner.

#### Scenario: Attaching to a daemon that is playing

- **WHEN** a Client attaches to the local daemon whose queue is non-empty
- **THEN** the Client SHALL display that owner's queue and cursor
- **THEN** the Client SHALL NOT overwrite that queue with the contents of the saved queue snapshot

#### Scenario: Attaching to an idle daemon

- **WHEN** a Client attaches to the local daemon whose queue is empty and a saved queue snapshot exists
- **THEN** the Client SHALL display an empty queue
- **THEN** the Client SHALL NOT restore the saved queue snapshot
- **THEN** the local daemon SHALL remain empty

#### Scenario: Restart restores the owner's queue

- **WHEN** the local daemon restarts after holding a queue
- **THEN** it SHALL reload that queue and its source before serving Clients
- **AND** attached Clients SHALL display the reloaded queue

#### Scenario: Quitting with stay-alive off preserves the queue

- **WHEN** stay-alive is disabled, the queue is non-empty, and the user quits the client
- **WHEN** the user starts mbv again
- **THEN** the new local daemon SHALL reload that queue and its source
- **THEN** the Client SHALL display it without restoring any Client-side snapshot

### Requirement: Local clients persist launch-scoped reconnect state

A client launched without an explicit daemon endpoint SHALL attempt to restore a saved
auto-reconnect record on startup, and it SHALL write the auto-reconnect record at teardown,
including writing a clear when no remote connection is tracked at exit. This persistence SHALL be
governed by how the client was launched (attached to this machine's local daemon, vs. an explicit
daemon endpoint), not by which connection type it happens to be using at the moment of exit.
Persistence SHALL be skipped only for clients launched against an explicit daemon endpoint.

The client SHALL also write a clear auto-reconnect record when no remote connection is tracked at exit after restoration was attempted.

#### Scenario: Local-daemon client starts with a saved auto-reconnect record

- **WHEN** a client attaches to a same-host local daemon and auto-reconnect is enabled
- **WHEN** a previous session saved an auto-reconnect record (a library route or a direct session)
- **THEN** the client SHALL attempt to restore that connection on startup

#### Scenario: Local-daemon client exits with a tracked connection

- **WHEN** a client of a same-host local daemon exits with a library route, direct session, or
  direct-remote connection currently tracked
- **THEN** the client SHALL write that connection as the auto-reconnect record

#### Scenario: Local-daemon client exits with nothing tracked

- **WHEN** a client of a same-host local daemon exits with no remote connection tracked, having
  attempted to restore a saved record on startup
- **THEN** the client SHALL write a clear auto-reconnect record; this reflects that restoration
  was genuinely attempted and found nothing to keep, not that restoration never ran

#### Scenario: Local-daemon client reconnects to a genuinely remote target mid-session

- **WHEN** a client launched attached to a same-host local daemon restores or establishes a
  connection to a genuinely remote daemon during its run
- **THEN** the client SHALL still write that connection as the auto-reconnect record at teardown,
  even though it is no longer connected to the same-host local daemon it was launched against

#### Scenario: Remote-daemon client exits

- **WHEN** a client launched against an explicit daemon endpoint exits
- **THEN** the client SHALL NOT write the auto-reconnect record and SHALL NOT clear an existing one

#### Scenario: Local-daemon client exits

- **WHEN** a client of a same-host local daemon exits with no remote connection tracked after attempting restoration
- **THEN** the client SHALL clear the auto-reconnect record

### Requirement: Every local Client shows the owner's accepted queue

A Client attached to the local daemon SHALL use that owner's Bound queue, queue source, and playback status as its displayed Local queue state in every state, including while an attached Session or cast receiver is the playback target. An attached Session or cast is a playback target, not a queue owner; a Client has no separate Local queue of its own. Loading a playlist, editing the queue, Save As source changes, and clearing the queue SHALL be visible to other attached Clients through owner-accepted state; none SHALL create a private replacement that hides a different playing owner queue. A Client SHALL NOT claim a load succeeded until the owner has accepted it. An unavailable owner SHALL leave the last confirmed state visible, indicate disconnection or failure, and reconcile from the owner on reconnect instead of later submitting a private replacement.

#### Scenario: Two Clients see a load

- **WHEN** Client A loads a playlist into the local daemon while Client B is attached
- **THEN** both Clients SHALL show the owner's new playlist contents and source
- **AND** neither Client SHALL retain the previous owner queue as its active Local queue

#### Scenario: Concurrent Clients replace the queue

- **WHEN** two attached Clients send different accepted loads in sequence
- **THEN** both SHALL show the queue and source from the owner's latest accepted replacement
- **AND** neither SHALL restore its own earlier queue

#### Scenario: Load while disconnected or rejected

- **WHEN** a Client attempts to load a playlist but cannot reach the owner, or the owner rejects it
- **THEN** the Client SHALL report failure and SHALL NOT show the attempted playlist as its Local queue
- **AND** reconnect SHALL adopt the owner's current queue rather than retry the unaccepted load automatically

#### Scenario: Save the currently displayed queue under a new playlist name

- **WHEN** a Client's Save As operation succeeds for the owner queue it still displays
- **THEN** the updated Playlist source SHALL be reflected by the owner and all attached Clients
- **AND** queue contents and playback state SHALL remain unchanged

#### Scenario: A stay-alive Client watches a session or casts

- **WHEN** a Client attached to the local daemon is watching an attached Emby Session or casting to a receiver
- **THEN** its displayed Local queue SHALL remain the owner's Bound queue
- **AND** queue edits SHALL still be accepted by that owner rather than becoming a private queue

### Requirement: A client shows when playback outlives it
While stay-alive is enabled, a client SHALL show an indicator that playback is hosted by a local
daemon that outlives this terminal, so the user can tell that closing the terminal will not stop
playback.

#### Scenario: Client is attached with stay-alive enabled
- **WHEN** a client is attached to a local daemon and stay-alive is enabled
- **THEN** the UI SHALL show that playback is hosted outside this terminal

#### Scenario: Stay-alive disabled
- **WHEN** a client is attached to a local daemon and stay-alive is disabled
- **THEN** that indicator SHALL NOT be shown

### Requirement: Quitting a client follows Stay Alive and offers no detach
The client's quit action SHALL exit the client process. mbv SHALL NOT offer a separate detach
action, and SHALL NOT report detach success or failure. With stay-alive enabled, quitting SHALL NOT
stop playback; with stay-alive disabled, quitting SHALL stop playback and end the local daemon.

#### Scenario: User quits a client
- **WHEN** stay-alive is enabled and the user quits a client while media is playing
- **THEN** the client SHALL save its state and exit
- **THEN** playback SHALL continue in the local daemon

#### Scenario: Quit with stay-alive disabled
- **WHEN** stay-alive is disabled and the user quits the client
- **THEN** playback SHALL stop, the local daemon SHALL exit, and the client SHALL exit
