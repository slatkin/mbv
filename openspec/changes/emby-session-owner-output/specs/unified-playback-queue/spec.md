# Spec Delta

## MODIFIED Requirements

### Requirement: Each queue has one canonical ordered representation

Every Composed or Bound queue SHALL be represented by one ordered collection of queue slots containing `QueueItem` values. A Player owner SHALL hold the only authoritative collection for its Bound queue. A Client MAY hold a replaceable snapshot of a Bound queue, and a Playback run MAY hold an mpv execution projection, but neither SHALL independently decide canonical order, active slot, revision, or queue mutation outcome. A component SHALL NOT maintain parallel item-kind collections whose synchronization is required to determine queue contents, order, length, or current slot. An mpv projection MAY contain the full playable sequence or only the active materialized file as required by source lifecycle, but that projection SHALL NOT become queue authority. A Cast receiver output or an Emby Session output SHALL hold only the active slot's item and SHALL NOT become queue authority. A populate-only queue replacement MAY update a Client's Composed queue without submitting it to an owner only when that Client does not present the queue as the Stay-alive process's queue. A Client presenting the Stay-alive queue SHALL submit a populate-only replacement to that owner at load time and SHALL display only owner-accepted contents, including the queue source. Until explicit play submits a Composed replacement to another playback target, the Client SHALL NOT present an unconfirmed slot from that replacement as the playing slot.

#### Scenario: Mixed queue order

- **WHEN** a queue contains interleaved Emby items, Feed entries, and Audiobookshelf podcast episodes
- **THEN** every queue operation and view SHALL observe the Player owner's canonical slot order
- **AND** no item kind SHALL be constrained to a prefix or tail

#### Scenario: Bound queue is viewed by a Client

- **WHEN** a Client displays or mutates a Bound queue
- **THEN** it SHALL use the Player owner's latest queue snapshot
- **AND** its local representation SHALL NOT become an independent queue authority

#### Scenario: Queue coordinates

- **WHEN** a queue reports its length or current position
- **THEN** both values SHALL use the canonical slot sequence regardless of how many files mpv has materialized

#### Scenario: Owner-driven active-file projection

- **WHEN** a Playback run uses owner-driven projection
- **THEN** mpv SHALL contain exactly the active materialized file while the canonical queue retains every slot
- **AND** mpv playlist position/count observations SHALL NOT resize, reorder, or reposition the canonical queue

#### Scenario: Eager mpv projection

- **WHEN** a Playback run materializes multiple canonical slots in mpv
- **THEN** the materialized entries SHALL remain an execution projection of owner-assigned slots
- **AND** mpv playlist mutation or position SHALL NOT independently redefine the Bound queue

#### Scenario: Cast receiver projection

- **WHEN** a Cast receiver is the owner's active playback output
- **THEN** the receiver SHALL hold only the active slot's item while the canonical queue retains every slot
- **AND** receiver status SHALL NOT resize, reorder, or reposition the canonical queue

#### Scenario: Emby Session projection

- **WHEN** an Emby Session is the owner's active playback output
- **THEN** the Session SHALL be given only the active slot's item while the canonical queue retains every slot
- **AND** Session observations SHALL NOT resize, reorder, or reposition the canonical queue

#### Scenario: Populate-only load on Stay-alive

- **WHEN** a Client installs a replacement queue without starting playback on the owner while presenting the Stay-alive process's queue
- **THEN** the Client SHALL send the replacement to that owner at load time
- **AND** its displayed queue and source SHALL follow the owner's accepted snapshot, not a private staged queue
