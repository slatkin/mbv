# daemon-disconnect-handling Specification

## Purpose
TBD - created by archiving change retire-pty-relay-for-local-daemon-stay-alive. Update Purpose after archive.

## Requirements

### Requirement: A deliberate shutdown is announced before the socket closes
When a daemon shuts down in response to an explicit request, it SHALL broadcast a disconnect event
carrying a shutdown reason to every connected client before closing their connections.

#### Scenario: Daemon is stopped by `mbv -q`
- **WHEN** the daemon receives an explicit shutdown request from `mbv -q`
- **THEN** the daemon SHALL broadcast a disconnect event with the shutdown reason to all connected clients
- **THEN** the daemon SHALL then close the connections and exit

#### Scenario: Daemon is stopped from the tray
- **WHEN** the user selects quit from the tray
- **THEN** the daemon SHALL broadcast the same shutdown disconnect event before exiting

### Requirement: Clients exit cleanly on an announced shutdown
A client that receives the shutdown disconnect reason SHALL treat the following connection close as
expected: it SHALL print a single line explaining that the daemon was stopped, restore the
terminal, and exit. It SHALL NOT show a recovery prompt and SHALL NOT report an error.

#### Scenario: Client receives the shutdown reason
- **WHEN** a client receives a disconnect event with the shutdown reason
- **THEN** the client SHALL exit cleanly with a single explanatory message
- **THEN** the client SHALL NOT display the connection-lost recovery dialog

#### Scenario: Several clients are attached
- **WHEN** the daemon is stopped explicitly while several clients are attached
- **THEN** every attached client SHALL exit cleanly in the same way

### Requirement: An unannounced connection loss raises a recovery dialog
When a client's connection to a local daemon closes without a preceding shutdown disconnect event,
the client SHALL NOT exit. It SHALL display a dialog that blocks all other input until the user
chooses an option.

#### Scenario: The daemon dies without announcing
- **WHEN** a client's connection closes and no shutdown disconnect event was received
- **THEN** the client SHALL display the recovery dialog
- **THEN** the client SHALL NOT exit on its own and SHALL NOT act on other key presses while the dialog is open

#### Scenario: Dialog contents
- **WHEN** the recovery dialog is displayed
- **THEN** it SHALL offer restarting the daemon and resuming what was playing
- **THEN** it SHALL offer restarting the daemon without resuming
- **THEN** it SHALL offer quitting the client
- **THEN** it SHALL show diagnostics including the title of the last item known to be playing and the path to the daemon's log

### Requirement: Recovery options behave distinctly
Each recovery option SHALL have a distinct, predictable effect.

#### Scenario: Restart and resume
- **WHEN** the user chooses to restart and resume
- **THEN** the client SHALL ensure a local daemon exists again and attach to it
- **THEN** the queue and position SHALL be restored from the saved queue snapshot

#### Scenario: Restart without resuming
- **WHEN** the user chooses to restart without resuming
- **THEN** the client SHALL ensure a local daemon exists again and attach to it
- **THEN** the client SHALL NOT start playback of the item that was playing when the connection was lost
- **THEN** the saved queue snapshot SHALL NOT be replayed automatically

#### Scenario: Repeated crash on the same item
- **WHEN** restarting and resuming causes the daemon to die again on the same item
- **THEN** restarting without resuming SHALL give the user a working client with playback stopped

#### Scenario: Quit
- **WHEN** the user chooses to quit
- **THEN** the client SHALL restore the terminal and exit without starting a daemon

### Requirement: Restart is arbitrated by the existing lock
When several clients attempt to restart the daemon at the same time, the Player-owner lock SHALL
arbitrate. Exactly one SHALL start the daemon; the others SHALL find the control socket connectable
and attach. No additional coordination mechanism SHALL be introduced.

#### Scenario: Several clients restart at once
- **WHEN** several clients choose a restart option at the same time
- **THEN** exactly one local daemon SHALL be started
- **THEN** every one of those clients SHALL end up attached to that one daemon

### Requirement: Remote-daemon connection loss is detected and surfaced

A client of a daemon on another machine SHALL detect connection loss from
either direction: the reader thread observing a closed stream (as today) and
the writer thread observing a failed write. Writer-side loss SHALL set the
same disconnected indicator reader-side loss sets and SHALL be logged. A
client SHALL NOT accept queue-edit or playback commands for a connection it
knows is disconnected: queue edits SHALL roll back, and playback requests
SHALL surface a connection-lost warning instead of a request acknowledgement.
The recovery dialog SHALL still apply only to clients of a local daemon,
which is the only daemon a client is able to restart.

#### Scenario: The daemon closes the connection while the client is idle

- **WHEN** the daemon (or the network) closes the connection and the client's
  reader thread observes EOF
- **THEN** the client SHALL mark the connection disconnected and surface the
  loss to the user
- **THEN** the client SHALL NOT offer to restart that daemon

#### Scenario: The write side fails while the reader is still blocked

- **WHEN** a client command write fails because the connection is dead
- **THEN** the client SHALL mark the connection disconnected and log the
  failure
- **THEN** no further command SHALL be queued to that connection

#### Scenario: Queue edit on a dead connection

- **WHEN** the user enqueues or edits the queue while the remote connection
  is known disconnected
- **THEN** the queue SHALL roll back to its previous contents
- **THEN** the client SHALL show a connection-lost toast instead of silently
  doing nothing

#### Scenario: Playback request on a dead connection

- **WHEN** the user requests playback while the remote connection is known
  disconnected
- **THEN** the client SHALL show a connection-lost warning
- **THEN** the client SHALL NOT show the ordinary "Requesting playback…"
  acknowledgement

#### Scenario: Emby remote takes authority

- **WHEN** the daemon sends a disconnect event because an Emby remote took authority
- **THEN** the client SHALL treat it as a notification, SHALL remain connected, and SHALL NOT show the recovery dialog
