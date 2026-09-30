# local-daemon-single-instance Specification

## Purpose
TBD - created by archiving change retire-pty-relay-for-local-daemon-stay-alive. Update Purpose after archive.

## Requirements

### Requirement: Resolution probes the control socket
When the lock is already held, mbv SHALL decide what to do by attempting to connect to the user's
control socket. Socket-file existence SHALL NOT be treated as evidence of a live daemon; only a
successful connection SHALL count. A successful connection leads to an attach attempt; the daemon
may still refuse that attach, as an exclusive owner (mbv SHALL refuse to start) or as shutting down
(mbv SHALL retry resolution for a bounded period, per the `daemon-lifecycle` capability).

#### Scenario: Lock is free
- **WHEN** the lock can be acquired
- **THEN** mbv SHALL proceed as a fresh start

#### Scenario: Lock is held and the control socket accepts a connection
- **WHEN** the lock is held and the control socket accepts a connection that the daemon admits
- **THEN** mbv SHALL attach to the local daemon as a client

#### Scenario: Lock is held by an exclusive owner
- **WHEN** the lock is held and the daemon refuses the connection because Stay Alive is off and a client is already attached
- **THEN** mbv SHALL refuse to start

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

### Requirement: The local daemon holds the owner lock
mbv SHALL use an advisory lock file in the user's runtime directory to identify the Player owner.
The local daemon SHALL hold it whatever `stay_alive` is set to; no terminal UI SHALL hold it. The
lock SHALL be held for the Player owner's whole lifetime, and the Player owner SHALL write its
process ID into the lock file after acquiring it.

#### Scenario: The local daemon acquires the lock
- **WHEN** a local daemon starts
- **THEN** the daemon SHALL acquire the lock and write its process ID into the lock file
- **THEN** the terminal that started it SHALL NOT hold the lock

#### Scenario: The Player owner dies without cleanup
- **WHEN** the Player owner is killed without running any cleanup
- **THEN** the lock SHALL be released by the operating system
- **THEN** the next mbv launch SHALL be treated as a fresh start

### Requirement: Refusing a second terminal explains how to proceed
When mbv refuses to start because an exclusive local daemon already has a client, the message
SHALL identify the owning process, SHALL state that the restriction exists because only one
terminal may use playback while stay-alive is off, and SHALL give the user both remedies: close or
stop the running instance, or use stay-alive so several terminals can run at once.

#### Scenario: Refusing while an exclusive owner has a client
- **WHEN** mbv refuses to start because the local daemon refused it as an exclusive owner
- **THEN** the message SHALL report the owning process ID when it can be determined
- **THEN** the message SHALL offer stopping that instance as one remedy
- **THEN** the message SHALL offer running mbv with stay-alive for multiple terminals as the other remedy

### Requirement: `mbv -q` stops the local daemon
`mbv -q` SHALL read the process ID from the lock file and request a graceful shutdown of the
local daemon. It SHALL NOT require a terminal UI to be attached and SHALL NOT affect clients other
than by stopping the daemon they are attached to.

#### Scenario: Stopping a local daemon
- **WHEN** the user runs `mbv -q` while a local daemon owns playback
- **THEN** the daemon SHALL shut down gracefully, persisting its state
- **THEN** attached clients SHALL be notified of the deliberate shutdown

#### Scenario: Nothing is running
- **WHEN** the user runs `mbv -q` and no Player owner exists
- **THEN** mbv SHALL report that no running instance was found and SHALL exit with a non-zero status

### Requirement: A local Client never attaches to an Owner from a different build
A Client attaching to the local Owner process SHALL compare the Owner's application version,
as reported in the Owner's hello, with its own. The local Owner is the same binary as the
Client, so the application version is the only comparison; ctrl protocol compatibility is
decided by the `ctrl-protocol` capability alone and this requirement SHALL NOT add, remove, or
alter it. When the application versions differ, the Client SHALL NOT attach: it SHALL stop
before sending its own hello, so that no control credential is transmitted to the Owner, no
client is admitted, and no queue or playback state is received. Instead it SHALL ask the user,
before any terminal UI starts, whether to stop the Owner and relaunch it from this binary. This
rule SHALL apply only to the local Owner process; a Client launched against an explicit
`unix://` or `tcp://` endpoint SHALL NOT apply it.

The prompt SHALL show the Owner's version and this terminal's version, SHALL state that
restarting stops playback and closes any other mbv terminals attached to the Owner, and SHALL
offer exactly two choices: restart, or quit. There SHALL be no choice to continue against the
mismatched Owner.

#### Scenario: Owner and Client are the same build
- **WHEN** the Owner's application version equals the Client's
- **THEN** the Client SHALL attach as it would without this requirement
- **THEN** no prompt SHALL be shown

#### Scenario: Application versions differ
- **WHEN** a Client attaches to a running Owner whose application version differs from its own
- **THEN** the Client SHALL NOT send its hello or control credential to that Owner
- **THEN** the terminal SHALL show the mismatch prompt with both versions before any UI starts

#### Scenario: Only the ctrl protocol version differs
- **WHEN** the application versions are equal but the Owner reports a different ctrl protocol version
- **THEN** the Client SHALL NOT show the mismatch prompt
- **THEN** the refusal SHALL be decided by the `ctrl-protocol` capability exactly as without this requirement

#### Scenario: User chooses restart
- **WHEN** the user chooses restart at the mismatch prompt
- **THEN** mbv SHALL ask the Owner to stop the same way `mbv -q` does, so the Owner persists its state and exits
- **THEN** mbv SHALL start a fresh Owner from this binary once the old one has released the lock, and attach to it
- **THEN** mbv SHALL NOT show the prompt again for the Owner that is stopping

#### Scenario: The old Owner does not exit in time
- **WHEN** the user chose restart and the old Owner is still running after the bounded wait
- **THEN** mbv SHALL exit with a non-zero status and a message that the Owner is still shutting down, naming `mbv -q`

#### Scenario: User chooses quit
- **WHEN** the user chooses quit, presses Enter alone, enters any other input, or input reaches end of file
- **THEN** mbv SHALL exit with a non-zero status without signalling the Owner
- **THEN** the message SHALL name `mbv -q` and relaunching as the way to proceed

#### Scenario: Standard input is not a terminal
- **WHEN** a mismatch is detected and standard input is not a terminal
- **THEN** mbv SHALL NOT prompt, SHALL NOT signal the Owner, and SHALL exit with a non-zero status
- **THEN** the message SHALL state both versions and name `mbv -q`

#### Scenario: Explicit endpoint reports a different version
- **WHEN** a Client launched with an explicit `unix://` or `tcp://` endpoint receives a hello from a different application version
- **THEN** the Client SHALL NOT show the mismatch prompt and SHALL NOT signal any process
- **THEN** ctrl protocol compatibility SHALL be decided by the `ctrl-protocol` capability alone
