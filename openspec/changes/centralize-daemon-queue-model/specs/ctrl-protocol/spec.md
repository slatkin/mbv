## MODIFIED Requirements

### Requirement: Protocol version 8
The ctrl protocol version SHALL be 8. During one release window, a v8 daemon SHALL accept a
v7 client hello and mark that connection as legacy. A v8 client SHALL require a v8 daemon.

#### Scenario: v8 client connects to v8 daemon
- **WHEN** a client and daemon negotiate protocol version 8
- **THEN** the connection SHALL use slot-based queue commands
- **AND** queue state snapshots SHALL include slot IDs, revision, and active slot ID

#### Scenario: v7 client connects to v8 daemon during the compatibility window
- **WHEN** a client sends a protocol version 7 hello to a version 8 daemon
- **THEN** the daemon SHALL accept the connection as a legacy peer
- **AND** the daemon SHALL accept index-based queue commands only on that connection
- **AND** the daemon SHALL send legacy full-state snapshots to that connection

#### Scenario: v8 client connects to v7 daemon
- **WHEN** a version 8 client connects to a daemon that supports only version 7
- **THEN** the daemon SHALL reject the connection with a protocol version mismatch error

## ADDED Requirements

### Requirement: Slot-based queue mutation commands
For v8 peers, queue mutation commands SHALL address items by `QueueSlotId` instead of
positional index. Structural mutation commands SHALL carry the client's last-known
`QueueRevision`.

#### Scenario: Remove item by slot ID
- **WHEN** a v8 client sends `QueueRemoveBySlot { slot_id, revision }` and the revision matches the daemon's current revision
- **THEN** the daemon SHALL remove the identified slot
- **AND** the daemon SHALL increment the revision
- **AND** the daemon SHALL broadcast a full slot-aware state snapshot

#### Scenario: Remove item by slot ID with stale revision
- **WHEN** a client sends `QueueRemoveBySlot { slot_id, revision }` and the revision does not match
- **THEN** the daemon SHALL reject the command with reason "stale revision"
- **AND** the daemon SHALL send a full reconciliation snapshot to the requesting client

#### Scenario: Move item by slot ID
- **WHEN** a v8 client sends `QueueMoveBySlot { slot_id, to_position, revision }` and the revision matches
- **THEN** the daemon SHALL move the identified slot to the specified position
- **AND** the daemon SHALL increment the revision
- **AND** the daemon SHALL broadcast a full slot-aware state snapshot

#### Scenario: Jump to item by slot ID
- **WHEN** a v8 client sends `JumpToSlot { slot_id }`
- **THEN** the daemon SHALL set that slot as active
- **AND** the daemon SHALL begin playback of that slot

### Requirement: Active-item removal command
The protocol SHALL include `QueueRemoveActive { revision }`, which logically removes the
currently active slot and advances the active marker without waiting for player teardown.

#### Scenario: Remove active item transactionally
- **WHEN** a client sends `QueueRemoveActive { revision }` and the revision matches
- **THEN** the daemon SHALL remove the active slot
- **AND** the active marker SHALL advance to the successor or become `None`
- **AND** the daemon SHALL broadcast a full slot-aware snapshot with the new revision
- **AND** playback teardown and progress finalization SHALL continue asynchronously

#### Scenario: Remove active when no item is active
- **WHEN** a client sends `QueueRemoveActive` and no slot is active
- **THEN** the daemon SHALL reject the command
- **AND** the daemon SHALL send its current full state to the requesting client

### Requirement: Full state uses the peer's negotiated format
The daemon SHALL broadcast a complete queue state snapshot after every accepted structural
mutation. The format SHALL be selected from the connection's negotiated peer version.

#### Scenario: State broadcast to v8 peer
- **WHEN** the daemon broadcasts state to a v8 client
- **THEN** `CtrlState` SHALL include ordered items and parallel `QueueSlotId` values
- **AND** `CtrlState` SHALL include the current `QueueRevision`
- **AND** `CtrlState` SHALL include `active_slot_id: Option<QueueSlotId>`
- **AND** `CtrlState` SHALL NOT include client UI selection

#### Scenario: State broadcast to v7 peer
- **WHEN** the daemon broadcasts state to a legacy v7 client
- **THEN** `CtrlState` SHALL use the legacy positional cursor format
- **AND** the state SHALL NOT expose v8 slot or revision fields

### Requirement: Append, adopt, and restore communicate daemon identity
After append, adopt, or undo restoration changes queue membership, the daemon SHALL assign
canonical slot IDs and communicate them through the resulting full state snapshot.

#### Scenario: Append succeeds
- **WHEN** a client appends one or more items and the daemon accepts the command
- **THEN** the daemon SHALL assign new slot IDs
- **AND** the daemon SHALL broadcast a full state snapshot containing those IDs

#### Scenario: Adopt succeeds
- **WHEN** a cold daemon accepts an adopted queue
- **THEN** the daemon SHALL assign canonical slot IDs to every slot
- **AND** the daemon SHALL broadcast a full state snapshot containing those IDs

#### Scenario: Removed item is restored by undo
- **WHEN** a client sends `QueueInsertAt { item, position, revision }` with the current revision
- **THEN** the daemon SHALL assign the restored slot a new slot ID
- **AND** the daemon SHALL broadcast a full state snapshot containing the restored slot

### Requirement: Legacy queue handlers are version-gated
The v8 daemon SHALL expose index-based wire queue handlers only to connections negotiated as
v7. Bare-mode in-process index operations MAY remain, but SHALL NOT be reachable as v8 wire
commands.

#### Scenario: v8 peer sends a legacy index command
- **WHEN** a v8 connection sends an index-based queue mutation
- **THEN** the daemon SHALL reject the command

#### Scenario: v7 peer sends a legacy index command
- **WHEN** a v7 compatibility connection sends an index-based queue mutation
- **THEN** the daemon SHALL translate the addressed index to the current daemon slot
- **AND** the daemon SHALL apply the operation through the canonical queue model
