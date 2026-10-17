# owner-playback-output Specification

## Purpose
Defines the single place where a Player owner plays its active queue slot: local playback or one remote receiver. It also defines how the owner selects that output, hands playback between outputs, and advances the queue from the state an output reports.

## Requirements

### Requirement: A Player owner has exactly one active playback output

A Player owner SHALL have exactly one active playback output at any time: local playback, or one remote receiver. The owner SHALL hold the queue, the active slot, and the play order for every output. An output SHALL play only the item of the active slot. An output SHALL never hold, advance, or reconcile a copy of the queue.

#### Scenario: Owner starts

- **WHEN** a Player owner starts
- **THEN** its active playback output is local playback

#### Scenario: A remote receiver is the output

- **WHEN** a remote receiver is the active playback output
- **THEN** local playback is not running
- **AND** the receiver has been given only the item of the active slot

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

A stop by the user on the device, a replacement by another sender, or an error SHALL NOT advance the queue. On an error the owner SHALL report the failure and SHALL NOT skip ahead.

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
