# Spec Delta

## Purpose

Defines the GUI Client: a Client that presents mbv in a native desktop window, with the same Panel layouts, keyboard-first interaction, and Owner relationship as the TUI Client.

## ADDED Requirements

### Requirement: The GUI Client is a separate program

mbv SHALL ship the GUI Client as a program separate from the `mbv` terminal program. The `mbv` program SHALL NOT load or link the windowing and graphics stack that the GUI Client uses.

#### Scenario: The terminal program carries no window stack

- **WHEN** the `mbv` program is built
- **THEN** its dependency graph contains no windowing or GPU rendering library that the GUI Client uses

#### Scenario: The GUI Client starts on its own

- **WHEN** the user starts the GUI Client program
- **THEN** it opens one native window and presents mbv in it without a terminal

### Requirement: The GUI Client attaches to the Owner process as a Client

The GUI Client SHALL be a Client of the Owner process under the same rules as a TUI Client. It SHALL find, start, and attach to the Owner process as a TUI Client does. It SHALL NOT hold the owner lock. It SHALL NOT become a Player owner.

#### Scenario: No Owner process is running

- **WHEN** the GUI Client starts and no Owner process is running
- **THEN** the GUI Client starts the Owner process from the `mbv` program and attaches to it

#### Scenario: A TUI Client and a GUI Client are attached at once

- **GIVEN** Stay-alive is enabled and a TUI Client is attached
- **WHEN** the GUI Client starts
- **THEN** it attaches alongside the TUI Client without displacing it
- **AND** both show the same Bound queue and playback state

#### Scenario: Stay-alive is disabled and a Client is attached

- **GIVEN** Stay-alive is disabled and a TUI Client is attached
- **WHEN** the GUI Client starts
- **THEN** the GUI Client refuses to start with the same explanation a second TUI Client gets

### Requirement: The GUI Client never runs playback

The GUI Client SHALL NOT start, embed, or control mpv directly. It SHALL send playback and queue requests to the Owner process over ctrl. Video SHALL play in the window of the mpv child process, as it does for a TUI Client.

#### Scenario: Video plays from the GUI Client

- **WHEN** the user plays a video item from the GUI Client
- **THEN** the Owner process plays it in the mpv child process window
- **AND** the GUI Client window does not show the video

### Requirement: The first GUI slice presents the Queue, Queue playback, and Status bar

The GUI Client SHALL present the Queue panel, the Queue playback panel, and the Status bar. Each SHALL show the same content, Panel arrangement, and states as its TUI counterpart in the queue-only Panel mode. A surface that the GUI Client does not yet present SHALL be absent. It SHALL NOT be a stub or a placeholder.

#### Scenario: The queue has slots

- **GIVEN** the Owner process holds a Bound queue with slots
- **WHEN** the GUI Client window is open
- **THEN** the Queue panel lists the slots in canonical order and marks the active slot
- **AND** the Queue playback panel shows the active item and its progress
- **AND** the Status bar is visible

#### Scenario: The queue is empty

- **GIVEN** the Owner process holds an empty queue
- **WHEN** the GUI Client window is open
- **THEN** the Queue panel shows its empty-queue state as the TUI Client does

#### Scenario: A surface is not yet ported

- **WHEN** the user presses a key bound to a surface that the GUI Client does not present
- **THEN** no surface opens and no placeholder appears

### Requirement: GUI keyboard interaction matches the TUI Client

For every surface the GUI Client presents, each key chord SHALL have the same effect as in the TUI Client. The GUI Client SHALL use the same `[keys]` configuration and the same keyboard router.

#### Scenario: A configured binding is used in both Clients

- **GIVEN** the user rebinds a queue action in `[keys]`
- **WHEN** the user presses the new chord in the GUI Client
- **THEN** the GUI Client runs that queue action, as the TUI Client does

#### Scenario: Prefix mode

- **WHEN** the user arms prefix mode in the GUI Client
- **THEN** the Status bar shows the armed state and the next chord resolves in the prefix namespace, as in the TUI Client

### Requirement: Exactly one focus is visible

The GUI Client SHALL show exactly one focused surface and one focused item within it at all times. Focus SHALL move only by a key or by a pointer press. It SHALL NOT move when the pointer hovers.

#### Scenario: Hover does not move focus

- **WHEN** the pointer moves over a queue row that does not have focus
- **THEN** focus and the queue cursor stay where they were

### Requirement: Every action is reachable from the keyboard

Every action available in a GUI Client surface SHALL have a key chord. No action SHALL be available only by pointer, and no affordance SHALL appear only when the pointer hovers.

#### Scenario: A pointer-only control is proposed

- **WHEN** a GUI surface offers a control
- **THEN** a key chord runs the same action and the control is visible without hover

### Requirement: Scrolling follows the cursor

A GUI list SHALL keep the cursor row fully visible after every keyboard movement. Lists SHALL use fixed row heights so that capacity is a whole number of rows.

#### Scenario: The cursor moves past the last visible row

- **WHEN** the user moves the queue cursor below the last fully visible row
- **THEN** the list scrolls so that the cursor row is fully visible

### Requirement: Motion never delays input

Any animation in the GUI Client SHALL be feedback only. A key press SHALL take effect on the next frame, whether or not an animation is in progress.

#### Scenario: Keys arrive during an animation

- **WHEN** the user presses a movement key while a transition is in progress
- **THEN** the movement applies on the next frame and the transition does not queue or delay it

### Requirement: GUI pointer input resolves through semantic targets

A pointer press or wheel step in the GUI Client SHALL resolve to the semantic target that the GUI painted at that point in the current frame. The effect SHALL be the same as the matching TUI pointer gesture. The first slice SHALL support row selection, row activation, wheel scrolling, and the transport and seek controls.

#### Scenario: A queue row is clicked

- **WHEN** the user clicks a painted queue row
- **THEN** the queue cursor moves to that slot, as a click does in the TUI Client

#### Scenario: The wheel moves over the queue

- **WHEN** the user scrolls the wheel over the Queue panel
- **THEN** the queue viewport moves by the uniform wheel step and the cursor follows the TUI wheel rules

#### Scenario: A press lands on no target

- **WHEN** the user clicks a point with no painted target
- **THEN** nothing changes

### Requirement: The GUI Client derives its breakpoint from its window

The GUI Client SHALL map its window size to the same layout breakpoint classes as the TUI Client. It SHALL apply the same layout behaviour for each class.

#### Scenario: The window is narrowed

- **WHEN** the user narrows the window below the GUI Wide threshold
- **THEN** the GUI Client applies the Narrow layout behaviour that the TUI Client applies below its Wide threshold
