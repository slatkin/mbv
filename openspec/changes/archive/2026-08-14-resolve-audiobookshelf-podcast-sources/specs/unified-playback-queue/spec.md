## MODIFIED Requirements

### Requirement: Each queue has one canonical ordered representation

Every Composed or Bound queue SHALL be represented by one ordered collection of queue slots containing `QueueItem` values. A component SHALL NOT maintain parallel item-kind collections whose synchronization is required to determine queue contents, order, length, or current slot. An mpv projection MAY materialize only the active slot when a source requires a server lifecycle, but that projection SHALL NOT become queue authority.

#### Scenario: Mixed queue order

- **WHEN** a queue contains interleaved Emby items, Feed entries, and Audiobookshelf podcast episodes
- **THEN** every queue operation and view SHALL observe the same canonical slot order
- **AND** no item kind SHALL be constrained to a prefix or tail

#### Scenario: Queue coordinates

- **WHEN** a queue reports its length or current position
- **THEN** both values SHALL use the canonical slot sequence regardless of how many files mpv has materialized

#### Scenario: Owner-driven active-file projection

- **WHEN** a Playback run uses owner-driven projection
- **THEN** mpv SHALL contain exactly the active materialized file while the canonical queue retains every slot
- **AND** mpv playlist position/count observations SHALL NOT resize, reorder, or reposition the canonical queue

### Requirement: Queue operations are item-kind agnostic

Append, replace, remove, move, clear, consume, and play-existing-slot operations SHALL accept every `QueueItem` kind and apply the same canonical ordering and mutation semantics. In owner-driven projection, inactive mutations SHALL update the canonical queue without requiring an inactive mpv playlist entry.

#### Scenario: Append an inactive item

- **WHEN** an item is appended after the active slot during owner-driven projection
- **THEN** it SHALL appear in canonical order without being prepared or inserted into mpv

#### Scenario: Reorder a mixed queue

- **WHEN** a user moves an inactive item across another item during owner-driven projection
- **THEN** canonical queue state, UI, and persistence SHALL reflect the new order
- **AND** mpv SHALL continue representing only the active slot

#### Scenario: Active slot is selected or removed

- **WHEN** an explicit selection, removal, consume, skip, or natural completion changes the active canonical slot
- **THEN** the prior materialized file SHALL be finalized as required and replaced by the newly active slot

#### Scenario: Consume one duplicate

- **WHEN** one of two slots containing the same content is consumed
- **THEN** only the consumed slot SHALL be removed
