# remote-queue-disconnect Specification

## Purpose

Governing how a client's queue-scope presentation returns to local after it disconnects from a
remote mbvd, so the queue-scope pill and remote queue management are never left lit for a
disconnected remote.

## Requirements

### Requirement: Disconnecting from a remote mbvd returns to the local unified queue

When a client that is attached to a local daemon (`home_is_local_daemon`) disconnects from a
remote mbvd — by user action, by unannounced connection loss, or by an announced daemon
shutdown — it SHALL return to the plain local-daemon presentation: the queue panel SHALL show
the local Owner queue, SHALL name the local Owner as its Player owner, and SHALL offer no
control of the remote queue. Disconnecting SHALL NOT empty the queue — the daemon's current
queue SHALL remain displayed. A user disconnect SHALL NOT stop or change the remote playback or
queue. The return-to-local behavior SHALL be driven by the disconnect itself, not only by the
user's disconnect key: a reader- or writer-detected connection loss SHALL trigger the same
presentation restore, and the client SHALL NOT remain presenting (or accepting commands for)
the stale adopted remote queue snapshot.

#### Scenario: Stay-alive client disconnects from a remote device

- **WHEN** a stay-alive client attached to a local daemon is connected to a remote mbvd and the
  user disconnects (`d`)
- **THEN** the client SHALL present the local daemon's queue
- **THEN** the client SHALL NOT present remote queue control
- **THEN** the queue panel SHALL name the local Owner
- **THEN** the daemon's current queue items SHALL remain visible
- **THEN** the remote mbvd SHALL continue its playback

#### Scenario: Remote device disconnects on its own

- **WHEN** a stay-alive client attached to a local daemon is connected to a remote mbvd and the
  remote device disconnects without a user action
- **THEN** the client SHALL present the local daemon's queue
- **THEN** the queue panel SHALL name the local Owner

#### Scenario: Daemon loss and announced shutdown

- **WHEN** a stay-alive client attached to a local daemon loses its remote mbvd connection through
  unannounced daemon loss or an announced daemon shutdown
- **THEN** the client SHALL present the local daemon's queue
- **THEN** the queue panel SHALL name the local Owner

#### Scenario: Write-side loss with no reader event

- **WHEN** the client detects the loss from a failed command write (no reader
  event has been processed)
- **THEN** the client SHALL perform the same return-to-local presentation as
  a reader-detected loss
- **THEN** the client SHALL NOT leave the adopted remote queue snapshot on
  screen

### Requirement: Reconnect does not create a remote queue for the local daemon

When a stay-alive client reconnects to its local daemon after a remote disconnect, the daemon's
reconnected queue SHALL be adopted as the local Owner queue that the queue panel shows. The
reconnect SHALL NOT present the queue of any remote Player owner and SHALL be equivalent to the
presentation the client established at startup for the same local-daemon baseline.

#### Scenario: Local daemon holds items at reconnect

- **WHEN** a stay-alive client disconnects from a remote mbvd and reconnects to its local daemon
  while that daemon holds a non-empty queue
- **THEN** the client SHALL show those items in the queue panel
- **THEN** the queue panel SHALL name the local Owner

#### Scenario: Local daemon holds no items at reconnect

- **WHEN** a stay-alive client disconnects from a remote mbvd and reconnects to its local daemon
  while that daemon holds an empty queue
- **THEN** the client SHALL show the empty local Owner queue
- **THEN** the queue panel SHALL name the local Owner
