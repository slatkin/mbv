# queue-play-order Specification

## Purpose

Defines how the Player owner orders playback of a Bound queue. The owner can play the queue in Shuffle order, one shared pick drives every forward advance, Previous is disabled in Shuffle, and a whole-queue replacement resets the order to Sequential.

## Requirements

### Requirement: The Player owner holds the playback order
The Player owner SHALL hold a playback order for its Bound queue. The order SHALL be Sequential or Shuffle. A new Player owner SHALL start in Sequential. The order SHALL NOT change the order of the queue slots. Every Client of the owner SHALL see the same order, and the order SHALL apply when no Client is connected.

#### Scenario: Queue order is kept
- **WHEN** the user turns on Shuffle for a queue of slots A, B, C, D
- **THEN** the Queue list still shows A, B, C, D in that order

#### Scenario: Order applies without a Client
- **WHEN** Shuffle is on and the last Client disconnects from a Stay-alive owner
- **THEN** the owner continues to advance in Shuffle order

### Requirement: The status payload carries the playback order
The ctrl status payload SHALL carry the playback order in one place. `PlayerStatus` SHALL carry a `play_order` field with a serde default of Sequential, so a payload from an owner built before the field decodes as Sequential without a protocol version change. `UnifiedQueueStateData` SHALL carry the order only through its `status` field and SHALL NOT repeat the order at its own level. Every Client SHALL read the same order mode from that one value.

#### Scenario: Status payload without the field
- **WHEN** a Client decodes a status payload that has no `play_order` field
- **THEN** the order reads as Sequential

### Requirement: Shuffle picks a random slot other than the active slot
In Shuffle, each advance SHALL go to a slot picked at random from all queue slots except the active slot. A slot that already played SHALL be a valid pick. If the queue has no slot other than the active slot, the advance SHALL behave as the end of the queue does in Sequential.

#### Scenario: End of file in Shuffle
- **WHEN** Shuffle is on, the queue has slots A, B, C, D, and B finishes naturally
- **THEN** playback continues with one of A, C, or D, and never with B

#### Scenario: Played slots can play again
- **WHEN** Shuffle is on and every slot in the queue has played one time
- **THEN** playback continues with another random slot other than the active slot

#### Scenario: Only one slot
- **WHEN** Shuffle is on and the active slot is the only slot in the queue
- **THEN** playback stops at the end of that slot, as at the end of a Sequential queue

#### Scenario: Consume empties the queue
- **WHEN** Shuffle is on, consume removes each finished slot, and the last remaining slot finishes
- **THEN** playback stops

### Requirement: One pick drives every forward advance
In Shuffle, the end-of-file advance, the Next command, and the Next Up card SHALL use the same next-slot pick. The Next Up card SHALL name the slot that will really play next. A queue edit that removes the picked slot SHALL cause a new pick.

#### Scenario: Next command in Shuffle
- **WHEN** Shuffle is on and the user sends Next
- **THEN** playback jumps to a random slot other than the active slot

#### Scenario: Next Up names the real next slot
- **WHEN** Shuffle is on and the Next Up card shows for the active episode
- **THEN** the card names the episode that playback goes to when the active episode ends

#### Scenario: Picked slot is removed
- **WHEN** Shuffle is on and the user removes the slot that was picked to play next
- **THEN** a new slot other than the active slot is picked

### Requirement: Previous is disabled in Shuffle
In Shuffle, the Previous command SHALL have no effect on the active slot. MPRIS SHALL report that the player cannot go to the previous item while Shuffle is on. In Sequential, Previous SHALL behave as before.

#### Scenario: Previous in Shuffle
- **WHEN** Shuffle is on and the user sends Previous
- **THEN** the active slot does not change

#### Scenario: MPRIS previous capability
- **WHEN** Shuffle is on
- **THEN** MPRIS reports that the player cannot go to the previous item

### Requirement: Replacing the queue resets the order
A replacement of the whole Bound queue SHALL set the order to Sequential. Appending, removing, and moving slots SHALL keep the current order.

#### Scenario: Queue replacement
- **WHEN** Shuffle is on and the user plays a new folder that replaces the queue
- **THEN** the order is Sequential

#### Scenario: Append keeps Shuffle
- **WHEN** Shuffle is on and the user adds an item to the queue
- **THEN** the order stays Shuffle and the new slot is a valid pick

### Requirement: The Queue list menu toggles Shuffle
The context menu of every Queue row SHALL offer one shuffle-order entry, whatever the media kind of the row. When the order is Sequential, the entry SHALL turn Shuffle on. When the order is Shuffle, the entry SHALL turn Shuffle off. Choosing the entry SHALL NOT change the active slot or the queue order. No menu outside the Queue list SHALL offer this entry.

#### Scenario: Turn Shuffle on
- **WHEN** the order is Sequential and the user chooses the shuffle-order entry on a Queue row
- **THEN** the order becomes Shuffle and the active slot continues to play

#### Scenario: Turn Shuffle off
- **WHEN** the order is Shuffle and the user chooses the shuffle-order entry on a Queue row
- **THEN** the order becomes Sequential and the next advance goes to the slot after the active slot

#### Scenario: Non-Emby Queue row
- **WHEN** the user opens the context menu on a Feed or Audiobookshelf Queue row
- **THEN** the menu offers the shuffle-order entry

#### Scenario: Library row
- **WHEN** the user opens the context menu on a library row
- **THEN** the menu does not offer the shuffle-order entry

### Requirement: The footer shows Shuffle
While the order is Shuffle, the footer SHALL show a shuffle indicator, whatever panel has focus. While the order is Sequential, the footer SHALL NOT show it.

#### Scenario: Indicator appears
- **WHEN** the user turns on Shuffle
- **THEN** the footer shows the shuffle indicator

#### Scenario: Indicator clears on replacement
- **WHEN** Shuffle is on and the queue is replaced
- **THEN** the footer no longer shows the shuffle indicator

### Requirement: MPRIS reports the shuffle state
MPRIS SHALL report Shuffle as true while the order is Shuffle and false while it is Sequential. A request from an MPRIS client to change the shuffle state is out of scope and SHALL NOT change the order.

#### Scenario: MPRIS reads Shuffle
- **WHEN** the user turns on Shuffle
- **THEN** MPRIS reports Shuffle as true
