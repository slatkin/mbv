# Spec Delta

## MODIFIED Requirements

### Requirement: The first GUI slice presents the player panel of the design canvas

The GUI Client SHALL present one player panel whose content, arrangement, and states follow the design canvas. The TUI layout SHALL NOT be the reference. The footer line SHALL show the prefix-armed state and the toasts of the shared Status bar projection. A surface that the GUI Client does not yet present SHALL be absent, with no stub and no placeholder.

#### Scenario: The queue has slots

- **GIVEN** the Owner process holds a Bound queue with slots
- **WHEN** the GUI Client window is open
- **THEN** the expanded panel shows, from top to bottom, a header, the artwork, the title, the scrubber, the transport, the track chips, the queue header, the queue rows, and a footer line
- **AND** the header shows the playback target with a chooser, the playback state with a leading dot, and a collapse control
- **AND** the artwork shows the active item with its season and episode badge
- **AND** the transport has previous, back 10 s, play/pause, forward 30 s, and next
- **AND** the queue lists the slots in canonical order, marks the active slot, and numbers every row with its 1-based position in the queue

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

#### Scenario: The artwork has a still slot

- **WHEN** the expanded panel paints the artwork of the active item
- **THEN** the artwork paints in the canvas's fixed still slot with a caption that names the image source, and the season and episode badge sits at the bottom left as the canvas places it

#### Scenario: The footer shows the library context

- **GIVEN** the queue's items carry a series with seasons
- **WHEN** the expanded panel paints the footer line
- **THEN** the footer's left line shows the series name and its season range as the canvas paints it
- **AND** while the queue carries no series context, the footer's left line shows the queue's source text instead

### Requirement: The panel changes state with its own size

The panel SHALL pick one of four states from its own size plus the Client core's toggles: expanded, compact strip, expanded without queue, or rail. The `z` key SHALL toggle the collapse and the `q` key SHALL toggle the queue fold. The collapse toggle SHALL rank above the queue-fold toggle, and both SHALL rank above the container measurement, so no measurement that a window resize of these rules produced can replace the state the toggle set. The front end SHALL pick the layout from the measured size of the panel container, the same measurement that it reports to the Client core in `LayoutReport`, so the painted layout, the list capacity, and the hit map follow one size. The Client core SHALL hold the two toggles.

#### Scenario: The container is shorter than 520 px

- **GIVEN** neither toggle is set
- **WHEN** the panel container is shorter than 520 px
- **THEN** the panel shows the compact strip: a 96×54 still, the title, the parent line with the next item, play/pause, next, and a thin progress line

#### Scenario: The queue is folded

- **WHEN** the user presses `q` in the expanded panel
- **THEN** the queue rows are replaced by one line with the remaining count, the queue finish clock with its next-day suffix, and a control that expands the queue

#### Scenario: The panel is collapsed to the rail

- **WHEN** the user presses `z`
- **THEN** the panel shows the 72 px rail: a progress ring around the artwork, play/pause, next, the remaining time in compact minutes, and a control that expands the panel
- **AND** pressing `z` again restores the previous state

#### Scenario: A toggle survives a short container

- **GIVEN** the user pressed `q` and a window resize of this requirement shrank the container below 520 px
- **WHEN** the front end measures the container
- **THEN** the panel still paints the folded line, never the compact strip

### Requirement: The scrubber seeks and previews

The scrubber SHALL show the progress, the chapter boundaries, the intro and credits marks, and a thumb, with the elapsed time and one right-hand line that joins the remaining time and the finish clock. A press on the bar SHALL seek to that position. The back 10 s and forward 30 s chords SHALL do the same work as the transport buttons.

#### Scenario: The bar shows the canvas segments

- **GIVEN** the active item carries chapters and intro and credits marks
- **WHEN** the scrubber paints
- **THEN** the bar shows the intro and credits marks, the chapter ticks, the played fill, and the thumb whose ring the canvas paints

#### Scenario: The pointer hovers the bar

- **WHEN** the pointer is over the scrubber bar
- **THEN** a position line marks the pointer's position on the bar and a preview shows the trickplay frame, the time at the pointer, and the chapter name
- **AND** focus and playback do not change

#### Scenario: The bar is pressed

- **WHEN** the user presses the bar at a position
- **THEN** the Owner process seeks to that position

#### Scenario: No trickplay frames exist

- **GIVEN** the active item has no trickplay data
- **WHEN** the pointer is over the scrubber bar
- **THEN** the preview shows the time and the chapter without a frame

### Requirement: Track chips open popovers

The panel SHALL show one chip each for the video stream, the audio track, and the subtitle track of the active item, painted as the canvas's chips. Each chip SHALL carry the canvas's chevron. The audio and subtitle chips SHALL each open a popover that lists the choices and marks the chosen one, and the open chip SHALL show the canvas's open state. The video chip's press SHALL change nothing: the video chip shows the resolved stream label only and SHALL NOT open a popover. The chosen track SHALL be sent to the Owner process. The `c` key SHALL cycle the subtitle track.

#### Scenario: A subtitle track is chosen

- **WHEN** the user opens the subtitle popover and chooses a track
- **THEN** the Owner process switches to that track and the popover marks it
- **AND** the popover lists Off and each subtitle track with its format

#### Scenario: The open chip shows its state

- **WHEN** a chip's popover is open
- **THEN** that chip paints the canvas's open state and the other two do not

#### Scenario: The video chip is pressed

- **WHEN** the user presses the video chip
- **THEN** no popover opens and no request leaves the Client

### Requirement: The queue shows finish times, a row menu, and drag reorder

The queue header SHALL show the remaining count as the canvas forms it (`Up next · N`), the queue finish clock as the canvas forms it, a switch between "Length" and "Ends at" for the right column, and a filter control. The remaining count SHALL cover every row that does not carry the playhead. Rows SHALL have a fixed height of 30 px and SHALL carry the canvas's one-line row: its queue ordinal, its title line, its progress where the state has one, and its right-column value. Every queue edit SHALL be a request to the Owner process. The window content SHALL NOT change the canonical queue.

#### Scenario: The right column is switched

- **WHEN** the user switches the right column to "Ends at"
- **THEN** each row shows the clock time at which it ends, from the current position onward

#### Scenario: Rows show their state

- **WHEN** the queue holds a watched row, the active row, and a row with partial progress
- **THEN** the watched row is struck through, the active row is filled, and the partial row shows a thin progress bar under its title
- **AND** each row carries its 1-based queue ordinal as the canvas paints it

#### Scenario: The queue header counts the queue

- **GIVEN** the queue holds slots before the playhead that are watched
- **WHEN** the queue header paints
- **THEN** the remaining count covers those rows too, as the canvas's sample script increments for every non-current row

#### Scenario: The row menu opens

- **WHEN** the pointer is over a row
- **THEN** the row shows a grip and a row-actions button
- **AND** the button opens a menu with Play now (`⏎`), Play next (`n`), Mark watched (`w`), Go to episode (`g`), and Remove from queue (`dd`)

#### Scenario: The destructive entry is distinct

- **WHEN** the row menu paints
- **THEN** Remove from queue paints in the canvas's destructive colour, separated from the other entries by the menu divider

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

Text SHALL use IBM Plex Sans. Times, key hints, chips, and counters SHALL use JetBrains Mono with tabular figures. Both faces SHALL ship with the front end, and the window SHALL fetch no font at runtime. Every clock the panel paints SHALL use the canvas's zero-padded 24-hour `HH:MM` form, and every minus sign in a time SHALL be U+2212. Colours SHALL come from the theme that the Client core sends.

#### Scenario: Fonts load with no network

- **GIVEN** the machine has no network
- **WHEN** the GUI Client window opens
- **THEN** text and times show in the two bundled faces

#### Scenario: A finish clock passes midnight

- **GIVEN** the queue's finish time falls on the next day
- **WHEN** the panel paints a finish clock
- **THEN** the time shows as zero-padded 24-hour `HH:MM`, and the queue finish clock carries the canvas's next-day suffix

## ADDED Requirements

### Requirement: The window resizes with the panel toggles

The `z` collapse SHALL shrink the window to the collapsed state's natural size, and expanding SHALL restore the remembered pre-collapse size, as a mini-player does. The `q` fold SHALL shrink the window by the queue section's height, and unfolding SHALL restore. The Client core SHALL keep the two toggles as the only authority for user intent, and a window resize SHALL NOT change the painted panel state.

#### Scenario: The panel collapses

- **WHEN** the user presses `z` and the panel paints the rail
- **THEN** the window shrinks to the rail's natural size

#### Scenario: The panel expands again

- **WHEN** the user presses `z` again on a collapsed panel
- **THEN** the window restores the size it held before the collapse

#### Scenario: The queue folds

- **WHEN** the user presses `q` in the expanded panel
- **THEN** the window shrinks by the height of the queue section that the fold removes

#### Scenario: The queue unfolds

- **WHEN** the user presses `q` again on a folded queue
- **THEN** the window restores the size it held before the fold

#### Scenario: A resize never repicks the state

- **GIVEN** a toggle is set
- **WHEN** the window resizes and the front end re-measures the container
- **THEN** the painted panel state is still the toggle's state
