## ADDED Requirements

### Requirement: Audiobookshelf queue transport is separately capability-gated
Audiobookshelf podcast items SHALL cross the unified ctrl queue boundary only when both peers support the additive Audiobookshelf queue capability. The capability SHALL describe static protocol support and SHALL NOT make a daemon owner eligible to bind or play the item.

#### Scenario: Both peers support Audiobookshelf queue transport
- **WHEN** a unified-queue operation or snapshot contains an Audiobookshelf podcast episode and both peers advertise Audiobookshelf queue support
- **THEN** the wire representation SHALL carry the provider-qualified episode in canonical slot order

#### Scenario: Audiobookshelf queue capability is absent
- **WHEN** either peer lacks Audiobookshelf queue support
- **THEN** the episode SHALL NOT be sent to or represented as Bound by that peer
- **THEN** every previously supported QueueItem kind SHALL retain existing behavior

### Requirement: Every queue transport direction applies compatibility gating
Audiobookshelf capability checks SHALL apply to incoming unified queue commands, initial owner snapshots, later owner broadcasts, and reconnect adoption. A compatible internal queue SHALL remain canonical and SHALL NOT be replaced by an item-kind-specific queue model.

#### Scenario: Older unified peer connects to an owner holding an episode
- **WHEN** a peer supports unified queues but not Audiobookshelf queue transport
- **THEN** it SHALL receive no Audiobookshelf QueueItem variant
- **THEN** the owner SHALL retain one canonical internal queue

#### Scenario: Older peer submits an unsupported episode
- **WHEN** a peer without negotiated Audiobookshelf queue support submits an Audiobookshelf QueueItem
- **THEN** the owner SHALL reject the unsupported operation without mutating its Bound queue

### Requirement: Audiobookshelf queue transport carries no lifecycle secrets
Audiobookshelf unified queue commands and snapshots SHALL contain provider-qualified media identity and ordinary queue metadata but SHALL NOT contain an API key, Authorization header, resolved source URL, or playback `sessionId`.

#### Scenario: Capable client receives an episode slot
- **WHEN** a capable client receives queue state containing an Audiobookshelf episode
- **THEN** it SHALL receive stable episode and slot identity without owner credentials or ephemeral playback state

### Requirement: Transport does not enable daemon owner admission
During this change every daemon Player owner SHALL continue treating Audiobookshelf podcast episodes as unplayable even when queue transport is negotiated. Transported values MAY be decoded and compatibility-filtered but SHALL NOT enter a daemon Bound queue or start playback.

#### Scenario: Capable peers negotiate transport before activation
- **WHEN** a client submits an Audiobookshelf episode to a daemon owner after this change
- **THEN** the owner SHALL visibly reject admission without source preparation or Bound queue mutation
