# Spec Delta

## MODIFIED Requirements

### Requirement: A live daemon queue is never overwritten by the saved queue snapshot

The Stay-alive process SHALL hold the authoritative queue and queue source, persist them on accepted queue-changing operations and on graceful shutdown, and reload them at startup. A Client attaching to the Stay-alive process SHALL adopt that owner's live queue and SHALL NOT replace it with a saved Client snapshot. An empty queue SHALL persist as empty: a persisted empty queue SHALL NOT be replaced by an older saved snapshot. A Client SHALL NOT seed the Stay-alive queue from its own saved queue snapshot and SHALL NOT persist the Stay-alive queue; Client-side queue persistence applies only to queues the Client owns (Bare mode).

#### Scenario: Attaching to a daemon that is playing

- **WHEN** a Client attaches to the Stay-alive process whose queue is non-empty
- **THEN** the Client SHALL display that owner's queue and cursor
- **THEN** the Client SHALL NOT overwrite that queue with the contents of the saved queue snapshot

#### Scenario: Attaching to an idle daemon

- **WHEN** a Client attaches to the Stay-alive process whose queue is empty and a saved queue snapshot exists
- **THEN** the Client SHALL display an empty queue
- **THEN** the Client SHALL NOT restore the saved queue snapshot
- **THEN** the Stay-alive process SHALL remain empty

#### Scenario: Restart restores the owner's queue

- **WHEN** the Stay-alive process restarts after holding a queue
- **THEN** it SHALL reload that queue and its source before serving Clients
- **AND** attached Clients SHALL display the reloaded queue

## ADDED Requirements

### Requirement: Every Stay-alive Client shows the owner's accepted queue

A Client attached to the Stay-alive process SHALL use that owner's Bound queue, queue source, and playback status as its displayed Local queue state in every state, including while an attached Session or cast receiver is the playback target. An attached Session or cast is a playback target, not a queue owner; a Client attached to the Stay-alive process has no separate Local queue of its own. Loading a playlist without starting playback, editing the queue, Save As source changes, and clearing the queue SHALL be visible to other attached Clients through owner-accepted state; none SHALL create a private replacement that hides a different playing owner queue. A Client SHALL NOT claim a load succeeded until the owner has accepted it. An unavailable owner SHALL leave the last confirmed state visible, indicate disconnection or failure, and reconcile from the owner on reconnect instead of later submitting a private replacement.

#### Scenario: Two Clients see a load

- **WHEN** Client A loads a playlist into Stay-alive without starting it while Client B is attached
- **THEN** both Clients SHALL show the owner's new playlist contents and source with no now-playing row
- **AND** neither Client SHALL retain the previous owner queue as its active Local queue

#### Scenario: Concurrent Clients replace the queue

- **WHEN** two attached Clients send different accepted loads in sequence
- **THEN** both SHALL show the queue and source from the owner's latest accepted replacement
- **AND** neither SHALL restore its own earlier queue because a local generation happens to match

#### Scenario: Load while disconnected or rejected

- **WHEN** a Client attempts to load a playlist but cannot reach the owner, or the owner rejects it
- **THEN** the Client SHALL report failure and SHALL NOT show the attempted playlist as its Local queue
- **AND** reconnect SHALL adopt the owner's current queue rather than retry the unaccepted load automatically

#### Scenario: Save the currently displayed queue under a new playlist name

- **WHEN** a Client's Save As operation succeeds for the Stay-alive queue it still displays
- **THEN** the updated Playlist source SHALL be reflected by the owner and all attached Clients
- **AND** queue contents and playback state SHALL remain unchanged

#### Scenario: A stay-alive Client watches a session or casts

- **WHEN** a Client attached to the Stay-alive process is watching an attached Emby Session or casting to a receiver
- **THEN** its displayed Local queue SHALL remain the owner's Bound queue
- **AND** queue edits SHALL still be accepted by that owner rather than becoming a private queue
