# Spec Delta

## MODIFIED Requirements

### Requirement: Each queue has one canonical ordered representation

Every Composed or Bound queue SHALL be represented by one ordered collection of queue slots containing `QueueItem` values. A Player owner SHALL hold the only authoritative collection for its Bound queue. A Client MAY hold a replaceable snapshot of a Bound queue, and a Playback run MAY hold an mpv execution projection, but neither SHALL independently decide canonical order, active slot, revision, or queue mutation outcome. A component SHALL NOT maintain parallel item-kind collections whose synchronization is required to determine queue contents, order, length, or current slot. An mpv projection MAY contain the full playable sequence or only the active materialized file as required by source lifecycle, but that projection SHALL NOT become queue authority. A populate-only queue replacement MAY update a Client's Composed queue without submitting it to an owner only when that Client does not present the queue as the Stay-alive process's queue. A Client presenting the Stay-alive queue SHALL submit a populate-only replacement to that owner at load time and SHALL display only owner-accepted contents, including the queue source. Until explicit play submits a Composed replacement to another playback target, the Client SHALL NOT present an unconfirmed slot from that replacement as the playing slot.

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

#### Scenario: Populate-only load on Stay-alive

- **WHEN** a Client loads a playlist without starting playback while presenting the Stay-alive process's queue
- **THEN** the Client SHALL send the replacement to that owner at load time
- **AND** its displayed queue and source SHALL follow the owner's accepted snapshot, not a private staged queue

## ADDED Requirements

### Requirement: Stay-alive replacement stops playback before publishing the new queue

When a Client loads a playlist into the Stay-alive process's queue without requesting playback, that owner SHALL stop and finalize the prior playing item, replace its entire Bound queue and Queue source with the admitted playlist and source, clear the observed active slot and pending playback transitions, and publish a stopped snapshot with no playing row. The replacement SHALL preserve a valid selected queue cursor for subsequent explicit Play but SHALL NOT start playback. The owner SHALL NOT publish a state in which an item is playing outside its Bound queue. A load rejected before acceptance SHALL NOT replace or stop the existing queue. If delivery is uncertain because the connection fails in flight, the Client SHALL reconcile from the owner rather than assume either outcome.

#### Scenario: Load while old item plays

- **WHEN** the owner is playing an old queue and a Client loads a different playlist without autostart
- **THEN** the owner SHALL stop the old item and replace its queue and source with the loaded playlist
- **AND** every Client SHALL observe the new queue as stopped with no playing slot
- **AND** the old item SHALL NOT resume or appear as playing in the new queue

#### Scenario: Play after a successful idle load

- **WHEN** a Client explicitly plays a slot from the newly loaded queue
- **THEN** the owner SHALL start that owner-assigned slot without resubmitting a private copy of the queue

#### Scenario: A late observation from the old Playback run arrives

- **WHEN** a prior run reports progress, completion, or a track change after the replacement is accepted
- **THEN** the report SHALL NOT mark a new-queue slot as playing, completed, or consumed

#### Scenario: Empty playlist load

- **WHEN** a Client loads an empty playlist into the Stay-alive queue
- **THEN** the owner SHALL stop playback, leave its queue empty, and publish no active slot

#### Scenario: Rejected replacement

- **WHEN** a requested idle replacement is rejected before acceptance
- **THEN** the existing owner queue and playback SHALL remain authoritative
- **AND** the Client SHALL display an error rather than claiming the playlist was loaded
