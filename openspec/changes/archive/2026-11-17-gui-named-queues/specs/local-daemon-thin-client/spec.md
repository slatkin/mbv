## MODIFIED Requirements

### Requirement: A live daemon queue is never overwritten by the saved queue snapshot

The local daemon SHALL hold every authoritative Named queue with its name, queue source, slot identities, and start slot, and which queue is current. It SHALL persist them on accepted queue-changing operations, on create, rename, and close, and on graceful shutdown, and reload them at startup. A single-queue state file from an earlier version SHALL load as one current queue named `Default`. A Client attaching to the local daemon SHALL adopt that owner's live queues and SHALL NOT replace them with a saved Client snapshot. An empty queue SHALL persist as empty: a persisted empty queue SHALL NOT be replaced by an older saved snapshot. No Client SHALL persist, restore, or seed a queue: queue persistence belongs only to the Player owner.

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

#### Scenario: Restart restores every named queue

- **GIVEN** the local daemon holds `Default`, current and stopped with its third slot as its start slot, and the resting `Kids`
- **WHEN** the local daemon restarts
- **THEN** it SHALL reload both queues with their names, sources, slot identities, and start slots
- **AND** `Default` SHALL be current

#### Scenario: An earlier single-queue state loads

- **GIVEN** the state directory holds only the single-queue state file of an earlier version
- **WHEN** the local daemon starts
- **THEN** it SHALL hold one current Named queue named `Default` with that queue's slots and source

#### Scenario: Quitting with stay-alive off preserves the queue

- **WHEN** stay-alive is disabled, the queue is non-empty, and the user quits the client
- **WHEN** the user starts mbv again
- **THEN** the new local daemon SHALL reload that queue and its source
- **THEN** the Client SHALL display it without restoring any Client-side snapshot

### Requirement: Every local Client shows the owner's accepted queue

A Client attached to the local daemon SHALL use that owner's Named queues, queue sources, and playback status as its displayed Local queue state in every state, including while an attached Session or cast receiver is the playback target. A TUI Client SHALL display the current queue; a GUI Client SHALL display the Named queue its Queue chooser shows. An attached Session or cast is a playback target, not a queue owner; a Client has no separate Local queue of its own. Loading a playlist, editing the queue, Save As source changes, and clearing the queue SHALL be visible to other attached Clients through owner-accepted state; none SHALL create a private replacement that hides a different playing owner queue. A Client SHALL NOT claim a load succeeded until the owner has accepted it. An unavailable owner SHALL leave the last confirmed state visible, indicate disconnection or failure, and reconcile from the owner on reconnect instead of later submitting a private replacement.

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

#### Scenario: The TUI and the GUI show different queues

- **GIVEN** a TUI Client and a GUI Client are attached, `Default` is current, and the GUI Client shows `Kids`
- **WHEN** both Clients paint their queue
- **THEN** the TUI Client shows `Default` and the GUI Client shows `Kids`
