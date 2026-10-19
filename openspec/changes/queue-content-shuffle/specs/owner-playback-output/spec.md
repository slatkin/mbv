# Spec Delta

## MODIFIED Requirements

### Requirement: The Emby Session device holds the active item and the next item

Each load on an Emby Session SHALL give the device the active item and the first slot after it, in queue order, that the Session can play. When the device moves from the loaded item to the queued next item by itself, the owner SHALL make that slot the active slot without a new load, SHALL record completion of the old slot only when it ended within 10 seconds of its runtime, and SHALL append the next playable slot to the device. A queue edit SHALL NOT interrupt the active item on the device.

#### Scenario: Device moves to the queued next item

- **WHEN** the loaded episode ends on the device and the device starts the queued next item
- **THEN** the owner records completion for the old slot
- **AND** the queued slot becomes the active slot with no new load on the device
- **AND** the owner appends the slot after it to the device

#### Scenario: Device does not autoplay its own next episode

- **WHEN** the loaded episode ends on the device and the queue has a next slot
- **THEN** the device plays the item of the next slot, not the next episode of the series

#### Scenario: Last slot of the queue

- **WHEN** the owner loads the last playable slot of the queue on an Emby Session
- **THEN** the device receives only that item

#### Scenario: Queue edit removes the queued next slot

- **WHEN** the user removes the slot that is queued next on the device while the active item plays
- **THEN** the active item plays on without an interruption
- **AND** when the device starts the removed item, the owner loads the new next slot in its place

#### Scenario: Queue edit removes the only next slot

- **WHEN** the user removes the slot that is queued next on the device and the queue has no other slot after the active slot
- **THEN** the active item plays on without an interruption
- **AND** when the device starts the removed item, the owner stops the device and the queue ends on the old slot

#### Scenario: Shuffle is on

- **WHEN** the user chose Shuffle on the Queue list and the owner loads a slot onto an Emby Session
- **THEN** the device receives the active item and the first playable slot after it in the shuffled queue as the next item

#### Scenario: Queue Shuffle changes the queued next slot

- **WHEN** the user chooses Shuffle on the Queue list while the active item plays and a different slot is now first after it
- **THEN** the active item plays on without an interruption
- **AND** when the device starts the old queued item, the owner loads the new next slot in its place

#### Scenario: Queue grows after the last slot

- **WHEN** nothing is queued on the device and the user appends a slot to the queue
- **THEN** the owner appends that slot to the device without an interruption

### Requirement: The owner hands the rest of the queue to the Emby Session at exit

When the owner exits while an Emby Session is the output and the device plays the loaded item, the owner SHALL give the device every playable slot after the queued next item, in queue order, so that the device plays the rest of the queue without the owner. The active item SHALL play on, unless a queue edit made the queued next item stale. A stay-alive owner that outlives its Clients SHALL hand over nothing.

#### Scenario: Owner exits during a queue

- **WHEN** the owner exits while the device plays slot 3 of 10 and slot 4 is queued next on the device
- **THEN** the device receives slots 5 to 10 appended after slot 4
- **AND** slot 3 plays on without an interruption

#### Scenario: Owner exits with Shuffle on

- **WHEN** the user chose Shuffle on the Queue list and the owner exits while the device holds the active item and the queued next item
- **THEN** the device receives every playable slot after the queued next item, in the shuffled queue order

#### Scenario: Client quits while the owner stays alive

- **WHEN** a Client quits and the stay-alive owner keeps running with an Emby Session output
- **THEN** the owner hands over nothing and keeps advancing the queue

#### Scenario: Owner exits while the device plays other content

- **WHEN** the owner exits while the device plays an item that the owner did not load
- **THEN** the owner sends nothing to the device
