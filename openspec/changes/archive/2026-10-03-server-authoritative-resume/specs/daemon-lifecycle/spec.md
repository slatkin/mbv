# Spec Delta

## MODIFIED Requirements

### Requirement: Coordinated shutdown durably preserves the authoritative queue

Before accepting coordinated shutdown, the daemon SHALL persist its authoritative queue,
cursor, source, and the current playback positions of non-audio feed entries to disk. Emby and
Audiobookshelf positions SHALL NOT be persisted; their servers hold them, and the final stop
report delivers the current one. It SHALL use daemon-owned state rather than a requesting
client's shadow. An empty queue at quit SHALL NOT erase an older non-empty snapshot; only an
explicit Clear Queue action may do that.

#### Scenario: Concurrent client changed the queue

- **WHEN** another client changes the queue before the daemon evaluates the shutdown request
- **THEN** the persisted snapshot SHALL contain the daemon's resulting authoritative queue
  and cursor rather than the requester's older shadow

#### Scenario: Requester is routed away from the local daemon

- **WHEN** the requesting TUI has not been receiving local-daemon queue broadcasts because
  it is currently routed elsewhere
- **THEN** coordinated shutdown SHALL still persist the local daemon's current queue

#### Scenario: Mid-track position is preserved

- **WHEN** the local daemon is playing a non-audio item when it evaluates the request
- **THEN** for a feed entry, the persisted snapshot SHALL include the latest valid playback
  position before the daemon accepts and stops
- **THEN** for an Emby or Audiobookshelf item, the persisted snapshot SHALL carry no position,
  and the item's next play SHALL resume from the position its server holds

#### Scenario: Durable write fails

- **WHEN** directory creation, serialization, temporary-file write, or atomic rename fails
- **THEN** the daemon SHALL reject shutdown and leave the previous snapshot intact
