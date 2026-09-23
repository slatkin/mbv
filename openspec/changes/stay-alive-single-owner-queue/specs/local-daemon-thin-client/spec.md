# Spec Delta

## ADDED Requirements

### Requirement: Every Stay-alive Client shows the owner's accepted queue

A Client attached to the Stay-alive process SHALL use that owner's Bound queue, queue source, and playback status as its displayed Local queue state. Loading a playlist without starting playback, editing the queue, Save As source changes, and clearing the queue SHALL be visible to other attached Clients through owner-accepted state; none SHALL create a private replacement that hides a different playing owner queue. A Client SHALL NOT claim a load succeeded until the owner has accepted it. An unavailable owner SHALL leave the last confirmed state visible, indicate disconnection or failure, and reconcile from the owner on reconnect instead of later submitting a private replacement.

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
