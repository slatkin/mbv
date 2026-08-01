## MODIFIED Requirements

### Requirement: Commands accepted from any connected client
The daemon SHALL accept commands from any connected ctrl client when ctrl has authority.
Queue mutation commands SHALL carry a `QueueRevision` for conflict detection. The daemon
SHALL process commands in the order received and SHALL reject commands whose revision does
not match the daemon's current revision.

#### Scenario: Client A sends command while Client B is connected
- **WHEN** Client A and Client B are both connected
- **WHEN** Client A sends a slot-based queue mutation command with a valid revision
- **THEN** the daemon SHALL execute the command
- **THEN** both Client A and Client B SHALL receive the state update

#### Scenario: Rapid commands from multiple clients with conflicting revisions
- **WHEN** Client A sends a mutation that increments the revision
- **WHEN** Client B sends a mutation carrying the old revision before receiving the update
- **THEN** the daemon SHALL reject Client B's command with a stale revision error
- **AND** the daemon SHALL send a full state snapshot to Client B for reconciliation
- **AND** Client A's mutation SHALL remain applied

#### Scenario: Concurrent commands processed in arrival order
- **WHEN** Client A and Client B both send valid mutations with the current revision in rapid succession
- **THEN** the daemon SHALL process them in the order they arrive on the daemon's event channel
- **AND** the first command SHALL succeed and increment the revision
- **AND** the second command SHALL be rejected as stale revision
- **AND** the rejected client SHALL receive a reconciliation snapshot

## ADDED Requirements

### Requirement: Version-aware full-state broadcast fan-out
The daemon SHALL broadcast complete queue state snapshots to all connected clients after an
accepted mutation, using the format appropriate to each connection's negotiated protocol
version. v8 clients SHALL receive slot-aware snapshots; v7 compatibility clients SHALL receive
legacy positional snapshots.

#### Scenario: Mixed client broadcast after queue mutation
- **WHEN** the daemon applies a queue mutation
- **AND** one client uses protocol v8 and one client uses the v7 compatibility path
- **THEN** the v8 client SHALL receive a full `CtrlState` containing slot IDs and revision
- **AND** the legacy client SHALL receive a full `CtrlState` without slot metadata
- **AND** both clients' views SHALL be consistent with the daemon's authoritative queue

#### Scenario: Broadcast after wholesale queue replacement
- **WHEN** the daemon processes a `ReplaceQueue` or `AdoptQueue` command
- **THEN** all connected clients SHALL receive full `CtrlState` snapshots in their negotiated formats
