# daemon-multi-connection Specification

## Purpose
TBD - created by archiving change daemon-multi-connection. Update Purpose after archive.
## Requirements
### Requirement: Multiple concurrent ctrl client connections
The daemon SHALL accept multiple simultaneous ctrl client connections over Unix socket and TCP. Each connection SHALL be independent and SHALL NOT evict or disconnect other ctrl clients.

#### Scenario: Second client connects while first is active
- **WHEN** a ctrl client is already connected and a second ctrl client connects
- **THEN** both clients SHALL remain connected
- **THEN** both clients SHALL receive state broadcasts

#### Scenario: Third client connects while two are active
- **WHEN** two ctrl clients are connected and a third connects
- **THEN** all three clients SHALL remain connected
- **THEN** all three clients SHALL receive state broadcasts

### Requirement: Commands accepted from any connected client
The daemon SHALL accept commands from any connected ctrl client when ctrl has authority. Commands SHALL be processed in the order received (last command wins).

#### Scenario: Client A sends command while Client B is connected
- **WHEN** Client A and Client B are both connected
- **WHEN** Client A sends a player command (e.g., pause)
- **THEN** the daemon SHALL execute the command
- **THEN** both Client A and Client B SHALL receive the state broadcast

#### Scenario: Rapid commands from multiple clients
- **WHEN** Client A sends "pause" and Client B sends "play" in quick succession
- **THEN** the daemon SHALL process both commands in order
- **THEN** the final state SHALL reflect the last command processed
- **THEN** all clients SHALL receive broadcasts for each state change

### Requirement: Broadcast fan-out to all connected clients
The daemon SHALL send state broadcasts (player events, queue changes, status updates) to all connected ctrl clients.

#### Scenario: Queue change while multiple clients connected
- **WHEN** three ctrl clients are connected
- **WHEN** any client sends a queue append command
- **THEN** all three clients SHALL receive the updated queue state broadcast

#### Scenario: Periodic status broadcast to multiple clients
- **WHEN** two ctrl clients are connected
- **WHEN** the periodic status broadcast timer fires
- **THEN** both clients SHALL receive the `StatusOnly` event

### Requirement: Connection removal on disconnect
The daemon SHALL remove a ctrl client from the connection registry when the client disconnects (connection closed or send failure). Other connected clients SHALL NOT be affected. Authority SHALL go to `None` only when the last ctrl client disconnects.

#### Scenario: One client disconnects while others remain
- **WHEN** Client A and Client B are connected
- **WHEN** Client A disconnects
- **THEN** Client B SHALL remain connected
- **THEN** Client B SHALL continue receiving broadcasts
- **THEN** authority SHALL remain `Ctrl` (if it was `Ctrl`)

#### Scenario: Last client disconnects
- **WHEN** only Client A is connected
- **WHEN** Client A disconnects
- **THEN** the daemon SHALL continue running
- **THEN** playback SHALL continue (daemon does not stop on disconnect)
- **THEN** authority SHALL change to `None`

### Requirement: No client awareness of other clients
The daemon SHALL NOT provide information about other connected clients to any ctrl client. Clients are independent and unaware of each other.

#### Scenario: Client connects and queries state
- **WHEN** a new client connects while other clients are already connected
- **WHEN** the new client receives the initial state snapshot
- **THEN** the state snapshot SHALL contain only player status, queue, cursor, and source
- **THEN** the state snapshot SHALL NOT contain information about other connected clients

