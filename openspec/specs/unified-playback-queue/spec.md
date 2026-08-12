# unified-playback-queue Specification

## Purpose
Define one queue and playback-submission model shared by every QueueItem across composed editing, Player ownership, local and ctrl control, persistence, and mpv playback.
## Requirements
### Requirement: Each queue has one canonical ordered representation

Every Composed or Bound queue SHALL be represented by one ordered collection of queue slots containing `QueueItem` values. A component SHALL NOT maintain parallel item-kind collections whose synchronization is required to determine queue contents, order, length, or current slot.

#### Scenario: Mixed queue order

- **WHEN** a queue contains interleaved Emby items and Feed entries
- **THEN** every queue operation and view SHALL observe the same slot order
- **AND** no item kind SHALL be constrained to a prefix or tail

#### Scenario: Queue coordinates

- **WHEN** a queue reports its length or current position
- **THEN** both values SHALL use the canonical slot sequence regardless of item kind

### Requirement: Queue occurrences have stable slot identity

Each occurrence of a `QueueItem` SHALL have stable runtime slot identity independent of the item's Emby ID, Feed identity, or source URL. Operations on an existing queue occurrence SHALL target its slot identity or an index resolved against the same canonical queue.

#### Scenario: Duplicate content occurrences

- **WHEN** the same `QueueItem` is appended twice
- **THEN** the queue SHALL contain two independently addressable slots

#### Scenario: Play an existing slot

- **WHEN** the user plays an item already present in the queue
- **THEN** playback SHALL select that slot
- **AND** SHALL NOT append another occurrence as a side effect

### Requirement: Queue operations are item-kind agnostic

Append, replace, remove, move, clear, consume, and play-existing-slot operations SHALL accept any `QueueItem` and apply the same ordering and mutation semantics to Emby items and Feed entries.

#### Scenario: Append a Feed entry

- **WHEN** a Feed entry is appended to a queue containing Emby items
- **THEN** it SHALL be inserted using the same append operation as an Emby item
- **AND** subsequent ordinary mutations SHALL remain available

#### Scenario: Reorder a mixed queue

- **WHEN** a user moves either kind of item across the other kind
- **THEN** the resulting order SHALL be reflected by the queue owner, UI, persistence, and player playlist

#### Scenario: Consume one duplicate

- **WHEN** one of two slots containing the same content is consumed
- **THEN** only the consumed slot SHALL be removed

### Requirement: Completion and consumption address the canonical slot

Natural completion and explicit consumption SHALL identify the affected canonical queue slot and apply the queue's existing consume policy without branching by item kind. Content identity SHALL NOT be used to remove other occurrences.

#### Scenario: Feed slot completes naturally

- **WHEN** playback naturally completes a Feed entry whose slot is eligible for consumption
- **THEN** the owner SHALL consume that slot through the same slot-based queue operation used for an Emby item
- **AND** SHALL preserve any other slot containing the same Feed entry

#### Scenario: Slot is retained by policy

- **WHEN** playback completes a slot that the active consume policy retains
- **THEN** the slot SHALL remain in the canonical queue regardless of item kind

### Requirement: Playback submission uses one lifecycle-capable boundary

Local Players, stay-alive daemons, and directly controlled remote Player owners SHALL receive item-generic queue submissions through the same semantic boundary. The boundary SHALL be capable of starting a cold Player, reusing or replacing an active Player as required, and reporting submission failure through the existing user-visible error path.

#### Scenario: Cold local owner

- **WHEN** a valid QueueItem is submitted to a Player owner with no running playback process
- **THEN** the owner SHALL start playback for that item without requiring a pre-existing command channel

#### Scenario: Directly controlled remote owner

- **WHEN** a valid QueueItem is submitted through a compatible ctrl connection
- **THEN** the remote owner SHALL apply the same queue and lifecycle semantics as a local owner

#### Scenario: Submission cannot reach its owner

- **WHEN** the selected owner lacks the required capability or its command channel is unavailable
- **THEN** the submission SHALL fail visibly
- **AND** no component SHALL report the item as accepted into that owner's Bound queue

### Requirement: A Player owner binds only playable items

Owner admission SHALL evaluate every `QueueItem` through one canonical media-kind classification. An audio-only owner SHALL never bind a video item, regardless of whether it is an Emby item or Feed entry, and existing Composed-to-Bound fall-through rules SHALL apply by media kind rather than item variant.

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

### Requirement: The Player branches only at source and reporting boundaries

The playback pipeline SHALL treat all queue slots uniformly through admission, ordering, lifecycle, status, and mpv playlist management. Item-kind branching SHALL occur only to resolve the media source and to select progress-reporting behavior.

#### Scenario: Resolve an Emby item

- **WHEN** an Emby item reaches the play boundary
- **THEN** the Player owner SHALL resolve its authenticated Emby stream URL
- **AND** SHALL use Emby playback reporting

#### Scenario: Resolve a Feed entry

- **WHEN** a Feed entry reaches the play boundary
- **THEN** the Player owner SHALL resolve its enclosure URL or fallback link directly
- **AND** SHALL NOT report progress to Emby

### Requirement: Bound queue state synchronizes atomically

A ctrl peer supporting the unified queue capability SHALL receive the Player owner's canonical queue slots, current slot, and status as one coherent state model. Initial connection, mutation, playback changes, and reconnect SHALL use the same queue representation.

#### Scenario: Reconnect to a mixed Bound queue

- **WHEN** a compatible client reconnects to an owner holding a mixed queue
- **THEN** it SHALL reconstruct the same slots, order, and current slot without concatenating item-kind-specific collections

#### Scenario: Player reports a slot change

- **WHEN** mpv advances to any slot in a mixed queue
- **THEN** the owner and connected client SHALL report that slot using the canonical queue coordinates

### Requirement: Queue persistence round-trips every QueueItem

Persisted queue state SHALL serialize the canonical tagged `QueueItem` sequence and restore every supported item kind in the same order. Legacy untagged Emby-only state SHALL remain readable.

#### Scenario: Restore a mixed queue

- **WHEN** persisted state contains Emby items and Feed entries
- **THEN** restoration SHALL preserve each slot's item kind, ordering, and playback fields

#### Scenario: Restore legacy state

- **WHEN** persisted state contains the legacy untagged Emby-item shape
- **THEN** restoration SHALL interpret those values as Emby queue items without error

### Requirement: Unified ctrl behavior is capability-gated and additive

The ctrl protocol SHALL advertise an additive capability for unified queue state and operations without changing `CTRL_PROTOCOL_VERSION`. Capable peers SHALL use the unified representation. Compatibility handling for older peers SHALL be confined to the ctrl boundary and SHALL NOT create a second internal queue model.

#### Scenario: Both peers support unified queues

- **WHEN** both ctrl peers advertise the unified queue capability
- **THEN** all queue state and operations SHALL carry item-generic slots

#### Scenario: Capable peer mutates a mixed queue

- **WHEN** a capable peer appends or replaces queue contents containing both Emby items and Feed entries
- **THEN** the ctrl operation SHALL carry the tagged `QueueItem` values and their canonical order
- **AND** the owner SHALL apply the same operation without translating them into item-kind-specific collections

#### Scenario: Legacy peer connects

- **WHEN** a peer does not advertise the unified queue capability
- **THEN** it SHALL retain its existing representable behavior through a compatibility adapter
- **AND** the owner SHALL continue to hold one canonical internal queue

