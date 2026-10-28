# Spec Delta

## MODIFIED Requirements

### Requirement: Queue edits reach the owner that holds the queue

A Client SHALL dispatch every canonical-queue edit (slot removal, slot move, item append or insert, clear, refresh) to the Player owner whose queue the queue panel shows: the directly controlled peer Player owner while the Client is connected to one, otherwise the local daemon. A Client has no canonical queue of its own: while it is not connected to a peer, its shown queue is the local daemon's Bound queue in every state, including while an Emby Session or a cast receiver is the playback output, so its edits SHALL reach that owner.

#### Scenario: Direct-remote edits still reach the owner

- **WHEN** the Client is connected to a peer Player owner and the user edits the shown queue
- **THEN** the edit SHALL be dispatched to that peer as a slot-addressed command

#### Scenario: Local edit while a session or cast is the target

- **WHEN** a Client is not connected to a peer, an Emby Session or a cast receiver is the playback output, and the user edits the shown queue
- **THEN** the edit SHALL be dispatched to the local daemon
- **AND** SHALL NOT be applied only to a private Client copy

### Requirement: Clients hold no editable queue

A Client's displayed queue SHALL change only by adopting a snapshot from the Player owner that holds that queue. A Client SHALL NOT insert, remove, move, replace, clear, refresh-merge, consume, or mark progress on a queue it displays, SHALL NOT predict an edit's or a playback transition's outcome, and SHALL NOT persist, restore, or seed a queue.

#### Scenario: Edit awaiting the owner

- **WHEN** the user removes a queue entry and the owner has not yet answered
- **THEN** the displayed queue SHALL still show the owner's last accepted state

#### Scenario: Owner rejects an edit

- **WHEN** the owner rejects a queue edit
- **THEN** the Client SHALL report that the edit did not apply
- **AND** the displayed queue SHALL be the owner's current state, with nothing to roll back

#### Scenario: Queue refresh

- **WHEN** the user refreshes the queue
- **THEN** the Client SHALL ask the owner to refresh its queue
- **AND** the refreshed items SHALL appear only through the owner's resulting snapshot

### Requirement: Queue edits are answered before the next input

Every queue edit a Client sends SHALL carry an identity, and the Player owner SHALL answer the sending Client with that identity and either the resulting snapshot or a rejection; other attached Clients SHALL receive the resulting snapshot as usual. The Client SHALL adopt the answer before handling its next input, so an accepted edit is visible in the next frame after the owner applies it. The Client SHALL wait for the answer for a bounded time only; when the bound passes, it SHALL report that the owner did not respond, keep showing the owner's last accepted state, and continue to adopt later snapshots, including a late answer to that edit. A remote Player owner that does not advertise answered queue edits SHALL still receive the edit, and the Client SHALL NOT wait for an answer from it.

#### Scenario: Rapid repeated removals

- **WHEN** the user presses the remove key three times in quick succession on consecutive entries of the local Owner queue
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

- **WHEN** the Client holds replacement content while the owner is still stopping the playing item
- **THEN** the Client SHALL keep handling input
- **AND** SHALL show nothing as loaded until an owner snapshot or an explicit play's answer carries it

#### Scenario: Older remote owner

- **WHEN** the user edits the queue of a connected peer Player owner that does not advertise answered queue edits
- **THEN** the edit SHALL be sent and the Client SHALL NOT wait for an answer
- **AND** the displayed queue SHALL follow that owner's later snapshots
