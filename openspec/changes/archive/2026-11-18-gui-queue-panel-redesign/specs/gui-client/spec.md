# Spec Delta

## MODIFIED Requirements

### Requirement: The first GUI slice presents the player panel of the design canvas

The GUI Client SHALL present one player panel whose content, arrangement, and states follow the design canvas. The TUI layout SHALL NOT be the reference. The window SHALL show the top bar as one global header across the full window width, above the Library panel and the Queue column, in every arrangement; the Settings screen SHALL show no global header. The panel SHALL show no header row above its artwork: the artwork SHALL be the panel's topmost content, directly under the global header. The footer line SHALL show the playback-target chooser and the collapse control at its left edge. The GUI Client SHALL paint no now-playing status word on any surface. The footer line SHALL show the prefix-armed state and the toasts of the shared Status bar projection while the Library panel does not show; while the Library panel shows, the Library Status bar shows them. The footer line SHALL show the total time of the shown queue and, while rows are marked, the total time of the marked rows. A surface, or a Library destination, that the GUI Client does not yet present SHALL be absent, with no stub and no placeholder: the top bar carries only the destination tabs that the GUI Client presents, and the Library Status bar carries the Search field only once Search is presented.

#### Scenario: The queue has slots

- **GIVEN** the Owner process holds a Bound queue with slots
- **WHEN** the GUI Client window is open
- **THEN** the expanded panel shows, from top to bottom, the artwork with its overlays, the title, the queue header, the queue rows, and a footer line
- **AND** the footer line's left shows the playback target with a chooser and the collapse control
- **AND** the artwork shows the active item, with the track chips at its top left, the season and episode badge at its top right, and the player controls at its bottom
- **AND** the player controls have two rows: the scrubber alone across the full width, and under it play/pause, previous, back 10 s, forward 30 s, and next at the left and the elapsed and remaining clocks at the right
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
- **AND** no placeholder image and no source caption paint
- **AND** the season and episode badge sits at the top right of the artwork

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

#### Scenario: The header spans both columns

- **GIVEN** the Library panel and the Queue column show, or the Queue column is collapsed to its rail
- **WHEN** the window paints
- **THEN** the top bar spans the full window width, and the Library column and the Queue column or its rail start below it

#### Scenario: The Settings screen shows

- **WHEN** the Settings screen shows
- **THEN** no global header paints

### Requirement: The scrubber seeks and previews

The scrubber SHALL show the played progress and a thumb. It SHALL show no chapter boundaries and no intro or credits marks. The elapsed time and the remaining time show on the transport row. The whole scrubber row SHALL be the bar's hit area. A press on the bar SHALL seek to that position. While the pointer drags on the bar, the bar SHALL keep its hover state and the time tip SHALL follow the pointer. The back 10 s and forward 30 s chords SHALL do the same work as the transport buttons.

#### Scenario: The pointer hovers the bar

- **WHEN** the pointer is over the scrubber row
- **THEN** the bar shows its hover state and a tip shows the time at the pointer, with no trickplay frame and no chapter
- **AND** focus and playback do not change

#### Scenario: The bar is pressed

- **WHEN** the user presses the bar at a position
- **THEN** the Owner process seeks to that position

#### Scenario: No trickplay frames exist

- **GIVEN** the active item has trickplay data, or has none
- **WHEN** the pointer is over the scrubber row
- **THEN** the tip shows the time at the pointer and no frame, the same in both cases

#### Scenario: The bar is dragged

- **WHEN** the user drags on the bar
- **THEN** the bar keeps its hover state and the time tip follows the pointer

#### Scenario: The item has chapters

- **GIVEN** the active item has chapters and intro and credits segments
- **WHEN** the scrubber paints
- **THEN** the bar shows no chapter boundary and no intro or credits mark


### Requirement: The queue shows a row menu and drag reorder

The queue header SHALL hold only what the requirement "The queue header holds the chooser and its controls" (`gui-queue-chooser`) names. Every row SHALL have the same fixed height and SHALL carry the canvas's one-line row: its queue ordinal, its title, and its progress where the state has one. A row's progress bar SHALL paint inside the row along its bottom and SHALL NOT add height to the row. A row SHALL carry no right-column value. The shown queue's start slot SHALL be marked as playing while its queue plays and as the start of playback while its queue is stopped or resting, and the two marks SHALL be distinct. A shown queue whose owner reports no start slot, such as a peer without named queues, SHALL carry the playing mark on its observed active slot, and no start mark. Every queue edit SHALL be a request to the Player owner of the shown queue. The window content SHALL NOT change the canonical queue.

#### Scenario: Rows show their state

- **WHEN** the queue holds a watched row, the active row, and a row with partial progress
- **THEN** the watched row is struck through, the active row is filled, and the partial row shows a thin progress bar inside the row along its bottom
- **AND** every row has the same height, the partial row included
- **AND** each row carries its 1-based queue ordinal as the canvas paints it

#### Scenario: The right column is switched

- **WHEN** the queue header and the queue rows paint
- **THEN** the header shows no right-column switch and no row shows a length or an end time
- **AND** no GUI key switches a right column

#### Scenario: The queue header counts the queue

- **WHEN** the queue header paints
- **THEN** it shows no remaining count and no queue finish clock

#### Scenario: A stopped queue marks its start slot

- **GIVEN** the shown queue is stopped with its third slot as its start slot
- **WHEN** the queue rows paint
- **THEN** the third row carries the start mark, and no row carries the playing mark

#### Scenario: A peer queue marks its playing row

- **GIVEN** the GUI Client shows a peer that reports no start slot and plays its second slot
- **WHEN** the queue rows paint
- **THEN** the second row carries the playing mark

#### Scenario: Play turns the start mark into the playing mark

- **GIVEN** the shown queue is stopped with its third slot as its start slot
- **WHEN** the user presses Play
- **THEN** the third row carries the playing mark

#### Scenario: A row is hovered

- **WHEN** the pointer is over a queue row
- **THEN** the row paints the canvas's hovered row: the grip replaces the row's ordinal and the row-actions button shows at the row's end

#### Scenario: The row menu opens

- **WHEN** the row-actions button is pressed
- **THEN** the menu opens with Play now (`⏎`), Play next (`n`), Mark watched (`w`), Go to episode (`g`), and Remove from queue (`dd`)

#### Scenario: The destructive entry is distinct

- **WHEN** the row menu paints
- **THEN** Remove from queue paints in the canvas's destructive colour, separated from the other entries by the menu divider

#### Scenario: A row is removed from the menu

- **WHEN** the user chooses Remove from queue
- **THEN** the Player owner of the shown queue removes that slot from it

#### Scenario: A row is dragged

- **WHEN** the user drags a row to a new position and releases
- **THEN** the queue shows an insertion line and a drag ghost during the drag
- **AND** the Player owner of the shown queue moves the slot to the new position

#### Scenario: A row is moved by key

- **WHEN** the user presses `Alt+J` on a row
- **THEN** the row moves one place down, as the drag would do
