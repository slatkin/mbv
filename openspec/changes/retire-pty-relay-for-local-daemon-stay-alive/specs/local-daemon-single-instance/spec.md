## ADDED Requirements

### Requirement: The lock identifies the Player owner
mbv SHALL use an advisory lock file in the user's runtime directory to identify the Player owner.
The bare-mode application SHALL hold it in bare mode; the local daemon SHALL hold it in stay-alive
mode. The lock SHALL be held for the Player owner's whole lifetime, and the Player owner SHALL
write its process ID into the lock file after acquiring it.

#### Scenario: Bare mode acquires the lock
- **WHEN** a bare-mode mbv starts and no Player owner exists
- **THEN** it SHALL acquire the lock and write its process ID into the lock file

#### Scenario: The local daemon acquires the lock
- **WHEN** a local daemon starts
- **THEN** the daemon SHALL acquire the lock and write its process ID into the lock file
- **THEN** the terminal that started it SHALL NOT hold the lock

#### Scenario: The Player owner dies without cleanup
- **WHEN** the Player owner is killed without running any cleanup
- **THEN** the lock SHALL be released by the operating system
- **THEN** the next mbv launch SHALL be treated as a fresh start

### Requirement: Resolution probes the control socket
When the lock is already held, mbv SHALL decide what to do by attempting to connect to the user's
control socket. Socket-file existence SHALL NOT be treated as evidence of a live daemon; only a
successful connection SHALL count.

#### Scenario: Lock is free
- **WHEN** the lock can be acquired
- **THEN** mbv SHALL proceed as a fresh start

#### Scenario: Lock is held and the control socket accepts a connection
- **WHEN** the lock is held and the control socket accepts a connection
- **THEN** mbv SHALL attach to the local daemon as a client

#### Scenario: Lock is held and the control socket refuses a connection
- **WHEN** the lock is held and no connection to the control socket can be established
- **THEN** mbv SHALL refuse to start

#### Scenario: A stale socket file is present
- **WHEN** the lock is held and a socket file exists but does not accept connections
- **THEN** mbv SHALL refuse to start rather than report an attachable daemon

### Requirement: Attaching never displaces an existing client
The attach outcome SHALL mean joining an existing local daemon alongside any other attached
clients. It SHALL NOT disconnect, evict, or suspend a client that is already attached.

#### Scenario: Attaching while other clients are attached
- **WHEN** mbv resolves to attach and other clients are already attached to that daemon
- **THEN** all previously attached clients SHALL remain attached and usable

### Requirement: Clients take no lock
A client SHALL NOT acquire the lock, whether it is a client of a local daemon or of a daemon
reached through an explicit endpoint. Only the Player owner SHALL hold it.

#### Scenario: Several clients run at once
- **WHEN** several clients of the same local daemon are running
- **THEN** exactly one process — the daemon — SHALL hold the lock

#### Scenario: Explicit-endpoint client starts
- **WHEN** mbv starts with an explicit daemon endpoint
- **THEN** mbv SHALL NOT acquire the lock and SHALL NOT be affected by whether it is held

### Requirement: Refusal explains how to proceed
When mbv refuses to start because a bare-mode instance owns playback, the message SHALL identify
the owning process, SHALL state that the restriction exists because only one process can own the
audio device and the Emby session, and SHALL give the user both remedies: stop the running
instance, or use stay-alive so several terminals can run at once.

#### Scenario: Refusing while a bare instance owns playback
- **WHEN** mbv refuses to start because a bare-mode instance holds the lock
- **THEN** the message SHALL report the owning process ID when it can be determined
- **THEN** the message SHALL offer stopping that instance as one remedy
- **THEN** the message SHALL offer running mbv with stay-alive for multiple terminals as the other remedy

### Requirement: `mbv -q` stops the Player owner
`mbv -q` SHALL read the process ID from the lock file and request a graceful shutdown of the
Player owner, whether that is a bare-mode instance or a local daemon. It SHALL NOT require a
terminal UI to be attached and SHALL NOT affect clients other than by stopping the daemon they
are attached to.

#### Scenario: Stopping a local daemon
- **WHEN** the user runs `mbv -q` while a local daemon owns playback
- **THEN** the daemon SHALL shut down gracefully, persisting its state
- **THEN** attached clients SHALL be notified of the deliberate shutdown

#### Scenario: Stopping a bare instance
- **WHEN** the user runs `mbv -q` while a bare-mode instance owns playback
- **THEN** that instance SHALL shut down gracefully

#### Scenario: Nothing is running
- **WHEN** the user runs `mbv -q` and no Player owner exists
- **THEN** mbv SHALL report that no running instance was found and SHALL exit with a non-zero status
