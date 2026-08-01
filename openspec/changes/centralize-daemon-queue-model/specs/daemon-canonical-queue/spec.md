## ADDED Requirements

### Requirement: Daemon owns canonical PlaybackQueue
The daemon SHALL maintain a single `PlaybackQueue` instance as the authoritative queue
model. All queue state — item ordering, active item, slot identities, and revision — SHALL
be derived from this single model. The daemon SHALL NOT maintain a separate `Vec<MediaItem>`
or positional cursor as authoritative state.

#### Scenario: Daemon initializes with an empty queue
- **WHEN** the daemon starts with no persisted queue state
- **THEN** the daemon SHALL initialize an empty `PlaybackQueue` with revision 0
- **AND** the active slot SHALL be `None`

#### Scenario: Daemon restores queue from persisted state with slot IDs
- **WHEN** the daemon starts and `queue_state.json` contains slot IDs
- **THEN** the daemon SHALL reconstruct the `PlaybackQueue` preserving the persisted slot IDs
- **AND** the revision SHALL be set to the persisted revision plus one

#### Scenario: Daemon restores queue from persisted state without slot IDs
- **WHEN** the daemon starts and `queue_state.json` does not contain slot IDs
- **THEN** the daemon SHALL construct a fresh `PlaybackQueue` from the persisted items
- **AND** new slot IDs SHALL be allocated monotonically starting from 1
- **AND** the revision SHALL be 0

### Requirement: Slot identity is stable across mutations
Each queue slot SHALL retain its `QueueSlotId` across reordering, selection changes, and
metadata updates. A slot ID SHALL NOT be reused after the slot is removed.

#### Scenario: Item is moved within the queue
- **WHEN** a slot is moved from position A to position B
- **THEN** the slot's `QueueSlotId` SHALL remain unchanged
- **AND** the queue revision SHALL be incremented

#### Scenario: Item is removed and a new item is added
- **WHEN** a slot is removed and a different item is subsequently appended
- **THEN** the new slot SHALL receive a new `QueueSlotId` that has not been previously used
- **AND** the removed slot's ID SHALL NOT be reused

### Requirement: Queue revision advances on structural mutation
The daemon SHALL increment the `QueueRevision` on every structural mutation: insert, remove,
move, replace, and adopt. Non-structural updates (metadata refresh, progress updates) SHALL
NOT increment the revision.

#### Scenario: Items are appended to the queue
- **WHEN** one or more items are appended to the queue
- **THEN** the queue revision SHALL be incremented by exactly one
- **AND** each new item SHALL receive a unique slot ID

#### Scenario: Metadata is updated without structural change
- **WHEN** an item's played status or resume position is updated
- **THEN** the queue revision SHALL NOT change

### Requirement: Active-item deletion is an atomic daemon transaction
The daemon SHALL support a `QueueRemoveActive` command that atomically removes the active
slot and advances the active marker to the next slot (or clears it if the queue becomes
empty). Progress finalization for the removed item SHALL complete asynchronously after the
logical removal.

#### Scenario: Active item is removed while playing
- **WHEN** the daemon receives `QueueRemoveActive` and the active slot exists
- **THEN** the daemon SHALL remove the active slot from the queue
- **AND** the active marker SHALL advance to the slot now at the former active position (or the last slot if the queue shrank past that index)
- **AND** the daemon SHALL broadcast the updated queue state with the new revision
- **AND** progress finalization for the removed item SHALL be dispatched asynchronously

#### Scenario: Confirmed active deletion is visible without waiting for player shutdown
- **WHEN** a client confirms active-item deletion
- **THEN** the client SHALL close the confirmation modal
- **AND** the intended queue removal SHALL be visible in the next rendered frame
- **AND** the client SHALL NOT wait for mpv shutdown before hiding the removed slot
- **AND** the next daemon snapshot SHALL confirm or reconcile the optimistic projection

#### Scenario: Active item is removed when queue has one item
- **WHEN** the daemon receives `QueueRemoveActive` and the queue contains exactly one slot
- **THEN** the daemon SHALL remove the slot
- **AND** the active marker SHALL become `None`
- **AND** playback SHALL stop

#### Scenario: Active item removal with stale revision
- **WHEN** the daemon receives `QueueRemoveActive` with a revision that does not match the current queue revision
- **THEN** the daemon SHALL reject the command
- **AND** the daemon SHALL send a reconciliation state snapshot to the requesting client

### Requirement: Client selection is local and identity-preserving
Each client SHALL own its queue selection locally as an optional `QueueSlotId`. Selection
SHALL NOT be sent to or stored by the daemon and SHALL NOT affect playback.

#### Scenario: Selected slot survives a queue mutation
- **WHEN** a client receives a new queue snapshot and its selected slot still exists
- **THEN** the same slot SHALL remain selected regardless of its new positional index

#### Scenario: Selected slot is deleted
- **WHEN** a client's selected slot is removed
- **THEN** the client SHALL select the slot now at the deleted slot's former visual position
- **AND** if there is no successor at that position, the client SHALL select the predecessor
- **AND** if the queue is empty, selection SHALL become `None`

#### Scenario: Client connects without retained selection
- **WHEN** a client connects or reconnects and receives a full queue snapshot
- **THEN** selection SHALL default to the active slot when one exists
- **AND** otherwise selection SHALL default to the first slot

### Requirement: Persistence is owned by the daemon
The daemon SHALL be the sole writer of `queue_state.json`. Clients connected to a daemon
SHALL NOT write queue state to disk. A bare-mode client (no daemon) SHALL continue to write
its own queue state.

#### Scenario: Daemon persists queue state on structural mutation
- **WHEN** the daemon applies a structural mutation (insert, remove, move, replace)
- **THEN** the daemon SHALL persist the updated queue state including slot IDs and revision

#### Scenario: Client connected to daemon does not persist queue state
- **WHEN** a client connected to a daemon mutates the queue
- **THEN** the client SHALL NOT write `queue_state.json`
- **AND** the daemon SHALL be responsible for persistence

#### Scenario: Bare-mode client persists queue state
- **WHEN** a bare-mode client (no daemon) mutates the queue
- **THEN** the client SHALL write `queue_state.json` as it does today

### Requirement: Reconnect bootstrap provides full slot-aware state
When a client connects or reconnects to a daemon, the daemon SHALL provide a complete queue
snapshot including slot IDs, revision, active slot, and source. The client SHALL build its
local queue projection from this snapshot.

#### Scenario: Client connects to a daemon with an active queue
- **WHEN** a client completes the hello handshake with a daemon that has a non-empty queue
- **THEN** the daemon SHALL send a `CtrlState` containing items with slot IDs, the current revision, the active slot ID, and the queue source
- **AND** the client SHALL construct its local `PlaybackQueue` using the daemon-assigned slot IDs

#### Scenario: Client reconnects after a disconnection
- **WHEN** a client reconnects to a daemon after a connection loss
- **THEN** the client SHALL receive a fresh full snapshot
- **AND** the client SHALL replace its local queue state entirely
- **AND** the client's undo stack SHALL be cleared

### Requirement: Undo is bounded by connection lifetime and revision
Client-local undo SHALL be scoped to the current connection and the revision at which the
mutation was applied. Undo of a mutation whose revision no longer matches the daemon's
current revision SHALL be rejected locally without sending a command.

#### Scenario: Undo within the same revision
- **WHEN** the user invokes undo and the undo entry's revision matches the daemon's current revision
- **THEN** the client SHALL send the inverse mutation to the daemon using the slot ID from the undo entry

#### Scenario: Undo after another client has mutated the queue
- **WHEN** the user invokes undo and the undo entry's revision does not match the daemon's current revision
- **THEN** the client SHALL NOT send any command to the daemon
- **AND** the client SHALL indicate that undo is unavailable because the queue has changed

#### Scenario: Undo stack is cleared on reconnect
- **WHEN** a client reconnects to the daemon
- **THEN** the client's undo stack SHALL be cleared

#### Scenario: Undo active-item deletion restores membership without resuming playback
- **WHEN** the user invokes a revision-valid undo of an active-item deletion
- **THEN** the client SHALL request insertion of the removed item at its recorded prior position
- **AND** the daemon SHALL assign the restored slot a new slot ID
- **AND** the restored item SHALL NOT automatically become active
- **AND** playback SHALL NOT automatically resume or redirect to the restored item
