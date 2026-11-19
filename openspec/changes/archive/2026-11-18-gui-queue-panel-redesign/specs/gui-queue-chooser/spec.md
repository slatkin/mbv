# Spec Delta

## MODIFIED Requirements

### Requirement: The queue header holds the chooser and its controls

The GUI queue header SHALL show, in this order from its leading edge: a queue label, the Queue chooser, the control that makes a new queue, and the control that closes the shown entry. The Queue filter control SHALL sit at the header's trailing edge. The Queue chooser SHALL be a dropdown list, not a row of tabs. The chooser SHALL show at all times, also while the list holds exactly one entry. It SHALL show the name of the shown entry, or `Queue` while that name is empty, and the shown entry's slot count. The header SHALL show nothing else.

#### Scenario: One queue and no peer

- **GIVEN** the local Owner holds only `Default` with 26 slots and the GUI Client is connected to no peer
- **WHEN** the queue header paints
- **THEN** it shows the queue label, the chooser with `Default` and the count 26, the new-queue control, the disabled close control, and the Queue filter control at the trailing edge
- **AND** a press on the chooser opens the dropdown with the one entry

#### Scenario: Several entries

- **GIVEN** the local Owner holds `Default` and `Kids`, and `Kids` is shown
- **WHEN** the queue header paints
- **THEN** it shows the queue label, the chooser with `Kids` and its slot count, the new-queue control, the close control, and the Queue filter control at the trailing edge

#### Scenario: The filter narrows the rows

- **GIVEN** the Queue filter narrows the shown queue
- **WHEN** the queue header paints
- **THEN** the chooser's count still counts every slot of the shown queue

### Requirement: The chooser lists local queues and connected peers

The Queue chooser SHALL list every Named queue of the local Owner in queue order, then one entry for each peer Player owner the GUI Client is connected to, named by the peer's device name. Each entry SHALL show its queue's slot count. The list SHALL mark the shown entry and the current queue, and SHALL mark a peer entry as a peer. While the local Owner plays the current queue, its entry SHALL carry a playing mark in place of the current mark; a stopped or paused current queue keeps the current mark. Choosing an entry SHALL show it and SHALL NOT start, stop, or move playback.

#### Scenario: Choose a resting queue

- **GIVEN** `Default` plays
- **WHEN** the user chooses `Kids` in the chooser
- **THEN** the Queue panel shows `Kids` and `Default` keeps playing

#### Scenario: The current queue is marked while it plays

- **GIVEN** `Default` is the current queue and plays, and `Kids` is shown
- **WHEN** the user opens the chooser
- **THEN** `Default` carries the playing mark, `Kids` carries the shown mark, and no entry carries both the playing mark and the current mark

#### Scenario: Each entry shows its slot count

- **GIVEN** `Default` holds 26 slots and plays, `Kids` holds 4 slots and rests, and the GUI Client is connected to a peer whose queue holds 9 slots
- **WHEN** the user opens the chooser
- **THEN** `Default` shows 26, `Kids` shows 4, and the peer entry shows 9

#### Scenario: The current queue's count follows edits

- **GIVEN** `Default` is the current queue with 26 slots
- **WHEN** the user removes one slot from `Default` and opens the chooser
- **THEN** `Default` shows 25
