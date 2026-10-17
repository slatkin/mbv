# owner-playback-output Specification

## Purpose
Defines the single place where a Player owner plays its active queue slot: local playback or one remote receiver. It also defines how the owner selects that output, hands playback between outputs, and advances the queue from the state an output reports.

## Requirements

### Requirement: A Player owner has exactly one active playback output

A Player owner SHALL have exactly one active playback output at any time: local playback, one Cast receiver, or one Emby Session. A Cast receiver and an Emby Session are remote receivers. The owner SHALL hold the queue, the active slot, and the play order for every output. An output SHALL play only the item of the active slot. An output SHALL never hold, advance, or reconcile a copy of the queue.

#### Scenario: Owner starts

- **WHEN** a Player owner starts
- **THEN** its active playback output is local playback

#### Scenario: A remote receiver is the output

- **WHEN** a remote receiver is the active playback output
- **THEN** local playback is not running
- **AND** the receiver has been given only the item of the active slot

#### Scenario: An Emby Session is the output

- **WHEN** the user plays a queue of several Emby items while an Emby Session is the active output
- **THEN** the owner gives the Session only the item of the active slot
- **AND** never gives the Session a list of items

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

A stop by the user on the device, a replacement by another sender, or an error SHALL NOT advance the queue. On an error the owner SHALL report the failure and SHALL NOT skip ahead. An Emby Session reports no end reason, so for an Emby Session the owner SHALL apply the inferred end rule and the inferred load failure of this capability; a stop by the user near the end of the item is then a natural end.

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
- **AND** loads only the next slot of its play order onto the Session

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

- **WHEN** an Emby Session item ends and the next slot is a Feed entry followed by an Emby episode
- **THEN** the owner reports the Feed entry
- **AND** loads the Emby episode onto the Session

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

### Requirement: Content the owner did not load is presented, not advanced

When an Emby Session presents an item that the owner did not load, the owner SHALL present that item, its position, and its pause state to every Client and SHALL NOT advance, record completion, or change the active slot. Play, pause, seek, volume, mute, and track commands SHALL act on that item. Next and previous SHALL be sent to the device as its own next and previous commands.

#### Scenario: Another app starts an item on the device

- **WHEN** someone starts a different item on the device while an Emby Session is the output
- **THEN** every Client presents that item with its title and position
- **AND** the owner's active slot is unchanged

#### Scenario: Next on foreign content

- **WHEN** the user presses next while the Session plays an item the owner did not load
- **THEN** the device receives its own next command
- **AND** the owner loads nothing

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

Packaged `mbvd` SHALL have local playback as its only output. It SHALL NOT offer a remote receiver output, and it SHALL reject a request to select one with a stated reason.

#### Scenario: Client connected to mbvd

- **WHEN** a Client is connected to a packaged `mbvd` owner
- **THEN** it does not offer a remote receiver as an output

#### Scenario: Request reaches mbvd

- **WHEN** a request to select a remote receiver reaches packaged `mbvd`
- **THEN** `mbvd` rejects it with a reason and keeps local playback as the output
