# Spec Delta

## MODIFIED Requirements

### Requirement: The first GUI slice presents the player panel of the design canvas

The GUI Client SHALL present one player panel whose content, arrangement, and states follow the design canvas. The TUI layout SHALL NOT be the reference. The panel SHALL show no header row above its artwork: the artwork SHALL be the panel's topmost content. The footer line SHALL show the playback-target chooser and the collapse control at its left edge. The GUI Client SHALL paint no now-playing status word on any surface. The footer line SHALL show the prefix-armed state and the toasts of the shared Status bar projection while the Library panel does not show; while the Library panel shows, the Library Status bar shows them. The footer line SHALL show the total time of the shown queue and, while rows are marked, the total time of the marked rows. A surface, or a Library destination, that the GUI Client does not yet present SHALL be absent, with no stub and no placeholder: the top bar carries only the destination tabs that the GUI Client presents, and it carries the Search field only once Search is presented.

#### Scenario: The queue has slots

- **GIVEN** the Owner process holds a Bound queue with slots
- **WHEN** the GUI Client window is open
- **THEN** the expanded panel shows, from top to bottom, the artwork, the title, the scrubber, the transport, the track chips, the queue header, the queue rows, and a footer line
- **AND** the footer line's left shows the playback target with a chooser and the collapse control
- **AND** the artwork shows the active item with its season and episode badge
- **AND** the transport has previous, back 10 s, play/pause, forward 30 s, and next
- **AND** the queue lists the slots in canonical order, marks the start slot, and numbers every row with its 1-based position in the queue

#### Scenario: The Status bar projection holds a toast

- **WHEN** the shared Status bar projection holds a toast
- **THEN** the GUI Client shows the toast in the Library Status bar while the Library panel shows, and otherwise in the footer line or in a conventional GUI form

#### Scenario: The prefix is armed

- **WHEN** the user arms prefix mode
- **THEN** the Library Status bar shows the armed state while the Library panel shows, and otherwise the footer line shows it

#### Scenario: The queue is empty

- **GIVEN** the Owner process holds an empty queue
- **WHEN** the GUI Client window is open
- **THEN** the panel shows an empty-queue state in the type, colour, and spacing of the canvas

#### Scenario: A surface is not yet ported

- **WHEN** the user presses a key bound to a surface that the GUI Client does not present
- **THEN** no surface opens and no placeholder appears

#### Scenario: A destination is not yet presented

- **GIVEN** the GUI Client presents only the Music destination
- **WHEN** the top bar paints
- **THEN** it shows no tab for another destination, and a digit key for one opens nothing

#### Scenario: The artwork has a still slot

- **WHEN** the expanded panel paints the artwork of the active item
- **THEN** a loaded image fills the panel's width and takes its height from the artwork's aspect, as the TUI's visual slot paints at full column width
- **AND** the placeholder paints in the canvas's fixed 225 px box and hugs its image
- **AND** the still slot paints a caption that names the image source while no image lands, and the season and episode badge sits at the bottom left as the canvas places it

#### Scenario: The footer paints the queue's source text

- **WHEN** the expanded panel paints the footer line
- **THEN** the left line shows the playback-target chooser and the collapse control, then the queue's existing source text — the playlist text the frame carries — without a leading icon, the right line shows the shown queue's total time, and the line uses the canvas footer's type and border
- **AND** the footer shows no Player-owner label and no liveness dot
- **AND** the prefix, toast, autosave, and dirty states still paint in the footer line when they are active and the Library panel does not show

#### Scenario: The footer's target chooser opens above the band

- **WHEN** the user presses the playback-target control in the footer line
- **THEN** the chooser's popover opens above the footer band, anchored to the pressed control, and lists the live choice list with the attached output marked
- **AND** Esc and a press outside the popover close it

#### Scenario: No now-playing status word paints

- **WHEN** the window paints at any playback state
- **THEN** no surface paints a now-playing status word

#### Scenario: The footer totals the marked rows

- **GIVEN** three rows of the shown queue are marked
- **WHEN** the footer line paints
- **THEN** it shows the total time of the three marked rows beside the total time of the queue

#### Scenario: No row is marked

- **WHEN** no row of the shown queue is marked
- **THEN** the footer shows only the total time of the queue

#### Scenario: A runtime is unknown

- **GIVEN** the shown queue holds a slot whose runtime is unknown
- **WHEN** the footer line paints
- **THEN** the total shows the sum of the known runtimes and marks it as a lower bound

#### Scenario: The filter narrows the rows

- **GIVEN** the Queue filter narrows the shown queue
- **WHEN** the footer line paints
- **THEN** the total time still covers every slot of the shown queue
