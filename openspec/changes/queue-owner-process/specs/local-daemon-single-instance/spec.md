## MODIFIED Requirements

### Requirement: Resolution probes the control socket
When the lock is already held, mbv SHALL decide what to do by attempting to connect to the user's
control socket. Socket-file existence SHALL NOT be treated as evidence of a live daemon; only a
successful connection SHALL count. A connection the daemon refuses as an exclusive owner SHALL be
treated as a refusal to start, not as an attach failure.

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

## ADDED Requirements

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

## REMOVED Requirements

### Requirement: The lock identifies the Player owner
**Reason**: It had a Bare-mode holder. The local daemon is now the only holder.
**Migration**: Replaced by "The local daemon holds the owner lock".

### Requirement: Refusal explains how to proceed
**Reason**: The refusal was triggered by a Bare instance holding the lock. It is now triggered by an exclusive local daemon that already has a client.
**Migration**: Replaced by "Refusing a second terminal explains how to proceed", which carries the same message content.

### Requirement: `mbv -q` stops the Player owner
**Reason**: It had a Bare-instance scenario. `mbv -q` now always addresses the local daemon.
**Migration**: Replaced by "`mbv -q` stops the local daemon".
