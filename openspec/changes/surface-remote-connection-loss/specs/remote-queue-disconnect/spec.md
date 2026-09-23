# remote-queue-disconnect Modifications

## MODIFIED Requirements

### Requirement: Disconnecting from a remote mbvd returns to the local unified queue

When a client that is attached to a local daemon (`home_is_local_daemon`) disconnects from a
remote mbvd — by user action, by unannounced connection loss, or by an announced daemon
shutdown — it SHALL return to the plain local-daemon presentation: a single unified queue owned
by the daemon, no separate remote queue tab, no remote queue control, and no queue-scope pill.
Disconnecting SHALL NOT empty the queue — the daemon's current queue SHALL remain displayed.
The return-to-local behavior SHALL be driven by the disconnect itself, not only by the user's
disconnect key: a reader- or writer-detected connection loss SHALL trigger the same
presentation restore, and the client SHALL NOT remain presenting (or accepting commands for)
the stale adopted remote queue snapshot.

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

#### Scenario: Write-side loss with no reader event

- **WHEN** the client detects the loss from a failed command write (no reader
  event has been processed)
- **THEN** the client SHALL perform the same return-to-local presentation as
  a reader-detected loss
- **THEN** the client SHALL NOT leave the adopted remote queue snapshot on
  screen
