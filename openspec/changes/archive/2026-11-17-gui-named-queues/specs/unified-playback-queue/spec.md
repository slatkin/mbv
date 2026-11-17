## MODIFIED Requirements

### Requirement: Queue edits reach the owner that holds the queue

A Client SHALL dispatch every canonical-queue edit (slot removal, slot move, item append or insert, clear, refresh) to the Player owner whose queue the queue panel shows: the directly controlled peer Player owner while the Client shows a peer, otherwise the local daemon. A Client has no canonical queue of its own: while it shows no peer, its shown queue is one of the local daemon's Named queues in every state, including while an Emby Session or a cast receiver is the playback output, so its edits SHALL reach that owner. A GUI Client SHALL name the shown Named queue in each edit to the local daemon; a TUI Client shows the current queue, and its edits SHALL reach the current queue.

#### Scenario: Direct-remote edits still reach the owner

- **WHEN** the Client is connected to a peer Player owner, shows that peer, and the user edits the shown queue
- **THEN** the edit SHALL be dispatched to that peer as a slot-addressed command

#### Scenario: Local edit while a session or cast is the target

- **WHEN** a Client shows no peer, an Emby Session or a cast receiver is the playback output, and the user edits the shown queue
- **THEN** the edit SHALL be dispatched to the local daemon
- **AND** SHALL NOT be applied only to a private Client copy

#### Scenario: A GUI edit of a resting queue

- **GIVEN** the GUI Client shows the resting Named queue `Kids` while `Default` plays
- **WHEN** the user removes a row
- **THEN** the edit SHALL be dispatched to the local daemon naming `Kids`
- **AND** `Default` SHALL NOT change

#### Scenario: A GUI Client connected to a peer shows a local queue

- **GIVEN** the GUI Client is connected to a peer and shows the local Named queue `Default`
- **WHEN** the user edits the shown queue
- **THEN** the edit SHALL be dispatched to the local daemon, not to the peer
