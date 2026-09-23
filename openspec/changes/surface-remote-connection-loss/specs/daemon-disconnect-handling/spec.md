# daemon-disconnect-handling Modifications

## MODIFIED Requirements

### Requirement: Remote-daemon disconnects are unaffected

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

#### Scenario: A remote daemon connection is lost

- **WHEN** a client of a daemon on another machine loses its connection
- **THEN** the existing disconnect behavior for remote daemons SHALL apply
- **THEN** the client SHALL NOT offer to restart that daemon

#### Scenario: Emby remote takes authority

- **WHEN** the daemon sends a disconnect event because an Emby remote took authority
- **THEN** the client SHALL treat it as a notification, SHALL remain connected, and SHALL NOT show the recovery dialog
