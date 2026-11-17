# Spec Delta

## Purpose

The local Owner process holds several durable, named queues. One is the current queue, which
holds the playhead. This capability defines how the queues are made, played, closed, and kept.

## ADDED Requirements

### Requirement: The Owner holds named queues

The local Owner process SHALL hold one or more Named queues. Each Named queue SHALL have a stable queue identity, a user-visible name, its own ordered slots, its own Queue source, its own lineage, and its own start slot. Slot identity SHALL be unique within its queue. Exactly one Named queue SHALL be the current queue at all times.

#### Scenario: A first start makes one queue

- **GIVEN** the Owner starts with no persisted queue state
- **WHEN** it serves its first Client
- **THEN** it holds exactly one Named queue, named `Default`, which is the current queue and is empty

#### Scenario: Queues keep their own content

- **GIVEN** the Owner holds the Named queues `Default` and `Kids`
- **WHEN** a slot is removed from `Kids`
- **THEN** `Default` keeps every slot, its source, and its start slot

### Requirement: Only the current queue plays

Only the current queue SHALL hold the playhead, drive a Playback run, reach a Playback output, consume, and save on consume. A Named queue that is not current SHALL rest: it SHALL keep its slots and its start slot unchanged by playback, and no completion, consume, or progress observation SHALL change it. The next and previous items SHALL always come from the current queue.

#### Scenario: The next item comes from the current queue

- **GIVEN** `Default` is current and plays its second slot, and a Client shows `Kids`
- **WHEN** the item ends
- **THEN** playback advances to the third slot of `Default`

#### Scenario: A resting queue does not consume

- **GIVEN** consume is on and `Kids` rests
- **WHEN** an item of `Default` completes
- **THEN** `Kids` keeps every slot

### Requirement: Each queue has a start slot

Each Named queue SHALL have at most one start slot. While the queue plays, the start slot SHALL be the active slot. A stop in the middle of an item SHALL keep that slot as the start slot. A natural completion SHALL make the next slot the start slot, after any consume. When the last slot completes, the queue SHALL have no start slot. Removing the start slot SHALL make the slot now at its position the start slot, or none when no slot remains there.

#### Scenario: A stop keeps the slot

- **GIVEN** `Default` plays its third slot
- **WHEN** the user stops playback
- **THEN** the start slot of `Default` is its third slot

#### Scenario: A completion moves the start slot

- **GIVEN** `Default` plays its third slot and the queue has a fourth slot
- **WHEN** the item completes and playback then stops
- **THEN** the start slot of `Default` is the slot that followed the completed one

#### Scenario: The last slot completes

- **GIVEN** `Default` plays its last slot
- **WHEN** the item completes
- **THEN** `Default` has no start slot

### Requirement: Play starts a queue at its start slot

A request to play a Named queue SHALL make it the current queue and SHALL start its start slot, or its first slot when it has no start slot. The started item SHALL take its position from its Resume source. A request to play a slot of a Named queue SHALL make that queue current and start that slot. When another queue was current and playing, its playback SHALL stop first and its start slot SHALL follow the stop rule. A request to play an empty queue SHALL change nothing.

#### Scenario: Play a resting queue

- **GIVEN** `Default` is current and stopped, and `Kids` rests with its second slot as its start slot
- **WHEN** a Client requests play of `Kids`
- **THEN** `Kids` becomes current and its second slot plays

#### Scenario: Play a row of a resting queue while another queue plays

- **GIVEN** `Default` plays its fourth slot and `Kids` rests
- **WHEN** a Client requests play of the first slot of `Kids`
- **THEN** `Default` stops with its fourth slot as its start slot
- **AND** `Kids` becomes current and its first slot plays

### Requirement: A Client creates and renames queues

A Client SHALL be able to ask the Owner to create a Named queue and to rename a Named queue. A new queue SHALL be empty, SHALL rest, and SHALL take the name `Queue N`, where N is the lowest number from 2 up that no queue named `Queue N` uses. A rename SHALL accept any non-empty name after the Owner trims surrounding whitespace, and two queues MAY share a name. The Owner SHALL publish the result to every Client that negotiated named queues.

#### Scenario: A new queue is made

- **GIVEN** the Owner holds `Default` and `Queue 2`
- **WHEN** a Client asks for a new queue
- **THEN** the Owner holds a third, empty, resting queue named `Queue 3`

#### Scenario: The first new queue

- **GIVEN** the Owner holds only `Default`
- **WHEN** a Client asks for a new queue
- **THEN** the new queue is named `Queue 2`

#### Scenario: A freed number is used again

- **GIVEN** the Owner holds `Default` and `Queue 3`
- **WHEN** a Client asks for a new queue
- **THEN** the new queue is named `Queue 2`

#### Scenario: An empty name is refused

- **WHEN** a Client asks to rename a queue to a name of only spaces
- **THEN** the Owner rejects the request and the name does not change

### Requirement: A Client closes queues

A Client SHALL be able to ask the Owner to close a Named queue. The Owner SHALL refuse to close its last Named queue. Closing a resting queue SHALL delete it. Closing the current queue SHALL stop playback when it plays, delete the queue, and make the first remaining queue in queue order current, without playing it.

#### Scenario: Close a resting queue

- **WHEN** a Client closes the resting queue `Kids`
- **THEN** the Owner deletes `Kids` and playback does not change

#### Scenario: Close the playing queue

- **GIVEN** `Kids` is current and plays, and `Default` rests
- **WHEN** a Client closes `Kids`
- **THEN** playback stops, the Owner deletes `Kids`, and `Default` becomes current without playing

#### Scenario: The last queue does not close

- **GIVEN** the Owner holds only `Default`
- **WHEN** a Client asks to close it
- **THEN** the Owner rejects the request and `Default` remains

### Requirement: Queue edits name their queue

Every queue edit from a Client that negotiated named queues SHALL name the Named queue it edits. The Owner SHALL apply the edit to that queue under the same slot-addressed rules as an edit of the current queue. An edit that names a queue the Owner no longer holds SHALL be rejected without a change. An edit that names no queue SHALL apply to the current queue.

#### Scenario: Append to a resting queue

- **GIVEN** `Default` plays and `Kids` rests
- **WHEN** a Client appends an item to `Kids`
- **THEN** the item is the last slot of `Kids` and playback does not change

#### Scenario: An edit names a closed queue

- **WHEN** a Client removes a slot from a queue that another Client closed
- **THEN** the Owner rejects the edit and no queue changes

### Requirement: Clients without named queues see the current queue

A Client that did not negotiate named queues SHALL see only the current queue, through the existing queue snapshot, and its edits SHALL apply to the current queue. When the current queue changes, such a Client SHALL receive the new current queue as a whole-queue snapshot.

#### Scenario: The TUI follows the current queue

- **GIVEN** a TUI Client and a GUI Client are attached, and `Default` is current
- **WHEN** the GUI Client plays a row of `Kids`
- **THEN** the TUI Client shows `Kids` and its playback

### Requirement: Named queues are local-Owner only

Only the local Owner process SHALL offer named queues. A packaged `mbvd` SHALL keep one queue and SHALL NOT advertise named queues.

#### Scenario: A Client connects to mbvd

- **WHEN** a GUI Client connects to a packaged `mbvd`
- **THEN** it sees one queue for that peer and no create, rename, or close of peer queues
