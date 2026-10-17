# Spec Delta

## MODIFIED Requirements

### Requirement: A Player owner has exactly one active playback output

A Player owner SHALL have exactly one active playback output at any time: local playback, one Cast receiver, or one Emby Session. A Cast receiver and an Emby Session are remote receivers. The owner SHALL hold the queue, the active slot, and the play order for every output. An output SHALL play only the item of the active slot. An Emby Session SHALL also hold the item of the next slot, so that the device plays it in place of its own autoplay. An output SHALL never hold, advance, or reconcile a copy of the queue.

#### Scenario: Owner starts

- **WHEN** a Player owner starts
- **THEN** its active playback output is local playback

#### Scenario: A remote receiver is the output

- **WHEN** a remote receiver is the active playback output
- **THEN** local playback is not running
- **AND** the receiver has been given only the item of the active slot, and an Emby Session also the item of the next slot

#### Scenario: An Emby Session is the output

- **WHEN** the user plays a queue of several Emby items while an Emby Session is the active output
- **THEN** the owner gives the Session the item of the active slot and the item of the next slot
- **AND** never gives the Session more than two items while the owner runs

### Requirement: Only a natural end advances

A stop by the user on the device, a replacement by another sender, or an error SHALL NOT advance the queue. On an error the owner SHALL report the failure and SHALL NOT skip ahead. An Emby Session reports no end reason, so for an Emby Session the owner SHALL apply the inferred end rule and the inferred load failure of this capability; a stop by the user near the end of the item is then a natural end. When an Emby Session device moves by itself from the loaded item to the queued next item, the owner SHALL follow it to that slot, and SHALL record completion of the old slot only for a natural end.

#### Scenario: User presses next on the Emby Session device

- **WHEN** the user presses next on the device 20 minutes before the end of the loaded item, and the device starts the queued next item
- **THEN** the queued slot becomes the active slot
- **AND** the owner records no completion for the old slot

#### Scenario: User stops on the device

- **WHEN** the receiver reports that playback stopped because of a command that did not come from the owner
- **THEN** the owner does not advance and presents no active playback

#### Scenario: Receiver fails to play the item

- **WHEN** the receiver reports an error for the active item
- **THEN** the owner reports the failure
- **AND** does not load another slot

#### Scenario: Owner's own load replaces the item

- **WHEN** the owner loads a new slot and the receiver reports that the previous item was interrupted
- **THEN** the owner does not treat that report as an end of either item

#### Scenario: User stops an Emby Session early

- **WHEN** the user stops the loaded item on the Emby Session device more than 10 seconds before its end
- **THEN** the owner does not advance and presents no active playback

## ADDED Requirements

### Requirement: The owner infers a natural end on an Emby Session

The owner SHALL treat the loaded item as ended when the Emby Session stops presenting any item after it presented the loaded item, and the owner's estimated position is within 10 seconds of the item's runtime. The estimate SHALL be the last observed position plus the time since that observation while playing. The runtime SHALL be the queue item's own runtime, or the runtime that the Session reports when the queue item has none. Each loaded item SHALL advance at most once.

#### Scenario: Item plays to its end

- **WHEN** the Emby Session played the loaded item and goes idle while the estimated position is 3 seconds before the runtime
- **THEN** the owner records completion for the slot
- **AND** loads the next slot of its play order and the slot after it onto the Session

#### Scenario: Idle before the item starts

- **WHEN** the owner loaded an item and the Session is idle before it ever presented that item
- **THEN** the owner does not treat the idle Session as an end

#### Scenario: Same idle seen in two polls

- **WHEN** the owner already advanced from an ended item and a later observation shows the same idle Session
- **THEN** the owner does not advance again

#### Scenario: Session reports no runtime

- **WHEN** the Session reports a runtime of zero for the loaded item
- **THEN** the owner uses the queue item's runtime for the end rule

### Requirement: The owner infers a load failure on an Emby Session

When an Emby Session does not present the loaded item within 30 seconds after the owner loaded it, the owner SHALL report the failure by item name and SHALL NOT load another slot.

#### Scenario: Device never starts the item

- **WHEN** the owner loads an item and the Session does not present it within 30 seconds
- **THEN** the owner reports that the item did not start on the Session
- **AND** does not advance

### Requirement: An Emby Session plays only Emby items it can play

The owner SHALL load onto an Emby Session only an Emby item whose media kind the Session advertises as playable. A Session that advertises no playable media kinds SHALL count as able to play every Emby item. Any other slot cannot play on this output: the owner SHALL report it by name and reason, and SHALL step past it on an advance, limited by the queue length.

#### Scenario: Feed slot is next

- **WHEN** the owner picks the next slot for the Emby Session device and the slot after the active slot is a Feed entry followed by an Emby episode
- **THEN** the owner reports the Feed entry one time
- **AND** gives the Emby episode to the Session as the next item

#### Scenario: Explicit play of a slot the Session cannot play

- **WHEN** the user plays an Audiobookshelf slot while an Emby Session is the output
- **THEN** the owner reports the item by name and reason
- **AND** loads nothing onto the Session

#### Scenario: Video on an audio-only Session

- **WHEN** the next slot is an Emby video and the Session advertises only audio as playable
- **THEN** the owner reports the video and steps past it

### Requirement: The Emby Session device reports its own progress

While an Emby Session is the output, the owner SHALL NOT report playback start, progress, or stop to Emby for the loaded item. The device reports them. The owner SHALL still record completion and consume rules in its own queue.

#### Scenario: Item plays on a Session

- **WHEN** an Emby item plays on the Emby Session output
- **THEN** the owner sends no playback progress report to Emby for that item

### Requirement: The Emby Session output follows the device

The owner SHALL identify the Emby Session output by its Emby device identity, not by its session identity. When the Session of that device is missing from three observations in a row, the output SHALL stay the Emby Session in a disconnected state, with the queue and active slot unchanged. When a Session of the same device appears again, the owner SHALL attach to it.

#### Scenario: Device app restarts

- **WHEN** the device app restarts and Emby lists the device under a new session identity
- **THEN** the owner attaches to the new session
- **AND** the queue and the active slot are unchanged

#### Scenario: Device disappears

- **WHEN** the Session of the output device is missing from three observations in a row
- **THEN** every Client presents the Emby Session output as disconnected
- **AND** the owner does not advance

### Requirement: The Emby Session device holds the active item and the next item

Each load on an Emby Session SHALL give the device the active item and the first slot after it, in play order, that the Session can play. When the device moves from the loaded item to the queued next item by itself, the owner SHALL make that slot the active slot without a new load, SHALL record completion of the old slot only when it ended within 10 seconds of its runtime, and SHALL append the next playable slot to the device. A queue edit SHALL NOT interrupt the active item on the device.

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

- **WHEN** play order is Shuffle and the owner loads a slot onto an Emby Session
- **THEN** the device receives the active item and the current shuffle pick as the next item

#### Scenario: Queue grows after the last slot

- **WHEN** nothing is queued on the device and the user appends a slot to the queue
- **THEN** the owner appends that slot to the device without an interruption

### Requirement: Content the owner did not load is presented, not advanced

When an Emby Session presents an item that the owner did not load (neither the loaded item nor the queued next item), the owner SHALL present that item, its position, and its pause state to every Client and SHALL NOT advance, record completion, or change the active slot. Play, pause, seek, volume, mute, and track commands SHALL act on that item. Next and previous SHALL be sent to the device as its own next and previous commands.

#### Scenario: Another app starts an item on the device

- **WHEN** someone starts a different item on the device while an Emby Session is the output
- **THEN** every Client presents that item with its title and position
- **AND** the owner's active slot is unchanged

#### Scenario: Next on foreign content

- **WHEN** the user presses next while the Session plays an item the owner did not load
- **THEN** the device receives its own next command
- **AND** the owner loads nothing

### Requirement: The owner hands the rest of the queue to the Emby Session at exit

When the owner exits while an Emby Session is the output and the device plays the loaded item, the owner SHALL give the device every playable slot after the queued next item, in play order, so that the device plays the rest of the queue without the owner. The active item SHALL play on, unless a queue edit made the queued next item stale. A stay-alive owner that outlives its Clients SHALL hand over nothing.

#### Scenario: Owner exits during a queue

- **WHEN** the owner exits while the device plays slot 3 of 10 and slot 4 is queued next on the device
- **THEN** the device receives slots 5 to 10 appended after slot 4
- **AND** slot 3 plays on without an interruption

#### Scenario: Owner exits with Shuffle on

- **WHEN** the owner exits while play order is Shuffle and the device holds the active item and the current shuffle pick
- **THEN** the owner sends nothing more to the device

#### Scenario: Client quits while the owner stays alive

- **WHEN** a Client quits and the stay-alive owner keeps running with an Emby Session output
- **THEN** the owner hands over nothing and keeps advancing the queue

#### Scenario: Owner exits while the device plays other content

- **WHEN** the owner exits while the device plays an item that the owner did not load
- **THEN** the owner sends nothing to the device

### Requirement: Launch never selects an Emby Session output

A Client SHALL NOT select an Emby Session as the output at launch. Launch auto-reconnect SHALL restore only a Library route or Direct remote control. A stay-alive owner SHALL keep its current output across Client restarts.

#### Scenario: Client restarts while a Session is the output

- **WHEN** a Client exits while the stay-alive owner's output is an Emby Session, and a new Client starts
- **THEN** the new Client presents the Emby Session as the active output without a new selection

#### Scenario: Saved record names a plain Session

- **WHEN** a Client starts and its saved auto-reconnect record names a device that offers no Direct remote control
- **THEN** the Client does not select that device's Session as the output
