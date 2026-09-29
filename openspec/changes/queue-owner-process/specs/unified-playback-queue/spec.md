## ADDED Requirements

### Requirement: Client-initiated slot jumps are requested from the Player owner

A Client SHALL request every user-initiated jump to a canonical slot from the Player owner that
holds the queue, using the owner-resolved slot request. The terminal UI process is never a Player
owner, so a Client SHALL NOT resolve a jump locally. A Client SHALL NOT construct or transmit a
command that the ctrl transport has no wire form for. On completion of a requested jump, the
Client SHALL NOT set its own active slot from the requested slot; the active slot SHALL continue
to follow the owner's queue snapshot.

#### Scenario: Owner accepts the on-screen Next-Up accept action

- **WHEN** the mpv Next-Up accept affordance is activated
- **THEN** the Client SHALL request the jump from the Player owner
- **AND** the requested slot SHALL become active only through the owner's queue snapshot
- **AND** the Client process SHALL remain running

#### Scenario: Every client-initiated jump uses the same dispatch

- **WHEN** any client-initiated action jumps to an existing canonical slot
- **THEN** it SHALL be requested from the Player owner rather than constructing a local jump
  command directly

### Requirement: Queue edits reach the owner that holds the queue

A Client SHALL dispatch every canonical-queue edit (slot removal, slot move, item append or insert, clear, refresh) to the Player owner that holds the queue being edited: the local daemon for Local scope, or the directly controlled remote Player owner for Remote scope. A Client has no canonical queue of its own: its displayed Local queue is the local daemon's Bound queue in every state, including while an attached Emby session or a cast receiver is the playback target, so its Local-scope edits SHALL reach that owner. Direct-remote queue management is unchanged: edits addressed to Remote scope reach that owner.

#### Scenario: Direct-remote edits still reach the owner

- **WHEN** the user edits a directly controlled remote Player owner's queue in Remote scope
- **THEN** the edit SHALL still be dispatched to that owner as a slot-addressed command

#### Scenario: Local edit while a session or cast is the target

- **WHEN** a Client is watching an attached Emby session or casting to a receiver and edits the Local queue
- **THEN** the edit SHALL be dispatched to the local daemon
- **AND** SHALL NOT be applied only to a private Client copy


### Requirement: Clients hold no editable queue

A Client's displayed queue for each queue scope SHALL change only by adopting a snapshot from the Player owner that holds that queue. A Client SHALL NOT insert, remove, move, replace, clear, refresh-merge, consume, or mark progress on a queue it displays, SHALL NOT predict an edit's or a playback transition's outcome, and SHALL NOT persist, restore, or seed a queue.

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

Every queue edit a Client sends SHALL carry an identity, and the Player owner SHALL answer the sending Client with that identity and either the resulting snapshot or a rejection; other attached Clients SHALL receive the resulting snapshot as usual. The Client SHALL adopt the answer before handling its next input, so an accepted edit is visible in the next frame after the owner applies it. The Client SHALL wait for the answer for a bounded time only; when the bound passes, it SHALL report that the owner did not respond, keep showing the owner's last accepted state, and continue to adopt later snapshots. A remote Player owner that does not advertise answered queue edits SHALL still receive the edit, and the Client SHALL NOT wait for an answer from it.

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

#### Scenario: Older remote owner

- **WHEN** the user edits the Remote scope queue of a directly controlled owner that does not advertise answered queue edits
- **THEN** the edit SHALL be sent and the Client SHALL NOT wait for an answer
- **AND** the displayed queue SHALL follow that owner's later snapshots

### Requirement: Queue selection follows the selected slot

A Client's queue selection SHALL identify the selected queue slot, not a position. When the Client adopts the answer to its own queue edit, the selection SHALL stay on the selected slot wherever it moved; when that slot is gone, the selection SHALL move to the entry now at its former position, or to the last entry when that position no longer exists. The answer to the Client's own edit SHALL NOT move the selection to the playing entry. A snapshot the Client did not cause SHALL keep today's rule: the selection follows the active slot unless the user navigated the queue recently, in which case it stays on the user's slot as above. A whole-queue replacement SHALL move the selection to the replacement's start entry.

#### Scenario: Removing the selected entry

- **WHEN** the selected entry is removed and the owner's snapshot is adopted
- **THEN** the selection SHALL be on the entry that followed it, or on the new last entry when the removed entry was last

#### Scenario: Moving the selected entry

- **WHEN** the selected entry is moved and the owner's snapshot is adopted
- **THEN** the selection SHALL remain on that entry at its new position

#### Scenario: Track advance while browsing the queue

- **WHEN** the owner advances to the next track shortly after the user navigated to a different entry
- **THEN** the selection SHALL remain on the user's entry

#### Scenario: Track advance while not browsing

- **WHEN** the owner advances to the next track and the user has not navigated the queue recently
- **THEN** the selection SHALL move to the newly active entry

#### Scenario: Whole-queue replacement

- **WHEN** an adopted snapshot comes from a whole-queue replacement
- **THEN** the selection SHALL move to the replacement's start entry

### Requirement: Queue undo is an owner operation

Undoing a queue edit SHALL send the inverse edit to the Player owner and SHALL follow the same answered-edit rule as any other edit. Undoing a removal SHALL insert the removed item before the entry now at the removed item's former position, or at the end when that position no longer exists; the restored entry is a new occurrence. Undoing a move SHALL move the same slot back to its former position. Undo history SHALL belong to the Client that made the edits.

#### Scenario: Undo a removal

- **WHEN** the user removes the third entry and then undoes
- **THEN** the owner SHALL hold that item at the third position again
- **AND** every attached Client SHALL display it there

#### Scenario: Undo a move whose slot is gone

- **WHEN** the user undoes a move after another Client removed the moved entry
- **THEN** the owner SHALL reject the undo as stale slot addressing
- **AND** the Client SHALL report that the undo did not apply

## REMOVED Requirements

### Requirement: A client-initiated slot jump dispatches by where the Player owner runs
**Reason**: The terminal UI process is never a Player owner, so there is no longer a branch for a local jump.
**Migration**: Replaced by "Client-initiated slot jumps are requested from the Player owner".

### Requirement: Canonical-queue edits follow the playback target
**Reason**: No Client owns a canonical queue, so the branch for editing a private queue while a session or cast is the target no longer exists.
**Migration**: Replaced by "Queue edits reach the owner that holds the queue".
