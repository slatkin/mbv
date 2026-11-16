# Spec Delta

## Purpose

The GUI Client's Queue chooser picks which queue the Queue panel shows: one of the local Owner's
Named queues or the queue of a connected peer Player owner. This capability defines the chooser,
its controls, and what follows the shown queue.

## ADDED Requirements

### Requirement: The queue header holds the chooser and its controls

The GUI queue header SHALL show the Queue chooser, the Queue filter control, a control that makes a new queue, and a control that closes the shown entry. The Queue chooser SHALL be a dropdown list, not a row of tabs. It SHALL show the name of the shown entry and SHALL be absent while the list holds exactly one entry. The header SHALL show nothing else.

#### Scenario: One queue and no peer

- **GIVEN** the local Owner holds only `Default` and the GUI Client is connected to no peer
- **WHEN** the queue header paints
- **THEN** it shows the Queue filter control and the new-queue control, and no chooser

#### Scenario: Several entries

- **GIVEN** the local Owner holds `Default` and `Kids`
- **WHEN** the queue header paints
- **THEN** it shows the chooser with the name of the shown queue, the Queue filter control, the new-queue control, and the close control

### Requirement: The chooser lists local queues and connected peers

The Queue chooser SHALL list every Named queue of the local Owner in queue order, then one entry for each peer Player owner the GUI Client is connected to, named by the peer's device name. The list SHALL mark the shown entry and the current queue, and SHALL mark a peer entry as a peer. Choosing an entry SHALL show it and SHALL NOT start, stop, or move playback.

#### Scenario: Choose a resting queue

- **GIVEN** `Default` plays
- **WHEN** the user chooses `Kids` in the chooser
- **THEN** the Queue panel shows `Kids` and `Default` keeps playing

### Requirement: A connect adds a peer entry

A connect to a peer Player owner from the playback-target chooser SHALL add that peer's entry to the Queue chooser and SHALL show it. A disconnect SHALL remove the entry and SHALL show the current local queue when the peer entry was shown. A lost connection SHALL remove the entry in the same way.

#### Scenario: Connect to mbvd

- **WHEN** the user picks a direct peer in the playback-target chooser
- **THEN** the Queue chooser lists the peer and the Queue panel shows the peer queue

#### Scenario: The connection is lost

- **GIVEN** the peer entry is shown
- **WHEN** the connection to the peer is lost
- **THEN** the peer entry leaves the chooser and the Queue panel shows the current local queue

### Requirement: The player panel follows the shown entry's owner

While a local Named queue is shown, the player panel (artwork, title, scrubber, transport, and track chips) SHALL show and control the local Owner's playback. While a peer entry is shown, it SHALL show and control that peer's playback. The transport SHALL control the current queue of the owner it shows, whichever of that owner's queues is shown.

#### Scenario: A resting queue is shown while another plays

- **GIVEN** `Default` plays and the GUI Client shows `Kids`
- **WHEN** the user presses pause
- **THEN** the playback of `Default` pauses

#### Scenario: A peer entry is shown

- **GIVEN** the GUI Client is connected to a peer that plays
- **WHEN** the user shows the peer entry
- **THEN** the player panel shows the peer's item and position, and its transport controls the peer

### Requirement: Play with nothing playing starts the shown queue

While the local Owner plays nothing and a local Named queue is shown, Play SHALL ask the Owner to play the shown queue. Showing a queue SHALL NOT start playback. Activating a row SHALL ask the Owner to play that slot of the shown queue.

#### Scenario: Play a stopped queue

- **GIVEN** nothing plays and the GUI Client shows `Kids`, whose start slot is its third slot
- **WHEN** the user presses Play
- **THEN** `Kids` becomes current and its third slot plays

### Requirement: Edits and enqueue reach the shown queue

Every queue edit, enqueue, play-next, and whole-queue replacement from the GUI Client SHALL reach the shown entry: the shown local Named queue, or the shown peer's queue. A replacement of a resting local queue SHALL make it current and play it, as a replacement does today. The confirmation before a populated queue is replaced SHALL apply to the shown queue. An undo SHALL send its inverse edit to the queue that the undone edit reached, whichever entry is shown.

#### Scenario: Enqueue to a resting queue

- **GIVEN** `Default` plays and the GUI Client shows `Kids`
- **WHEN** the user adds an album from the Library panel
- **THEN** the album's tracks are appended to `Kids` and `Default` keeps playing

#### Scenario: Undo reaches the queue it edited

- **GIVEN** the user removed a row from `Kids` and then showed `Default`
- **WHEN** the user undoes the removal
- **THEN** the row returns to `Kids` and `Default` does not change

### Requirement: The new-queue control makes and shows a queue

The new-queue control SHALL ask the local Owner for a new Named queue, SHALL show it when the Owner reports it, and SHALL open its name for edit. Confirming the edit SHALL rename the queue; cancelling it SHALL keep the given name. The user SHALL also be able to rename any local Named queue from the Queue chooser. A peer entry SHALL NOT be renamed.

#### Scenario: Make a queue

- **WHEN** the user presses the new-queue control
- **THEN** the Queue panel shows a new empty queue and its name is open for edit

### Requirement: The close control closes the shown entry

The close control SHALL act on the shown entry. It SHALL ask for confirmation only on the current queue while it plays, naming the queue and saying that playback stops. Every other local queue SHALL close with no confirmation, and a peer entry SHALL disconnect. The control SHALL be disabled on the only local queue. After a close the Queue panel SHALL show the current local queue.

#### Scenario: Close the playing queue

- **GIVEN** `Kids` is current and plays
- **WHEN** the user presses the close control and confirms
- **THEN** playback stops, `Kids` is gone, and the Queue panel shows the new current queue

#### Scenario: Close the stopped current queue

- **GIVEN** `Kids` is current and stopped, and `Default` rests
- **WHEN** the user presses the close control
- **THEN** `Kids` is gone with no confirmation and `Default` becomes current without playing

#### Scenario: Cancel the close

- **GIVEN** `Kids` is current and plays
- **WHEN** the user presses the close control and cancels
- **THEN** nothing changes

#### Scenario: Close a peer entry

- **WHEN** the user presses the close control on a shown peer entry
- **THEN** the GUI Client disconnects from the peer and the peer keeps playing

### Requirement: The Queue filter covers only the shown queue

The Queue filter SHALL match the rows of the shown entry only, with the fuzzy word match that the Library uses. Showing another entry SHALL close and clear the Queue filter.

#### Scenario: Filter then switch

- **GIVEN** the Queue filter narrows `Kids` to two rows
- **WHEN** the user shows `Default`
- **THEN** the Queue filter is closed and empty, and `Default` shows every row

### Requirement: Showing an entry resets the Queue active row and marks

Showing another entry SHALL clear the marks and SHALL make the shown queue's start slot the Queue's active row, or its first row when it has no start slot.

#### Scenario: Switch with marks

- **GIVEN** three rows of `Kids` are marked
- **WHEN** the user shows `Default`
- **THEN** no row is marked and the start slot of `Default` is the active row

### Requirement: The chooser and its controls are reachable from the keyboard

While the pointer target is the Queue list box, the GUI-only key context SHALL bind `t` to open and close the Queue chooser, `Ctrl+T` to the new-queue control, and `Ctrl+W` to the close control. The TUI Client's bindings SHALL NOT change.

#### Scenario: Open the chooser by key

- **WHEN** the pointer rests over the Queue list box and the user presses `t`
- **THEN** the Queue chooser opens with its cursor on the shown entry

#### Scenario: The TUI keeps its keys

- **WHEN** the user presses `Ctrl+W` in the TUI Client
- **THEN** no queue closes

### Requirement: The open chooser takes its keys

While the Queue chooser is open, the arrow keys SHALL move its cursor, Enter SHALL show the entry under the cursor, `r` SHALL open the name of the local queue under the cursor for edit, and Esc SHALL close the chooser. While a name is open for edit, Enter SHALL confirm it and Esc SHALL cancel it.

#### Scenario: Rename by key

- **GIVEN** the chooser is open with its cursor on `Kids`
- **WHEN** the user presses `r`, types a new name, and presses Enter
- **THEN** the Owner renames `Kids` to the typed name
