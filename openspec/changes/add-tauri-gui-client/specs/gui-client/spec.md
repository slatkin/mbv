# Spec Delta

## RENAMED Requirements

- FROM: `### Requirement: The first GUI slice presents the Queue, Queue playback, and Status bar`
- TO: `### Requirement: The first GUI slice presents the player panel of the design canvas`

## MODIFIED Requirements

### Requirement: The first GUI slice presents the player panel of the design canvas

The GUI Client SHALL present one player panel whose content, arrangement, and states follow the design canvas. The TUI layout SHALL NOT be the reference. The footer line SHALL show the prefix-armed state and the toasts of the shared Status bar projection. A surface that the GUI Client does not yet present SHALL be absent, with no stub and no placeholder.

#### Scenario: The queue has slots

- **GIVEN** the Owner process holds a Bound queue with slots
- **WHEN** the GUI Client window is open
- **THEN** the expanded panel shows, from top to bottom, a header, the artwork, the title, the scrubber, the transport, the track chips, the queue header, the queue rows, and a footer line
- **AND** the header shows the playback target with a chooser, the playback state label, and a collapse control
- **AND** the artwork shows the active item with its season and episode badge
- **AND** the transport has previous, back 10 s, play/pause, forward 30 s, and next
- **AND** the queue lists the slots in canonical order and marks the active slot

#### Scenario: The Status bar projection holds a toast

- **WHEN** the shared Status bar projection holds a toast
- **THEN** the GUI Client shows the toast in the footer line or in a conventional GUI form

#### Scenario: The prefix is armed

- **WHEN** the user arms prefix mode
- **THEN** the footer line shows the armed state

#### Scenario: The queue is empty

- **GIVEN** the Owner process holds an empty queue
- **WHEN** the GUI Client window is open
- **THEN** the panel shows an empty-queue state in the type, colour, and spacing of the canvas

#### Scenario: A surface is not yet ported

- **WHEN** the user presses a key bound to a surface that the GUI Client does not present
- **THEN** no surface opens and no placeholder appears

### Requirement: GUI keyboard interaction matches the TUI Client

For every action the GUI Client presents, each key chord SHALL have the same effect as in the TUI Client. The GUI Client SHALL use the same `[keys]` configuration and the same keyboard router. The key hints that the canvas shows are the default bindings.

#### Scenario: A configured binding is used in both Clients

- **GIVEN** the user rebinds a queue action in `[keys]`
- **WHEN** the user presses the new chord in the GUI Client
- **THEN** the GUI Client runs that queue action, as the TUI Client does

#### Scenario: Prefix mode

- **WHEN** the user arms prefix mode in the GUI Client
- **THEN** the footer line shows the armed state and the next chord resolves in the prefix namespace, as in the TUI Client

### Requirement: Every action is reachable from the keyboard

Every action in the panel SHALL have a key chord. No action SHALL be available only by pointer. A hover MAY show an enhancement (the scrubber preview, the row grip, the row-actions button), but the action behind it SHALL have a key chord.

#### Scenario: A pointer-only control is proposed

- **WHEN** the panel offers a control
- **THEN** a key chord runs the same action

#### Scenario: A hover control appears

- **WHEN** the pointer is over a row and the row-actions button appears
- **THEN** the same menu opens from the keyboard

### Requirement: Scrolling follows the cursor

The queue SHALL keep the cursor row fully visible after every keyboard movement. The queue SHALL use fixed row heights so that capacity is a whole number of rows.

#### Scenario: The cursor moves past the last visible row

- **WHEN** the user moves the queue cursor below the last fully visible row
- **THEN** the queue scrolls so that the cursor row is fully visible

### Requirement: GUI pointer input resolves through semantic targets

A pointer press, drag, or wheel step in the GUI Client SHALL resolve to the semantic target that the GUI painted at that point in the current frame. The effect SHALL be the gesture that the design canvas defines. Where the canvas is silent and the TUI has a matching gesture, the effect SHALL be the same.

#### Scenario: The slice gestures

- **WHEN** the user uses the pointer on the panel
- **THEN** the panel supports row selection, row activation, the row-actions button and menu, row drag, wheel scrolling, the transport controls, the scrubber, the track chips and their popovers, the playback-target chooser, and the collapse and fold controls

#### Scenario: A queue row is clicked

- **WHEN** the user clicks a painted queue row
- **THEN** the queue cursor moves to that slot, as a click does in the TUI Client

#### Scenario: The wheel moves over the queue

- **WHEN** the user scrolls the wheel over the queue
- **THEN** the queue viewport moves by the uniform wheel step and the cursor follows the TUI wheel rules

#### Scenario: A control is clicked in a surface without focus

- **GIVEN** the queue has focus
- **WHEN** the user clicks the play/pause control
- **THEN** playback toggles

#### Scenario: A press lands on no target

- **WHEN** the user clicks a point with no painted target
- **THEN** nothing changes

### Requirement: The window content holds no interaction state

The GUI Client's interaction state (focus, cursor, selection, marks, viewport, prefix mode, the collapse and queue-fold toggles, the drag, and the open menu or popover) SHALL live in the Client core, not in the window content. The window content SHALL paint what the Client core projects and SHALL forward key and pointer input to it. The layout that the container size picks is not interaction state.

#### Scenario: The window content reloads

- **GIVEN** the queue cursor is on a slot and the queue has focus
- **WHEN** the window content reloads
- **THEN** the queue has focus and the cursor is on the same slot

#### Scenario: A key arrives in the window content

- **WHEN** the user presses a key while the GUI window has focus
- **THEN** the Client core resolves the chord through the one keyboard router
- **AND** the window content does not interpret the key itself

## ADDED Requirements

### Requirement: The panel changes state with its own size

The panel SHALL pick one of four states from its own size, not from the window size: expanded, compact strip, expanded without queue, or rail. The `z` key SHALL toggle the collapse and the `q` key SHALL toggle the queue fold. The Client core SHALL hold the two toggles. The front end SHALL pick the layout from the container size with container queries.

#### Scenario: The container is shorter than 520 px

- **WHEN** the panel container is shorter than 520 px
- **THEN** the panel shows the compact strip: a 96×54 still, the title, the parent line with the next item, play/pause, next, and a thin progress line

#### Scenario: The queue is folded

- **WHEN** the user presses `q` in the expanded panel
- **THEN** the queue rows are replaced by one line with the remaining count, the queue finish clock, and a control that expands the queue

#### Scenario: The panel is collapsed to the rail

- **WHEN** the user presses `z`
- **THEN** the panel shows the 72 px rail: a progress ring around the artwork, play/pause, next, the remaining time, and a control that expands the panel
- **AND** pressing `z` again restores the previous state

### Requirement: The scrubber seeks and previews

The scrubber SHALL show the progress, the chapter boundaries, the intro and credits marks, and a thumb, with the elapsed time and the remaining time with the finish clock. A press on the bar SHALL seek to that position. The back 10 s and forward 30 s chords SHALL do the same work as the transport buttons.

#### Scenario: The pointer hovers the bar

- **WHEN** the pointer is over the scrubber bar
- **THEN** a preview shows the trickplay frame, the time at the pointer, and the chapter name
- **AND** focus and playback do not change

#### Scenario: The bar is pressed

- **WHEN** the user presses the bar at a position
- **THEN** the Owner process seeks to that position

#### Scenario: No trickplay frames exist

- **GIVEN** the active item has no trickplay data
- **WHEN** the pointer is over the scrubber bar
- **THEN** the preview shows the time and the chapter without a frame

### Requirement: Track chips open popovers

The panel SHALL show one chip each for the video stream, the audio track, and the subtitle track of the active item. A chip SHALL open a popover that lists the choices and marks the chosen one. The chosen track SHALL be sent to the Owner process. The `c` key SHALL cycle the subtitle track and the `[` and `]` keys SHALL change the subtitle offset.

#### Scenario: A subtitle track is chosen

- **WHEN** the user opens the subtitle popover and chooses a track
- **THEN** the Owner process switches to that track and the popover marks it
- **AND** the popover lists Off and each subtitle track with its format

#### Scenario: The subtitle offset changes

- **WHEN** the user presses `]` or the later control
- **THEN** the subtitle offset grows by one step and the popover shows the new value

### Requirement: The queue shows finish times, a row menu, and drag reorder

The queue header SHALL show the remaining count, the queue finish clock, a switch between "Length" and "Ends at" for the right column, and a filter control. Rows SHALL have a fixed height of 30 px. Every queue edit SHALL be a request to the Owner process. The window content SHALL NOT change the canonical queue.

#### Scenario: The right column is switched

- **WHEN** the user switches the right column to "Ends at"
- **THEN** each row shows the clock time at which it ends, from the current position onward

#### Scenario: Rows show their state

- **WHEN** the queue holds a watched row, the active row, and a row with partial progress
- **THEN** the watched row is struck through, the active row is filled, and the partial row shows a thin progress bar under its title

#### Scenario: The row menu opens

- **WHEN** the pointer is over a row
- **THEN** the row shows a grip and a row-actions button
- **AND** the button opens a menu with Play now (`Enter`), Play next (`n`), Mark watched (`w`), Go to episode (`g`), and Remove from queue (`dd`)

#### Scenario: A row is removed from the menu

- **WHEN** the user chooses Remove from queue
- **THEN** the Owner process removes that slot from the queue

#### Scenario: A row is dragged

- **WHEN** the user drags a row to a new position and releases
- **THEN** the queue shows an insertion line and a drag ghost during the drag
- **AND** the Owner process moves the slot to the new position

#### Scenario: A row is moved by key

- **WHEN** the user presses `Alt+J` on a row
- **THEN** the row moves one place down, as the drag would do

### Requirement: The GUI Client uses the typography and colours of the canvas

Text SHALL use IBM Plex Sans. Times, key hints, chips, and counters SHALL use JetBrains Mono with tabular figures. Both faces SHALL ship with the front end, and the window SHALL fetch no font at runtime. Icons SHALL be Lucide icons. Colours SHALL come from the theme that the Client core sends.

#### Scenario: Fonts load with no network

- **GIVEN** the machine has no network
- **WHEN** the GUI Client window opens
- **THEN** text and times show in the two bundled faces

## REMOVED Requirements

### Requirement: The GUI Client derives its breakpoint from its window

**Reason**: The canvas picks the panel state from the panel's own size through container queries (see "The panel changes state with its own size"). The window-width breakpoint has no effect in the slice.

**Migration**: Layout follows the panel states. The shared TUI breakpoint classes stay in the Client core for later surfaces.
