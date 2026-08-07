# remote-queue-disconnect Specification

## Purpose

Governing how a client's queue-scope presentation returns to local after it disconnects from a
remote mbvd, so the queue-scope pill and remote queue management are never left lit for a
disconnected remote.

## Requirements

### Requirement: Disconnecting from a remote mbvd returns to the local unified queue

When a client that is attached to a local daemon (`home_is_local_daemon`) disconnects from a
remote mbvd, it SHALL return to the plain local-daemon presentation: a single unified queue owned
by the daemon, no separate remote queue tab, no remote queue control, and no queue-scope pill.
Disconnecting SHALL NOT empty the queue — the daemon's current queue SHALL remain displayed.

#### Scenario: Stay-alive client disconnects from a remote device

- **WHEN** a stay-alive client attached to a local daemon is connected to a remote mbvd and the
  user disconnects (`d`)
- **THEN** the client SHALL present the local daemon's unified queue
- **THEN** the client SHALL NOT present a remote queue tab or remote queue control
- **THEN** the queue-scope pill SHALL NOT be shown
- **THEN** the daemon's current queue items SHALL remain visible

#### Scenario: Remote device disconnects on its own

- **WHEN** a stay-alive client attached to a local daemon is connected to a remote mbvd and the
  remote device disconnects without a user action
- **THEN** the client SHALL present the local daemon's unified queue
- **THEN** the queue-scope pill SHALL NOT be shown

#### Scenario: Daemon loss and announced shutdown

- **WHEN** a stay-alive client attached to a local daemon loses its remote mbvd connection through
  unannounced daemon loss or an announced daemon shutdown
- **THEN** the client SHALL present the local daemon's unified queue
- **THEN** the queue-scope pill SHALL NOT be shown

### Requirement: Reconnect does not create a remote queue for the local daemon

When a stay-alive client reconnects to its local daemon after a remote disconnect, the daemon's
reconnected queue SHALL be adopted into the client's single unified queue. The reconnect SHALL NOT
create a remote queue tab, SHALL NOT set queue scope to `Remote`, and SHALL be equivalent to the
presentation the client established at startup for the same local-daemon baseline.

#### Scenario: Local daemon holds items at reconnect

- **WHEN** a stay-alive client disconnects from a remote mbvd and reconnects to its local daemon
  while that daemon holds a non-empty queue
- **THEN** the client SHALL show those items in its unified queue
- **THEN** the queue scope SHALL be `Local`

#### Scenario: Local daemon holds no items at reconnect

- **WHEN** a stay-alive client disconnects from a remote mbvd and reconnects to its local daemon
  while that daemon holds an empty queue
- **THEN** the client SHALL show its unified queue
- **THEN** the queue scope SHALL be `Local`
