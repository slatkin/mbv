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

- **WHEN** a Client installs a replacement queue without starting playback on the owner, because an attached Emby Session or cast target is the playback target, while presenting the Stay-alive process's queue
- **THEN** the Client SHALL send the replacement to that owner at load time
- **AND** its displayed queue and source SHALL follow the owner's accepted snapshot, not a private staged queue

### Requirement: Queue edits are answered before the next input

Every queue edit a Client sends, other than a populate-only load (a replacement installed without starting playback), SHALL carry an identity, and the Player owner SHALL answer the sending Client with that identity and either the resulting snapshot or a rejection; other attached Clients SHALL receive the resulting snapshot as usual. The Client SHALL adopt the answer before handling its next input, so an accepted edit is visible in the next frame after the owner applies it. The Client SHALL wait for the answer for a bounded time only; when the bound passes, it SHALL report that the owner did not respond, keep showing the owner's last accepted state, and continue to adopt later snapshots, including a late answer to that edit. A populate-only load SHALL keep its own load result: the Client SHALL NOT wait for it before handling input, and SHALL NOT show the load as applied until that result or an owner snapshot contains it. A remote Player owner that does not advertise answered queue edits SHALL still receive the edit, and the Client SHALL NOT wait for an answer from it.

#### Scenario: Rapid repeated removals

- **WHEN** the user presses the remove key three times in quick succession on consecutive entries of the Local queue
- **THEN** each removal SHALL be sent against the queue as updated by the previous removal's answer
- **AND** three entries SHALL be removed

#### Scenario: Other Clients see the edit

- **WHEN** one Client's queue edit is accepted while a second Client is attached
- **THEN** the second Client SHALL display the resulting queue from the owner's snapshot

#### Scenario: Owner does not answer in time

- **WHEN** the owner does not answer a queue edit within the bound
- **THEN** the Client SHALL report that the owner did not respond
- **AND** SHALL NOT show the edit as applied until an owner snapshot contains it

#### Scenario: Late answer arrives

- **WHEN** the owner's answer to an edit arrives after the Client stopped waiting
- **THEN** the Client SHALL adopt that answer's state like any other owner snapshot

#### Scenario: Idle load while an item plays

- **WHEN** a populate-only load is sent while the owner is still stopping the playing item
- **THEN** the Client SHALL keep handling input
- **AND** SHALL show the loaded queue once the owner's load result or snapshot contains it

#### Scenario: Older remote owner

- **WHEN** the user edits the Remote scope queue of a directly controlled owner that does not advertise answered queue edits
- **THEN** the edit SHALL be sent and the Client SHALL NOT wait for an answer
- **AND** the displayed queue SHALL follow that owner's later snapshots
