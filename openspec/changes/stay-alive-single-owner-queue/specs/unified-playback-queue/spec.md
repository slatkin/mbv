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

### Requirement: Canonical-queue edits follow the playback target

A Client SHALL dispatch canonical-queue edit commands (slot removal, slot move, item append) to the Player owner that holds the queue being edited: the in-process bare player, the Stay-alive process the Client is attached to, or the directly controlled remote Player owner for Remote scope. A Client attached to the Stay-alive process has no separate canonical queue: its displayed Local queue is that owner's Bound queue in every state, including while an attached Emby session or a cast receiver is the playback target, so its canonical-queue edits SHALL reach that owner. A Client that owns its canonical queue and directs playback at an attached Emby session or a cast receiver SHALL apply the edit to its own canonical queue only and SHALL NOT transmit a queue command to a Player owner, because that owner does not hold the queue the user is editing. Direct-remote queue management (editing a directly controlled remote Player owner over ctrl) is unchanged: edits addressed to that owner's scope continue to reach it.

#### Scenario: Queue edit while an attached session plays

- **WHEN** a Client that owns its canonical queue has an attached Emby session as its playback target and the user removes, moves, or appends an item in that queue
- **THEN** the Client SHALL mutate its own canonical queue
- **AND** SHALL NOT send a queue command to a Player owner

#### Scenario: Visible queue survives a stale owner

- **WHEN** a Player owner holds a Bound queue that differs from a Client's own canonical queue while that Client has an attached session as the playback target
- **THEN** a canonical-queue edit SHALL NOT cause the Client to replace its visible queue with the owner's snapshot
- **AND** the user's loaded playlist source and contents SHALL remain unchanged

#### Scenario: Direct-remote edits still reach the owner

- **WHEN** the user edits a directly controlled remote Player owner's queue in Remote scope
- **THEN** the edit SHALL still be dispatched to that owner as a slot-addressed command

#### Scenario: Stay-alive edit while a session or cast is the target

- **WHEN** a Client attached to the Stay-alive process is watching an attached Emby session or casting to a receiver and edits the queue
- **THEN** the edit SHALL be dispatched to that Stay-alive owner
- **AND** SHALL NOT be applied only to a private Client copy

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

### Requirement: The Stay-alive process holds the queue source

The Stay-alive process SHALL hold the queue source as part of its queue state. Every whole-queue replacement it accepts, every clear, and every source-only update SHALL set its source; a clear SHALL reset it to Unknown. A source-only update SHALL apply only to the queue lineage the owner held when the update was requested; a delayed update from an earlier queue SHALL NOT rename a later queue. A Client attached to that owner SHALL display the owner's source and SHALL NOT maintain an independent authoritative source for it.

#### Scenario: Playing a different source updates the owner

- **WHEN** a Client replaces the Stay-alive queue with items from a new album, playlist, or other source and starts playback
- **THEN** the owner's queue source SHALL become that new source
- **AND** every attached Client SHALL display the new source

#### Scenario: Clearing resets the source

- **WHEN** the Stay-alive queue is cleared
- **THEN** the owner's queue source SHALL reset to Unknown
- **AND** attached Clients SHALL display an empty queue with no source label

#### Scenario: A delayed Save As cannot rename a later queue

- **WHEN** a source-only update from an earlier queue arrives after another Client replaced the queue
- **THEN** the owner SHALL reject it
- **AND** the later queue's source SHALL remain unchanged
