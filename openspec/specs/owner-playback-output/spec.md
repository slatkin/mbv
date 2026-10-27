# owner-playback-output Specification

## Purpose
Defines the single place where a Player owner plays its active queue slot: local playback or one remote receiver. It also defines how the owner selects that output, hands playback between outputs, and advances the queue from the state an output reports.

## Requirements

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

### Requirement: Every Client shares the active output

The owner SHALL broadcast its active playback output to every connected Client, and a newly connected Client SHALL receive it on connect. A playback command from any Client SHALL act on the active output. A Client SHALL NOT keep a playback output of its own.

#### Scenario: One Client selects a receiver

- **WHEN** one Client selects a remote receiver while a second Client is connected
- **THEN** both Clients present the receiver as the active output
- **AND** play, pause, next, previous, and stop from either Client act on that receiver

#### Scenario: Client connects to an owner that is casting

- **WHEN** a Client connects to an owner whose active output is a remote receiver
- **THEN** the Client presents that receiver as the active output without a new selection

### Requirement: Selecting an output hands off the active slot

When the active output changes, the owner SHALL stop the old output and continue the active slot on the new output at its current position, with the same paused or playing state. A slot is active only while the old output plays or pauses it. When no slot is active (for example, after a stop), the new output SHALL become active without playing anything.

#### Scenario: Local playback moves to a receiver

- **WHEN** local playback plays a slot at a position and the user selects a remote receiver
- **THEN** local playback stops
- **AND** the receiver plays the same slot from that position

#### Scenario: Paused playback moves

- **WHEN** the active slot is paused and the user selects a different output
- **THEN** the new output holds the same slot at the same position, paused

#### Scenario: Receiver playback returns to local

- **WHEN** a remote receiver plays the active slot and the user selects local playback
- **THEN** the receiver stops
- **AND** local playback continues the same slot from the receiver's position

#### Scenario: Nothing is active

- **WHEN** no slot is active and the user selects a different output
- **THEN** the new output becomes active and nothing starts playing

#### Scenario: Active slot cannot play on the new output

- **WHEN** the user selects an output that cannot play the item of the active slot
- **THEN** the owner reports the item by name and reason
- **AND** the new output becomes active without playing the item

### Requirement: The owner advances on a natural end

When the active output reports that the item of the active slot played to its natural end, the owner SHALL record completion for that slot and SHALL load the next slot from its own play order onto the same output. At the end of the queue, the output SHALL stay idle.

#### Scenario: Item ends on a receiver

- **WHEN** a remote receiver reports that the active item finished
- **THEN** the owner moves the active slot to the next slot of its play order
- **AND** loads only that slot onto the receiver

#### Scenario: Last item ends

- **WHEN** the active output reports that the last slot finished
- **THEN** the owner loads nothing and the output stays idle

#### Scenario: Queue is edited before the end

- **WHEN** the user reorders or removes upcoming slots while a receiver plays
- **THEN** the next advance follows the edited queue

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

### Requirement: Content the owner did not load is presented, not advanced

When an Emby Session presents an item that the owner did not load (neither the loaded item nor the queued next item) and the device did not begin it as the queue's end continuation of the tracked item, the owner SHALL present that item, its position, and its pause state to every Client and SHALL NOT advance, record completion, or change the active slot. Play, pause, seek, volume, mute, and track commands SHALL act on that item. Next and previous SHALL be sent to the device as its own next and previous commands. The queue's end continuation of the tracked item is adopted as the next queue slot instead.

#### Scenario: Another app starts an item on the device

- **WHEN** someone starts a different item on the device while an Emby Session is the output
- **THEN** every Client presents that item with its title and position
- **AND** the owner's active slot is unchanged

#### Scenario: Next on foreign content

- **WHEN** the user presses next while the Session plays an item the owner did not load
- **THEN** the device receives its own next command
- **AND** the owner loads nothing

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

### Requirement: Launch never selects an Emby Session output

A Client SHALL NOT select an Emby Session as the output at launch. Launch auto-reconnect SHALL restore only a Library route or Direct remote control. A stay-alive owner SHALL keep its current output across Client restarts.

#### Scenario: Client restarts while a Session is the output

- **WHEN** a Client exits while the stay-alive owner's output is an Emby Session, and a new Client starts
- **THEN** the new Client presents the Emby Session as the active output without a new selection

#### Scenario: Saved record names a plain Session

- **WHEN** a Client starts and its saved auto-reconnect record names a device that offers no Direct remote control
- **THEN** the Client does not select that device's Session as the output

### Requirement: Next and previous load the target slot directly

Next and previous on a remote output SHALL be resolved by the owner's relative step over its own play order. The owner SHALL load only the target slot onto the output. At an edge of the queue the command SHALL do nothing.

#### Scenario: Next on a receiver

- **WHEN** the user presses next while a receiver plays a slot that has a successor
- **THEN** the owner loads only the successor onto the receiver

#### Scenario: Previous at the first slot

- **WHEN** the user presses previous on the first slot of a sequential queue
- **THEN** nothing is loaded

### Requirement: The packaged owner has only local playback

Packaged `mbvd` SHALL have local playback as its only output. Its runtime setup SHALL have no way to express a remote output: no Cast receiver attachment and no Emby Session output can be configured for the packaged role, so the exclusion does not rest on a value that the packaged process could set differently. Packaged `mbvd` SHALL NOT advertise a remote-output ctrl capability, a Client connected to it SHALL NOT offer a remote output, and it SHALL reject a request to select one with a stated reason while local playback stays the output.

#### Scenario: Client connected to mbvd

- **WHEN** a Client is connected to a packaged `mbvd` owner
- **THEN** it does not offer a Cast receiver or an Emby Session as an output

#### Scenario: Request reaches mbvd

- **WHEN** a request to select a Cast receiver or an Emby Session reaches packaged `mbvd`
- **THEN** `mbvd` rejects it with a stated reason and keeps local playback as the output

#### Scenario: Packaged hello carries no remote-output capability

- **WHEN** packaged `mbvd` greets a ctrl Client
- **THEN** its hello carries neither the Cast playback-output capability nor the Emby Session output capability

#### Scenario: Packaged runtime setup has no remote-output choice

- **WHEN** the packaged daemon is started
- **THEN** its runtime setup cannot be given a Cast receiver attachment or an Emby Session output in any build of the packaged program

### Requirement: The device's own continuation at the queue's end becomes the next queue slot

When the tracked item on an Emby Session ends within 10 seconds of its runtime and the device holds no queued next item, the owner SHALL adopt the next item the device starts and the owner did not load: it SHALL append that item at the end of its queue, make it the active slot, and track it. The owner SHALL adopt an item only when the Session first presents it within 30 seconds of that end, and SHALL record the ended slot's completion once.

#### Scenario: The device autoplays its next episode at the queue's end

- **WHEN** the queue's last slot plays to its end and the device starts the next episode of its own
- **THEN** the owner appends that episode as a new slot at the end of the queue
- **AND** the new slot becomes the active slot
- **AND** the ended slot records completion under the same rule as a device move to the queued next item

#### Scenario: The device autoplays after an idle gap

- **WHEN** the queue's last slot plays to its end, the Session is idle for one observation, and within 30 seconds the device starts an item of its own
- **THEN** the owner appends that item as the active slot as in the direct case
- **AND** the ended slot has recorded completion once

#### Scenario: A late foreign item is not adopted

- **WHEN** the Session first presents an item the owner did not load more than 30 seconds after the tracked item's end
- **THEN** the owner presents that item and appends nothing to the queue

#### Scenario: The item lookup fails

- **WHEN** the device starts an item that meets the adoption rule and the owner's lookup of that item fails
- **THEN** the owner presents that item, appends nothing to the queue, and logs the failure

#### Scenario: Adoption repeats

- **WHEN** an adopted item reaches its end and the device starts another item of its own
- **THEN** that item is appended to the queue and tracked the same way

#### Scenario: The adopted item is tracked like a loaded item

- **WHEN** an adopted item plays on the device
- **THEN** the owner records its position from the Session's own reports and records completion when it ends
- **AND** when the device then goes idle, the owner loads nothing and the output stays idle

#### Scenario: A stale queued item is corrected, not adopted

- **WHEN** the device starts the stale queued item after a queue edit removed it
- **THEN** the owner corrects the device as before and appends nothing to the queue

#### Scenario: Next on an adopted item follows the queue

- **WHEN** the user presses next while the device plays an adopted item that is the queue's last slot
- **THEN** the owner loads nothing and the device receives no next command
