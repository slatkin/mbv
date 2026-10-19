# Spec Delta

## Purpose

Defines the Shuffle action of the Queue list. The action reorders the contents of the queue one time, at random, and keeps the playing item first so that playback is not interrupted. The Player owner has no separate playback order.

## ADDED Requirements

### Requirement: The Queue list menu offers Shuffle
The context menu of every Queue row SHALL offer one entry named Shuffle, whatever the media kind of the row. The entry SHALL be the same on every Queue row and SHALL NOT change with the state of the queue. No menu outside the Queue list SHALL offer this entry.

#### Scenario: Queue row of any kind
- **WHEN** the user opens the context menu on an Emby, Feed, or Audiobookshelf Queue row
- **THEN** the menu offers Shuffle

#### Scenario: Library row
- **WHEN** the user opens the context menu on a library row
- **THEN** the menu does not offer the Queue list Shuffle entry

#### Scenario: Owner without the capability
- **WHEN** the Player owner does not advertise the queue-shuffle capability
- **THEN** the Queue row menu does not offer Shuffle

### Requirement: Shuffle reorders the queue and keeps the playing item first
Choosing Shuffle SHALL reorder the slots of the queue at random as one queue edit. When a slot is playing, that slot SHALL move to the first position and SHALL keep playing, and the other slots SHALL follow it in random order. When no slot is playing, all slots SHALL be reordered at random. Shuffle SHALL NOT add, remove, or change a slot, and SHALL NOT change which slot is active.

#### Scenario: Shuffle with a playing item
- **WHEN** the queue has slots A, B, C, D, slot C plays, and the user chooses Shuffle
- **THEN** the queue starts with C
- **AND** A, B, and D follow C in a random order
- **AND** C plays on without an interruption

#### Scenario: Shuffle with nothing playing
- **WHEN** the queue has slots A, B, C, D, no slot plays, and the user chooses Shuffle
- **THEN** the queue holds A, B, C, and D in a random order

#### Scenario: Playing item already first
- **WHEN** slot A plays at the first position and the user chooses Shuffle
- **THEN** the queue still starts with A and the other slots follow it in a random order

#### Scenario: Queue with one slot
- **WHEN** the queue has one slot and the user chooses Shuffle
- **THEN** the queue does not change

### Requirement: Shuffle is one queue edit
The reorder SHALL reach every Client as one queue snapshot, so that no Client sees an intermediate order. The reorder SHALL reach the playback run, and the media player playlist, as one reorder. After the reorder, a natural end, the Next command, the Previous command, and the Next Up card SHALL all follow the new order.

#### Scenario: Client sees one snapshot
- **WHEN** the user chooses Shuffle for a queue of ten slots
- **THEN** every connected Client receives one snapshot that holds the new order and no snapshot with a partial order

#### Scenario: Advance follows the new order
- **WHEN** the user chooses Shuffle while slot C plays and the new order is C, D, A, B
- **THEN** playback goes to D when C ends
- **AND** the Next Up card names D

#### Scenario: Previous follows the new order
- **WHEN** the user chooses Shuffle while slot C plays and then sends Previous
- **THEN** the active slot does not change, because C is first in the queue

### Requirement: Shuffle persists with the queue
The shuffled order SHALL be the order of the queue from then on. The owner SHALL persist it as it persists any other queue edit, and a Client that connects later SHALL read the shuffled order. A later queue edit, such as append, remove, or move, SHALL act on the shuffled order.

#### Scenario: Late Client
- **WHEN** the user chooses Shuffle and a new Client connects afterward
- **THEN** the new Client shows the shuffled order

#### Scenario: Append after Shuffle
- **WHEN** the user chooses Shuffle and then adds an item to the queue
- **THEN** the new slot is at the end of the shuffled order

### Requirement: Shuffle has its own ctrl command and capability
Shuffle SHALL reach the Player owner as one ctrl command. The owner SHALL advertise a queue-shuffle capability in its hello, and a Client SHALL send the command only to an owner that advertises it. The capability SHALL be additive: the ctrl protocol version SHALL NOT change. Like other queue edits, the command SHALL carry an optional queue operation id, and the owner SHALL answer it with the result of the edit.

#### Scenario: Old owner
- **WHEN** a Client connects to an owner built before this capability
- **THEN** the Client does not offer Shuffle and sends no shuffle command

#### Scenario: Operation result
- **WHEN** a Client sends the shuffle command with an operation id
- **THEN** the owner answers that id with the applied queue state
