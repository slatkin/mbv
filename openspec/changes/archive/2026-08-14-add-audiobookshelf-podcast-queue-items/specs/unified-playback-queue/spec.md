## MODIFIED Requirements

### Requirement: Each queue has one canonical ordered representation

Every Composed or Bound queue SHALL be represented by one ordered collection of queue slots containing `QueueItem` values. A component SHALL NOT maintain parallel item-kind collections whose synchronization is required to determine queue contents, order, length, or current slot.

#### Scenario: Mixed queue order

- **WHEN** a queue contains interleaved Emby items, Feed entries, and Audiobookshelf podcast episodes
- **THEN** every queue operation and view SHALL observe the same slot order
- **AND** no item kind SHALL be constrained to a prefix or tail

#### Scenario: Queue coordinates

- **WHEN** a queue reports its length or current position
- **THEN** both values SHALL use the canonical slot sequence regardless of item kind

### Requirement: Queue occurrences have stable slot identity

Each occurrence of a `QueueItem` SHALL have stable runtime slot identity independent of its provider-qualified content identity or source URL. Operations on an existing queue occurrence SHALL target its slot identity or an index resolved against the same canonical queue.

#### Scenario: Duplicate content occurrences

- **WHEN** the same `QueueItem` is appended twice
- **THEN** the queue SHALL contain two independently addressable slots

#### Scenario: Play an existing slot

- **WHEN** the user plays an item already present in the queue
- **THEN** playback SHALL select that slot
- **AND** SHALL NOT append another occurrence as a side effect

### Requirement: Queue operations are item-kind agnostic

Append, replace, remove, move, clear, consume, and play-existing-slot operations SHALL accept every `QueueItem` kind and apply the same ordering and mutation semantics to Emby items, Feed entries, and Audiobookshelf podcast episodes.

#### Scenario: Append an Audiobookshelf episode

- **WHEN** an Audiobookshelf episode is appended to a Composed queue containing other item kinds
- **THEN** it SHALL be inserted using the same append operation as every other QueueItem
- **AND** subsequent ordinary mutations SHALL remain available

#### Scenario: Reorder a mixed queue

- **WHEN** a user moves any item kind across another item kind
- **THEN** the resulting canonical order SHALL be reflected by queue state, UI, and persistence

#### Scenario: Consume one duplicate

- **WHEN** one of two slots containing the same content is consumed
- **THEN** only the consumed slot SHALL be removed

### Requirement: Playback submission uses one lifecycle-capable boundary

Every Player owner SHALL receive item-generic queue submissions through the same semantic boundary. The boundary SHALL start a cold Player, reuse or replace an active Player as required, enforce the destination owner's item and Service capabilities before binding, and report submission failure through the existing user-visible error path.

#### Scenario: Cold local owner

- **WHEN** a valid QueueItem is submitted to a capable in-process Player owner with no running playback process
- **THEN** the owner SHALL start playback for that item without requiring a pre-existing command channel

#### Scenario: Compatible directly controlled owner

- **WHEN** a valid QueueItem is submitted through a ctrl connection whose owner advertises every capability required by that item
- **THEN** the remote owner SHALL apply the same queue and lifecycle semantics as a local owner

#### Scenario: Submission cannot reach a capable owner

- **WHEN** the selected owner lacks an item-kind or Service capability required by the submission, or its command channel is unavailable
- **THEN** the submission SHALL fail visibly
- **AND** no component SHALL report the item as accepted into that owner's Bound queue

### Requirement: A Player owner binds only playable items

Owner admission SHALL evaluate every `QueueItem` through canonical media-kind and required-Service classification. An owner SHALL never bind an item whose media kind or required Remote Service capability it cannot play. Existing Composed-to-Bound stripping and explicit-submission behavior SHALL apply at binding without constraining Composed queue editing.

#### Scenario: Audio Feed entry submitted to an audio-only owner

- **WHEN** a Feed entry classified as Audio is submitted to an audio-only owner
- **THEN** it SHALL be eligible for that owner's Bound queue under the same rules as an audio Emby item

#### Scenario: Video Feed entry submitted to an audio-only owner

- **WHEN** a Feed entry classified as Video is explicitly submitted while directly controlling an audio-only owner
- **THEN** it SHALL follow the same local fall-through behavior as a video Emby item
- **AND** SHALL NOT enter the audio-only owner's queue

#### Scenario: Feed MIME is absent

- **WHEN** a Feed entry has no usable enclosure MIME type
- **THEN** its queued snapshot SHALL retain the subscription's `FeedKind` as its canonical media kind

#### Scenario: Owner lacks an item's Remote Service capability

- **WHEN** a queue containing an item from a Remote Service binds to an owner without that Service capability
- **THEN** that item SHALL be unplayable and SHALL NOT enter the owner's Bound queue
- **AND** other playable items SHALL remain eligible

### Requirement: Queue persistence round-trips every QueueItem

Persisted queue state SHALL serialize the canonical tagged `QueueItem` sequence and restore every supported item kind in the same order. Persisted items SHALL exclude Service credentials and ephemeral playback state. Legacy untagged Emby-only state SHALL remain readable.

#### Scenario: Restore a mixed queue

- **WHEN** persisted state contains Emby items, Feed entries, and Audiobookshelf podcast episodes
- **THEN** restoration SHALL preserve each slot's item kind, provider-qualified content identity, ordering, and playback fields
- **THEN** owner admission SHALL run before restored slots enter a Bound queue

#### Scenario: Restore legacy state

- **WHEN** persisted state contains the legacy untagged Emby-item shape
- **THEN** restoration SHALL interpret those values as Emby queue items without error

#### Scenario: Inspect persisted Audiobookshelf item

- **WHEN** an Audiobookshelf podcast episode is persisted
- **THEN** its representation SHALL contain no Service credential, playback `sessionId`, resolved URL, or request header

### Requirement: Unified ctrl behavior is capability-gated and additive

The ctrl protocol SHALL advertise additive capabilities for every QueueItem kind transported through unified queue state and operations without changing `CTRL_PROTOCOL_VERSION`. A QueueItem kind without a negotiated transport capability SHALL remain ineligible for that peer's Bound queue. Compatibility handling SHALL remain confined to the ctrl boundary and SHALL NOT create a second internal queue model.

#### Scenario: Both peers support an item's queue transport

- **WHEN** both ctrl peers advertise the capabilities required by every submitted QueueItem
- **THEN** queue state and operations SHALL carry the tagged QueueItem values and their canonical order

#### Scenario: Audiobookshelf transport is not negotiated

- **WHEN** a queue contains an Audiobookshelf episode and no Audiobookshelf transport capability is negotiated
- **THEN** that episode SHALL NOT be submitted to or represented as Bound by that owner
- **AND** no Audiobookshelf credential SHALL cross ctrl

#### Scenario: Legacy peer connects

- **WHEN** a peer does not advertise the unified queue capability
- **THEN** it SHALL retain its existing representable behavior through a compatibility adapter
- **AND** the owner SHALL continue to hold one canonical internal queue
