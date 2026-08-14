## Purpose

Defines provider-native queue identity, persistence, Service ownership, and admission constraints for downloaded Audiobookshelf podcast episodes before any Player owner is enabled to play them.

## ADDED Requirements

### Requirement: Downloaded podcast episodes are provider-qualified queue items
mbv SHALL represent a queued Audiobookshelf podcast episode with the Audiobookshelf Service kind, `libraryItemId`, and `episodeId`. Each queued snapshot SHALL retain the metadata required for queue presentation and progress while excluding the Service credential, playback `sessionId`, resolved media URL, and request headers.

#### Scenario: Episode enters a Composed queue
- **WHEN** a downloaded Audiobookshelf podcast episode is staged in a Composed queue
- **THEN** its QueueItem SHALL retain its provider-qualified episode identity and display metadata
- **THEN** no Audiobookshelf credential or ephemeral playback state SHALL enter the queue

#### Scenario: Same episode occurs twice
- **WHEN** the same Audiobookshelf episode is added more than once
- **THEN** both occurrences SHALL share the same provider-qualified content identity
- **THEN** each occurrence SHALL retain independent queue-slot identity

### Requirement: Queueing support does not enable playback
This capability SHALL make Audiobookshelf episodes representable and editable without making them playable by any current Player owner. A Composed queue SHALL retain them, while every Bound owner SHALL treat them as unplayable until the later playback capability explicitly enables an eligible owner.

#### Scenario: User edits a Composed queue
- **WHEN** the client appends, removes, or reorders an Audiobookshelf episode in a Composed queue
- **THEN** the ordinary canonical queue operation SHALL succeed without opening a playback session

#### Scenario: Queue binds before playback is enabled
- **WHEN** a Composed or restored queue containing Audiobookshelf episodes binds to any Player owner after this change alone
- **THEN** Audiobookshelf episodes SHALL NOT enter that owner's Bound queue
- **THEN** other playable items SHALL remain eligible

#### Scenario: Explicit submission occurs before playback is enabled
- **WHEN** an Audiobookshelf episode is explicitly submitted after this change alone
- **THEN** the submission SHALL fail visibly without mutating a Bound queue or falling through to another owner

### Requirement: Audiobookshelf queue state follows the Service lifecycle
Credential rejection SHALL preserve repairable Composed and persisted Audiobookshelf queue snapshots while making them ineligible for every Bound queue. Confirmed Service replacement or removal SHALL purge Audiobookshelf items from Composed, Bound, and persisted queue state without changing Emby or Feed items.

#### Scenario: Persisted credential is rejected
- **WHEN** Audiobookshelf explicitly rejects its persisted Service credential
- **THEN** repairable Composed and persisted Audiobookshelf queue snapshots SHALL remain available
- **THEN** no Audiobookshelf item SHALL remain eligible for a Bound queue until repair succeeds and playback is enabled

#### Scenario: Service is replaced or removed
- **WHEN** the user confirms Audiobookshelf Service replacement or removal
- **THEN** every Audiobookshelf item SHALL be removed from Composed, Bound, and persisted queue state
- **THEN** Emby and Feed queue items SHALL remain unchanged

### Requirement: Audiobookshelf queue representation does not add transport
Audiobookshelf QueueItems SHALL NOT be transmitted to or represented as Bound by a ctrl peer in this capability. No Audiobookshelf Service credential SHALL cross ctrl.

#### Scenario: Queue targets a ctrl owner
- **WHEN** a queue containing Audiobookshelf episodes is prepared for a Local daemon or remote Player owner
- **THEN** Audiobookshelf episodes SHALL be ineligible for that owner's Bound queue and ctrl state
- **THEN** no new Audiobookshelf ctrl capability SHALL be advertised
