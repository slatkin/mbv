## ADDED Requirements

### Requirement: A later client adopts the live daemon Audiobookshelf queue and progress
A capable client attaching to a daemon that owns active Audiobookshelf playback SHALL adopt the daemon's live canonical queue, active slot, playback status, and last-acknowledged Audiobookshelf progress as authoritative, and SHALL NOT overwrite that daemon authority with a saved local or shared queue snapshot. Adopted Audiobookshelf slots SHALL carry provider-qualified identity in canonical slot order and SHALL reconcile browse state on adoption.

#### Scenario: Client attaches while the daemon holds an Audiobookshelf queue
- **WHEN** a capable client attaches to a daemon whose canonical queue contains one or more Audiobookshelf episodes
- **THEN** the client SHALL adopt the live queue, active slot, and status rather than its persisted snapshot

#### Scenario: A stale saved snapshot is present at attach
- **WHEN** the attaching client holds a saved local or shared queue snapshot that differs from the daemon's live Audiobookshelf queue
- **THEN** the daemon's live queue SHALL win and the client SHALL NOT push its snapshot as authoritative

#### Scenario: Incapable peer attaches to an Audiobookshelf-holding owner
- **WHEN** a peer that did not negotiate Audiobookshelf queue transport attaches to an owner holding Audiobookshelf slots
- **THEN** it SHALL receive no Audiobookshelf QueueItem variant and every previously supported queue behavior SHALL continue
